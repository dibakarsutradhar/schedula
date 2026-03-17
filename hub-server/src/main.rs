mod db;
mod models;
mod scheduler;
mod auth;
mod handlers;
mod license;

use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Extension, Json, Router,
};
use models::SessionPayload;
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::json;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use clap::Parser;

const HUB_UI: &str = include_str!("../ui/index.html");

#[derive(Clone)]
struct AppState {
    db:           Arc<Mutex<Connection>>,
    jwt_secret:   String,
    tx:           broadcast::Sender<String>,
    db_path:      PathBuf,
    ws_count:     Arc<AtomicUsize>,
    start_time:   Arc<Instant>,
    listen_addr:  String,
    /// In-memory plan cache — populated at startup from a verified JWT and
    /// refreshed by the background refresh loop every 24 h.  All feature gates
    /// read this instead of hitting the DB, so SQLite manipulation has zero
    /// runtime effect (takes effect only after a hub restart, where it is
    /// re-derived from JWT verification again).
    current_plan: Arc<RwLock<String>>,

    /// Today's 256-bit symmetric key (hex), received from the license server
    /// during the last successful /v1/refresh call.  Empty string means no
    /// successful refresh has occurred yet (or the license was revoked).
    /// Available to the API layer for HMAC signing / app-side validation.
    current_secret: Arc<RwLock<String>>,

    /// True while a device-based checkout polling task is running.
    /// Set to true when /api/license/checkout is called; cleared when the
    /// polling task finds the license or times out.
    checkout_pending: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Parser)]
#[command(name = "schedula-hub", about = "Schedula Hub Server")]
struct Args {
    #[arg(long, default_value = "7878")]
    port: u16,
    #[arg(long, default_value = "./schedula-hub.db")]
    db_path: String,
    #[arg(long, default_value = "")]
    jwt_secret: String,
}

fn to_response<T: serde::Serialize>(result: Result<T, String>) -> Response {
    match result {
        Ok(data) => match serde_json::to_value(data) {
            Ok(v) => (StatusCode::OK, Json(v)).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
        },
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({"error": e}))).into_response(),
    }
}

fn broadcast_change(tx: &broadcast::Sender<String>, entity: &str, action: &str) {
    let _ = tx.send(json!({"entity": entity, "action": action}).to_string());
}

// ─── Auth middleware ──────────────────────────────────────────────────────────

async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let token = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .unwrap_or("");

    match auth::decode_jwt(token, &state.jwt_secret) {
        Ok(session) => {
            req.extensions_mut().insert(session);
            next.run(req).await
        }
        Err(_) => (StatusCode::UNAUTHORIZED, Json(json!({"error": "Unauthorized"}))).into_response(),
    }
}

// ─── WebSocket ────────────────────────────────────────────────────────────────

async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| async move {
        use axum::extract::ws::Message;
        state.ws_count.fetch_add(1, Ordering::Relaxed);
        let mut rx = state.tx.subscribe();
        let (mut sender, _) = socket.split();
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
        state.ws_count.fetch_sub(1, Ordering::Relaxed);
    })
}

async fn ui_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        [("content-type", "text/html; charset=utf-8")],
        HUB_UI,
    )
}

#[derive(serde::Serialize)]
struct ServerStatus {
    version:     &'static str,
    uptime_secs: u64,
    ws_clients:  usize,
    listen_addr: String,
    db_path:     String,
    ws_endpoint: String,
}

async fn server_status_handler(State(state): State<AppState>) -> Response {
    let uptime = state.start_time.elapsed().as_secs();
    Json(ServerStatus {
        version:     env!("CARGO_PKG_VERSION"),
        uptime_secs: uptime,
        ws_clients:  state.ws_count.load(Ordering::Relaxed),
        listen_addr: state.listen_addr.clone(),
        db_path:     state.db_path.display().to_string(),
        ws_endpoint: format!("ws://{}/ws", state.listen_addr),
    }).into_response()
}

// ─── Plan handler ─────────────────────────────────────────────────────────────

async fn plan_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
) -> Response {
    let cached_plan   = state.current_plan.read().unwrap().clone();
    let cached_secret = state.current_secret.read().unwrap().clone();
    to_response(handlers::get_plan(&cached_plan, &cached_secret, &sess))
}

// ─── License handlers ─────────────────────────────────────────────────────────

async fn get_license_handler(State(state): State<AppState>) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::get_license(&conn))
}

