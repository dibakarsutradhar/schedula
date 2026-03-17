/// Billing integration for Schedula License Server.
///
/// Routes (added in main.rs):
///   POST /billing/checkout              — Stripe Checkout Session
///   GET  /billing/portal                — Stripe Customer Portal redirect
///   POST /billing/webhook               — Stripe subscription events
///   GET  /billing/success               — post-checkout success page
///   POST /billing/paddle/webhook        — Paddle subscription events
///   GET  /billing/config                — public Paddle client config
///   POST /billing/invoice-request       — submit PO/invoice request
///   GET  /billing/invoice-requests      — list invoice requests (admin)
///   POST /billing/invoice-requests/:id/issue — issue license for invoice (admin)
///   GET  /admin                         — admin dashboard HTML

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    Json,
};
use chrono::Utc;
use hmac::{Hmac, Mac};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

// ─── Config ───────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct BillingConfig {
    pub http:                  reqwest::Client,
    // ── Stripe ───────────────────────────────────────────────────────────────
    pub stripe_secret:         String,
    pub stripe_webhook_secret: String,
    pub price_pro_monthly:     String,
    pub price_pro_annual:      String,
    pub price_inst_monthly:    String,
    pub price_inst_annual:     String,
    pub app_url:               String,
    // ── Paddle ───────────────────────────────────────────────────────────────
    pub paddle_api_key:            String,
    pub paddle_webhook_secret:     String,
    pub paddle_price_pro_monthly:  String,
    pub paddle_price_pro_annual:   String,
    pub paddle_price_inst_monthly: String,
    pub paddle_price_inst_annual:  String,
    pub paddle_client_token:       String,
    // ── Sales notifications ───────────────────────────────────────────────────
    pub sales_email:           String,
    pub slack_webhook_url:     String,
    // ── Email ─────────────────────────────────────────────────────────────────
    pub email:                 EmailConfig,
}

#[derive(Clone)]
pub struct EmailConfig {
    pub smtp_host:     String,
    pub smtp_port:     u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub smtp_from:     String,
}

// ─── Stripe API helper ────────────────────────────────────────────────────────

async fn stripe_post(
    http:   &reqwest::Client,
    secret: &str,
    path:   &str,
    form:   &[(&str, &str)],
) -> Result<serde_json::Value, String> {
    let url = format!("https://api.stripe.com/v1{path}");
    let resp = http
        .post(&url)
        .basic_auth(secret, Some(""))
        .form(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    if json["error"].is_object() {
        let msg = json["error"]["message"].as_str().unwrap_or("Stripe error");
        return Err(msg.to_string());
    }
    Ok(json)
}

// ─── Request / Response types ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CheckoutRequest {
    pub email:          String,
    pub plan:           String,  // "pro" | "institution"
    pub billing_period: String,  // "monthly" | "annual"
}

#[derive(Serialize)]
pub struct CheckoutResponse {
    pub checkout_url: String,
}

#[derive(Deserialize)]
pub struct PortalQuery {
    pub email:      String,
    pub return_url: Option<String>,
}

// ─── POST /billing/checkout ───────────────────────────────────────────────────

pub async fn checkout_handler(
    State(state): State<crate::AppState>,
    Json(body):   Json<CheckoutRequest>,
) -> Response {
    if state.billing.stripe_secret.is_empty() {
        return (StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Stripe not configured on this server"}))).into_response();
    }

    // Resolve price ID for the chosen plan + billing period
    let price_id: &str = match (body.plan.as_str(), body.billing_period.as_str()) {
        ("pro",         "monthly") => &state.billing.price_pro_monthly,
        ("pro",         "annual")  => &state.billing.price_pro_annual,
        ("institution", "monthly") => &state.billing.price_inst_monthly,
        ("institution", "annual")  => &state.billing.price_inst_annual,
        _ => return (StatusCode::BAD_REQUEST,
                     Json(serde_json::json!({"error": "Invalid plan or billing_period"}))).into_response(),
    };

    if price_id.is_empty() {
        return (StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Price ID not configured for this plan"}))).into_response();
    }

    // Find or create Stripe customer
    let customer_id = match get_or_create_customer(&state, &body.email).await {
        Ok(id) => id,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR,
                          Json(serde_json::json!({"error": e}))).into_response(),
    };

    // Check if this customer already has an active/trialing subscription (skip trial offer)
    let already_subscribed = {
        let conn = state.db.lock().unwrap();
        conn.query_row(
            "SELECT status FROM customers WHERE stripe_customer_id=?1",
            params![customer_id],
            |r| r.get::<_, String>(0),
        ).map(|s| s == "active" || s == "trialing").unwrap_or(false)
    };

    let success_url = format!("{}/billing/success?session_id={{CHECKOUT_SESSION_ID}}",
                              state.billing.app_url);
    let cancel_url  = format!("{}/#pricing", state.billing.app_url);

    // Build form; trial params added only for first-time subscribers
    let mut form: Vec<(&str, &str)> = vec![
        ("customer",                    &customer_id),
        ("mode",                        "subscription"),
        ("line_items[0][price]",        price_id),
        ("line_items[0][quantity]",     "1"),
        ("success_url",                 &success_url),
        ("cancel_url",                  &cancel_url),
        ("allow_promotion_codes",       "true"),
    ];

    if !already_subscribed {
        // 14-day free trial; card is collected but not charged until trial ends.
        // If the user doesn't add a card, the subscription is canceled at trial end.
        form.push(("subscription_data[trial_period_days]", "14"));
        form.push((
            "subscription_data[trial_settings][end_behavior][missing_payment_method]",
            "cancel",
        ));
        form.push(("payment_method_collection", "if_required"));
    }

    match stripe_post(&state.billing.http, &state.billing.stripe_secret,
                      "/checkout/sessions", &form).await
    {
        Ok(session) => {
            let url = session["url"].as_str().unwrap_or("").to_string();
            Json(CheckoutResponse { checkout_url: url }).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR,
                   Json(serde_json::json!({"error": e}))).into_response(),
    }
}

// ─── GET /billing/portal ──────────────────────────────────────────────────────

pub async fn portal_handler(
    State(state): State<crate::AppState>,
    Query(q):     Query<PortalQuery>,
) -> Response {
    if state.billing.stripe_secret.is_empty() {
        return (StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "Stripe not configured"}))).into_response();
    }

    let customer_id: Option<String> = {
        let conn = state.db.lock().unwrap();
        conn.query_row(
            "SELECT stripe_customer_id FROM customers WHERE email=?1",
            params![q.email],
            |r| r.get(0),
        ).ok()
    };

    let customer_id = match customer_id {
        Some(id) => id,
        None => return (StatusCode::NOT_FOUND,
                        Json(serde_json::json!({"error": "No subscription found for this email"}))).into_response(),
    };

    let return_url = q.return_url.as_deref()
        .unwrap_or(&state.billing.app_url)
        .to_string();

    let form = [
        ("customer",   customer_id.as_str()),
        ("return_url", return_url.as_str()),
    ];

    match stripe_post(&state.billing.http, &state.billing.stripe_secret,
                      "/billing_portal/sessions", &form).await
    {
        Ok(session) => {
            let url = session["url"].as_str()
                .unwrap_or(&state.billing.app_url)
                .to_string();
            Redirect::temporary(&url).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR,
                   Json(serde_json::json!({"error": e}))).into_response(),
    }
}

