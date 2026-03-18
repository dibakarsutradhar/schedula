/// Schedula License Server
///
/// Issues and validates RS256-signed license JWTs for Schedula Hub deployments.
///
/// Routes:
///   POST /v1/issue             — (admin-key) issue a new license JWT
///   POST /v1/activate          — (public) redeem a single-use activation code → returns JWT
///   POST /v1/validate          — validate a token (called by hub on 24 h re-check)
///   POST /v1/refresh           — daily refresh: returns new 48-hour JWT + today's symmetric key
///   GET  /v1/licenses          — (admin-key) list all issued licenses
///   DELETE /v1/licenses/:jti   — (admin-key) revoke a license
///   GET  /health
///
///   POST /billing/checkout     — create Stripe Checkout Session
///   GET  /billing/portal       — redirect to Stripe Customer Portal
///   POST /billing/webhook      — handle Stripe subscription events
///   GET  /billing/success      — post-checkout success page
///
/// Configuration (env vars or CLI flags):
///   --port                  Listen port (default: 8080)
///   --db-path               SQLite path (default: ./schedula-license.db)
///   --private-key           RS256 private key PEM path (default: ./keys/private.pem)
///   --public-key            RS256 public key PEM path  (default: ./keys/public.pem)
///   --admin-key             Secret for protected endpoints (required)
///
///   Billing (all optional — set to enable Stripe integration):
///   --stripe-secret-key         STRIPE_SECRET_KEY
///   --stripe-webhook-secret     STRIPE_WEBHOOK_SECRET
///   --stripe-price-pro-monthly  STRIPE_PRICE_PRO_MONTHLY  (Stripe price ID)
///   --stripe-price-pro-annual   STRIPE_PRICE_PRO_ANNUAL
///   --stripe-price-inst-monthly STRIPE_PRICE_INST_MONTHLY
///   --stripe-price-inst-annual  STRIPE_PRICE_INST_ANNUAL
///   --app-url                   APP_URL (for redirect URLs, default: https://schedula.app)
///
///   Email (all optional — set to enable transactional emails):
///   --smtp-host     SMTP_HOST
///   --smtp-port     SMTP_PORT (default: 587)
///   --smtp-username SMTP_USERNAME
///   --smtp-password SMTP_PASSWORD
///   --smtp-from     SMTP_FROM (default: noreply@schedula.app)

mod billing;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use clap::Parser;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use rand::RngCore;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

// ─── CLI ─────────────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(name = "schedula-license", about = "Schedula License Server")]
struct Args {
    #[arg(long, default_value = "8080")]
    port: u16,

    #[arg(long, default_value = "schedula-license.db")]
    db_path: String,

    #[arg(long, default_value = "keys/private.pem")]
    private_key: String,

    #[arg(long, default_value = "keys/public.pem")]
    public_key: String,

    /// Secret header value required for /v1/issue and /v1/licenses
    #[arg(long, env = "SCHEDULA_ADMIN_KEY")]
    admin_key: String,

    // ── Stripe ───────────────────────────────────────────────────────────────

    #[arg(long, env = "STRIPE_SECRET_KEY", default_value = "")]
    stripe_secret_key: String,

    #[arg(long, env = "STRIPE_WEBHOOK_SECRET", default_value = "")]
    stripe_webhook_secret: String,

    #[arg(long, env = "STRIPE_PRICE_PRO_MONTHLY", default_value = "")]
    price_pro_monthly: String,

    #[arg(long, env = "STRIPE_PRICE_PRO_ANNUAL", default_value = "")]
    price_pro_annual: String,

    #[arg(long, env = "STRIPE_PRICE_INST_MONTHLY", default_value = "")]
    price_inst_monthly: String,

    #[arg(long, env = "STRIPE_PRICE_INST_ANNUAL", default_value = "")]
    price_inst_annual: String,

    #[arg(long, env = "APP_URL", default_value = "https://schedula.app")]
    app_url: String,

    // ── SMTP ─────────────────────────────────────────────────────────────────

    #[arg(long, env = "SMTP_HOST", default_value = "")]
    smtp_host: String,

    #[arg(long, env = "SMTP_PORT", default_value_t = 587)]
    smtp_port: u16,