async fn activate_license_handler(
    State(state): State<AppState>,
    Json(body): Json<models::ActivateLicenseReq>,
) -> Response {
    // Resolve the JWT: either redeem an activation code (new flow) or accept a
    // raw JWT directly (legacy / admin-issued tokens).
    let (claims, token) = match (&body.code, &body.token) {
        (Some(code), _) => {
            // Code path: hub calls license server, gets JWT back server-to-server
            match license::activate_with_code(code.trim()).await {
                Ok(ct) => ct,
                Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({"error": e}))).into_response(),
            }
        }
        (None, Some(token)) => {
            // Legacy path: caller supplied a raw JWT (admin tooling / invoice flow)
            match license::validate_token(token.trim()) {
                Ok(c)  => (c, token.trim().to_string()),
                Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({"error": e}))).into_response(),
            }
        }
        (None, None) => {
            return (StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Provide either 'code' or 'token'"}))).into_response();
        }
    };

    let result = {
        let conn = state.db.lock().unwrap();
        license::store_license(&conn, &claims, &token)
            .map(|_| license::get_license_info(&conn))
    };

    if result.is_ok() {
        broadcast_change(&state.tx, "license", "activate");
    }
    to_response(result)
}

/// POST /api/license/checkout — initiate a Stripe checkout session for the hub.
///
/// The hub's persistent device_id is embedded in the Stripe subscription metadata.
/// After the user completes payment, the license server stores the JWT in
/// `device_licenses`. A background polling task fetches and auto-activates it.
///
/// Returns `{ checkout_url }` — the caller should open this in the system browser.
async fn checkout_license_handler(
    State(state): State<AppState>,
    Json(body):   Json<models::CheckoutReq>,
) -> Response {
    let device_id = {
        let conn = state.db.lock().unwrap();
        license::get_or_create_device_id(&conn)
    };

    let billing_period = body.billing_period.as_deref().unwrap_or("monthly");

    match license::initiate_checkout(&body.plan, billing_period, &device_id).await {
        Ok(checkout_url) => {
            state.checkout_pending.store(true, std::sync::atomic::Ordering::Relaxed);

            // Spawn polling task: checks every 30 s for up to 60 min
            let db             = state.db.clone();
            let current_plan   = state.current_plan.clone();
            let current_secret = state.current_secret.clone();
            let tx             = state.tx.clone();
            let pending        = state.checkout_pending.clone();

            tokio::spawn(async move {
                license::poll_for_device_license(device_id, db, current_plan, current_secret, tx).await;
                pending.store(false, std::sync::atomic::Ordering::Relaxed);
            });

            Json(json!({ "checkout_url": checkout_url })).into_response()
        }
        Err(e) => (StatusCode::BAD_GATEWAY, Json(json!({"error": e}))).into_response(),
    }
}

/// GET /api/license/checkout/status — check if a checkout is in progress.
async fn checkout_status_handler(State(state): State<AppState>) -> Response {
    let pending = state.checkout_pending.load(std::sync::atomic::Ordering::Relaxed);
    Json(json!({ "pending": pending })).into_response()
}

async fn deactivate_license_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
) -> Response {
    if sess.role != "super_admin" {
        return (axum::http::StatusCode::FORBIDDEN, "Super admin access required").into_response();
    }
    let conn = state.db.lock().unwrap();
    let result = handlers::deactivate_license(&conn);
    if result.is_ok() {
        broadcast_change(&state.tx, "license", "deactivate");
    }
    to_response(result)
}

// ─── Auth handlers ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LoginBody { username: String, password: String }

async fn login_handler(State(state): State<AppState>, Json(body): Json<LoginBody>) -> Response {
    let conn = state.db.lock().unwrap();
    match handlers::login(&conn, &body.username, &body.password) {
        Ok(session) => match auth::encode_jwt(&session, &state.jwt_secret) {
            Ok(token) => {
                broadcast_change(&state.tx, "auth", "login");
                Json(json!({"token": token, "session": session})).into_response()
            }
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))).into_response(),
        },
        Err(e) => (StatusCode::UNAUTHORIZED, Json(json!({"error": e}))).into_response(),
    }
}

async fn logout_handler() -> Response {
    Json(json!({"ok": true})).into_response()
}

async fn get_session_handler(Extension(sess): Extension<SessionPayload>) -> Response {
    Json(serde_json::to_value(&sess).unwrap()).into_response()
}

async fn has_users_handler(State(state): State<AppState>) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::has_users(&conn))
}

// ─── User handlers ────────────────────────────────────────────────────────────

async fn get_users_handler(State(state): State<AppState>, Extension(sess): Extension<SessionPayload>) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::get_users(&conn, &sess))
}