// ─── POST /billing/webhook ────────────────────────────────────────────────────

pub async fn webhook_handler(
    State(state): State<crate::AppState>,
    headers:      HeaderMap,
    body:         Bytes,
) -> Response {
    // Verify Stripe signature before processing
    let sig = match headers.get("stripe-signature").and_then(|v| v.to_str().ok()) {
        Some(s) => s.to_string(),
        None    => return (StatusCode::BAD_REQUEST, "Missing Stripe-Signature header").into_response(),
    };

    if !verify_stripe_signature(&body, &sig, &state.billing.stripe_webhook_secret) {
        return (StatusCode::UNAUTHORIZED, "Invalid webhook signature").into_response();
    }

    let event: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v)  => v,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid JSON").into_response(),
    };

    let event_type = event["type"].as_str().unwrap_or("").to_string();
    let obj        = event["data"]["object"].clone();

    tracing::info!("Stripe webhook: {}", event_type);

    match event_type.as_str() {
        "customer.subscription.created" | "customer.subscription.updated" => {
            handle_subscription_active(&state, &obj, &event_type).await;
        }
        "customer.subscription.deleted" => {
            handle_subscription_deleted(&state, &obj).await;
        }
        "invoice.payment_failed" => {
            handle_payment_failed(&state, &obj).await;
        }
        _ => {}
    }

    (StatusCode::OK, "ok").into_response()
}

// ─── GET /billing/success ─────────────────────────────────────────────────────