    #[arg(long, env = "SMTP_USERNAME", default_value = "")]
    smtp_username: String,

    #[arg(long, env = "SMTP_PASSWORD", default_value = "")]
    smtp_password: String,

    #[arg(long, env = "SMTP_FROM", default_value = "noreply@schedula.app")]
    smtp_from: String,

    // ── Paddle ───────────────────────────────────────────────────────────────

    #[arg(long, env = "PADDLE_API_KEY", default_value = "")]
    paddle_api_key: String,

    #[arg(long, env = "PADDLE_WEBHOOK_SECRET", default_value = "")]
    paddle_webhook_secret: String,

    #[arg(long, env = "PADDLE_PRICE_PRO_MONTHLY", default_value = "")]
    paddle_price_pro_monthly: String,

    #[arg(long, env = "PADDLE_PRICE_PRO_ANNUAL", default_value = "")]
    paddle_price_pro_annual: String,

    #[arg(long, env = "PADDLE_PRICE_INST_MONTHLY", default_value = "")]
    paddle_price_inst_monthly: String,

    #[arg(long, env = "PADDLE_PRICE_INST_ANNUAL", default_value = "")]
    paddle_price_inst_annual: String,

    /// Paddle.js client token (safe to embed in frontend)
    #[arg(long, env = "PADDLE_CLIENT_TOKEN", default_value = "")]
    paddle_client_token: String,

    // ── Sales notifications ───────────────────────────────────────────────────

    /// Email address to notify when a new invoice request arrives
    #[arg(long, env = "SALES_EMAIL", default_value = "")]
    sales_email: String,

    /// Slack incoming webhook URL for invoice notifications
    #[arg(long, env = "SLACK_WEBHOOK_URL", default_value = "")]
    slack_webhook_url: String,
}

// ─── State ────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub db:           Arc<Mutex<Connection>>,
    pub encoding_key: Arc<EncodingKey>,
    decoding_key:     Arc<DecodingKey>,
    pub admin_key:    String,
    pub billing:      Arc<billing::BillingConfig>,
    /// True when STRIPE_SECRET_KEY starts with "sk_test_".
    /// Passed to the success page so users see a test-mode banner.
    pub stripe_test_mode: bool,
}

impl AppState {
    /// Returns true if the `X-Admin-Key` header matches the configured admin key.
    pub fn require_admin(&self, headers: &axum::http::HeaderMap) -> bool {
        headers.get("x-admin-key")
            .and_then(|v| v.to_str().ok())
            .map(|v| v == self.admin_key)
            .unwrap_or(false)
    }
}

// ─── Models ───────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LicenseClaims {
    pub iss:      String,
    pub sub:      String,
    pub plan:     String,
    pub org_name: Option<String>,
    pub exp:      i64,
    pub iat:      i64,
    pub jti:      String,
}

#[derive(Debug, Deserialize)]
struct IssueRequest {
    plan:          String,
    org_name:      Option<String>,
    duration_days: Option<i64>,
}

