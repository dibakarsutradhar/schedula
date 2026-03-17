/// License JWT validation and lifecycle management for the Schedula Hub Server.
///
/// The hub validates license JWTs locally using the embedded RS256 public key
/// (no network call per request).  A background task re-validates against the
/// licensing server every 24 h; if unreachable, a 7-day grace period applies.

use std::sync::{Arc, Mutex, RwLock};
use rusqlite::{Connection, params};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use crate::models::*;

/// RS256 public key embedded at compile time from hub-server/keys/license_public.pem.
pub const LICENSE_PUBLIC_KEY: &str = include_str!("../keys/license_public.pem");

/// Licensing server base URL — override with `SCHEDULA_LICENSE_URL` env var.
pub const DEFAULT_LICENSE_URL: &str = "https://license.schedula.app";

// ─── Validation ───────────────────────────────────────────────────────────────

/// Decode and validate a license JWT using the embedded public key.
/// Returns `LicenseClaims` or an error message.
pub fn validate_token(token: &str) -> Result<LicenseClaims, String> {
    let decoding_key = DecodingKey::from_rsa_pem(LICENSE_PUBLIC_KEY.as_bytes())
        .map_err(|e| format!("Failed to load license public key: {}", e))?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&["schedula-license"]);
    // exp = 0 means perpetual; we handle that manually below
    validation.validate_exp = false;

    let token_data = decode::<LicenseClaims>(token, &decoding_key, &validation)
        .map_err(|e| format!("Invalid license token: {}", e))?;

    let claims = token_data.claims;

    // Validate expiry manually (exp = 0 → perpetual)
    if claims.exp > 0 {
        let now = chrono::Utc::now().timestamp();
        if now > claims.exp {
            return Err("License token has expired".into());
        }
    }

    Ok(claims)
}

// ─── DB helpers ───────────────────────────────────────────────────────────────

/// Get current active license info from the database.
pub fn get_license_info(conn: &Connection) -> LicenseInfo {
    let row = conn.query_row(
        "SELECT plan, org_name, expires_at, status, last_validated_at
         FROM licenses
         WHERE status IN ('active','grace')
         ORDER BY activated_at DESC LIMIT 1",
        [],
        |r| Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, Option<String>>(4)?,
        )),
    );

    match row {
        Ok((plan, org_name, expires_at, status, last_validated_at)) => {
            let days_until_expiry = expires_at.as_deref().and_then(|exp| {
                chrono::DateTime::parse_from_rfc3339(exp).ok().map(|dt| {
                    let secs = dt.timestamp() - chrono::Utc::now().timestamp();
                    (secs / 86400).max(0)
                })
            });
            LicenseInfo {
                active: true,
                plan,
                org_name,
                expires_at,
                days_until_expiry,
                status,
                last_validated_at,
            }
        }
        Err(_) => LicenseInfo {
            active:            false,
            plan:              PLAN_FREE.to_string(),
            org_name:          None,
            expires_at:        None,
            days_until_expiry: None,
            status:            "none".into(),
            last_validated_at: None,
        },
    }
}

/// Effective plan derived from the stored JWT — **re-verifies the RS256
/// signature on every call**.  Trusting the plain-text `plan` column would
/// allow a sysadmin with SQLite access to escalate their own plan by editing
/// a single field; verifying the token means only a JWT signed by our private
/// key is accepted.
///
/// Attack surfaces closed:
///   `UPDATE licenses SET plan='institution'` → ignored (we read the token)
///   `UPDATE licenses SET status='active'`   → JWT still verified; forgery fails
///   Inserting a fabricated row             → RS256 sig check fails → Free
///   Deleting all rows                      → no token → Free (correct)
pub fn effective_plan(conn: &Connection) -> String {
    // Read the raw JWT token — not the convenience `plan` text column.
    let token: Result<String, _> = conn.query_row(
        "SELECT token FROM licenses WHERE status IN ('active','grace')
         ORDER BY activated_at DESC LIMIT 1",
        [],
        |r| r.get(0),
    );
    match token {
        Ok(t) => validate_token(&t)
            .map(|claims| claims.plan)
            .unwrap_or_else(|_| PLAN_FREE.to_string()),
        Err(_) => PLAN_FREE.to_string(),
    }
}