pub async fn success_handler() -> impl IntoResponse {
    Html(r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Payment Successful — Schedula</title>
  <style>
    *  { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      font-family: system-ui, -apple-system, sans-serif;
      background: #0a0f1e; color: #e2e8f0;
      display: flex; align-items: center; justify-content: center;
      min-height: 100vh;
    }
    .card {
      background: #131929; border: 1px solid #1e293b; border-radius: 16px;
      padding: 3rem 4rem; text-align: center; max-width: 500px; width: 90%;
    }
    .icon  { font-size: 3rem; margin-bottom: 1rem; }
    h1     { color: #10b981; font-size: 1.75rem; margin-bottom: 1rem; font-weight: 700; }
    p      { color: #94a3b8; line-height: 1.7; margin-bottom: 1rem; }
    code   { background: #0f1623; border: 1px solid #1e293b; padding: .2rem .6rem;
             border-radius: 4px; font-size: .85rem; color: #6366f1; }
    a      { color: #6366f1; text-decoration: none; }
    a:hover { text-decoration: underline; }
  </style>
</head>
<body>
  <div class="card">
    <div class="icon">✓</div>
    <h1>You're all set!</h1>
    <p>Your Schedula subscription is now active. Check your email — your license key will arrive shortly.</p>
    <p>Once you receive it, paste it into your Hub admin panel under<br>
       <code>Settings → License → Activate License</code></p>
    <p style="margin-top:2rem"><a href="/">← Back to Schedula</a></p>
  </div>
</body>
</html>"#)
}

// ─── Subscription lifecycle ───────────────────────────────────────────────────

async fn handle_subscription_active(
    state:      &crate::AppState,
    sub:        &serde_json::Value,
    event_type: &str,
) {
    let status      = sub["status"].as_str().unwrap_or("unknown");
    let customer_id = sub["customer"].as_str().unwrap_or("");
    let sub_id      = sub["id"].as_str().unwrap_or("");
    let period_end  = sub["current_period_end"].as_i64().unwrap_or(0);
    let trial_end   = sub["trial_end"].as_i64();
    let price_id    = sub["items"]["data"][0]["price"]["id"].as_str().unwrap_or("");
    let plan        = resolve_plan(state, price_id);

    let email: Option<String> = {
        let conn = state.db.lock().unwrap();
        conn.query_row(
            "SELECT email FROM customers WHERE stripe_customer_id=?1",
            params![customer_id],
            |r| r.get(0),
        ).ok()
    };
    let email = match email { Some(e) => e, None => return };

    let period_end_str = unix_to_rfc3339(period_end);
    let trial_end_str  = trial_end.map(unix_to_rfc3339);

    // Update subscription state in DB
    {
        let conn = state.db.lock().unwrap();
        conn.execute(
            "UPDATE customers SET plan=?1, subscription_id=?2, status=?3, \
             period_end=?4, trial_end=?5 WHERE stripe_customer_id=?6",
            params![plan, sub_id, status, period_end_str, trial_end_str, customer_id],
        ).ok();
    }

    if status != "active" && status != "trialing" {
        return;
    }

    // Issue license JWT valid until period_end + 7-day buffer
    let expiry_days = {
        let now  = Utc::now().timestamp();
        let end  = period_end + 7 * 86400;
        ((end - now) / 86400).max(1)
    };

    let result = {
        let conn = state.db.lock().unwrap();
        crate::issue_license_core(&conn, &state.encoding_key, &plan,
                                  Some(email.as_str()), Some(expiry_days))
    };

    match result {
        Ok((token, jti)) => {
            {
                let conn = state.db.lock().unwrap();
                conn.execute(
                    "UPDATE customers SET jti=?1 WHERE stripe_customer_id=?2",
                    params![jti, customer_id],
                ).ok();
            }

            let is_new   = event_type == "customer.subscription.created";
            let is_trial = status == "trialing";

            if is_new && is_trial {
                send_trial_started_email(state, &email, &token,
                                         trial_end_str.as_deref()).await;
            } else {
                send_license_email(state, &email, &token, &plan, &period_end_str).await;
            }
        }
        Err(e) => tracing::error!("Failed to issue license for {}: {}", email, e),
    }
}

async fn handle_subscription_deleted(state: &crate::AppState, sub: &serde_json::Value) {
    let customer_id    = sub["customer"].as_str().unwrap_or("");
    let period_end     = sub["current_period_end"].as_i64().unwrap_or(0);
    let period_end_str = unix_to_rfc3339(period_end);

    let email: Option<String> = {
        let conn = state.db.lock().unwrap();
        conn.query_row(
            "SELECT email FROM customers WHERE stripe_customer_id=?1",
            params![customer_id],
            |r| r.get(0),
        ).ok()
    };

    // Mark canceled but do NOT revoke the license — it expires naturally at period_end
    {
        let conn = state.db.lock().unwrap();
        conn.execute(
            "UPDATE customers SET status='canceled', period_end=?1 WHERE stripe_customer_id=?2",
            params![period_end_str, customer_id],
        ).ok();
    }

    if let Some(email) = email {
        send_cancellation_email(state, &email, &period_end_str).await;
    }
}

async fn handle_payment_failed(state: &crate::AppState, invoice: &serde_json::Value) {
    let customer_id = invoice["customer"].as_str().unwrap_or("");
    let email: Option<String> = {
        let conn = state.db.lock().unwrap();
        conn.query_row(
            "SELECT email FROM customers WHERE stripe_customer_id=?1",
            params![customer_id],
            |r| r.get(0),
        ).ok()
    };
    if let Some(email) = email {
        send_payment_failed_email(state, &email).await;
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

async fn get_or_create_customer(
    state: &crate::AppState,
    email: &str,
) -> Result<String, String> {
    // Return cached customer ID if we already know this email
    {
        let conn = state.db.lock().unwrap();
        if let Ok(id) = conn.query_row(
            "SELECT stripe_customer_id FROM customers WHERE email=?1",
            params![email],
            |r| r.get::<_, String>(0),
        ) {
            return Ok(id);
        }
    }

    // Create a new Stripe customer
    let form = [("email", email)];
    let res  = stripe_post(&state.billing.http, &state.billing.stripe_secret,
                           "/customers", &form).await?;
    let id   = res["id"].as_str()
        .ok_or_else(|| "No id in Stripe customer response".to_string())?
        .to_string();

    {
        let conn = state.db.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO customers \
             (stripe_customer_id, email, status) VALUES (?1, ?2, 'none')",
            params![id, email],
        ).ok();
    }

    Ok(id)
}

fn resolve_plan(state: &crate::AppState, price_id: &str) -> String {
    let b = &state.billing;
    if price_id == b.price_pro_monthly || price_id == b.price_pro_annual {
        "pro".to_string()
    } else if price_id == b.price_inst_monthly || price_id == b.price_inst_annual {
        "institution".to_string()
    } else {
        "pro".to_string() // safe default for unknown prices
    }
}

fn unix_to_rfc3339(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|d| d.to_rfc3339())
        .unwrap_or_default()
}

// ─── Webhook signature verification ──────────────────────────────────────────

fn verify_stripe_signature(payload: &[u8], sig_header: &str, secret: &str) -> bool {
    if secret.is_empty() {
        tracing::warn!("STRIPE_WEBHOOK_SECRET not set — skipping signature check");
        return true; // dev/test mode
    }

    let mut timestamp  = "";
    let mut signatures: Vec<&str> = vec![];
    for part in sig_header.split(',') {
        if let Some(v) = part.strip_prefix("t=")  { timestamp  = v; }
        if let Some(v) = part.strip_prefix("v1=") { signatures.push(v); }
    }

    if timestamp.is_empty() || signatures.is_empty() { return false; }

    let signed_payload = format!("{}.{}", timestamp, String::from_utf8_lossy(payload));

    type HmacSha256 = Hmac<Sha256>;
    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m)  => m,
        Err(_) => return false,
    };
    mac.update(signed_payload.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());

    signatures.iter().any(|s| *s == expected)
}

// ─── Email senders ────────────────────────────────────────────────────────────

async fn send_license_email(
    state:      &crate::AppState,
    to:         &str,
    token:      &str,
    plan:       &str,
    period_end: &str,
) {
    let label = plan_label(plan);
    let subject = format!("Your Schedula {label} License Key");
    let body = format!(
"Hello,

Thank you for subscribing to Schedula {label}!

Your license key is:

{token}

To activate it, open your Schedula Hub admin panel and go to:
  Settings → License → Activate License

Paste the key into the text field and click Activate.

Your subscription is active until {period_end}.

If you have questions, just reply to this email.

— The Schedula Team"
    );
    send_email(state, to, &subject, &body).await;
}

async fn send_trial_started_email(
    state:     &crate::AppState,
    to:        &str,
    token:     &str,
    trial_end: Option<&str>,
) {
    let trial_info = trial_end
        .map(|d| format!("Your trial runs until {}.", d))
        .unwrap_or_else(|| "Your 14-day trial has started.".to_string());

    let subject = "Your Schedula Pro Trial Has Started".to_string();
    let app_url = state.billing.app_url.clone();
    let body = format!(
"Hello,

Welcome to Schedula Pro! Your 14-day free trial is now active.

{trial_info}

Your trial license key is:

{token}

To activate it, open your Schedula Hub admin panel and go to:
  Settings → License → Activate License

After your trial, your card on file will be charged automatically.
You can manage or cancel your subscription at any time:
  {app_url}/billing/portal?email={to}

— The Schedula Team"
    );
    send_email(state, to, &subject, &body).await;
}

async fn send_cancellation_email(
    state:      &crate::AppState,
    to:         &str,
    period_end: &str,
) {
    let app_url = state.billing.app_url.clone();
    let subject = "Your Schedula Subscription Has Been Canceled".to_string();
    let body = format!(
"Hello,

Your Schedula subscription has been canceled.

Your access continues until {period_end}. After that date, your hub will
automatically revert to the Free plan (10 batches, 1 admin).

If you change your mind, you can reactivate at any time:
  {app_url}/#pricing

— The Schedula Team"
    );
    send_email(state, to, &subject, &body).await;
}

async fn send_payment_failed_email(state: &crate::AppState, to: &str) {
    let app_url = state.billing.app_url.clone();
    let subject = "Schedula Payment Failed — Action Required".to_string();
    let body = format!(
"Hello,

We were unable to process your last Schedula payment.

Please update your payment method to avoid interruption to your service:
  {app_url}/billing/portal?email={to}

— The Schedula Team"
    );
    send_email(state, to, &subject, &body).await;
}

fn plan_label(plan: &str) -> &'static str {
    match plan {
        "institution" => "Institution",
        _             => "Pro",
    }
}

// ─── Core SMTP sender ─────────────────────────────────────────────────────────

async fn send_email(state: &crate::AppState, to: &str, subject: &str, body: &str) {
    use lettre::{
        message::header::ContentType,
        transport::smtp::authentication::Credentials,
        AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    };

    let cfg = &state.billing.email;
    if cfg.smtp_host.is_empty() {
        tracing::warn!("SMTP not configured — skipping email to {to} ({subject})");
        return;
    }

    let from_addr = cfg.smtp_from
        .parse()
        .unwrap_or_else(|_| "noreply@schedula.app".parse().unwrap());

    let to_addr = match to.parse() {
        Ok(a)  => a,
        Err(_) => { tracing::error!("Invalid email address: {to}"); return; }
    };

    let email = match Message::builder()
        .from(from_addr)
        .to(to_addr)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body.to_string())
    {
        Ok(m)  => m,
        Err(e) => { tracing::error!("Failed to build email: {e}"); return; }
    };

    let mailer: AsyncSmtpTransport<Tokio1Executor> = if cfg.smtp_username.is_empty() {
        // Unauthenticated (local dev / relay without auth)
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&cfg.smtp_host)
            .port(cfg.smtp_port)
            .build()
    } else {
        match AsyncSmtpTransport::<Tokio1Executor>::relay(&cfg.smtp_host) {
            Ok(b)  => b.credentials(Credentials::new(
                cfg.smtp_username.clone(),
                cfg.smtp_password.clone(),
            )).build(),
            Err(e) => { tracing::error!("SMTP relay error: {e}"); return; }
        }
    };

    match mailer.send(email).await {
        Ok(_)  => tracing::info!("Email sent → {to} ({subject})"),
        Err(e) => tracing::error!("Failed to send email to {to}: {e}"),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PHASE 4 — Invoice flow, Paddle integration, Admin dashboard
// ═══════════════════════════════════════════════════════════════════════════════

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct InvoiceRequest {
    pub org_name:      String,
    pub contact_name:  String,
    pub contact_email: String,
    pub plan:          String,  // "pro" | "institution"
    pub user_count:    Option<i64>,
    pub country:       Option<String>,
    pub notes:         Option<String>,
}

#[derive(Serialize)]
pub struct InvoiceRecord {
    pub id:            String,
    pub org_name:      String,
    pub contact_name:  String,
    pub contact_email: String,
    pub plan:          String,
    pub user_count:    Option<i64>,
    pub country:       Option<String>,
    pub notes:         Option<String>,
    pub status:        String,
    pub created_at:    String,
    pub paid_at:       Option<String>,
    pub issued_at:     Option<String>,
    pub jti:           Option<String>,
}

#[derive(Deserialize)]
pub struct AdminQuery {
    pub key: Option<String>,
}

// ─── POST /billing/invoice-request ───────────────────────────────────────────

pub async fn invoice_request_handler(
    State(state): State<crate::AppState>,
    Json(body):   Json<InvoiceRequest>,
) -> Response {
    let valid_plans = ["pro", "institution"];
    if body.org_name.is_empty() || body.contact_email.is_empty() {
        return (StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "org_name and contact_email are required"}))).into_response();
    }
    if !valid_plans.contains(&body.plan.as_str()) {
        return (StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "plan must be 'pro' or 'institution'"}))).into_response();
    }

    let id = uuid::Uuid::new_v4().to_string();

    {
        let conn = state.db.lock().unwrap();
        conn.execute(
            "INSERT INTO invoice_requests \
             (id, org_name, contact_name, contact_email, plan, user_count, country, notes) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![id, body.org_name, body.contact_name, body.contact_email,
                    body.plan, body.user_count, body.country, body.notes],
        ).ok();
    }

    // Notify the sales team
    let notification = format!(
        "New invoice request\nOrg: {} | Plan: {} | Country: {}\nContact: {} <{}>\nUsers: {}\nNotes: {}\nID: {}",
        body.org_name, body.plan,
        body.country.as_deref().unwrap_or("—"),
        body.contact_name, body.contact_email,
        body.user_count.map_or("—".to_string(), |n| n.to_string()),
        body.notes.as_deref().unwrap_or("—"),
        id,
    );
    notify_sales(&state, &body.org_name, &body.plan, &body.contact_email, &notification).await;

    // Send acknowledgement to the customer
    send_invoice_ack_email(&state, &body.contact_email, &body.contact_name,
                           &body.org_name, &body.plan).await;

    Json(serde_json::json!({
        "id": id,
        "message": "Thank you! We'll be in touch within 1 business day with a quote."
    })).into_response()
}