#[derive(Debug, Serialize)]
struct IssueResponse {
    token:      String,
    jti:        String,
    plan:       String,
    org_name:   Option<String>,
    expires_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ValidateRequest {
    token: String,
}

#[derive(Debug, Serialize)]
struct ValidateResponse {
    valid:    bool,
    plan:     Option<String>,
    org_name: Option<String>,
    jti:      Option<String>,
    message:  Option<String>,
}

#[derive(Debug, Serialize)]
struct LicenseRecord {
    jti:        String,
    plan:       String,
    org_name:   Option<String>,
    issued_at:  String,
    expires_at: Option<String>,
    revoked:    bool,
}

/// POST /v1/refresh — daily token refresh request from hub or app
#[derive(Debug, Deserialize)]
struct RefreshRequest {
    token: String,
}

/// Response body for /v1/refresh
#[derive(Debug, Serialize)]
struct RefreshResponse {
    /// A new RS256-signed JWT valid for 48 h (same JTI, fresh expiry)
    new_token:  String,
    /// Today's 256-bit HMAC key (hex-encoded) — rotates at UTC midnight
    secret_key: String,
    /// UTC date this key is valid for: "YYYY-MM-DD"
    key_date:   String,
    /// Plan from the original license claims
    plan:       String,
    org_name:   Option<String>,
    /// ISO-8601 expiry of new_token (48 h from now)
    expires_at: String,
}

// ─── DB ───────────────────────────────────────────────────────────────────────

fn open_db(path: &str) -> Connection {
    let conn = Connection::open(path).expect("Failed to open license DB");
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;").ok();

    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS licenses (
            jti        TEXT    PRIMARY KEY,
            plan       TEXT    NOT NULL,
            org_name   TEXT,
            token      TEXT    NOT NULL,
            issued_at  TEXT    NOT NULL DEFAULT (datetime('now')),
            expires_at TEXT,
            revoked    INTEGER NOT NULL DEFAULT 0
        );

        -- Daily symmetric key rotation table.
        -- Each row stores one 256-bit HMAC key valid for a UTC calendar day.
        -- Rows older than 8 days are purged automatically by the rotation task.
        CREATE TABLE IF NOT EXISTS daily_keys (
            key_date   TEXT PRIMARY KEY,           -- YYYY-MM-DD (UTC)
            key_hex    TEXT NOT NULL,              -- 256-bit key, hex-encoded (64 chars)
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS customers (
            stripe_customer_id TEXT PRIMARY KEY,
            email              TEXT NOT NULL UNIQUE,
            plan               TEXT,
            subscription_id    TEXT,
            status             TEXT NOT NULL DEFAULT 'none',
            period_end         TEXT,
            trial_end          TEXT,
            jti                TEXT,
            paddle_customer_id TEXT,
            created_at         TEXT NOT NULL DEFAULT (datetime('now'))
        );

        -- Single-use activation codes issued on payment; burned when the hub redeems them.
        -- The JWT is never stored in email — only the short-lived code is.
        CREATE TABLE IF NOT EXISTS activation_codes (
            code          TEXT    PRIMARY KEY,
            customer_email TEXT   NOT NULL,
            plan          TEXT    NOT NULL,
            org_name      TEXT,
            duration_days INTEGER,
            expires_at    TEXT    NOT NULL,    -- ISO-8601 UTC; code is invalid after this
            used_at       TEXT,               -- NULL = not yet used
            jti           TEXT               -- set to the license JTI when redeemed
        );

        CREATE TABLE IF NOT EXISTS invoice_requests (
            id            TEXT PRIMARY KEY,
            org_name      TEXT NOT NULL,
            contact_name  TEXT NOT NULL DEFAULT '',
            contact_email TEXT NOT NULL,
            plan          TEXT NOT NULL,
            user_count    INTEGER,
            country       TEXT,
            notes         TEXT,
            status        TEXT NOT NULL DEFAULT 'pending',
            jti           TEXT,
            created_at    TEXT NOT NULL DEFAULT (datetime('now')),
            paid_at       TEXT,
            issued_at     TEXT
        );

        -- Device-linked licenses: stored here after a device-based Stripe checkout,
        -- polled and fetched by the hub sidecar, then deleted (single-use).
        CREATE TABLE IF NOT EXISTS device_licenses (
            device_id  TEXT    PRIMARY KEY,
            token      TEXT    NOT NULL,
            plan       TEXT    NOT NULL,
            jti        TEXT    NOT NULL,
            expires_at TEXT,
            created_at TEXT    NOT NULL DEFAULT (datetime('now'))
        );
    ").expect("Failed to create tables");

    // Migrate existing customers table (no-op if column already exists)
    conn.execute("ALTER TABLE customers ADD COLUMN paddle_customer_id TEXT", []).ok();

    conn
}

// ─── Auth guard ───────────────────────────────────────────────────────────────

fn require_admin(headers: &HeaderMap, admin_key: &str) -> bool {
    headers.get("x-admin-key")
        .and_then(|v| v.to_str().ok())
        .map(|v| v == admin_key)
        .unwrap_or(false)
}

pub fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, "Invalid or missing X-Admin-Key").into_response()
}