/// Store a validated license JWT in the database.
/// Marks any previously active license as superseded (sets status='expired').
pub fn store_license(conn: &Connection, claims: &LicenseClaims, token: &str) -> Result<(), String> {
    // Expire old licenses
    conn.execute(
        "UPDATE licenses SET status='expired' WHERE status='active'",
        [],
    ).map_err(|e| e.to_string())?;

    let expires_at: Option<String> = if claims.exp > 0 {
        let dt = chrono::DateTime::from_timestamp(claims.exp, 0)
            .map(|d| d.to_rfc3339());
        dt
    } else {
        None
    };

    let issued_at = chrono::DateTime::from_timestamp(claims.iat, 0)
        .map(|d| d.to_rfc3339())
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

    conn.execute(
        "INSERT INTO licenses (token, plan, org_name, expires_at, issued_at, status)
         VALUES (?1, ?2, ?3, ?4, ?5, 'active')",
        params![token, claims.plan, claims.org_name, expires_at, issued_at],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

/// Update `last_validated_at` for the active license.
fn touch_validation(conn: &Connection) {
    let now = chrono::Utc::now().to_rfc3339();
    let _ = conn.execute(
        "UPDATE licenses SET last_validated_at=?1 WHERE status IN ('active','grace')",
        params![now],
    );
}

/// Apply grace period logic: if the active license's `last_validated_at` is
/// older than 7 days, downgrade the hub to Free by marking the license as expired.
fn apply_grace_or_expire(conn: &Connection) {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339();
    let needs_expire: bool = conn.query_row(
        "SELECT COUNT(*) FROM licenses
         WHERE status IN ('active','grace') AND last_validated_at < ?1",
        params![cutoff],
        |r| r.get::<_, i64>(0),
    ).unwrap_or(0) > 0;

    if needs_expire {
        // Move from grace → expired
        let _ = conn.execute(
            "UPDATE licenses SET status='expired' WHERE status IN ('active','grace') AND last_validated_at < ?1",
            params![cutoff],
        );
        tracing::warn!("License validation grace period exceeded — hub downgraded to Free");
    } else {
        // Within grace period: keep running, update status to 'grace' so UI shows it
        let _ = conn.execute(
            "UPDATE licenses SET status='grace' WHERE status='active'",
            [],
        );
        tracing::warn!("License server unreachable — running in grace period");
    }
}

// ─── Background re-validation ─────────────────────────────────────────────────

/// Spawned as a tokio task on hub startup.  Validates the active license
/// against the licensing server every 24 hours and refreshes the in-memory
/// plan cache so feature gates reflect the updated state immediately.
pub async fn background_validation_loop(
    db:           Arc<Mutex<Connection>>,
    current_plan: Arc<RwLock<String>>,
) {
    // First run after 24 h; subsequent runs on 24 h intervals
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(24 * 3600)).await;

        let token = {
            let conn = db.lock().unwrap();
            conn.query_row(
                "SELECT token FROM licenses WHERE status IN ('active','grace')
                 ORDER BY activated_at DESC LIMIT 1",
                [],
                |r| r.get::<_, String>(0),
            ).ok()
        };

        let Some(token) = token else { continue };

        let license_url = std::env::var("SCHEDULA_LICENSE_URL")
            .unwrap_or_else(|_| DEFAULT_LICENSE_URL.to_string());
        let url = format!("{}/v1/validate", license_url);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        match client.post(&url).json(&serde_json::json!({"token": token})).send().await {
            Ok(resp) if resp.status().is_success() => {
                let conn = db.lock().unwrap();
                touch_validation(&conn);
                let plan = effective_plan(&conn);  // re-derives from JWT signature
                *current_plan.write().unwrap() = plan.clone();
                tracing::info!("License re-validated — plan={}", plan);
            }
            Ok(resp) => {
                let status = resp.status();
                tracing::warn!("License validation rejected by server: {}", status);
                let conn = db.lock().unwrap();
                let _ = conn.execute(
                    "UPDATE licenses SET status='revoked' WHERE status IN ('active','grace')",
                    [],
                );
                *current_plan.write().unwrap() = PLAN_FREE.to_string();
            }
            Err(e) => {
                tracing::warn!("License server unreachable: {} — applying grace period logic", e);
                let conn = db.lock().unwrap();
                apply_grace_or_expire(&conn);
                // Re-derive from JWT after grace logic (may have downgraded)
                let plan = effective_plan(&conn);
                *current_plan.write().unwrap() = plan;
            }
        }
    }
}

/// Called on hub startup to immediately sync local license state.
pub fn startup_license_check(conn: &Connection) {
    // If any license has exp set and it's past, mark expired
    let now = chrono::Utc::now().to_rfc3339();
    let _ = conn.execute(
        "UPDATE licenses SET status='expired'
         WHERE status IN ('active','grace') AND expires_at IS NOT NULL AND expires_at < ?1",
        params![now],
    );
    let info = get_license_info(conn);
    if info.active {
        tracing::info!("License loaded: plan={} org={:?} status={}",
            info.plan, info.org_name, info.status);
    } else {
        tracing::info!("No active license — running on Free plan");
    }
}