#[derive(Deserialize)]
struct CreateUserBody {
    username: String,
    display_name: String,
    password: String,
    role: String,
    org_id: Option<i64>,
}

async fn create_user_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Json(body): Json<CreateUserBody>,
) -> Response {
    use models::NewUser;
    let user = NewUser {
        username: body.username,
        display_name: body.display_name,
        password: body.password,
        role: body.role,
        org_id: body.org_id,
    };
    let cached_plan = state.current_plan.read().unwrap().clone();
    let conn = state.db.lock().unwrap();
    let result = handlers::create_user(&conn, &sess, user, &cached_plan);
    if result.is_ok() { broadcast_change(&state.tx, "users", "create"); }
    to_response(result)
}

async fn delete_user_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Path(id): Path<i64>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::delete_user(&conn, &sess, id);
    if result.is_ok() { broadcast_change(&state.tx, "users", "delete"); }
    to_response(result)
}

#[derive(Deserialize)]
struct ChangePasswordBody { old_password: String, new_password: String }

async fn change_password_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Json(body): Json<ChangePasswordBody>,
) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::change_password(&conn, &sess, body.old_password, body.new_password))
}

#[derive(Deserialize)]
struct UpdateDisplayNameBody { new_name: String }

async fn update_display_name_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Json(body): Json<UpdateDisplayNameBody>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::update_display_name(&conn, &sess, body.new_name);
    if result.is_ok() { broadcast_change(&state.tx, "users", "update"); }
    to_response(result)
}

#[derive(Deserialize)]
struct AdminResetPasswordBody { new_password: String }

async fn admin_reset_password_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Path(user_id): Path<i64>,
    Json(body): Json<AdminResetPasswordBody>,
) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::admin_reset_password(&conn, &sess, user_id, body.new_password))
}

#[derive(Deserialize)]
struct SetUserActiveBody { active: bool }

async fn set_user_active_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Path(user_id): Path<i64>,
    Json(body): Json<SetUserActiveBody>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::set_user_active(&conn, &sess, user_id, body.active);
    if result.is_ok() { broadcast_change(&state.tx, "users", "update"); }
    to_response(result)
}

// ─── Organization handlers ────────────────────────────────────────────────────

async fn get_organizations_handler(State(state): State<AppState>, Extension(_sess): Extension<SessionPayload>) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::get_organizations(&conn))
}

async fn create_organization_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Json(body): Json<models::NewOrganization>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::create_organization(&conn, &sess, body);
    if result.is_ok() { broadcast_change(&state.tx, "organizations", "create"); }
    to_response(result)
}

async fn update_organization_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Path(id): Path<i64>,
    Json(body): Json<models::NewOrganization>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::update_organization(&conn, &sess, id, body);
    if result.is_ok() { broadcast_change(&state.tx, "organizations", "update"); }
    to_response(result)
}

async fn delete_organization_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Path(id): Path<i64>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::delete_organization(&conn, &sess, id);
    if result.is_ok() { broadcast_change(&state.tx, "organizations", "delete"); }
    to_response(result)
}

// ─── Semester handlers ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct OrgIdQuery { org_id: Option<i64> }

async fn get_semesters_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Query(q): Query<OrgIdQuery>,
) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::get_semesters(&conn, &sess, q.org_id))
}

async fn create_semester_handler(
    State(state): State<AppState>,
    Extension(_sess): Extension<SessionPayload>,
    Json(body): Json<models::NewSemester>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::create_semester(&conn, body);
    if result.is_ok() { broadcast_change(&state.tx, "semesters", "create"); }
    to_response(result)
}

async fn update_semester_handler(
    State(state): State<AppState>,
    Extension(_sess): Extension<SessionPayload>,
    Path(id): Path<i64>,
    Json(body): Json<models::NewSemester>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::update_semester(&conn, id, body);
    if result.is_ok() { broadcast_change(&state.tx, "semesters", "update"); }
    to_response(result)
}

async fn delete_semester_handler(
    State(state): State<AppState>,
    Extension(_sess): Extension<SessionPayload>,
    Path(id): Path<i64>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::delete_semester(&conn, id);
    if result.is_ok() { broadcast_change(&state.tx, "semesters", "delete"); }
    to_response(result)
}

// ─── Course handlers ──────────────────────────────────────────────────────────

async fn get_courses_handler(State(state): State<AppState>, Extension(sess): Extension<SessionPayload>) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::get_courses(&conn, &sess))
}