fn json_err(msg: &str) -> Response {
    (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": msg}))).into_response()
}

// ─── Core license issuance ────────────────────────────────────────────────────
//
// Shared by the HTTP handler (/v1/issue) and the Stripe billing webhook.
// Returns (jwt_token_string, jti).

pub fn issue_license_core(
    conn:          &Connection,
    encoding_key:  &EncodingKey,
    plan:          &str,
    org_name:      Option<&str>,
    duration_days: Option<i64>,
) -> Result<(String, String), String> {
    let jti = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();

    let (exp, expires_at) = match duration_days {
        Some(d) if d > 0 => {
            let ts = now + d * 86400;
            let s  = chrono::DateTime::from_timestamp(ts, 0).map(|d| d.to_rfc3339());
            (ts, s)
        }
        _ => (0, None), // perpetual
    };

    let claims = LicenseClaims {
        iss:      "schedula-license".into(),
        sub:      jti.clone(),
        plan:     plan.to_string(),
        org_name: org_name.map(|s| s.to_string()),
        exp,
        iat:      now,
        jti:      jti.clone(),
    };

    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("schedula-v1".into());

    let token = encode(&header, &claims, encoding_key)
        .map_err(|e| format!("Failed to sign token: {e}"))?;

    let issued_at = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO licenses (jti, plan, org_name, token, issued_at, expires_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![jti, plan, org_name, token, issued_at, expires_at],
    ).map_err(|e| e.to_string())?;

    Ok((token, jti))
}

// ─── Activation codes ─────────────────────────────────────────────────────────

/// Generate a short, human-readable single-use activation code.
/// Format: `SCHED-XXXX-XXXX-XXXX-XXXX`  (16 uppercase hex chars, 64-bit entropy).
/// Sufficient for a short-lived (30-minute) code — brute-force infeasible.
pub fn generate_activation_code() -> String {
    use rand::Rng;
    let bytes: [u8; 8] = rand::thread_rng().gen();
    let hex = bytes.iter().map(|b| format!("{:02X}", b)).collect::<String>();
    format!("SCHED-{}-{}-{}-{}", &hex[0..4], &hex[4..8], &hex[8..12], &hex[12..16])
}

/// Store an activation code tied to an email + plan.
/// `duration_days`: None = perpetual, Some(n) = expires after n days from activation.
pub fn store_activation_code(
    conn:          &Connection,
    code:          &str,
    customer_email: &str,
    plan:          &str,
    org_name:      Option<&str>,
    duration_days: Option<i64>,
) -> Result<(), String> {
    let expires_at = (chrono::Utc::now() + chrono::Duration::minutes(30)).to_rfc3339();
    conn.execute(
        "INSERT INTO activation_codes
             (code, customer_email, plan, org_name, duration_days, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![code, customer_email, plan, org_name, duration_days, expires_at],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/// POST /v1/issue — issue a new license JWT (admin only)
async fn issue_handler(
    State(state): State<AppState>,
    headers:      HeaderMap,
    Json(body):   Json<IssueRequest>,
) -> Response {
    if !require_admin(&headers, &state.admin_key) { return forbidden(); }

    let valid_plans = ["pro", "institution"];
    if !valid_plans.contains(&body.plan.as_str()) {
        return json_err("plan must be 'pro' or 'institution'");
    }

    let conn = state.db.lock().unwrap();
    match issue_license_core(&conn, &state.encoding_key, &body.plan,
                             body.org_name.as_deref(), body.duration_days)
    {
        Ok((token, jti)) => {
            let expires_at: Option<String> = conn.query_row(
                "SELECT expires_at FROM licenses WHERE jti=?1",
                params![jti],
                |r| r.get(0),
            ).ok();
            Json(IssueResponse {
                token,
                jti,
                plan:       body.plan,
                org_name:   body.org_name,
                expires_at,
            }).into_response()
        }
        Err(e) => json_err(&e),
    }
}

/// POST /v1/activate — redeem a single-use activation code, receive a JWT
///
/// Called by the hub directly (server-to-server). The JWT is never sent via email;
/// only the short-lived code is. This keeps the actual license credential off email.
async fn activate_handler(
    State(state): State<AppState>,
    Json(body):   Json<serde_json::Value>,
) -> Response {
    let code = match body["code"].as_str() {
        Some(c) => c.trim().to_uppercase(),
        None    => return json_err("'code' field is required"),
    };

    let conn = state.db.lock().unwrap();

    // Look up the code
    let row: Option<(String, String, Option<String>, Option<i64>, String, Option<String>)> = conn.query_row(
        "SELECT customer_email, plan, org_name, duration_days, expires_at, used_at
         FROM activation_codes WHERE code=?1",
        params![code],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
    ).ok();

    let (_customer_email, plan, org_name, duration_days, expires_at_str, used_at) = match row {
        Some(r) => r,
        None    => return (StatusCode::NOT_FOUND,
                           Json(serde_json::json!({"error": "Invalid activation code"}))).into_response(),
    };

    if used_at.is_some() {
        return (StatusCode::GONE,
                Json(serde_json::json!({"error": "Activation code has already been used"}))).into_response();
    }

    // Check expiry
    if let Ok(exp) = chrono::DateTime::parse_from_rfc3339(&expires_at_str) {
        if chrono::Utc::now() > exp {
            return (StatusCode::GONE,
                    Json(serde_json::json!({"error": "Activation code has expired. Request a new one from your confirmation email or contact support."}))).into_response();
        }
    }

    // Issue the JWT
    match issue_license_core(&conn, &state.encoding_key, &plan, org_name.as_deref(), duration_days) {
        Ok((token, jti)) => {
            // Burn the code immediately (single-use)
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "UPDATE activation_codes SET used_at=?1, jti=?2 WHERE code=?3",
                params![now, jti, code],
            ).ok();

            let expires_at: Option<String> = conn.query_row(
                "SELECT expires_at FROM licenses WHERE jti=?1",
                params![jti],
                |r| r.get(0),
            ).ok();

            Json(serde_json::json!({
                "token":      token,
                "jti":        jti,
                "plan":       plan,
                "org_name":   org_name,
                "expires_at": expires_at,
            })).into_response()
        }
        Err(e) => json_err(&e),
    }
}

/// GET /v1/license/device/:device_id — poll for a device-linked license
///
/// Called by the hub sidecar after initiating a Stripe checkout. Returns the JWT
/// once the webhook has been processed. Deletes the record on first fetch
/// (the hub stores it locally; no need to keep it here).
async fn device_license_handler(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
) -> Response {
    let conn = state.db.lock().unwrap();

    let row: Option<(String, String, Option<String>)> = conn.query_row(
        "SELECT token, plan, expires_at FROM device_licenses WHERE device_id=?1",
        params![device_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    ).ok();

    match row {
        None => (StatusCode::NOT_FOUND,
                 Json(serde_json::json!({"error": "No license ready for this device yet"}))).into_response(),
        Some((token, plan, expires_at)) => {
            // Single-use: delete after first successful fetch
            conn.execute("DELETE FROM device_licenses WHERE device_id=?1", params![device_id]).ok();
            Json(serde_json::json!({
                "token":      token,
                "plan":       plan,
                "expires_at": expires_at,
            })).into_response()
        }
    }
}

/// POST /v1/validate — validate a token (called by hub every 24 h)
async fn validate_handler(
    State(state): State<AppState>,
    Json(body):   Json<ValidateRequest>,
) -> Response {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&["schedula-license"]);
    validation.validate_exp = false; // We handle exp=0 (perpetual) manually

    let decoded = match decode::<LicenseClaims>(&body.token, &state.decoding_key, &validation) {
        Ok(d)  => d,
        Err(e) => return Json(ValidateResponse {
            valid: false, plan: None, org_name: None, jti: None,
            message: Some(format!("Invalid token: {e}")),
        }).into_response(),
    };

    let claims = decoded.claims;

    // Manual exp check (exp=0 means perpetual)
    if claims.exp > 0 {
        let now = chrono::Utc::now().timestamp();
        if now > claims.exp {
            return Json(ValidateResponse {
                valid: false, plan: None, org_name: None, jti: Some(claims.jti),
                message: Some("Token expired".into()),
            }).into_response();
        }
    }

    // Check revocation in DB
    let revoked: bool = {
        let conn = state.db.lock().unwrap();
        conn.query_row(
            "SELECT revoked FROM licenses WHERE jti=?1",
            params![claims.jti],
            |r| r.get::<_, i64>(0),
        ).map(|v| v == 1).unwrap_or(false)
    };

    if revoked {
        return Json(ValidateResponse {
            valid: false, plan: None, org_name: None, jti: Some(claims.jti),
            message: Some("License has been revoked".into()),
        }).into_response();
    }

    Json(ValidateResponse {
        valid:    true,
        plan:     Some(claims.plan),
        org_name: claims.org_name,
        jti:      Some(claims.jti),
        message:  None,
    }).into_response()
}

/// GET /v1/licenses — list all issued licenses (admin only)
async fn list_licenses_handler(
    State(state): State<AppState>,
    headers:      HeaderMap,
) -> Response {
    if !require_admin(&headers, &state.admin_key) { return forbidden(); }

    let conn = state.db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT jti, plan, org_name, issued_at, expires_at, revoked \
         FROM licenses ORDER BY issued_at DESC"
    ).unwrap();

    let records: Vec<LicenseRecord> = stmt
        .query_map([], |r| Ok(LicenseRecord {
            jti:        r.get(0)?,
            plan:       r.get(1)?,
            org_name:   r.get(2)?,
            issued_at:  r.get(3)?,
            expires_at: r.get(4)?,
            revoked:    r.get::<_, i64>(5)? == 1,
        }))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    Json(records).into_response()
}

/// DELETE /v1/licenses/:jti — revoke a license (admin only)
async fn revoke_handler(
    State(state): State<AppState>,
    headers:      HeaderMap,
    Path(jti):    Path<String>,
) -> Response {
    if !require_admin(&headers, &state.admin_key) { return forbidden(); }

    let conn = state.db.lock().unwrap();
    let rows = conn.execute(
        "UPDATE licenses SET revoked=1 WHERE jti=?1",
        params![jti],
    ).unwrap_or(0);

    if rows == 0 {
        return (StatusCode::NOT_FOUND, "License not found").into_response();
    }
    Json(serde_json::json!({"revoked": true, "jti": jti})).into_response()
}

// ─── Daily key rotation ────────────────────────────────────────────────────────

/// Generates today's daily symmetric key if it doesn't already exist,
/// then purges keys older than 8 days.  Called once at startup and once at
/// every UTC midnight thereafter.
fn ensure_todays_key(conn: &Connection) {
    let today = chrono::Utc::now().date_naive().to_string(); // "YYYY-MM-DD"

    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM daily_keys WHERE key_date=?1",
        params![today],
        |r| r.get(0),
    ).unwrap_or(0);

    if exists == 0 {
        let mut raw = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut raw);
        let key_hex = hex::encode(raw);
        conn.execute(
            "INSERT OR IGNORE INTO daily_keys (key_date, key_hex) VALUES (?1, ?2)",
            params![today, key_hex],
        ).ok();
        tracing::info!("Daily key generated for {}", today);
    }

    // Purge keys older than 8 days (keeps last week + today + one buffer day)
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(8))
        .date_naive().to_string();
    conn.execute(
        "DELETE FROM daily_keys WHERE key_date < ?1",
        params![cutoff],
    ).ok();
}

