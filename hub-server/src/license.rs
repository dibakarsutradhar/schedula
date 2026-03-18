/// License JWT validation and daily key-rotation lifecycle for the Schedula Hub Server.
///
/// ## How it works (Option A — Daily Symmetric Key Rotation)
///
/// 1. **Startup**: Hub immediately calls `POST /v1/refresh` on the license server.
///    - Receives a new 48-hour JWT (same JTI, fresh expiry) + today's 256-bit HMAC key.
///    - Stores both in SQLite and in the in-memory `AppState` caches.
///
/// 2. **Every 24 h**: `background_refresh_loop` repeats the refresh call.
///    - On success: caches are updated; hub operates normally.
///    - On failure (server unreachable): grace-period logic activates.
///
/// 3. **Grace period**: If the refresh has not succeeded for ≤7 days,
///    the hub continues operating on the last known plan with a warning.
///    After >7 days without a successful refresh the plan downgrades to Free.
///
/// 4. **Revocation**: If the license server returns 403/401 on a refresh,
///    the license is marked revoked immediately and the plan drops to Free.
///
/// ## Tamper resistance
///
/// - `effective_plan()` re-verifies the RS256 JWT signature on every call.
/// - Short-lived tokens (48 h expiry) mean a missed refresh is detected within
///   two days at the latest — no indefinite offline bypass.
/// - The `secret_key` is a daily 256-bit HMAC key that only orgs with an active
///   subscription receive. It is available to the desktop app layer for signing.

use std::sync::{Arc, Mutex, RwLock};
use rusqlite::{Connection, params};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use crate::models::*;

/// RS256 public key embedded at compile time from hub-server/keys/license_public.pem.
pub const LICENSE_PUBLIC_KEY: &str = include_str!("../keys/license_public.pem");

/// Licensing server base URL — override with `SCHEDULA_LICENSE_URL` env var.
pub const DEFAULT_LICENSE_URL: &str = "https://license.schedula.app";

// ─── Response model from /v1/refresh ─────────────────────────────────────────

#[derive(serde::Deserialize)]
struct RefreshResponse {
    new_token:  String,
    secret_key: String,
    key_date:   String,
    #[allow(dead_code)]
    plan:       String,
    #[allow(dead_code)]
    org_name:   Option<String>,
    expires_at: Option<String>,
}

// ─── Validation ───────────────────────────────────────────────────────────────

/// Decode and validate a license JWT using the embedded RS256 public key.
/// Returns `LicenseClaims` or an error string.
///
/// - `exp = 0` → perpetual license (no expiry check).
/// - `exp > 0` → short-lived token; expiry IS enforced.
pub fn validate_token(token: &str) -> Result<LicenseClaims, String> {
    let decoding_key = DecodingKey::from_rsa_pem(LICENSE_PUBLIC_KEY.as_bytes())
        .map_err(|e| format!("Failed to load license public key: {}", e))?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&["schedula-license"]);
    validation.validate_exp = false; // We handle exp=0 (perpetual) manually below

    let token_data = decode::<LicenseClaims>(token, &decoding_key, &validation)
        .map_err(|e| format!("Invalid license token: {}", e))?;

    let claims = token_data.claims;

    // Enforce expiry for short-lived tokens (exp = 0 → perpetual, skip check)
    if claims.exp > 0 {
        let now = chrono::Utc::now().timestamp();
        if now > claims.exp {
            return Err("License token has expired".into());
        }
    }

    Ok(claims)
}

/// Decode the plan from a token WITHOUT enforcing expiry.
/// Used by the grace-period path to preserve the plan after a missed refresh.
fn decode_plan_ignoring_expiry(token: &str) -> Option<String> {
    let decoding_key = DecodingKey::from_rsa_pem(LICENSE_PUBLIC_KEY.as_bytes()).ok()?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&["schedula-license"]);
    validation.validate_exp = false;
    decode::<LicenseClaims>(token, &decoding_key, &validation)
        .ok()
        .map(|td| td.claims.plan)
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