async fn create_course_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Json(body): Json<models::NewCourse>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::create_course(&conn, &sess, body);
    if result.is_ok() { broadcast_change(&state.tx, "courses", "create"); }
    to_response(result)
}

async fn update_course_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Path(id): Path<i64>,
    Json(body): Json<models::NewCourse>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::update_course(&conn, &sess, id, body);
    if result.is_ok() { broadcast_change(&state.tx, "courses", "update"); }
    to_response(result)
}

async fn delete_course_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Path(id): Path<i64>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::delete_course(&conn, &sess, id);
    if result.is_ok() { broadcast_change(&state.tx, "courses", "delete"); }
    to_response(result)
}

// ─── Lecturer handlers ────────────────────────────────────────────────────────

async fn get_lecturers_handler(State(state): State<AppState>, Extension(sess): Extension<SessionPayload>) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::get_lecturers(&conn, &sess))
}

async fn create_lecturer_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Json(body): Json<models::NewLecturer>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::create_lecturer(&conn, &sess, body);
    if result.is_ok() { broadcast_change(&state.tx, "lecturers", "create"); }
    to_response(result)
}

async fn update_lecturer_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Path(id): Path<i64>,
    Json(body): Json<models::NewLecturer>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::update_lecturer(&conn, &sess, id, body);
    if result.is_ok() { broadcast_change(&state.tx, "lecturers", "update"); }
    to_response(result)
}

async fn delete_lecturer_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Path(id): Path<i64>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::delete_lecturer(&conn, &sess, id);
    if result.is_ok() { broadcast_change(&state.tx, "lecturers", "delete"); }
    to_response(result)
}

// ─── Room handlers ────────────────────────────────────────────────────────────

async fn get_rooms_handler(State(state): State<AppState>, Extension(sess): Extension<SessionPayload>) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::get_rooms(&conn, &sess))
}

async fn create_room_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Json(body): Json<models::NewRoom>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::create_room(&conn, &sess, body);
    if result.is_ok() { broadcast_change(&state.tx, "rooms", "create"); }
    to_response(result)
}

async fn update_room_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Path(id): Path<i64>,
    Json(body): Json<models::NewRoom>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::update_room(&conn, &sess, id, body);
    if result.is_ok() { broadcast_change(&state.tx, "rooms", "update"); }
    to_response(result)
}

async fn delete_room_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Path(id): Path<i64>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::delete_room(&conn, &sess, id);
    if result.is_ok() { broadcast_change(&state.tx, "rooms", "delete"); }
    to_response(result)
}

// ─── Batch handlers ───────────────────────────────────────────────────────────

async fn get_batches_handler(State(state): State<AppState>, Extension(sess): Extension<SessionPayload>) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::get_batches(&conn, &sess))
}

async fn create_batch_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Json(body): Json<models::NewBatch>,
) -> Response {
    let cached_plan = state.current_plan.read().unwrap().clone();
    let conn = state.db.lock().unwrap();
    let result = handlers::create_batch(&conn, &sess, body, &cached_plan);
    if result.is_ok() { broadcast_change(&state.tx, "batches", "create"); }
    to_response(result)
}

async fn update_batch_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Path(id): Path<i64>,
    Json(body): Json<models::NewBatch>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::update_batch(&conn, &sess, id, body);
    if result.is_ok() { broadcast_change(&state.tx, "batches", "update"); }
    to_response(result)
}

async fn delete_batch_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Path(id): Path<i64>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::delete_batch(&conn, &sess, id);
    if result.is_ok() { broadcast_change(&state.tx, "batches", "delete"); }
    to_response(result)
}

// ─── Schedule handlers ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct GenerateScheduleBody {
    schedule_name: String,
    semester_id: Option<i64>,
    description: Option<String>,
    algorithm: Option<String>,
}

async fn generate_schedule_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Json(body): Json<GenerateScheduleBody>,
) -> Response {
    let cached_plan = state.current_plan.read().unwrap().clone();
    let conn = state.db.lock().unwrap();
    let result = handlers::generate_schedule(&conn, &sess, body.schedule_name, body.semester_id, body.description, body.algorithm, &cached_plan);
    if result.is_ok() { broadcast_change(&state.tx, "schedules", "create"); }
    to_response(result)
}

async fn get_schedules_handler(State(state): State<AppState>, Extension(sess): Extension<SessionPayload>) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::get_schedules(&conn, &sess))
}

async fn get_schedule_entries_handler(
    State(state): State<AppState>,
    Extension(_sess): Extension<SessionPayload>,
    Path(id): Path<i64>,
) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::get_schedule_entries(&conn, id))
}