// ─── GET /billing/invoice-requests ───────────────────────────────────────────

pub async fn list_invoices_handler(
    State(state): State<crate::AppState>,
    headers:      HeaderMap,
) -> Response {
    if !state.require_admin(&headers) { return crate::forbidden(); }

    let conn = state.db.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, org_name, contact_name, contact_email, plan, user_count, country, notes, \
         status, created_at, paid_at, issued_at, jti \
         FROM invoice_requests ORDER BY created_at DESC"
    ).unwrap();

    let records: Vec<InvoiceRecord> = stmt
        .query_map([], |r| Ok(InvoiceRecord {
            id:            r.get(0)?,
            org_name:      r.get(1)?,
            contact_name:  r.get(2)?,
            contact_email: r.get(3)?,
            plan:          r.get(4)?,
            user_count:    r.get(5)?,
            country:       r.get(6)?,
            notes:         r.get(7)?,
            status:        r.get(8)?,
            created_at:    r.get(9)?,
            paid_at:       r.get(10)?,
            issued_at:     r.get(11)?,
            jti:           r.get(12)?,
        }))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    Json(records).into_response()
}

// ─── POST /billing/invoice-requests/:id/issue ─────────────────────────────────

pub async fn issue_invoice_handler(
    State(state): State<crate::AppState>,
    headers:      HeaderMap,
    Path(id):     Path<String>,
) -> Response {
    if !state.require_admin(&headers) { return crate::forbidden(); }

    // Fetch pending invoice request
    let row: Option<(String, String, String, String)> = {
        let conn = state.db.lock().unwrap();
        conn.query_row(
            "SELECT plan, contact_email, contact_name, org_name \
             FROM invoice_requests WHERE id=?1 AND status='pending'",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        ).ok()
    };

    let (plan, contact_email, _contact_name, org_name) = match row {
        Some(r) => r,
        None    => return (StatusCode::NOT_FOUND,
                           Json(serde_json::json!({"error": "Invoice not found or not pending"}))).into_response(),
    };

    // Issue a 1-year license
    let result = {
        let conn = state.db.lock().unwrap();
        crate::issue_license_core(&conn, &state.encoding_key, &plan,
                                  Some(org_name.as_str()), Some(365))
    };

    match result {
        Ok((token, jti)) => {
            let now = chrono::Utc::now().to_rfc3339();
            {
                let conn = state.db.lock().unwrap();
                conn.execute(
                    "UPDATE invoice_requests SET status='issued', jti=?1, paid_at=?2, issued_at=?2 WHERE id=?3",
                    params![jti, now, id],
                ).ok();
            }

            // Email the license token
            let expiry = (chrono::Utc::now() + chrono::Duration::days(365)).to_rfc3339();
            send_license_email(&state, &contact_email, &token, &plan, &expiry).await;

            tracing::info!("Invoice {} issued → {}", &id[..8], contact_email);
            Json(serde_json::json!({
                "issued": true,
                "jti":    jti,
                "contact_email": contact_email,
                "message": format!("License issued and emailed to {contact_email}"),
            })).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR,
                   Json(serde_json::json!({"error": e}))).into_response(),
    }
}