/// Derive the effective plan from the stored JWT, with grace-period support.
///
/// Priority order:
///   1. JWT is valid and not expired → return JWT plan (RS256-verified).
///   2. JWT is expired but `last_validated_at` is within 7 days → return
///      plan from expired token (grace period), warn in logs.
///   3. Otherwise → PLAN_FREE.
///
/// This is called at startup and by `background_refresh_loop` after each
/// successful refresh.  All feature gates use the `AppState.current_plan`
/// cache that this function populates — SQLite tampering has zero runtime effect.
pub fn effective_plan(conn: &Connection) -> String {
    let result: Result<(String, Option<String>), _> = conn.query_row(
        "SELECT token, last_validated_at FROM licenses WHERE status IN ('active','grace')
         ORDER BY activated_at DESC LIMIT 1",
        [],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
    );

    match result {
        Ok((token, last_validated_at)) => {
            match validate_token(&token) {
                Ok(claims) => claims.plan,
                Err(e) if e.contains("expired") => {
                    // Token has expired — check grace period
                    let days_since = last_validated_at.as_deref()
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| (chrono::Utc::now() - dt.with_timezone(&chrono::Utc)).num_days())
                        .unwrap_or(i64::MAX);

                    if days_since <= 7 {
                        tracing::warn!(
                            "License token expired — in grace period ({} of 7 days used)",
                            days_since
                        );
                        decode_plan_ignoring_expiry(&token)
                            .unwrap_or_else(|| PLAN_FREE.to_string())
                    } else {
                        tracing::warn!("License grace period exceeded — downgraded to Free");
                        PLAN_FREE.to_string()
                    }
                }
                Err(_) => PLAN_FREE.to_string(),
            }
        }
        Err(_) => PLAN_FREE.to_string(),
    }
}

/// Store a validated license JWT in the database (initial activation).
/// Marks any previously active license as superseded (sets status='expired').
pub fn store_license(conn: &Connection, claims: &LicenseClaims, token: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE licenses SET status='expired' WHERE status='active'",
        [],
    ).map_err(|e| e.to_string())?;

    let expires_at: Option<String> = if claims.exp > 0 {
        chrono::DateTime::from_timestamp(claims.exp, 0).map(|d| d.to_rfc3339())
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

/// Persist the results of a successful `/v1/refresh` call.
/// Updates token, secret_key, key_date, expires_at, last_validated_at.
fn persist_refresh(
    conn:       &Connection,
    new_token:  &str,
    secret_key: &str,
    key_date:   &str,
    expires_at: Option<&str>,
) {
    let now = chrono::Utc::now().to_rfc3339();
    let _ = conn.execute(
        "UPDATE licenses
         SET token             = ?1,
             secret_key        = ?2,
             key_date          = ?3,
             expires_at        = ?4,
             last_validated_at = ?5,
             status            = 'active'
         WHERE status IN ('active','grace')",
        params![new_token, secret_key, key_date, expires_at, now],
    );
}

/// Apply grace / expire logic when the license server is unreachable.
/// If `last_validated_at` is older than 7 days the license is expired;
/// otherwise it is moved to 'grace' status.
fn apply_grace_or_expire(conn: &Connection) {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339();
    let needs_expire: bool = conn.query_row(
        "SELECT COUNT(*) FROM licenses
         WHERE status IN ('active','grace') AND last_validated_at < ?1",
        params![cutoff],
        |r| r.get::<_, i64>(0),
    ).unwrap_or(0) > 0;

    if needs_expire {
        let _ = conn.execute(
            "UPDATE licenses SET status='expired'
             WHERE status IN ('active','grace') AND last_validated_at < ?1",
            params![cutoff],
        );
        tracing::warn!("License grace period exceeded (>7 days) — hub downgraded to Free");
    } else {
        let _ = conn.execute(
            "UPDATE licenses SET status='grace' WHERE status='active'",
            [],
        );
        tracing::warn!("License server unreachable — running in grace period");
    }
}

// ─── Core refresh logic ───────────────────────────────────────────────────────

/// Attempt a single `/v1/refresh` call against the license server.
///
/// On success:  persists new token + secret key; updates both in-memory caches.
/// On 401/403:  revokes locally; zeroes caches; logs warning.
/// On network error: calls `apply_grace_or_expire`; updates plan from grace logic.
///
/// Returns `true` if the refresh succeeded.
async fn do_refresh(
    db:             &Arc<Mutex<Connection>>,
    current_plan:   &Arc<RwLock<String>>,
    current_secret: &Arc<RwLock<String>>,
) -> bool {
    // Read current token from DB
    let token = {
        let conn = db.lock().unwrap();
        conn.query_row(
            "SELECT token FROM licenses WHERE status IN ('active','grace')
             ORDER BY activated_at DESC LIMIT 1",
            [],
            |r| r.get::<_, String>(0),
        ).ok()
    };

    let Some(token) = token else {
        tracing::debug!("No active license — skipping refresh");
        return false;
    };

    let license_url = std::env::var("SCHEDULA_LICENSE_URL")
        .unwrap_or_else(|_| DEFAULT_LICENSE_URL.to_string());
    let url = format!("{}/v1/refresh", license_url);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_default();

    match client.post(&url).json(&serde_json::json!({"token": token})).send().await {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<RefreshResponse>().await {
                Ok(r) => {
                    let conn = db.lock().unwrap();
                    persist_refresh(&conn, &r.new_token, &r.secret_key, &r.key_date, r.expires_at.as_deref());
                    let plan = effective_plan(&conn);
                    *current_plan.write().unwrap()   = plan.clone();
                    *current_secret.write().unwrap() = r.secret_key;
                    tracing::info!("License refreshed — plan={} key_date={}", plan, r.key_date);
                    true
                }
                Err(e) => {
                    tracing::warn!("Failed to parse refresh response: {}", e);
                    false
                }
            }
        }
        Ok(resp) => {
            let status = resp.status();
            tracing::warn!("License refresh rejected by server: HTTP {}", status);
            // 401 or 403 means the license has been revoked or is invalid
            if status.as_u16() == 401 || status.as_u16() == 403 {
                let conn = db.lock().unwrap();
                let _ = conn.execute(
                    "UPDATE licenses SET status='revoked' WHERE status IN ('active','grace')",
                    [],
                );
                *current_plan.write().unwrap()   = PLAN_FREE.to_string();
                *current_secret.write().unwrap() = String::new();
                tracing::warn!("License revoked — hub downgraded to Free immediately");
            }
            false
        }
        Err(e) => {
            tracing::warn!("License server unreachable: {} — applying grace period", e);
            let conn = db.lock().unwrap();
            apply_grace_or_expire(&conn);
            let plan = effective_plan(&conn);
            *current_plan.write().unwrap() = plan;
            false
        }
    }
}