async fn activate_schedule_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Path(id): Path<i64>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::activate_schedule(&conn, &sess, id);
    if result.is_ok() { broadcast_change(&state.tx, "schedules", "activate"); }
    to_response(result)
}

async fn publish_schedule_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Path(id): Path<i64>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::publish_schedule(&conn, &sess, id);
    if result.is_ok() { broadcast_change(&state.tx, "schedules", "publish"); }
    to_response(result)
}

async fn revert_schedule_to_draft_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Path(id): Path<i64>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::revert_schedule_to_draft(&conn, &sess, id);
    if result.is_ok() { broadcast_change(&state.tx, "schedules", "revert"); }
    to_response(result)
}

async fn delete_schedule_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Path(id): Path<i64>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::delete_schedule(&conn, &sess, id);
    if result.is_ok() { broadcast_change(&state.tx, "schedules", "delete"); }
    to_response(result)
}

async fn export_schedule_csv_handler(
    State(state): State<AppState>,
    Extension(_sess): Extension<SessionPayload>,
    Path(id): Path<i64>,
) -> Response {
    let conn = state.db.lock().unwrap();
    match handlers::export_schedule_csv(&conn, id) {
        Ok(csv) => (
            StatusCode::OK,
            [("content-type", "text/csv"), ("content-disposition", "attachment; filename=\"schedule.csv\"")],
            csv,
        ).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({"error": e}))).into_response(),
    }
}

#[derive(Deserialize)]
struct UpdateScheduleEntryBody {
    day: String,
    time_slot: i64,
    room_id: i64,
}

async fn update_schedule_entry_handler(
    State(state): State<AppState>,
    Extension(_sess): Extension<SessionPayload>,
    Path(entry_id): Path<i64>,
    Json(body): Json<UpdateScheduleEntryBody>,
) -> Response {
    use models::UpdateScheduleEntryReq;
    let req = UpdateScheduleEntryReq { day: body.day, time_slot: body.time_slot, room_id: body.room_id };
    let conn = state.db.lock().unwrap();
    let result = handlers::update_schedule_entry(&conn, entry_id, req);
    if result.is_ok() { broadcast_change(&state.tx, "schedules", "update"); }
    to_response(result)
}

#[derive(Deserialize)]
struct UpdateScheduleDescBody { description: Option<String> }

async fn update_schedule_description_handler(
    State(state): State<AppState>,
    Extension(_sess): Extension<SessionPayload>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateScheduleDescBody>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::update_schedule_description(&conn, id, body.description);
    if result.is_ok() { broadcast_change(&state.tx, "schedules", "update"); }
    to_response(result)
}

// ─── Stats / info ─────────────────────────────────────────────────────────────

async fn get_stats_handler(State(state): State<AppState>, Extension(sess): Extension<SessionPayload>) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::get_stats(&conn, &sess))
}

async fn get_app_info_handler(State(state): State<AppState>, Extension(_sess): Extension<SessionPayload>) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::get_app_info(&conn))
}

async fn get_utilization_report_handler(
    State(state): State<AppState>,
    Extension(_sess): Extension<SessionPayload>,
    Path(id): Path<i64>,
) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::get_utilization_report(&conn, id))
}

// ─── Audit ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LimitQuery { limit: Option<i64> }

async fn get_audit_log_handler(
    State(state): State<AppState>,
    Extension(_sess): Extension<SessionPayload>,
    Query(q): Query<LimitQuery>,
) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::get_audit_log(&conn, q.limit.unwrap_or(100)))
}

// ─── Settings ─────────────────────────────────────────────────────────────────

async fn get_scheduling_settings_handler(
    State(state): State<AppState>,
    Extension(_sess): Extension<SessionPayload>,
    Path(org_id): Path<i64>,
) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::get_scheduling_settings(&conn, org_id))
}

async fn upsert_scheduling_settings_handler(
    State(state): State<AppState>,
    Extension(_sess): Extension<SessionPayload>,
    Json(body): Json<models::OrgSchedulingSettings>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::upsert_scheduling_settings(&conn, body);
    if result.is_ok() { broadcast_change(&state.tx, "settings", "update"); }
    to_response(result)
}

async fn clear_schedules_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::clear_schedules(&conn, &sess);
    if result.is_ok() { broadcast_change(&state.tx, "schedules", "clear"); }
    to_response(result)
}