/// Background task: ensures a fresh key exists each UTC day.
/// Wakes up at 00:01 UTC to generate the next day's key and purge stale ones.
async fn daily_key_rotation_task(db: Arc<Mutex<Connection>>) {
    loop {
        // Sleep until 00:01:00 UTC tomorrow
        let now    = chrono::Utc::now();
        let target = (now + chrono::Duration::days(1))
            .date_naive()
            .and_hms_opt(0, 1, 0)
            .unwrap()
            .and_utc();
        let secs = (target - now).num_seconds().max(1) as u64;
        tokio::time::sleep(tokio::time::Duration::from_secs(secs)).await;

        let conn = db.lock().unwrap();
        ensure_todays_key(&conn);
    }
}

// ─── Refresh handler ───────────────────────────────────────────────────────────

/// POST /v1/refresh — daily credential refresh for hub and desktop clients.
///
/// The caller sends their current license JWT (which may already be expired
/// within the 48-hour token window).  The server:
///   1. Verifies the RS256 signature (always — prevents forged tokens).
///   2. Checks the license is not revoked in the DB.
///   3. Returns today's 256-bit symmetric key + a fresh 48-hour JWT.
///
/// The caller should call this every ~24 h.  Missing one call leaves a 24-hour
/// buffer (the new token is valid 48 h).  Missing >7 days triggers the hub's
/// grace-period downgrade.
async fn refresh_handler(
    State(state): State<AppState>,
    Json(body):   Json<RefreshRequest>,
) -> Response {
    // ── Step 1: verify RS256 signature (exp=0 perpetual tokens accepted) ──────
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&["schedula-license"]);
    validation.validate_exp = false; // we accept expired tokens during the grace window

    let decoded = match decode::<LicenseClaims>(&body.token, &state.decoding_key, &validation) {
        Ok(d)  => d,
        Err(e) => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({
            "error": format!("Invalid token signature: {e}")
        }))).into_response(),
    };
    let claims = decoded.claims;

    // ── Step 2: check revocation ──────────────────────────────────────────────
    let conn = state.db.lock().unwrap();
    let revoked: bool = conn.query_row(
        "SELECT revoked FROM licenses WHERE jti=?1",
        params![claims.jti],
        |r| r.get::<_, i64>(0),
    ).map(|v| v == 1).unwrap_or(true); // treat missing license as revoked

    if revoked {
        tracing::warn!("Refresh rejected — license revoked: jti={}", claims.jti);
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({
            "error": "License has been revoked"
        }))).into_response();
    }

    // ── Step 3: get today's symmetric key ─────────────────────────────────────
    let today = chrono::Utc::now().date_naive().to_string();
    let secret_key: String = match conn.query_row(
        "SELECT key_hex FROM daily_keys WHERE key_date=?1",
        params![today],
        |r| r.get(0),
    ) {
        Ok(k) => k,
        Err(_) => {
            tracing::error!("No daily key for {} — rotation task may not be running", today);
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": "Daily key unavailable — contact support"
            }))).into_response();
        }
    };

    // ── Step 4: issue a fresh 48-hour JWT (same JTI, refreshed expiry) ────────
    let now     = chrono::Utc::now().timestamp();
    let new_exp = now + 2 * 86_400; // 48 h

    let new_claims = LicenseClaims {
        iss:      "schedula-license".into(),
        sub:      claims.jti.clone(),
        plan:     claims.plan.clone(),
        org_name: claims.org_name.clone(),
        exp:      new_exp,
        iat:      now,
        jti:      claims.jti.clone(), // same identity
    };

    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("schedula-v1".into());

    let new_token = match encode(&header, &new_claims, &state.encoding_key) {
        Ok(t)  => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "error": format!("Failed to sign refreshed token: {e}")
        }))).into_response(),
    };

    let expires_at = chrono::DateTime::from_timestamp(new_exp, 0)
        .map(|d| d.to_rfc3339())
        .unwrap_or_default();

    // Persist the refreshed token so it can be validated on next /v1/validate call
    conn.execute(
        "UPDATE licenses SET token=?1, expires_at=?2 WHERE jti=?3",
        params![new_token, expires_at, claims.jti],
    ).ok();

    tracing::info!(
        "Token refreshed: plan={} jti={} key_date={}",
        claims.plan, claims.jti, today
    );

    Json(RefreshResponse {
        new_token,
        secret_key,
        key_date:   today,
        plan:       claims.plan,
        org_name:   claims.org_name,
        expires_at,
    }).into_response()
}