// ─── Activation code redemption ───────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct ActivateCodeResponse {
    token: String,
}

/// Redeem a single-use activation code with the license server.
/// Returns the validated `LicenseClaims` on success.
///
/// The flow:
///   hub  →  POST {license_url}/v1/activate { code }
///   hub  ←  { token, jti, plan, … }
///   hub validates RS256 signature locally (paranoia check)
///   hub stores JWT in SQLite
pub async fn activate_with_code(code: &str) -> Result<(LicenseClaims, String), String> {
    let license_url = std::env::var("SCHEDULA_LICENSE_URL")
        .unwrap_or_else(|_| DEFAULT_LICENSE_URL.to_string());
    let url = format!("{}/v1/activate", license_url);

    // 45 s timeout: licence server may be on Render free tier (cold start ~30 s)
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .post(&url)
        .json(&serde_json::json!({ "code": code }))
        .send()
        .await
        .map_err(|e| format!("Could not reach license server: {e}"))?;

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Err("Invalid activation code — check for typos and try again.".into());
    }
    if resp.status() == reqwest::StatusCode::GONE {
        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        return Err(body["error"].as_str().unwrap_or("Activation code is expired or already used.").to_string());
    }
    if !resp.status().is_success() {
        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        return Err(body["error"].as_str().unwrap_or("Activation failed").to_string());
    }

    let data: ActivateCodeResponse = resp.json().await
        .map_err(|e| format!("Unexpected response from license server: {e}"))?;

    // Validate the returned JWT locally with the embedded public key
    let claims = validate_token(&data.token)?;

    Ok((claims, data.token))
}

// ─── Device-based checkout ────────────────────────────────────────────────────

/// Get or create a persistent device ID for this hub installation.
/// Stored in `device_config` table; survives restarts.
pub fn get_or_create_device_id(conn: &Connection) -> String {
    if let Ok(id) = conn.query_row(
        "SELECT value FROM device_config WHERE key='device_id'",
        [],
        |r| r.get::<_, String>(0),
    ) {
        return id;
    }

    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT OR IGNORE INTO device_config (key, value) VALUES ('device_id', ?1)",
        params![id],
    ).ok();
    id
}

/// Initiate a Stripe checkout session via the license server.
/// The `device_id` is embedded as subscription metadata so the webhook
/// can link the completed payment back to this hub automatically.
///
/// Returns the Stripe checkout URL to open in the user's browser.
pub async fn initiate_checkout(
    plan:           &str,
    billing_period: &str,
    device_id:      &str,
) -> Result<String, String> {
    let license_url = std::env::var("SCHEDULA_LICENSE_URL")
        .unwrap_or_else(|_| DEFAULT_LICENSE_URL.to_string());
    let url = format!("{}/billing/checkout", license_url);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "plan":           plan,
            "billing_period": billing_period,
            "device_id":      device_id,
        }))
        .send()
        .await
        .map_err(|e| format!("Could not reach license server: {e}"))?;

    if !resp.status().is_success() {
        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        return Err(body["error"].as_str().unwrap_or("Checkout initiation failed").to_string());
    }

    let data: serde_json::Value = resp.json().await
        .map_err(|e| format!("Unexpected response from license server: {e}"))?;

    data["checkout_url"].as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "License server returned no checkout URL".into())
}