async fn backup_database_handler(
    State(state): State<AppState>,
    Extension(_sess): Extension<SessionPayload>,
) -> Response {
    let conn = state.db.lock().unwrap();
    match handlers::backup_database(&conn) {
        Ok(json_str) => (
            StatusCode::OK,
            [("content-type", "application/json"), ("content-disposition", "attachment; filename=\"schedula-backup.json\"")],
            json_str,
        ).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))).into_response(),
    }
}

async fn get_max_admins_handler(
    State(state): State<AppState>,
    Extension(_sess): Extension<SessionPayload>,
) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::get_max_admins(&conn))
}

#[derive(Deserialize)]
struct SetMaxAdminsBody { max: i64 }

async fn set_max_admins_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Json(body): Json<SetMaxAdminsBody>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::set_max_admins(&conn, &sess, body.max);
    if result.is_ok() { broadcast_change(&state.tx, "settings", "update"); }
    to_response(result)
}

async fn get_admin_count_handler(
    State(state): State<AppState>,
    Extension(_sess): Extension<SessionPayload>,
) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::get_admin_count(&conn))
}

// ─── Pre-flight / data health ─────────────────────────────────────────────────

async fn get_preflight_warnings_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::get_preflight_warnings(&conn, &sess))
}

async fn get_data_health_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::get_data_health(&conn, &sess))
}

// ─── Password recovery handlers ───────────────────────────────────────────────

async fn get_security_question_handler(State(state): State<AppState>) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::get_security_question(&conn))
}

#[derive(Deserialize)]
struct ResetWithCodeBody { recovery_code: String, new_password: String }

async fn reset_with_code_handler(State(state): State<AppState>, Json(body): Json<ResetWithCodeBody>) -> Response {
    use models::ResetPasswordWithCodeRequest;
    let conn = state.db.lock().unwrap();
    to_response(handlers::reset_password_with_recovery_code(&conn, ResetPasswordWithCodeRequest {
        recovery_code: body.recovery_code,
        new_password: body.new_password,
    }))
}

#[derive(Deserialize)]
struct ResetWithAnswerBody { security_answer: String, new_password: String }

async fn reset_with_answer_handler(State(state): State<AppState>, Json(body): Json<ResetWithAnswerBody>) -> Response {
    use models::ResetPasswordWithAnswerRequest;
    let conn = state.db.lock().unwrap();
    to_response(handlers::reset_password_with_security_answer(&conn, ResetPasswordWithAnswerRequest {
        security_answer: body.security_answer,
        new_password: body.new_password,
    }))
}

async fn setup_recovery_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Json(body): Json<models::SetupRecoveryRequest>,
) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::setup_recovery(&conn, &sess, body))
}

// ─── Approval handlers ────────────────────────────────────────────────────────

async fn create_approval_handler(State(state): State<AppState>, Json(body): Json<models::CreateApprovalReq>) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::create_approval_request(&conn, body);
    if result.is_ok() { broadcast_change(&state.tx, "approvals", "create"); }
    to_response(result)
}

async fn get_my_approval_status_handler(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::get_my_approval_status(&conn, username))
}

async fn get_pending_approvals_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::get_pending_approvals(&conn, &sess))
}

async fn get_approval_count_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
) -> Response {
    let conn = state.db.lock().unwrap();
    to_response(handlers::get_approval_count(&conn, &sess))
}

#[derive(Deserialize)]
struct ResolveApprovalBody { approved: bool, rejection_reason: Option<String> }

async fn resolve_approval_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Path(id): Path<i64>,
    Json(body): Json<ResolveApprovalBody>,
) -> Response {
    let conn = state.db.lock().unwrap();
    let result = handlers::resolve_approval(&conn, &sess, id, body.approved, body.rejection_reason);
    if result.is_ok() { broadcast_change(&state.tx, "approvals", "resolve"); }
    to_response(result)
}

// ─── Bulk import handlers ─────────────────────────────────────────────────────

async fn bulk_import_lecturers_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Json(rows): Json<Vec<models::CsvLecturer>>,
) -> Response {
    let cached_plan = state.current_plan.read().unwrap().clone();
    let conn = state.db.lock().unwrap();
    let result = handlers::bulk_import_lecturers(&conn, &sess, rows, &cached_plan);
    if result.is_ok() { broadcast_change(&state.tx, "lecturers", "import"); }
    to_response(result)
}

async fn bulk_import_rooms_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Json(rows): Json<Vec<models::CsvRoom>>,
) -> Response {
    let cached_plan = state.current_plan.read().unwrap().clone();
    let conn = state.db.lock().unwrap();
    let result = handlers::bulk_import_rooms(&conn, &sess, rows, &cached_plan);
    if result.is_ok() { broadcast_change(&state.tx, "rooms", "import"); }
    to_response(result)
}