// ─── Main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("schedula_license=info".parse().unwrap())
        )
        .init();

    let args = Args::parse();

    // Keys can be supplied as env vars (PRIVATE_KEY_PEM / PUBLIC_KEY_PEM) so that
    // container deployments (Render, Fly.io) don't need the key files baked into
    // the image. Env var takes priority; falls back to the --private-key file path.
    let private_pem = std::env::var("PRIVATE_KEY_PEM").ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| std::fs::read_to_string(&args.private_key)
            .unwrap_or_else(|_| panic!("Cannot read private key: {} (or set PRIVATE_KEY_PEM env var)", args.private_key)));
    let public_pem = std::env::var("PUBLIC_KEY_PEM").ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| std::fs::read_to_string(&args.public_key)
            .unwrap_or_else(|_| panic!("Cannot read public key: {} (or set PUBLIC_KEY_PEM env var)", args.public_key)));

    let encoding_key = EncodingKey::from_rsa_pem(private_pem.as_bytes())
        .expect("Invalid RSA private key");
    let decoding_key = DecodingKey::from_rsa_pem(public_pem.as_bytes())
        .expect("Invalid RSA public key");

    let conn = open_db(&args.db_path);

    // Ensure today's symmetric key exists on startup
    ensure_todays_key(&conn);

    let billing = Arc::new(billing::BillingConfig {
        http:                  reqwest::Client::new(),
        stripe_secret:         args.stripe_secret_key,
        stripe_webhook_secret: args.stripe_webhook_secret,
        price_pro_monthly:     args.price_pro_monthly,
        price_pro_annual:      args.price_pro_annual,
        price_inst_monthly:    args.price_inst_monthly,
        price_inst_annual:     args.price_inst_annual,
        app_url:               args.app_url,
        paddle_api_key:            args.paddle_api_key,
        paddle_webhook_secret:     args.paddle_webhook_secret,
        paddle_price_pro_monthly:  args.paddle_price_pro_monthly,
        paddle_price_pro_annual:   args.paddle_price_pro_annual,
        paddle_price_inst_monthly: args.paddle_price_inst_monthly,
        paddle_price_inst_annual:  args.paddle_price_inst_annual,
        paddle_client_token:       args.paddle_client_token,
        sales_email:               args.sales_email,
        slack_webhook_url:         args.slack_webhook_url,
        email: billing::EmailConfig {
            smtp_host:     args.smtp_host,
            smtp_port:     args.smtp_port,
            smtp_username: args.smtp_username,
            smtp_password: args.smtp_password,
            smtp_from:     args.smtp_from,
        },
    });

    let stripe_test_mode = billing.stripe_secret.starts_with("sk_test_");
    let state = AppState {
        db:               Arc::new(Mutex::new(conn)),
        encoding_key:     Arc::new(encoding_key),
        decoding_key:     Arc::new(decoding_key),
        admin_key:        args.admin_key,
        billing,
        stripe_test_mode,
    };

    // Spawn daily key rotation: wakes at UTC midnight to generate the next day's key
    tokio::spawn(daily_key_rotation_task(state.db.clone()));

    let app = Router::new()
        // ── License management ──────────────────────────────────────────────
        .route("/v1/issue",                     post(issue_handler))
        .route("/v1/activate",                  post(activate_handler))
        .route("/v1/license/device/:device_id", get(device_license_handler))
        .route("/v1/validate",                  post(validate_handler))
        .route("/v1/refresh",         post(refresh_handler))
        .route("/v1/licenses",        get(list_licenses_handler))
        .route("/v1/licenses/:jti",   delete(revoke_handler))
        // ── Billing / Stripe ─────────────────────────────────────────────────
        .route("/billing/checkout",                    post(billing::checkout_handler))
        .route("/billing/portal",                      get(billing::portal_handler))
        .route("/billing/webhook",                     post(billing::webhook_handler))
        .route("/billing/success",                     get(billing::success_handler))
        // ── Billing / Paddle ─────────────────────────────────────────────────
        .route("/billing/paddle/webhook",              post(billing::paddle_webhook_handler))
        .route("/billing/config",                      get(billing::billing_config_handler))
        // ── Invoice / PO flow ────────────────────────────────────────────────
        .route("/billing/invoice-request",             post(billing::invoice_request_handler))
        .route("/billing/invoice-requests",            get(billing::list_invoices_handler))
        .route("/billing/invoice-requests/:id/issue",  post(billing::issue_invoice_handler))
        // ── Admin dashboard ──────────────────────────────────────────────────
        .route("/admin",                               get(billing::admin_handler))
        // ── Health ──────────────────────────────────────────────────────────
        .route("/health", get(|| async { Json(serde_json::json!({"status": "ok"})) }))
        .with_state(state)
        .layer(tower_http::cors::CorsLayer::permissive());

    let addr = format!("0.0.0.0:{}", args.port);
    tracing::info!("Schedula License Server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