// ─── GET /billing/config ──────────────────────────────────────────────────────
//
// Returns Paddle.js client config. Safe to expose publicly — no secrets here.

pub async fn billing_config_handler(State(state): State<crate::AppState>) -> Response {
    Json(serde_json::json!({
        "paddle_client_token": state.billing.paddle_client_token,
        "paddle_prices": {
            "pro_monthly":  state.billing.paddle_price_pro_monthly,
            "pro_annual":   state.billing.paddle_price_pro_annual,
            "inst_monthly": state.billing.paddle_price_inst_monthly,
            "inst_annual":  state.billing.paddle_price_inst_annual,
        },
        "stripe_enabled": !state.billing.stripe_secret.is_empty(),
        "paddle_enabled": !state.billing.paddle_client_token.is_empty(),
    })).into_response()
}

// ─── GET /admin ───────────────────────────────────────────────────────────────

pub async fn admin_handler(
    State(state): State<crate::AppState>,
    Query(q):     Query<AdminQuery>,
) -> Response {
    let provided = q.key.as_deref().unwrap_or("");
    if provided != state.admin_key {
        return (StatusCode::UNAUTHORIZED,
                Html("<h1 style='font-family:sans-serif;padding:2rem'>Unauthorized — use ?key=ADMIN_KEY</h1>"))
            .into_response();
    }
    Html(ADMIN_HTML.to_string()).into_response()
}