/// Poll the license server once for a device-linked license.
/// Returns `true` if the license was found, validated, and stored.
async fn try_fetch_device_license(
    device_id:      &str,
    db:             &Arc<Mutex<Connection>>,
    current_plan:   &Arc<RwLock<String>>,
    current_secret: &Arc<RwLock<String>>,
) -> bool {
    let license_url = std::env::var("SCHEDULA_LICENSE_URL")
        .unwrap_or_else(|_| DEFAULT_LICENSE_URL.to_string());
    let url = format!("{}/v1/license/device/{}", license_url, device_id);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_default();

    let resp = match client.get(&url).send().await {
        Ok(r)  => r,
        Err(e) => { tracing::debug!("Device license poll error: {}", e); return false; }
    };

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return false; // Payment not completed yet — keep polling
    }

    if !resp.status().is_success() {
        return false;
    }

    let data: serde_json::Value = match resp.json().await {
        Ok(v)  => v,
        Err(_) => return false,
    };

    let token = match data["token"].as_str() {
        Some(t) => t.to_string(),
        None    => return false,
    };

    // Validate the JWT locally with the embedded RS256 public key
    let claims = match validate_token(&token) {
        Ok(c)  => c,
        Err(e) => { tracing::error!("Device license token validation failed: {}", e); return false; }
    };

    // Store the license and derive the plan — drop the lock before the await below
    let plan = {
        let conn = db.lock().unwrap();
        if let Err(e) = store_license(&conn, &claims, &token) {
            tracing::error!("Failed to store device license: {}", e);
            return false;
        }
        effective_plan(&conn)
        // MutexGuard dropped here
    };

    *current_plan.write().unwrap() = plan.clone();
    // secret_key will be populated by the refresh call below
    *current_secret.write().unwrap() = String::new();

    tracing::info!("Device license activated automatically: plan={}", plan);

    // Immediately refresh to get today's secret key (lock is already released)
    do_refresh(db, current_plan, current_secret).await;

    true
}

/// Background task: polls the license server for a device-linked license after checkout.
/// Runs every 30 seconds for up to 60 minutes, then gives up.
/// Broadcasts a WebSocket event when the license is activated.
pub async fn poll_for_device_license(
    device_id:      String,
    db:             Arc<Mutex<Connection>>,
    current_plan:   Arc<RwLock<String>>,
    current_secret: Arc<RwLock<String>>,
    tx:             tokio::sync::broadcast::Sender<String>,
) {
    const MAX_POLLS: u32 = 120; // 120 × 30s = 60 minutes
    for attempt in 1..=MAX_POLLS {
        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;

        if try_fetch_device_license(&device_id, &db, &current_plan, &current_secret).await {
            tracing::info!("Checkout polling succeeded on attempt {}", attempt);
            let _ = tx.send(
                serde_json::json!({"entity": "license", "action": "activate"}).to_string()
            );
            return;
        }

        tracing::debug!("Checkout poll {}/{} — awaiting payment", attempt, MAX_POLLS);
    }

    tracing::warn!("Checkout polling timed out after {} minutes", MAX_POLLS / 2);
}

// ─── Background refresh loop ──────────────────────────────────────────────────

/// Spawned as a tokio task on hub startup.
///
/// Immediately attempts a refresh (so the hub has today's key from the first
/// second), then repeats every 24 hours.  Updates `current_plan` and
/// `current_secret` on every successful refresh.
pub async fn background_refresh_loop(
    db:             Arc<Mutex<Connection>>,
    current_plan:   Arc<RwLock<String>>,
    current_secret: Arc<RwLock<String>>,
) {
    // First attempt: immediately on startup
    do_refresh(&db, &current_plan, &current_secret).await;

    // Subsequent: every 24 hours
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(24 * 3600)).await;
        do_refresh(&db, &current_plan, &current_secret).await;
    }
}

/// Called synchronously on hub startup to expire any lapsed licenses and
/// log the current license state.  The async `background_refresh_loop` handles
/// the actual refresh; this is just a fast local check.
pub fn startup_license_check(conn: &Connection) {
    // Hard-expire tokens that have passed their JWT expiry AND exceeded the 7-day grace.
    // Tokens still within the grace window are handled by effective_plan().
    let grace_cutoff = (chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339();
    let now          = chrono::Utc::now().to_rfc3339();
    let _ = conn.execute(
        "UPDATE licenses SET status='expired'
         WHERE status IN ('active','grace')
           AND expires_at IS NOT NULL
           AND expires_at < ?1
           AND last_validated_at < ?2",
        params![now, grace_cutoff],
    );

    let info = get_license_info(conn);
    if info.active {
        tracing::info!(
            "License loaded: plan={} org={:?} status={}",
            info.plan, info.org_name, info.status
        );
    } else {
        tracing::info!("No active license — running on Free plan");
    }
}