async fn bulk_import_courses_handler(
    State(state): State<AppState>,
    Extension(sess): Extension<SessionPayload>,
    Json(rows): Json<Vec<models::CsvCourse>>,
) -> Response {
    let cached_plan = state.current_plan.read().unwrap().clone();
    let conn = state.db.lock().unwrap();
    let result = handlers::bulk_import_courses(&conn, &sess, rows, &cached_plan);
    if result.is_ok() { broadcast_change(&state.tx, "courses", "import"); }
    to_response(result)
}

// ─── Main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let jwt_secret = if !args.jwt_secret.is_empty() {
        args.jwt_secret.clone()
    } else {
        let key_file = PathBuf::from("hub-secret.key");
        if key_file.exists() {
            std::fs::read_to_string(&key_file).unwrap_or_default().trim().to_string()
        } else {
            let secret = uuid::Uuid::new_v4().to_string();
            std::fs::write(&key_file, &secret).ok();
            secret
        }
    };

    let db_path = PathBuf::from(&args.db_path);
    let conn = db::open(&db_path).expect("Failed to open database");

    // Startup: expire hard-lapsed tokens, derive initial plan from cached JWT
    license::startup_license_check(&conn);
    let initial_plan   = license::effective_plan(&conn); // RS256-verified, grace-aware

    // Recover the last known secret key from the DB (may be empty on first run)
    let initial_secret: String = conn.query_row(
        "SELECT COALESCE(secret_key, '') FROM licenses
         WHERE status IN ('active','grace')
         ORDER BY activated_at DESC LIMIT 1",
        [],
        |r| r.get(0),
    ).unwrap_or_default();

    let (tx, _) = broadcast::channel(64);
    let listen_addr    = format!("0.0.0.0:{}", args.port);
    let current_plan   = Arc::new(RwLock::new(initial_plan));
    let current_secret = Arc::new(RwLock::new(initial_secret));

    let state = AppState {
        db: Arc::new(Mutex::new(conn)),
        jwt_secret,
        tx,
        db_path,
        ws_count:         Arc::new(AtomicUsize::new(0)),
        start_time:       Arc::new(Instant::now()),
        listen_addr:      listen_addr.clone(),
        current_plan:     current_plan.clone(),
        current_secret:   current_secret.clone(),
        checkout_pending: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    };

    // Background refresh: calls /v1/refresh immediately on startup, then every 24 h.
    // Updates current_plan and current_secret caches on every successful refresh.
    tokio::spawn(license::background_refresh_loop(
        state.db.clone(),
        current_plan,
        current_secret,
    ));

    // Public routes (no auth required)
    let public_routes = Router::new()
        .route("/",          get(ui_handler))
        .route("/admin",     get(ui_handler))
        .route("/api/server/status", get(server_status_handler))
        .route("/api/auth/login", post(login_handler))
        .route("/api/auth/logout", post(logout_handler))
        .route("/api/auth/has-users", get(has_users_handler))
        .route("/api/recovery/question", get(get_security_question_handler))
        .route("/api/recovery/reset-with-code", post(reset_with_code_handler))
        .route("/api/recovery/reset-with-answer", post(reset_with_answer_handler))
        .route("/api/approvals", post(create_approval_handler))
        .route("/api/approvals/my/:username", get(get_my_approval_status_handler))
        .route("/ws", get(ws_handler))
        .route("/health", get(|| async { Json(json!({"status": "ok"})) }))
        .route("/api/license",                  get(get_license_handler))
        // License activation — RS256 JWT signature is proof of entitlement
        .route("/api/license/activate",         post(activate_license_handler))
        // Device-based checkout — device_id acts as identity, no session needed
        .route("/api/license/checkout",         post(checkout_license_handler))
        .route("/api/license/checkout/status",  get(checkout_status_handler));

    // Protected routes (JWT required)
    let protected_routes = Router::new()
        // Session
        .route("/api/auth/session", get(get_session_handler))
        // Users
        .route("/api/users", get(get_users_handler))
        .route("/api/users", post(create_user_handler))
        .route("/api/users/:id", delete(delete_user_handler))
        .route("/api/users/change-password", post(change_password_handler))
        .route("/api/settings/display-name", put(update_display_name_handler))
        .route("/api/users/:id/password", post(admin_reset_password_handler))
        .route("/api/users/:id/active", put(set_user_active_handler))
        // Organizations
        .route("/api/orgs", get(get_organizations_handler))
        .route("/api/orgs", post(create_organization_handler))
        .route("/api/orgs/:id", put(update_organization_handler))
        .route("/api/orgs/:id", delete(delete_organization_handler))
        // Semesters
        .route("/api/semesters", get(get_semesters_handler))
        .route("/api/semesters", post(create_semester_handler))
        .route("/api/semesters/:id", put(update_semester_handler))
        .route("/api/semesters/:id", delete(delete_semester_handler))
        // Courses
        .route("/api/courses", get(get_courses_handler))
        .route("/api/courses", post(create_course_handler))
        .route("/api/courses/:id", put(update_course_handler))
        .route("/api/courses/:id", delete(delete_course_handler))
        // Lecturers
        .route("/api/lecturers", get(get_lecturers_handler))
        .route("/api/lecturers", post(create_lecturer_handler))
        .route("/api/lecturers/:id", put(update_lecturer_handler))
        .route("/api/lecturers/:id", delete(delete_lecturer_handler))
        // Rooms
        .route("/api/rooms", get(get_rooms_handler))
        .route("/api/rooms", post(create_room_handler))
        .route("/api/rooms/:id", put(update_room_handler))
        .route("/api/rooms/:id", delete(delete_room_handler))
        // Batches
        .route("/api/batches", get(get_batches_handler))
        .route("/api/batches", post(create_batch_handler))
        .route("/api/batches/:id", put(update_batch_handler))
        .route("/api/batches/:id", delete(delete_batch_handler))
        // Plan / subscription
        .route("/api/plan", get(plan_handler))
        // License
        .route("/api/license/deactivate", post(deactivate_license_handler))
        // Schedules
        .route("/api/schedules/generate", post(generate_schedule_handler))
        .route("/api/schedules", get(get_schedules_handler))
        .route("/api/schedules/:id/entries", get(get_schedule_entries_handler))
        .route("/api/schedules/:id/activate", put(activate_schedule_handler))
        .route("/api/schedules/:id/publish", put(publish_schedule_handler))
        .route("/api/schedules/:id/draft", put(revert_schedule_to_draft_handler))
        .route("/api/schedules/:id", delete(delete_schedule_handler))
        .route("/api/schedules/:id/csv", get(export_schedule_csv_handler))
        .route("/api/schedules/:id/description", put(update_schedule_description_handler))
        .route("/api/schedule-entries/:id", put(update_schedule_entry_handler))
        // Stats / reports
        .route("/api/stats", get(get_stats_handler))
        .route("/api/settings/app-info", get(get_app_info_handler))
        .route("/api/reports/utilization/:id", get(get_utilization_report_handler))
        .route("/api/preflight", get(get_preflight_warnings_handler))
        .route("/api/data-health", get(get_data_health_handler))
        // Audit
        .route("/api/audit-log", get(get_audit_log_handler))
        // Settings
        .route("/api/settings/scheduling/:org_id", get(get_scheduling_settings_handler))
        .route("/api/settings/scheduling", put(upsert_scheduling_settings_handler))
        .route("/api/settings/clear-schedules", post(clear_schedules_handler))
        .route("/api/settings/backup", get(backup_database_handler))
        .route("/api/settings/max-admins", get(get_max_admins_handler))
        .route("/api/settings/max-admins", put(set_max_admins_handler))
        .route("/api/settings/admin-count", get(get_admin_count_handler))
        // Recovery
        .route("/api/recovery/setup", post(setup_recovery_handler))
        // Approvals (GET /api/approvals = pending list, POST /api/approvals = create handled in public)
        .route("/api/approvals", get(get_pending_approvals_handler))
        .route("/api/approvals/count", get(get_approval_count_handler))
        .route("/api/approvals/:id/resolve", put(resolve_approval_handler))
        // Bulk import
        .route("/api/import/lecturers", post(bulk_import_lecturers_handler))
        .route("/api/import/rooms", post(bulk_import_rooms_handler))
        .route("/api/import/courses", post(bulk_import_courses_handler))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    let app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(CorsLayer::very_permissive())
        .with_state(state);

    println!("Schedula Hub Server v{}", env!("CARGO_PKG_VERSION"));
    println!("Listening on  http://{}", listen_addr);
    println!("Admin UI      http://{}/", listen_addr);
    println!("Database:     {}", args.db_path);
    println!("WebSocket:    ws://{}/ws", listen_addr);

    let listener = tokio::net::TcpListener::bind(&listen_addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