const ADMIN_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>Schedula License Admin</title>
  <style>
    *{box-sizing:border-box;margin:0;padding:0}
    body{font-family:system-ui,sans-serif;background:#0f172a;color:#e2e8f0;min-height:100vh;padding:2rem}
    h1{color:#f8fafc;font-size:1.5rem;margin-bottom:1.5rem}
    h2{color:#94a3b8;font-size:.75rem;text-transform:uppercase;letter-spacing:.08em;margin:2.5rem 0 .875rem;border-bottom:1px solid #1e293b;padding-bottom:.5rem}
    table{width:100%;border-collapse:collapse;font-size:.875rem}
    th{text-align:left;padding:.5rem .75rem;background:#1e293b;color:#64748b;font-weight:500;font-size:.8rem;text-transform:uppercase;letter-spacing:.04em}
    td{padding:.625rem .75rem;border-bottom:1px solid #1e293b;vertical-align:middle}
    tr:hover td{background:#111827}
    .badge{display:inline-block;padding:.2rem .6rem;border-radius:999px;font-size:.7rem;font-weight:700;text-transform:uppercase;letter-spacing:.04em}
    .badge-pending{background:#fef9c3;color:#854d0e}
    .badge-issued {background:#dcfce7;color:#166534}
    .badge-pro    {background:#eff6ff;color:#1d4ed8}
    .badge-inst   {background:#f3e8ff;color:#7c3aed}
    button{padding:.375rem .875rem;border:none;border-radius:6px;cursor:pointer;font-size:.8125rem;font-weight:500;transition:background .15s}
    .btn-issue{background:#3b82f6;color:#fff}.btn-issue:hover{background:#2563eb}
    .btn-revoke{background:#ef4444;color:#fff}.btn-revoke:hover{background:#dc2626}
    #status{padding:.75rem 1rem;background:#1e293b;border-radius:8px;margin-bottom:1.5rem;color:#64748b;font-size:.875rem}
    a{color:#6366f1;text-decoration:none}.a:hover{text-decoration:underline}
    small{color:#64748b}
  </style>
</head>
<body>
<h1>Schedula License Admin</h1>
<div id="status">Initializing…</div>

<h2>Invoice Requests</h2>
<table>
  <thead><tr>
    <th>ID</th><th>Organization</th><th>Contact</th><th>Plan</th>
    <th>Country</th><th>Status</th><th>Requested</th><th>Action</th>
  </tr></thead>
  <tbody id="inv-body"><tr><td colspan="8" style="color:#475569">Loading…</td></tr></tbody>
</table>

<h2>Issued Licenses</h2>
<table>
  <thead><tr>
    <th>JTI</th><th>Plan</th><th>Org</th><th>Issued</th><th>Expires</th><th>Revoked</th><th>Action</th>
  </tr></thead>
  <tbody id="lic-body"><tr><td colspan="7" style="color:#475569">Loading…</td></tr></tbody>
</table>

<script>
const KEY = new URLSearchParams(location.search).get('key') || localStorage.getItem('sk') || '';
if (KEY) localStorage.setItem('sk', KEY);

const H = { 'X-Admin-Key': KEY };

async function api(path, opts={}) {
  const r = await fetch(path, { ...opts, headers: { ...H, ...(opts.headers||{}) } });
  return { ok: r.ok, status: r.status, data: await r.json().catch(()=>({})) };
}

async function load() {
  const [inv, lic] = await Promise.all([
    api('/billing/invoice-requests'),
    api('/v1/licenses'),
  ]);

  if (!inv.ok) {
    document.getElementById('status').textContent = `Auth error ${inv.status} — check ?key=`;
    return;
  }
  document.getElementById('status').textContent =
    `${inv.data.length} invoice request(s) · ${lic.data.length ?? '?'} license(s)`;

  // Invoice table
  const ib = document.getElementById('inv-body');
  ib.innerHTML = '';
  if (!inv.data.length) {
    ib.innerHTML = '<tr><td colspan="8" style="color:#475569;padding:1rem">No invoice requests yet.</td></tr>';
  }
  for (const r of (inv.data || [])) {
    const badge = r.status === 'issued'
      ? `<span class="badge badge-issued">issued</span>`
      : `<span class="badge badge-pending">${r.status}</span>`;
    const planBadge = r.plan === 'institution'
      ? `<span class="badge badge-inst">Institution</span>`
      : `<span class="badge badge-pro">Pro</span>`;
    const action = r.status === 'pending'
      ? `<button class="btn-issue" onclick="issueInvoice('${r.id}','${r.contact_email}')">Issue License</button>`
      : r.jti ? `<small>${r.jti.substring(0,8)}…</small>` : '—';
    const tr = document.createElement('tr');
    tr.innerHTML = `
      <td><small>${r.id.substring(0,8)}…</small></td>
      <td>${r.org_name}</td>
      <td>${r.contact_name}<br><small>${r.contact_email}</small></td>
      <td>${planBadge}</td>
      <td>${r.country||'—'}</td>
      <td>${badge}</td>
      <td>${r.created_at.substring(0,10)}</td>
      <td>${action}</td>`;
    ib.appendChild(tr);
  }

  // License table
  const lb = document.getElementById('lic-body');
  lb.innerHTML = '';
  if (!lic.data.length) {
    lb.innerHTML = '<tr><td colspan="7" style="color:#475569;padding:1rem">No licenses issued yet.</td></tr>';
  }
  for (const r of (lic.data || [])) {
    const planBadge = r.plan === 'institution'
      ? `<span class="badge badge-inst">Institution</span>`
      : `<span class="badge badge-pro">Pro</span>`;
    const revoked = r.revoked
      ? '<span class="badge badge-pending">Revoked</span>'
      : '<span class="badge badge-issued">Active</span>';
    const action = !r.revoked
      ? `<button class="btn-revoke" onclick="revokeLic('${r.jti}')">Revoke</button>`
      : '—';
    const tr = document.createElement('tr');
    tr.innerHTML = `
      <td><small>${r.jti.substring(0,8)}…</small></td>
      <td>${planBadge}</td>
      <td>${r.org_name||'—'}</td>
      <td>${r.issued_at.substring(0,10)}</td>
      <td>${r.expires_at ? r.expires_at.substring(0,10) : '∞ perpetual'}</td>
      <td>${revoked}</td>
      <td>${action}</td>`;
    lb.appendChild(tr);
  }
}

async function issueInvoice(id, email) {
  if (!confirm(`Issue 1-year license for invoice ${id.substring(0,8)}…?\nToken will be emailed to:\n${email}`)) return;
  const r = await api(`/billing/invoice-requests/${id}/issue`, { method: 'POST' });
  if (r.ok) { alert(`✓ License issued!\nJTI: ${r.data.jti}\nEmailed to: ${r.data.contact_email}`); load(); }
  else       { alert(`Error: ${r.data.error}`); }
}

async function revokeLic(jti) {
  if (!confirm(`Revoke license ${jti.substring(0,8)}…?\nThis cannot be undone.`)) return;
  const r = await api(`/v1/licenses/${jti}`, { method: 'DELETE' });
  if (r.ok) { alert('License revoked.'); load(); }
  else       { alert(`Error: ${r.data.error || r.status}`); }
}

if (KEY) load(); else document.getElementById('status').textContent = 'No admin key — add ?key=YOUR_KEY to the URL';
</script>
</body>
</html>"#;

// ─── POST /billing/paddle/webhook ─────────────────────────────────────────────

pub async fn paddle_webhook_handler(
    State(state): State<crate::AppState>,
    headers:      HeaderMap,
    body:         Bytes,
) -> Response {
    let sig = match headers.get("paddle-signature").and_then(|v| v.to_str().ok()) {
        Some(s) => s.to_string(),
        None    => return (StatusCode::BAD_REQUEST, "Missing Paddle-Signature header").into_response(),
    };

    if !verify_paddle_signature(&body, &sig, &state.billing.paddle_webhook_secret) {
        return (StatusCode::UNAUTHORIZED, "Invalid Paddle webhook signature").into_response();
    }

    let event: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v)  => v,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid JSON").into_response(),
    };

    let event_type = event["event_type"].as_str().unwrap_or("").to_string();
    let data       = event["data"].clone();

    tracing::info!("Paddle webhook: {}", event_type);

    match event_type.as_str() {
        "subscription.created" | "subscription.updated" => {
            handle_paddle_subscription_active(&state, &data, &event_type).await;
        }
        "subscription.canceled" => {
            handle_paddle_subscription_canceled(&state, &data).await;
        }
        "transaction.payment_failed" => {
            handle_paddle_payment_failed(&state, &data).await;
        }
        _ => {}
    }

    (StatusCode::OK, "ok").into_response()
}

// ─── Paddle subscription lifecycle ───────────────────────────────────────────

async fn handle_paddle_subscription_active(
    state:      &crate::AppState,
    sub:        &serde_json::Value,
    event_type: &str,
) {
    let status      = sub["status"].as_str().unwrap_or("unknown");
    let customer_id = sub["customer_id"].as_str().unwrap_or("");
    let price_id    = sub["items"][0]["price"]["id"].as_str().unwrap_or("");
    let plan        = resolve_paddle_plan(state, price_id);

    // Period end (ISO 8601) — convert to Unix timestamp
    let period_end_str = sub["current_billing_period"]["ends_at"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let period_end_ts = iso8601_to_timestamp(&period_end_str);

    // Trial end (optional)
    let trial_end_str = sub["trial_dates"]["ends_at"]
        .as_str()
        .map(|s| s.to_string());

    // Resolve customer email: prefer custom_data.email, fall back to Paddle API
    let email = if let Some(e) = sub["custom_data"]["email"].as_str() {
        e.to_string()
    } else {
        match paddle_get_customer_email(&state.billing.http, &state.billing.paddle_api_key,
                                        customer_id).await {
            Some(e) => e,
            None => {
                tracing::error!("Cannot resolve email for Paddle customer {}", customer_id);
                return;
            }
        }
    };

    if status != "active" && status != "trialing" {
        return;
    }

    // Issue license with period_end + 7-day buffer
    let expiry_days = {
        let end = period_end_ts + 7 * 86400;
        ((end - chrono::Utc::now().timestamp()) / 86400).max(1)
    };

    let result = {
        let conn = state.db.lock().unwrap();
        crate::issue_license_core(&conn, &state.encoding_key, &plan,
                                  Some(email.as_str()), Some(expiry_days))
    };

    match result {
        Ok((token, _jti)) => {
            let is_new   = event_type == "subscription.created";
            let is_trial = status == "trialing";

            if is_new && is_trial {
                send_trial_started_email(state, &email, &token,
                                         trial_end_str.as_deref()).await;
            } else {
                send_license_email(state, &email, &token, &plan, &period_end_str).await;
            }
        }
        Err(e) => tracing::error!("Failed to issue Paddle license for {}: {}", email, e),
    }
}

async fn handle_paddle_subscription_canceled(
    state: &crate::AppState,
    sub:   &serde_json::Value,
) {
    // Paddle sets status to "canceled" and may include scheduled_change.effective_at
    let customer_id = sub["customer_id"].as_str().unwrap_or("");
    let effective   = sub["scheduled_change"]["effective_at"]
        .as_str()
        .or_else(|| sub["current_billing_period"]["ends_at"].as_str())
        .unwrap_or("")
        .to_string();

    let email = if let Some(e) = sub["custom_data"]["email"].as_str() {
        e.to_string()
    } else {
        match paddle_get_customer_email(&state.billing.http, &state.billing.paddle_api_key,
                                        customer_id).await {
            Some(e) => e,
            None    => return,
        }
    };

    send_cancellation_email(state, &email, &effective).await;
}

async fn handle_paddle_payment_failed(state: &crate::AppState, data: &serde_json::Value) {
    let customer_id = data["customer_id"].as_str().unwrap_or("");
    let email = if let Some(e) = data["custom_data"]["email"].as_str() {
        e.to_string()
    } else {
        match paddle_get_customer_email(&state.billing.http, &state.billing.paddle_api_key,
                                        customer_id).await {
            Some(e) => e,
            None    => return,
        }
    };
    send_payment_failed_email(state, &email).await;
}

// ─── Paddle helpers ───────────────────────────────────────────────────────────

fn resolve_paddle_plan(state: &crate::AppState, price_id: &str) -> String {
    let b = &state.billing;
    if price_id == b.paddle_price_pro_monthly || price_id == b.paddle_price_pro_annual {
        "pro".to_string()
    } else if price_id == b.paddle_price_inst_monthly || price_id == b.paddle_price_inst_annual {
        "institution".to_string()
    } else {
        "pro".to_string()
    }
}

async fn paddle_get_customer_email(
    http:        &reqwest::Client,
    api_key:     &str,
    customer_id: &str,
) -> Option<String> {
    if api_key.is_empty() || customer_id.is_empty() { return None; }
    let url  = format!("https://api.paddle.com/customers/{}", customer_id);
    let resp = http.get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send().await.ok()?;
    let json: serde_json::Value = resp.json().await.ok()?;
    json["data"]["email"].as_str().map(|s| s.to_string())
}

/// Paddle Billing (v2) signature: Paddle-Signature header = "ts=...;h1=..."
/// Signed payload = "{ts}:{raw_body}"
fn verify_paddle_signature(payload: &[u8], sig_header: &str, secret: &str) -> bool {
    if secret.is_empty() {
        tracing::warn!("PADDLE_WEBHOOK_SECRET not set — skipping signature check");
        return true;
    }

    let mut timestamp  = "";
    let mut signatures: Vec<&str> = vec![];
    for part in sig_header.split(';') {
        if let Some(v) = part.strip_prefix("ts=") { timestamp  = v; }
        if let Some(v) = part.strip_prefix("h1=") { signatures.push(v); }
    }
    if timestamp.is_empty() || signatures.is_empty() { return false; }

    let signed = format!("{}:{}", timestamp, String::from_utf8_lossy(payload));

    type HmacSha256 = Hmac<Sha256>;
    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m)  => m,
        Err(_) => return false,
    };
    mac.update(signed.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());

    signatures.iter().any(|s| *s == expected)
}

fn iso8601_to_timestamp(s: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|d| d.timestamp())
        .unwrap_or(0)
}

// ─── Sales notifications ──────────────────────────────────────────────────────

async fn notify_sales(
    state:   &crate::AppState,
    org:     &str,
    plan:    &str,
    contact: &str,
    body:    &str,
) {
    // Email
    if !state.billing.sales_email.is_empty() {
        let sales_email = state.billing.sales_email.clone();
        send_email(state, &sales_email,
                   &format!("New invoice request: {org} ({plan} plan)"),
                   body).await;
    }

    // Slack
    if !state.billing.slack_webhook_url.is_empty() {
        let payload = serde_json::json!({
            "text": format!("🧾 New invoice request\n*Org:* {org}\n*Plan:* {plan}\n*Contact:* {contact}"),
        });
        state.billing.http
            .post(&state.billing.slack_webhook_url)
            .json(&payload)
            .send().await.ok();
    }
}

async fn send_invoice_ack_email(
    state: &crate::AppState,
    to:    &str,
    name:  &str,
    org:   &str,
    plan:  &str,
) {
    let label   = plan_label(plan);
    let app_url = state.billing.app_url.clone();
    send_email(
        state, to,
        &format!("Your Schedula {label} Invoice Request — Received"),
        &format!(
"Hello {name},

Thank you for your interest in Schedula {label} for {org}!

We've received your invoice request and will be in touch within 1 business day
with a quote and payment details.

Once payment is confirmed, your license key will be emailed to this address.
You'll paste it into your Hub admin panel under Settings → License to activate.

In the meantime, feel free to download and evaluate the app:
  {app_url}

If you have any questions, just reply to this email.

— The Schedula Team"
        ),
    ).await;
}
