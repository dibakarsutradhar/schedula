use rusqlite::{params, Connection};
use serde_json::Value;
use std::sync::Mutex;
use tauri::State;

use crate::models::*;
use crate::scheduler::{self, SchedulerInput};

pub struct DbState(pub Mutex<Connection>);
pub struct SessionState(pub Mutex<Option<SessionPayload>>);

fn db_err(e: impl std::fmt::Display) -> String { e.to_string() }

// ─── Plan helpers ─────────────────────────────────────────────────────────────

fn get_org_plan(conn: &Connection, org_id: Option<i64>) -> String {
    org_id.and_then(|id| {
        conn.query_row(
            "SELECT plan FROM organizations WHERE id=?1",
            params![id],
            |r| r.get::<_, String>(0),
        ).ok()
    }).unwrap_or_else(|| PLAN_FREE.to_string())
}

fn plan_limit_err(e: PlanLimitError) -> String {
    e.to_json_string()
}

// ─── Auth guard ───────────────────────────────────────────────────────────────

fn require_session(session: &State<SessionState>) -> Result<SessionPayload, String> {
    session.0.lock().map_err(db_err)?
        .clone()
        .ok_or_else(|| "Not logged in".into())
}

fn require_super_admin(session: &State<SessionState>) -> Result<SessionPayload, String> {
    let s = require_session(session)?;
    if s.role != "super_admin" {
        return Err("Super admin access required".into());
    }
    Ok(s)
}

// ══════════════════════════════════════════════════════════════════════════════
// AUTH
// ══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub fn login(
    db: State<DbState>,
    session: State<SessionState>,
    req: LoginRequest,
) -> Result<SessionPayload, String> {
    let conn = db.0.lock().map_err(db_err)?;
    let row = conn.query_row(
        "SELECT u.id, u.username, u.display_name, u.password_hash, u.role, u.org_id
         FROM users u WHERE u.username = ?1",
        params![req.username],
        |r| Ok((
            r.get::<_,i64>(0)?,
            r.get::<_,String>(1)?,
            r.get::<_,String>(2)?,
            r.get::<_,String>(3)?,
            r.get::<_,String>(4)?,
            r.get::<_,Option<i64>>(5)?,
        )),
    ).map_err(|_| "Invalid username or password".to_string())?;

    let (id, username, display_name, hash, role, org_id) = row;
    bcrypt::verify(&req.password, &hash)
        .map_err(db_err)?
        .then_some(())
        .ok_or_else(|| "Invalid username or password".to_string())?;

    let payload = SessionPayload { user_id: id, username, display_name, role, org_id };
    *session.0.lock().map_err(db_err)? = Some(payload.clone());
    Ok(payload)
}

#[tauri::command]
pub fn logout(session: State<SessionState>) -> Result<(), String> {
    *session.0.lock().map_err(db_err)? = None;
    Ok(())
}

#[tauri::command]
pub fn get_session(session: State<SessionState>) -> Result<Option<SessionPayload>, String> {
    Ok(session.0.lock().map_err(db_err)?.clone())
}

#[tauri::command]
pub fn has_users(db: State<DbState>) -> Result<bool, String> {
    let conn = db.0.lock().map_err(db_err)?;
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0)).map_err(db_err)?;
    Ok(count > 0)
}

// ══════════════════════════════════════════════════════════════════════════════
// USERS
// ══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub fn get_users(db: State<DbState>, session: State<SessionState>) -> Result<Vec<User>, String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;

    let sql = if s.role == "super_admin" {
        "SELECT u.id, u.username, u.display_name, u.role, u.org_id, o.name, u.is_active
         FROM users u LEFT JOIN organizations o ON o.id = u.org_id ORDER BY u.username".to_string()
    } else {
        format!(
            "SELECT u.id, u.username, u.display_name, u.role, u.org_id, o.name, u.is_active
             FROM users u LEFT JOIN organizations o ON o.id = u.org_id
             WHERE u.org_id = {} ORDER BY u.username",
            s.org_id.unwrap_or(-1)
        )
    };

    let mut stmt = conn.prepare(&sql).map_err(db_err)?;
    let rows: Result<Vec<User>, _> = stmt.query_map([], |row| Ok(User {
        id: row.get(0)?,
        username: row.get(1)?,
        display_name: row.get(2)?,
        role: row.get(3)?,
        org_id: row.get(4)?,
        org_name: row.get(5)?,
        is_active: row.get::<_,i64>(6).unwrap_or(1) != 0,
    })).map_err(db_err)?.collect();
    rows.map_err(db_err)
}

#[tauri::command]
pub fn create_user(
    db: State<DbState>,
    session: State<SessionState>,
    user: NewUser,
) -> Result<i64, String> {
    let s = require_super_admin(&session)?;
    let conn = db.0.lock().map_err(db_err)?;

    // Enforce: only one super_admin ever
    if user.role == "super_admin" {
        return Err("Only one super admin is allowed per app instance.".into());
    }

    // Enforce plan-based admin limit (per org)
    if user.role == "admin" {
        let org_id = user.org_id;
        let plan = get_org_plan(&conn, org_id);
        let limits = PlanLimits::for_plan(&plan);
        if limits.max_admins >= 0 {
            let current: i64 = conn.query_row(
                "SELECT COUNT(*) FROM users WHERE role='admin' AND is_active=1 AND (org_id IS ?1 OR org_id=?1)",
                params![org_id], |r| r.get(0),
            ).unwrap_or(0);
            if current >= limits.max_admins {
                return Err(plan_limit_err(PlanLimitError::new(
                    plan, "admins", limits.max_admins, current,
                )));
            }
        }
    }

    let hash = bcrypt::hash(&user.password, bcrypt::DEFAULT_COST).map_err(db_err)?;
    conn.execute(
        "INSERT INTO users (username, display_name, password_hash, role, org_id) VALUES (?1,?2,?3,?4,?5)",
        params![user.username, user.display_name, hash, user.role, user.org_id],
    ).map_err(db_err)?;
    let id = conn.last_insert_rowid();
    log_audit(&conn, &s, "create", "user", Some(id), Some(&user.username));
    Ok(id)
}

#[tauri::command]
pub fn delete_user(db: State<DbState>, session: State<SessionState>, id: i64) -> Result<(), String> {
    let s = require_super_admin(&session)?;
    if s.user_id == id { return Err("Cannot delete yourself".into()); }
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute("DELETE FROM users WHERE id=?1", params![id]).map_err(db_err)?;
    log_audit(&conn, &s, "delete", "user", Some(id), None);
    Ok(())
}

#[tauri::command]
pub fn change_password(
    db: State<DbState>,
    session: State<SessionState>,
    old_password: String,
    new_password: String,
) -> Result<(), String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    let hash: String = conn.query_row(
        "SELECT password_hash FROM users WHERE id=?1",
        params![s.user_id], |r| r.get(0),
    ).map_err(db_err)?;
    bcrypt::verify(&old_password, &hash).map_err(db_err)?
        .then_some(())
        .ok_or_else(|| "Old password incorrect".to_string())?;
    let new_hash = bcrypt::hash(&new_password, bcrypt::DEFAULT_COST).map_err(db_err)?;
    conn.execute("UPDATE users SET password_hash=?1 WHERE id=?2", params![new_hash, s.user_id]).map_err(db_err)?;
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// ORGANIZATIONS
// ══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub fn get_organizations(db: State<DbState>, session: State<SessionState>) -> Result<Vec<Organization>, String> {
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    let mut stmt = conn.prepare(
        "SELECT id, name, org_type, address, contact_email FROM organizations ORDER BY name"
    ).map_err(db_err)?;
    let rows: Result<Vec<Organization>, _> = stmt.query_map([], |row| Ok(Organization {
        id: row.get(0)?,
        name: row.get(1)?,
        org_type: row.get(2)?,
        address: row.get(3)?,
        contact_email: row.get(4)?,
    })).map_err(db_err)?.collect();
    rows.map_err(db_err)
}

#[tauri::command]
pub fn create_organization(
    db: State<DbState>,
    session: State<SessionState>,
    org: NewOrganization,
) -> Result<i64, String> {
    require_super_admin(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM organizations", [], |r| r.get(0)).map_err(db_err)?;
    if count > 0 {
        return Err("Only one organization is allowed per app instance. Edit the existing organization instead.".into());
    }
    conn.execute(
        "INSERT INTO organizations (name, org_type, address, contact_email) VALUES (?1,?2,?3,?4)",
        params![org.name, org.org_type, org.address, org.contact_email],
    ).map_err(db_err)?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_organization(
    db: State<DbState>,
    session: State<SessionState>,
    id: i64,
    org: NewOrganization,
) -> Result<(), String> {
    require_super_admin(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute(
        "UPDATE organizations SET name=?1, org_type=?2, address=?3, contact_email=?4 WHERE id=?5",
        params![org.name, org.org_type, org.address, org.contact_email, id],
    ).map_err(db_err)?;
    Ok(())
}

#[tauri::command]
pub fn delete_organization(
    db: State<DbState>,
    session: State<SessionState>,
    id: i64,
) -> Result<(), String> {
    require_super_admin(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute("DELETE FROM organizations WHERE id=?1", params![id]).map_err(db_err)?;
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// SEMESTERS
// ══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub fn get_semesters(
    db: State<DbState>,
    session: State<SessionState>,
    org_id_filter: Option<i64>,
) -> Result<Vec<Semester>, String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;

    // Admins are scoped to their org; super_admin can filter optionally
    let effective_org = if s.role == "super_admin" { org_id_filter } else { s.org_id };

    let sql = match effective_org {
        Some(oid) => format!(
            "SELECT s.id, s.org_id, o.name, s.name, s.start_date, s.end_date,
                    s.student_capacity, s.teaching_weeks,
                    s.midterm_start, s.midterm_end, s.study_break_start, s.study_break_end,
                    s.final_start, s.final_end, s.breaks_json, s.status
             FROM semesters s JOIN organizations o ON o.id = s.org_id
             WHERE s.org_id = {} ORDER BY s.start_date DESC", oid
        ),
        None => "SELECT s.id, s.org_id, o.name, s.name, s.start_date, s.end_date,
                        s.student_capacity, s.teaching_weeks,
                        s.midterm_start, s.midterm_end, s.study_break_start, s.study_break_end,
                        s.final_start, s.final_end, s.breaks_json, s.status
                 FROM semesters s JOIN organizations o ON o.id = s.org_id
                 ORDER BY s.start_date DESC".to_string(),
    };

    let mut stmt = conn.prepare(&sql).map_err(db_err)?;
    let rows: Result<Vec<Semester>, _> = stmt.query_map([], |row| Ok(Semester {
        id: row.get(0)?,
        org_id: row.get(1)?,
        org_name: row.get(2)?,
        name: row.get(3)?,
        start_date: row.get(4)?,
        end_date: row.get(5)?,
        student_capacity: row.get(6)?,
        teaching_weeks: row.get(7)?,
        midterm_start: row.get(8)?,
        midterm_end: row.get(9)?,
        study_break_start: row.get(10)?,
        study_break_end: row.get(11)?,
        final_start: row.get(12)?,
        final_end: row.get(13)?,
        breaks_json: row.get(14)?,
        status: row.get(15)?,
    })).map_err(db_err)?.collect();
    rows.map_err(db_err)
}

#[tauri::command]
pub fn create_semester(
    db: State<DbState>,
    session: State<SessionState>,
    sem: NewSemester,
) -> Result<i64, String> {
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute(
        "INSERT INTO semesters
         (org_id, name, start_date, end_date, student_capacity, teaching_weeks,
          midterm_start, midterm_end, study_break_start, study_break_end,
          final_start, final_end, breaks_json, status)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
        params![
            sem.org_id, sem.name, sem.start_date, sem.end_date,
            sem.student_capacity, sem.teaching_weeks,
            sem.midterm_start, sem.midterm_end,
            sem.study_break_start, sem.study_break_end,
            sem.final_start, sem.final_end,
            sem.breaks_json, sem.status
        ],
    ).map_err(db_err)?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_semester(
    db: State<DbState>,
    session: State<SessionState>,
    id: i64,
    sem: NewSemester,
) -> Result<(), String> {
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute(
        "UPDATE semesters SET org_id=?1, name=?2, start_date=?3, end_date=?4,
         student_capacity=?5, teaching_weeks=?6,
         midterm_start=?7, midterm_end=?8, study_break_start=?9, study_break_end=?10,
         final_start=?11, final_end=?12, breaks_json=?13, status=?14
         WHERE id=?15",
        params![
            sem.org_id, sem.name, sem.start_date, sem.end_date,
            sem.student_capacity, sem.teaching_weeks,
            sem.midterm_start, sem.midterm_end,
            sem.study_break_start, sem.study_break_end,
            sem.final_start, sem.final_end,
            sem.breaks_json, sem.status, id
        ],
    ).map_err(db_err)?;
    Ok(())
}

#[tauri::command]
pub fn delete_semester(
    db: State<DbState>,
    session: State<SessionState>,
    id: i64,
) -> Result<(), String> {
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute("DELETE FROM semesters WHERE id=?1", params![id]).map_err(db_err)?;
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// COURSES
// ══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub fn get_courses(db: State<DbState>, session: State<SessionState>) -> Result<Vec<Course>, String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    load_courses_scoped(&conn, s.org_id_filter())
}

#[tauri::command]
pub fn create_course(db: State<DbState>, session: State<SessionState>, course: NewCourse) -> Result<i64, String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute(
        "INSERT INTO courses (code, name, hours_per_week, room_type, class_type, frequency, lecturer_id, org_id)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![course.code, course.name, course.hours_per_week, course.room_type, course.class_type, course.frequency, course.lecturer_id, course.org_id],
    ).map_err(db_err)?;
    let id = conn.last_insert_rowid();
    log_audit(&conn, &s, "create", "course", Some(id), Some(&course.code));
    Ok(id)
}

#[tauri::command]
pub fn update_course(db: State<DbState>, session: State<SessionState>, id: i64, course: NewCourse) -> Result<(), String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute(
        "UPDATE courses SET code=?1, name=?2, hours_per_week=?3, room_type=?4, class_type=?5, frequency=?6, lecturer_id=?7, org_id=?8 WHERE id=?9",
        params![course.code, course.name, course.hours_per_week, course.room_type, course.class_type, course.frequency, course.lecturer_id, course.org_id, id],
    ).map_err(db_err)?;
    log_audit(&conn, &s, "update", "course", Some(id), Some(&course.code));
    Ok(())
}

#[tauri::command]
pub fn delete_course(db: State<DbState>, session: State<SessionState>, id: i64) -> Result<(), String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute("DELETE FROM courses WHERE id=?1", params![id]).map_err(db_err)?;
    log_audit(&conn, &s, "delete", "course", Some(id), None);
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// LECTURERS
// ══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub fn get_lecturers(db: State<DbState>, session: State<SessionState>) -> Result<Vec<Lecturer>, String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    load_lecturers_scoped(&conn, s.org_id_filter())
}

#[tauri::command]
pub fn create_lecturer(db: State<DbState>, session: State<SessionState>, lecturer: NewLecturer) -> Result<i64, String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute(
        "INSERT INTO lecturers (name, email, available_days, max_hours_per_day, max_hours_per_week, org_id,
                                preferred_slots_json, blackout_json, max_consecutive_hours)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![lecturer.name, lecturer.email, lecturer.available_days,
                lecturer.max_hours_per_day, lecturer.max_hours_per_week, lecturer.org_id,
                lecturer.preferred_slots_json, lecturer.blackout_json, lecturer.max_consecutive_hours],
    ).map_err(db_err)?;
    let id = conn.last_insert_rowid();
    log_audit(&conn, &s, "create", "lecturer", Some(id), Some(&lecturer.name));
    Ok(id)
}

#[tauri::command]
pub fn update_lecturer(db: State<DbState>, session: State<SessionState>, id: i64, lecturer: NewLecturer) -> Result<(), String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute(
        "UPDATE lecturers SET name=?1, email=?2, available_days=?3, max_hours_per_day=?4, max_hours_per_week=?5,
         org_id=?6, preferred_slots_json=?7, blackout_json=?8, max_consecutive_hours=?9 WHERE id=?10",
        params![lecturer.name, lecturer.email, lecturer.available_days,
                lecturer.max_hours_per_day, lecturer.max_hours_per_week, lecturer.org_id,
                lecturer.preferred_slots_json, lecturer.blackout_json, lecturer.max_consecutive_hours, id],
    ).map_err(db_err)?;
    log_audit(&conn, &s, "update", "lecturer", Some(id), Some(&lecturer.name));
    Ok(())
}

#[tauri::command]
pub fn delete_lecturer(db: State<DbState>, session: State<SessionState>, id: i64) -> Result<(), String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute("DELETE FROM lecturers WHERE id=?1", params![id]).map_err(db_err)?;
    log_audit(&conn, &s, "delete", "lecturer", Some(id), None);
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// ROOMS
// ══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub fn get_rooms(db: State<DbState>, session: State<SessionState>) -> Result<Vec<Room>, String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    load_rooms_scoped(&conn, s.org_id_filter())
}

#[tauri::command]
pub fn create_room(db: State<DbState>, session: State<SessionState>, room: NewRoom) -> Result<i64, String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute(
        "INSERT INTO rooms (name, capacity, room_type, available_days, org_id) VALUES (?1,?2,?3,?4,?5)",
        params![room.name, room.capacity, room.room_type, room.available_days, room.org_id],
    ).map_err(db_err)?;
    let id = conn.last_insert_rowid();
    log_audit(&conn, &s, "create", "room", Some(id), Some(&room.name));
    Ok(id)
}

#[tauri::command]
pub fn update_room(db: State<DbState>, session: State<SessionState>, id: i64, room: NewRoom) -> Result<(), String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute(
        "UPDATE rooms SET name=?1, capacity=?2, room_type=?3, available_days=?4, org_id=?5 WHERE id=?6",
        params![room.name, room.capacity, room.room_type, room.available_days, room.org_id, id],
    ).map_err(db_err)?;
    log_audit(&conn, &s, "update", "room", Some(id), Some(&room.name));
    Ok(())
}

#[tauri::command]
pub fn delete_room(db: State<DbState>, session: State<SessionState>, id: i64) -> Result<(), String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute("DELETE FROM rooms WHERE id=?1", params![id]).map_err(db_err)?;
    log_audit(&conn, &s, "delete", "room", Some(id), None);
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// BATCHES
// ══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub fn get_batches(db: State<DbState>, session: State<SessionState>) -> Result<Vec<Batch>, String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    load_batches_scoped(&conn, s.org_id_filter())
}

#[tauri::command]
pub fn create_batch(db: State<DbState>, session: State<SessionState>, batch: NewBatch) -> Result<i64, String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;

    // Enforce plan-based batch limit
    {
        let org_id = batch.org_id.or(s.org_id);
        let plan = get_org_plan(&conn, org_id);
        let limits = PlanLimits::for_plan(&plan);
        if limits.max_batches >= 0 {
            let current: i64 = conn.query_row(
                "SELECT COUNT(*) FROM batches WHERE org_id IS ?1 OR org_id=?1",
                params![org_id], |r| r.get(0),
            ).unwrap_or(0);
            if current >= limits.max_batches {
                return Err(plan_limit_err(PlanLimitError::new(
                    plan, "batches", limits.max_batches, current,
                )));
            }
        }
    }

    conn.execute(
        "INSERT INTO batches (name, department, semester, size, org_id, semester_id) VALUES (?1,?2,?3,?4,?5,?6)",
        params![batch.name, batch.department, batch.semester, batch.size, batch.org_id, batch.semester_id],
    ).map_err(db_err)?;
    let id = conn.last_insert_rowid();
    for cid in &batch.course_ids {
        conn.execute(
            "INSERT OR IGNORE INTO batch_courses (batch_id, course_id) VALUES (?1,?2)",
            params![id, cid],
        ).map_err(db_err)?;
    }
    log_audit(&conn, &s, "create", "batch", Some(id), Some(&batch.name));
    Ok(id)
}

#[tauri::command]
pub fn update_batch(db: State<DbState>, session: State<SessionState>, id: i64, batch: NewBatch) -> Result<(), String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute(
        "UPDATE batches SET name=?1, department=?2, semester=?3, size=?4, org_id=?5, semester_id=?6 WHERE id=?7",
        params![batch.name, batch.department, batch.semester, batch.size, batch.org_id, batch.semester_id, id],
    ).map_err(db_err)?;
    conn.execute("DELETE FROM batch_courses WHERE batch_id=?1", params![id]).map_err(db_err)?;
    for cid in &batch.course_ids {
        conn.execute(
            "INSERT OR IGNORE INTO batch_courses (batch_id, course_id) VALUES (?1,?2)",
            params![id, cid],
        ).map_err(db_err)?;
    }
    log_audit(&conn, &s, "update", "batch", Some(id), Some(&batch.name));
    Ok(())
}

#[tauri::command]
pub fn delete_batch(db: State<DbState>, session: State<SessionState>, id: i64) -> Result<(), String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute("DELETE FROM batches WHERE id=?1", params![id]).map_err(db_err)?;
    log_audit(&conn, &s, "delete", "batch", Some(id), None);
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// SCHEDULER
// ══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub fn generate_schedule(
    db: State<DbState>,
    session: State<SessionState>,
    schedule_name: String,
    semester_id: Option<i64>,
    description: Option<String>,
    algorithm: Option<String>,
) -> Result<Value, String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;

    let org_filter = s.org_id_filter();
    let courses = load_courses_scoped(&conn, org_filter)?;
    let lecturers = load_lecturers_scoped(&conn, org_filter)?;
    let rooms = load_rooms_scoped(&conn, org_filter)?;
    let batches = if let Some(sid) = semester_id {
        load_batches_for_semester(&conn, sid)?
    } else {
        load_batches_scoped(&conn, org_filter)?
    };

    let working_days: Vec<String> = if let Some(org_id) = s.org_id {
        conn.query_row(
            "SELECT working_days FROM org_scheduling_settings WHERE org_id=?1",
            params![org_id],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_else(|_| "Mon,Tue,Wed,Thu,Fri".into())
        .split(',').map(|d| d.trim().to_string()).collect()
    } else {
        vec!["Mon".into(),"Tue".into(),"Wed".into(),"Thu".into(),"Fri".into()]
    };

    let input = SchedulerInput { courses, lecturers, rooms, batches, working_days };
    let use_csp = algorithm.as_deref() == Some("csp");
    // Gate CSP algorithm behind Pro/Institution plan
    if use_csp {
        let plan = get_org_plan(&conn, s.org_id);
        let limits = PlanLimits::for_plan(&plan);
        if !limits.csp_algorithm {
            return Err(plan_limit_err(PlanLimitError::new(plan, "csp_algorithm", 0, 1)));
        }
    }
    let result = if use_csp { scheduler::generate_csp(&input) } else { scheduler::generate(&input) };

    let now = chrono::Local::now().to_rfc3339();
    conn.execute("UPDATE schedules SET is_active=0 WHERE org_id IS ?1 OR org_id=?1", params![s.org_id]).map_err(db_err)?;
    conn.execute(
        "INSERT INTO schedules (name, created_at, is_active, org_id, semester_id, description) VALUES (?1,?2,1,?3,?4,?5)",
        params![schedule_name, now, s.org_id, semester_id, description],
    ).map_err(db_err)?;
    let schedule_id = conn.last_insert_rowid();

    let tuples: Vec<(i64, i64, i64, i64, &str, i64, &str, i64)> = result.entries.iter().map(|e| {
        (e.course_id, e.lecturer_id, e.room_id, e.batch_id, e.day.as_str(), e.time_slot, e.class_type.as_str(), e.week_parity)
    }).collect();
    crate::db::replace_schedule_entries(&conn, schedule_id, &tuples).map_err(db_err)?;
    log_audit(&conn, &s, "generate", "schedule", Some(schedule_id), Some(&schedule_name));

    Ok(serde_json::json!({
        "schedule_id": schedule_id,
        "entry_count": result.entries.len(),
        "unscheduled": result.unscheduled,
    }))
}

#[tauri::command]
pub fn get_schedules(db: State<DbState>, session: State<SessionState>) -> Result<Vec<Schedule>, String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;

    let sql = if s.role == "super_admin" {
        "SELECT sch.id, sch.name, sch.created_at, sch.is_active, sch.status,
                (SELECT COUNT(*) FROM schedule_entries WHERE schedule_id=sch.id),
                sch.semester_id, sem.name, sch.description
         FROM schedules sch LEFT JOIN semesters sem ON sem.id=sch.semester_id
         ORDER BY sch.id DESC".to_string()
    } else {
        format!(
            "SELECT sch.id, sch.name, sch.created_at, sch.is_active, sch.status,
                    (SELECT COUNT(*) FROM schedule_entries WHERE schedule_id=sch.id),
                    sch.semester_id, sem.name, sch.description
             FROM schedules sch LEFT JOIN semesters sem ON sem.id=sch.semester_id
             WHERE sch.org_id IS {} OR sch.org_id={}
             ORDER BY sch.id DESC",
            s.org_id.map_or("NULL".to_string(), |x| x.to_string()),
            s.org_id.map_or("NULL".to_string(), |x| x.to_string()),
        )
    };

    let mut stmt = conn.prepare(&sql).map_err(db_err)?;
    let rows: Result<Vec<Schedule>, _> = stmt.query_map([], |row| Ok(Schedule {
        id: row.get(0)?,
        name: row.get(1)?,
        created_at: row.get(2)?,
        is_active: row.get::<_,i64>(3)? != 0,
        status: row.get::<_, Option<String>>(4)?.unwrap_or_else(|| "draft".into()),
        entry_count: row.get(5)?,
        semester_id: row.get(6)?,
        semester_name: row.get(7)?,
        description: row.get(8)?,
    })).map_err(db_err)?.collect();
    rows.map_err(db_err)
}

#[tauri::command]
pub fn get_schedule_entries(
    db: State<DbState>,
    session: State<SessionState>,
    schedule_id: i64,
) -> Result<Vec<ScheduleEntry>, String> {
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    let mut stmt = conn.prepare(
        "SELECT se.id, se.schedule_id,
                se.course_id,   c.code, c.name,  c.class_type, c.frequency,
                se.week_parity,
                se.lecturer_id, l.name,
                se.room_id,     r.name,
                se.batch_id,    b.name, b.department,
                se.day, se.time_slot
         FROM schedule_entries se
         JOIN courses   c ON c.id = se.course_id
         JOIN lecturers l ON l.id = se.lecturer_id
         JOIN rooms     r ON r.id = se.room_id
         JOIN batches   b ON b.id = se.batch_id
         WHERE se.schedule_id=?1 ORDER BY se.day, se.time_slot",
    ).map_err(db_err)?;
    let rows: Result<Vec<ScheduleEntry>, _> = stmt.query_map(params![schedule_id], |row| Ok(ScheduleEntry {
        id: row.get(0)?,
        schedule_id: row.get(1)?,
        course_id: row.get(2)?,
        course_code: row.get(3)?,
        course_name: row.get(4)?,
        class_type: row.get(5)?,
        frequency: row.get(6)?,
        week_parity: row.get(7)?,
        lecturer_id: row.get(8)?,
        lecturer_name: row.get(9)?,
        room_id: row.get(10)?,
        room_name: row.get(11)?,
        batch_id: row.get(12)?,
        batch_name: row.get(13)?,
        department: row.get(14)?,
        day: row.get(15)?,
        time_slot: row.get(16)?,
    })).map_err(db_err)?.collect();
    rows.map_err(db_err)
}

#[tauri::command]
pub fn activate_schedule(db: State<DbState>, session: State<SessionState>, id: i64) -> Result<(), String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute("UPDATE schedules SET is_active=0, status='draft' WHERE org_id IS ?1 OR org_id=?1", params![s.org_id]).map_err(db_err)?;
    conn.execute("UPDATE schedules SET is_active=1, status='published' WHERE id=?1", params![id]).map_err(db_err)?;
    log_audit(&conn, &s, "publish", "schedule", Some(id), None);
    Ok(())
}

#[tauri::command]
pub fn publish_schedule(db: State<DbState>, session: State<SessionState>, id: i64) -> Result<(), String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute("UPDATE schedules SET is_active=0, status='draft' WHERE org_id IS ?1 OR org_id=?1", params![s.org_id]).map_err(db_err)?;
    conn.execute("UPDATE schedules SET is_active=1, status='published' WHERE id=?1", params![id]).map_err(db_err)?;
    log_audit(&conn, &s, "publish", "schedule", Some(id), None);
    Ok(())
}

#[tauri::command]
pub fn revert_schedule_to_draft(db: State<DbState>, session: State<SessionState>, id: i64) -> Result<(), String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute("UPDATE schedules SET is_active=0, status='draft' WHERE id=?1", params![id]).map_err(db_err)?;
    log_audit(&conn, &s, "revert", "schedule", Some(id), None);
    Ok(())
}

#[tauri::command]
pub fn delete_schedule(db: State<DbState>, session: State<SessionState>, id: i64) -> Result<(), String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute("DELETE FROM schedules WHERE id=?1", params![id]).map_err(db_err)?;
    log_audit(&conn, &s, "delete", "schedule", Some(id), None);
    Ok(())
}

#[tauri::command]
pub fn export_schedule_csv(
    db: State<DbState>,
    session: State<SessionState>,
    schedule_id: i64,
) -> Result<String, String> {
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    let mut stmt = conn.prepare(
        "SELECT b.name, b.department, c.code, c.name, c.class_type, l.name, r.name, se.day, se.time_slot, se.week_parity
         FROM schedule_entries se
         JOIN courses c ON c.id=se.course_id JOIN lecturers l ON l.id=se.lecturer_id
         JOIN rooms r ON r.id=se.room_id     JOIN batches b ON b.id=se.batch_id
         WHERE se.schedule_id=?1 ORDER BY b.department, b.name, se.day, se.time_slot",
    ).map_err(db_err)?;

    let labels = ["08:00","09:00","10:00","11:00","13:00","14:00","15:00","16:00"];
    let labels_end = ["09:00","10:00","11:00","12:00","14:00","15:00","16:00","17:00"];
    let mut csv = "Batch,Department,Course Code,Course Name,Type,Lecturer,Room,Day,Time,Frequency\n".to_string();

    stmt.query_map(params![schedule_id], |row| {
        let slot: i64 = row.get(8)?;
        let parity: i64 = row.get(9)?;
        let time = format!("{}-{}", labels.get(slot as usize).copied().unwrap_or("?"), labels_end.get(slot as usize).copied().unwrap_or("?"));
        let freq = if parity == 0 { "weekly" } else { "biweekly" };
        Ok(format!("{},{},{},{},{},{},{},{},{},{}\n",
            row.get::<_,String>(0)?, row.get::<_,String>(1)?,
            row.get::<_,String>(2)?, row.get::<_,String>(3)?,
            row.get::<_,String>(4)?, row.get::<_,String>(5)?,
            row.get::<_,String>(6)?, row.get::<_,String>(7)?,
            time, freq))
    }).map_err(db_err)?.filter_map(|r| r.ok()).for_each(|l| csv.push_str(&l));

    Ok(csv)
}

// ── Stats ──────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_stats(db: State<DbState>, session: State<SessionState>) -> Result<Value, String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    let scope = s.org_id_filter();

    let courses: i64   = count_scoped(&conn, "courses", scope);
    let lecturers: i64 = count_scoped(&conn, "lecturers", scope);
    let rooms: i64     = count_scoped(&conn, "rooms", scope);
    let batches: i64   = count_scoped(&conn, "batches", scope);
    let schedules: i64 = count_scoped(&conn, "schedules", scope);
    let orgs: i64      = conn.query_row("SELECT COUNT(*) FROM organizations", [], |r| r.get(0)).unwrap_or(0);
    let sems: i64      = count_scoped(&conn, "semesters", scope);
    let active_entries: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schedule_entries se JOIN schedules s ON s.id=se.schedule_id WHERE s.is_active=1",
        [], |r| r.get(0)
    ).unwrap_or(0);

    Ok(serde_json::json!({
        "courses": courses, "lecturers": lecturers, "rooms": rooms,
        "batches": batches, "schedules": schedules, "active_entries": active_entries,
        "organizations": orgs, "semesters": sems,
    }))
}

// ══════════════════════════════════════════════════════════════════════════════
// SETTINGS
// ══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub fn update_display_name(
    db: State<DbState>,
    session: State<SessionState>,
    new_name: String,
) -> Result<(), String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute("UPDATE users SET display_name=?1 WHERE id=?2", params![new_name, s.user_id]).map_err(db_err)?;
    // Update session state too
    let mut sess = session.0.lock().map_err(db_err)?;
    if let Some(ref mut payload) = *sess {
        payload.display_name = new_name;
    }
    Ok(())
}

#[tauri::command]
pub fn admin_reset_password(
    db: State<DbState>,
    session: State<SessionState>,
    user_id: i64,
    new_password: String,
) -> Result<(), String> {
    require_super_admin(&session)?;
    let hash = bcrypt::hash(&new_password, bcrypt::DEFAULT_COST).map_err(db_err)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute("UPDATE users SET password_hash=?1 WHERE id=?2", params![hash, user_id]).map_err(db_err)?;
    Ok(())
}

#[tauri::command]
pub fn set_user_active(
    db: State<DbState>,
    session: State<SessionState>,
    user_id: i64,
    active: bool,
) -> Result<(), String> {
    let s = require_super_admin(&session)?;
    if s.user_id == user_id { return Err("Cannot deactivate yourself".into()); }
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute("UPDATE users SET is_active=?1 WHERE id=?2", params![active as i64, user_id]).map_err(db_err)?;
    Ok(())
}

#[tauri::command]
pub fn get_scheduling_settings(
    db: State<DbState>,
    session: State<SessionState>,
    org_id: i64,
) -> Result<OrgSchedulingSettings, String> {
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    let result = conn.query_row(
        "SELECT org_id, working_days, day_start_slot, day_end_slot, slot_duration
         FROM org_scheduling_settings WHERE org_id=?1",
        params![org_id],
        |row| Ok(OrgSchedulingSettings {
            org_id: row.get(0)?,
            working_days: row.get(1)?,
            day_start_slot: row.get(2)?,
            day_end_slot: row.get(3)?,
            slot_duration: row.get(4)?,
        }),
    );
    match result {
        Ok(s) => Ok(s),
        Err(_) => Ok(OrgSchedulingSettings {
            org_id,
            working_days: "Mon,Tue,Wed,Thu,Fri".into(),
            day_start_slot: 0,
            day_end_slot: 7,
            slot_duration: 60,
        }),
    }
}

#[tauri::command]
pub fn upsert_scheduling_settings(
    db: State<DbState>,
    session: State<SessionState>,
    settings: OrgSchedulingSettings,
) -> Result<(), String> {
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute(
        "INSERT INTO org_scheduling_settings (org_id, working_days, day_start_slot, day_end_slot, slot_duration, updated_at)
         VALUES (?1,?2,?3,?4,?5,datetime('now'))
         ON CONFLICT(org_id) DO UPDATE SET
           working_days=excluded.working_days,
           day_start_slot=excluded.day_start_slot,
           day_end_slot=excluded.day_end_slot,
           slot_duration=excluded.slot_duration,
           updated_at=datetime('now')",
        params![settings.org_id, settings.working_days, settings.day_start_slot, settings.day_end_slot, settings.slot_duration],
    ).map_err(db_err)?;
    Ok(())
}

#[tauri::command]
pub fn clear_schedules(
    db: State<DbState>,
    session: State<SessionState>,
) -> Result<i64, String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    let count: i64 = match s.org_id {
        Some(oid) => conn.query_row("SELECT COUNT(*) FROM schedules WHERE org_id=?1", params![oid], |r| r.get(0)).unwrap_or(0),
        None => conn.query_row("SELECT COUNT(*) FROM schedules", [], |r| r.get(0)).unwrap_or(0),
    };
    match s.org_id {
        Some(oid) => conn.execute("DELETE FROM schedules WHERE org_id=?1", params![oid]).map_err(db_err)?,
        None => conn.execute("DELETE FROM schedules", []).map_err(db_err)?,
    };
    Ok(count)
}

#[tauri::command]
pub fn backup_database(
    db: State<DbState>,
    session: State<SessionState>,
) -> Result<String, String> {
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    export_json_backup(&conn)
}

fn export_json_backup(conn: &Connection) -> Result<String, String> {
    use serde_json::{json, Map, Value};
    let tables = ["organizations","users","semesters","courses","lecturers","rooms",
                  "batches","batch_courses","schedules","schedule_entries","org_scheduling_settings"];
    let mut result = Map::new();
    for table in &tables {
        let mut stmt = conn.prepare(&format!("SELECT * FROM {}", table)).map_err(db_err)?;
        let cols: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
        let mut rows: Vec<Value> = Vec::new();
        let _ = stmt.query_map([], |row| {
            let mut obj = Map::new();
            for (i, col) in cols.iter().enumerate() {
                let v: Value = match row.get_ref(i) {
                    Ok(rusqlite::types::ValueRef::Integer(n)) => json!(n),
                    Ok(rusqlite::types::ValueRef::Real(f)) => json!(f),
                    Ok(rusqlite::types::ValueRef::Text(t)) => json!(std::str::from_utf8(t).unwrap_or("")),
                    _ => Value::Null,
                };
                obj.insert(col.clone(), v);
            }
            Ok(obj)
        }).map_err(db_err)?.filter_map(|r| r.ok()).for_each(|row| rows.push(Value::Object(row)));
        result.insert(table.to_string(), Value::Array(rows));
    }
    let json_str = serde_json::to_string_pretty(&Value::Object(result)).map_err(db_err)?;
    use base64::{Engine as _, engine::general_purpose};
    Ok(general_purpose::STANDARD.encode(json_str.as_bytes()))
}

#[tauri::command]
pub fn get_app_info(
    db: State<DbState>,
    session: State<SessionState>,
) -> Result<AppInfo, String> {
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    let user_count: i64     = conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0)).unwrap_or(0);
    let org_count: i64      = conn.query_row("SELECT COUNT(*) FROM organizations", [], |r| r.get(0)).unwrap_or(0);
    let schedule_count: i64 = conn.query_row("SELECT COUNT(*) FROM schedules", [], |r| r.get(0)).unwrap_or(0);
    let page_count: i64 = conn.query_row("PRAGMA page_count", [], |r| r.get(0)).unwrap_or(0);
    let page_size: i64  = conn.query_row("PRAGMA page_size",  [], |r| r.get(0)).unwrap_or(4096);
    let db_size: u64 = (page_count * page_size) as u64;
    Ok(AppInfo {
        version: "0.1.0".into(),
        db_size_bytes: db_size,
        user_count,
        org_count,
        schedule_count,
    })
}

// ── Admin quota ────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_max_admins(db: State<DbState>, session: State<SessionState>) -> Result<i64, String> {
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    let max: i64 = conn.query_row(
        "SELECT CAST(value AS INTEGER) FROM app_settings WHERE key='max_admins'",
        [], |r| r.get(0),
    ).unwrap_or(2);
    Ok(max)
}

#[tauri::command]
pub fn set_max_admins(
    db: State<DbState>,
    session: State<SessionState>,
    max: i64,
) -> Result<(), String> {
    require_super_admin(&session)?;
    if max < 1  { return Err("Max admins must be at least 1.".into()); }
    if max > 50 { return Err("Max admins cannot exceed 50.".into()); }
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('max_admins', ?1)",
        params![max.to_string()],
    ).map_err(db_err)?;
    Ok(())
}

#[tauri::command]
pub fn get_admin_count(db: State<DbState>, session: State<SessionState>) -> Result<i64, String> {
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM users WHERE role='admin' AND is_active=1",
        [], |r| r.get(0),
    ).unwrap_or(0);
    Ok(count)
}

// ── Utilization report ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_utilization_report(
    db: State<DbState>,
    session: State<SessionState>,
    schedule_id: i64,
) -> Result<UtilizationReport, String> {
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;

    let schedule_name: String = conn.query_row(
        "SELECT name FROM schedules WHERE id=?1", params![schedule_id], |r| r.get(0)
    ).map_err(|_| "Schedule not found".to_string())?;

    let total_entries: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schedule_entries WHERE schedule_id=?1", params![schedule_id], |r| r.get(0)
    ).unwrap_or(0);

    // Room utilization
    let mut stmt = conn.prepare(
        "SELECT r.id, r.name, r.room_type, r.capacity, r.available_days,
                COUNT(se.id) as booked
         FROM rooms r
         LEFT JOIN schedule_entries se ON se.room_id=r.id AND se.schedule_id=?1
         GROUP BY r.id ORDER BY booked DESC"
    ).map_err(db_err)?;
    let rooms: Vec<RoomUtilization> = stmt.query_map(params![schedule_id], |row| {
        let avail: String = row.get(4)?;
        let days = avail.split(',').count() as i64;
        let total = days * 8;
        let booked: i64 = row.get(5)?;
        let pct = if total > 0 { (booked as f64 / total as f64) * 100.0 } else { 0.0 };
        Ok(RoomUtilization {
            room_id: row.get(0)?,
            room_name: row.get(1)?,
            room_type: row.get(2)?,
            capacity: row.get(3)?,
            booked_slots: booked,
            total_available_slots: total,
            utilization_pct: (pct * 10.0).round() / 10.0,
        })
    }).map_err(db_err)?.filter_map(|r| r.ok()).collect();

    // Lecturer load
    let mut stmt2 = conn.prepare(
        "SELECT l.id, l.name, l.max_hours_per_week,
                COUNT(se.id) as scheduled
         FROM lecturers l
         LEFT JOIN schedule_entries se ON se.lecturer_id=l.id AND se.schedule_id=?1
         GROUP BY l.id ORDER BY scheduled DESC"
    ).map_err(db_err)?;
    let lecturer_loads: Vec<LecturerLoad> = stmt2.query_map(params![schedule_id], |row| {
        let max_h: i64 = row.get(2)?;
        let sched: i64 = row.get(3)?;
        let pct = if max_h > 0 { (sched as f64 / max_h as f64) * 100.0 } else { 0.0 };
        Ok(LecturerLoad {
            lecturer_id: row.get(0)?,
            lecturer_name: row.get(1)?,
            scheduled_hours: sched,
            max_hours_per_week: max_h,
            load_pct: (pct * 10.0).round() / 10.0,
        })
    }).map_err(db_err)?.filter_map(|r| r.ok()).collect();

    Ok(UtilizationReport { schedule_id, schedule_name, rooms, lecturer_loads, total_entries })
}

// ── Manual schedule entry edit ─────────────────────────────────────────────────

#[tauri::command]
pub fn update_schedule_entry(
    db: State<DbState>,
    session: State<SessionState>,
    entry_id: i64,
    req: UpdateScheduleEntryReq,
) -> Result<(), String> {
    require_session(&session)?;
    let valid_days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    if !valid_days.contains(&req.day.as_str()) {
        return Err(format!("Invalid day: {}", req.day));
    }
    if req.time_slot < 0 || req.time_slot > 7 {
        return Err(format!("Invalid time slot: {}", req.time_slot));
    }
    let conn = db.0.lock().map_err(db_err)?;

    // Fetch current entry info
    let (schedule_id, lecturer_id, batch_id, course_room_type): (i64, i64, i64, String) = conn.query_row(
        "SELECT se.schedule_id, se.lecturer_id, se.batch_id, c.room_type
         FROM schedule_entries se JOIN courses c ON c.id=se.course_id WHERE se.id=?1",
        params![entry_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    ).map_err(|_| "Entry not found".to_string())?;

    // Validate room type matches course requirement
    let new_room_type: String = conn.query_row(
        "SELECT room_type FROM rooms WHERE id=?1", params![req.room_id],
        |r| r.get(0),
    ).map_err(|_| "Room not found".to_string())?;
    if new_room_type != course_room_type {
        return Err(format!(
            "Room type mismatch: course needs '{}' room, selected room is '{}'",
            course_room_type, new_room_type
        ));
    }

    let ds_day = &req.day;
    let ds_slot = req.time_slot;

    // Check for conflicts in the same schedule (excluding this entry)
    let conflict: i64 = conn.query_row(
        "SELECT COUNT(*) FROM schedule_entries
         WHERE schedule_id=?1 AND id!=?2 AND day=?3 AND time_slot=?4
           AND (lecturer_id=?5 OR batch_id=?6 OR room_id=?7)",
        params![schedule_id, entry_id, ds_day, ds_slot, lecturer_id, batch_id, req.room_id],
        |r| r.get(0),
    ).unwrap_or(0);
    if conflict > 0 {
        return Err(format!(
            "Conflict: another entry at {} slot {} uses the same lecturer, batch, or room",
            req.day, req.time_slot
        ));
    }

    conn.execute(
        "UPDATE schedule_entries SET day=?1, time_slot=?2, room_id=?3 WHERE id=?4",
        params![req.day, req.time_slot, req.room_id, entry_id],
    ).map_err(db_err)?;
    Ok(())
}

// ── Audit log ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_audit_log(
    db: State<DbState>,
    session: State<SessionState>,
    limit: i64,
) -> Result<Vec<AuditEntry>, String> {
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    let mut stmt = conn.prepare(
        "SELECT id, user_id, username, action, entity_type, entity_id, details_json, created_at
         FROM audit_log ORDER BY id DESC LIMIT ?1"
    ).map_err(db_err)?;
    let rows: Result<Vec<AuditEntry>, _> = stmt.query_map(params![limit], |row| Ok(AuditEntry {
        id: row.get(0)?,
        user_id: row.get(1)?,
        username: row.get(2)?,
        action: row.get(3)?,
        entity_type: row.get(4)?,
        entity_id: row.get(5)?,
        details_json: row.get(6)?,
        created_at: row.get(7)?,
    })).map_err(db_err)?.collect();
    rows.map_err(db_err)
}

// ── Plan / Subscription ────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_plan(
    db: State<DbState>,
    session: State<SessionState>,
) -> Result<PlanInfo, String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    let plan = get_org_plan(&conn, s.org_id);
    let limits = PlanLimits::for_plan(&plan);
    Ok(PlanInfo { plan, limits })
}

/// Standalone mode: license info always returns "none" — licensing is hub-only.
#[tauri::command]
pub fn get_license() -> Result<LicenseInfo, String> {
    Ok(LicenseInfo {
        active:            false,
        plan:              PLAN_FREE.to_string(),
        org_name:          None,
        expires_at:        None,
        days_until_expiry: None,
        status:            "none".into(),
        last_validated_at: None,
    })
}

/// Standalone mode: license activation is handled in the hub server, not the desktop app.
#[tauri::command]
pub fn activate_license(_token: String) -> Result<LicenseInfo, String> {
    Err("License activation is only available in hub server mode. Connect to a hub and use the hub admin panel.".into())
}

/// Standalone mode: no-op.
#[tauri::command]
pub fn deactivate_license() -> Result<(), String> {
    Ok(())
}

// ── Bulk CSV import ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn bulk_import_lecturers(
    db: State<DbState>,
    session: State<SessionState>,
    rows: Vec<CsvLecturer>,
) -> Result<BulkImportResult, String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    {
        let plan = get_org_plan(&conn, s.org_id);
        let limits = PlanLimits::for_plan(&plan);
        if !limits.bulk_import {
            return Err(plan_limit_err(PlanLimitError::new(plan, "bulk_import", 0, 1)));
        }
    }
    let mut inserted = 0i64; let mut skipped = 0i64; let mut errors: Vec<String> = vec![];
    for r in &rows {
        if r.name.trim().is_empty() { errors.push("Row skipped: name is empty".into()); skipped += 1; continue; }
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM lecturers WHERE name=?1 AND (org_id=?2 OR org_id IS NULL)",
            params![r.name.trim(), s.org_id], |row| row.get(0),
        ).unwrap_or(0);
        if exists > 0 { skipped += 1; continue; }
        match conn.execute(
            "INSERT INTO lecturers (name, email, available_days, max_hours_per_day, max_hours_per_week, org_id)
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![r.name.trim(), r.email, r.available_days, r.max_hours_per_day, r.max_hours_per_week, s.org_id],
        ) {
            Ok(_) => { inserted += 1; }
            Err(e) => { errors.push(format!("{}: {}", r.name, e)); }
        }
    }
    if inserted > 0 { log_audit(&conn, &s, "import", "lecturer", None, Some(&format!(r#"{{"count":{}}}"#, inserted))); }
    Ok(BulkImportResult { inserted, skipped, errors })
}

#[tauri::command]
pub fn bulk_import_rooms(
    db: State<DbState>,
    session: State<SessionState>,
    rows: Vec<CsvRoom>,
) -> Result<BulkImportResult, String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    {
        let plan = get_org_plan(&conn, s.org_id);
        let limits = PlanLimits::for_plan(&plan);
        if !limits.bulk_import {
            return Err(plan_limit_err(PlanLimitError::new(plan, "bulk_import", 0, 1)));
        }
    }
    let mut inserted = 0i64; let mut skipped = 0i64; let mut errors: Vec<String> = vec![];
    for r in &rows {
        if r.name.trim().is_empty() { errors.push("Row skipped: name is empty".into()); skipped += 1; continue; }
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM rooms WHERE name=?1 AND (org_id=?2 OR org_id IS NULL)",
            params![r.name.trim(), s.org_id], |row| row.get(0),
        ).unwrap_or(0);
        if exists > 0 { skipped += 1; continue; }
        match conn.execute(
            "INSERT INTO rooms (name, capacity, room_type, available_days, org_id) VALUES (?1,?2,?3,?4,?5)",
            params![r.name.trim(), r.capacity, r.room_type, r.available_days, s.org_id],
        ) {
            Ok(_) => { inserted += 1; }
            Err(e) => { errors.push(format!("{}: {}", r.name, e)); }
        }
    }
    if inserted > 0 { log_audit(&conn, &s, "import", "room", None, Some(&format!(r#"{{"count":{}}}"#, inserted))); }
    Ok(BulkImportResult { inserted, skipped, errors })
}

#[tauri::command]
pub fn bulk_import_courses(
    db: State<DbState>,
    session: State<SessionState>,
    rows: Vec<CsvCourse>,
) -> Result<BulkImportResult, String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    {
        let plan = get_org_plan(&conn, s.org_id);
        let limits = PlanLimits::for_plan(&plan);
        if !limits.bulk_import {
            return Err(plan_limit_err(PlanLimitError::new(plan, "bulk_import", 0, 1)));
        }
    }
    let mut inserted = 0i64; let mut skipped = 0i64; let mut errors: Vec<String> = vec![];
    for r in &rows {
        if r.code.trim().is_empty() { errors.push("Row skipped: code is empty".into()); skipped += 1; continue; }
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM courses WHERE code=?1 AND (org_id=?2 OR org_id IS NULL)",
            params![r.code.trim(), s.org_id], |row| row.get(0),
        ).unwrap_or(0);
        if exists > 0 { skipped += 1; continue; }
        // Resolve lecturer email to id
        let lecturer_id: Option<i64> = r.lecturer_email.as_deref().and_then(|email| {
            conn.query_row(
                "SELECT id FROM lecturers WHERE email=?1 AND (org_id=?2 OR org_id IS NULL)",
                params![email, s.org_id], |row| row.get(0),
            ).ok()
        });
        if r.lecturer_email.is_some() && lecturer_id.is_none() {
            errors.push(format!("{}: lecturer email '{}' not found — imported without lecturer",
                r.code, r.lecturer_email.as_deref().unwrap_or("")));
        }
        match conn.execute(
            "INSERT INTO courses (code, name, hours_per_week, room_type, class_type, frequency, lecturer_id, org_id)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![r.code.trim(), r.name, r.hours_per_week, r.room_type, r.class_type, r.frequency, lecturer_id, s.org_id],
        ) {
            Ok(_) => { inserted += 1; }
            Err(e) => { errors.push(format!("{}: {}", r.code, e)); }
        }
    }
    if inserted > 0 { log_audit(&conn, &s, "import", "course", None, Some(&format!(r#"{{"count":{}}}"#, inserted))); }
    Ok(BulkImportResult { inserted, skipped, errors })
}

// Fire-and-forget audit insert — never panics, never blocks the caller
fn log_audit(conn: &Connection, sess: &SessionPayload, action: &str, entity_type: &str, entity_id: Option<i64>, details: Option<&str>) {
    let _ = conn.execute(
        "INSERT INTO audit_log (user_id, username, action, entity_type, entity_id, details_json)
         VALUES (?1,?2,?3,?4,?5,?6)",
        params![sess.user_id, sess.username, action, entity_type, entity_id, details],
    );
}

// ══════════════════════════════════════════════════════════════════════════════
// PRE-FLIGHT / DATA HEALTH
// ══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub fn get_preflight_warnings(
    db: State<DbState>,
    session: State<SessionState>,
) -> Result<Vec<PreflightWarning>, String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    let org = s.org_id_filter();
    let org_clause = match org {
        Some(id) => format!("AND org_id={}", id),
        None => String::new(),
    };

    let mut warnings = Vec::new();

    // Courses without a lecturer
    let no_lec: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM courses WHERE lecturer_id IS NULL {}", org_clause),
        [], |r| r.get(0),
    ).unwrap_or(0);
    if no_lec > 0 {
        warnings.push(PreflightWarning {
            severity: "warning".into(),
            category: "courses".into(),
            message: format!("{} course(s) have no lecturer assigned", no_lec),
        });
    }

    // Batches with no courses
    let no_courses_clause = match org {
        Some(id) => format!("WHERE b.org_id={} AND bc.batch_id IS NULL", id),
        None => "WHERE bc.batch_id IS NULL".into(),
    };
    let no_courses: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM batches b LEFT JOIN batch_courses bc ON b.id=bc.batch_id {}", no_courses_clause),
        [], |r| r.get(0),
    ).unwrap_or(0);
    if no_courses > 0 {
        warnings.push(PreflightWarning {
            severity: "error".into(),
            category: "batches".into(),
            message: format!("{} batch(es) have no courses enrolled — they will be skipped", no_courses),
        });
    }

    // Lecturers with no available days
    let no_days: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM lecturers WHERE (available_days='' OR available_days IS NULL) {}", org_clause),
        [], |r| r.get(0),
    ).unwrap_or(0);
    if no_days > 0 {
        warnings.push(PreflightWarning {
            severity: "error".into(),
            category: "lecturers".into(),
            message: format!("{} lecturer(s) have no available days set — their courses cannot be scheduled", no_days),
        });
    }

    // Courses with lab room type but no lab rooms exist
    let lab_courses: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM courses WHERE room_type='lab' {}", org_clause),
        [], |r| r.get(0),
    ).unwrap_or(0);
    if lab_courses > 0 {
        let lab_rooms: i64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM rooms WHERE room_type='lab' {}", org_clause),
            [], |r| r.get(0),
        ).unwrap_or(0);
        if lab_rooms == 0 {
            warnings.push(PreflightWarning {
                severity: "error".into(),
                category: "rooms".into(),
                message: format!("{} lab course(s) exist but no lab rooms are configured", lab_courses),
            });
        }
    }

    // No rooms at all
    let total_rooms: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM rooms WHERE 1=1 {}", org_clause),
        [], |r| r.get(0),
    ).unwrap_or(0);
    if total_rooms == 0 {
        warnings.push(PreflightWarning {
            severity: "error".into(),
            category: "rooms".into(),
            message: "No rooms configured — schedule cannot be generated".into(),
        });
    }

    // No batches at all
    let total_batches: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM batches WHERE 1=1 {}", org_clause),
        [], |r| r.get(0),
    ).unwrap_or(0);
    if total_batches == 0 {
        warnings.push(PreflightWarning {
            severity: "error".into(),
            category: "batches".into(),
            message: "No batches configured — schedule cannot be generated".into(),
        });
    }

    Ok(warnings)
}

#[tauri::command]
pub fn get_data_health(
    db: State<DbState>,
    session: State<SessionState>,
) -> Result<DataHealth, String> {
    let s = require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    let org = s.org_id_filter();
    let org_clause = match org {
        Some(id) => format!("AND org_id={}", id),
        None => String::new(),
    };

    let courses_without_lecturers: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM courses WHERE lecturer_id IS NULL {}", org_clause),
        [], |r| r.get(0),
    ).unwrap_or(0);

    let lab_courses: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM courses WHERE room_type='lab' {}", org_clause),
        [], |r| r.get(0),
    ).unwrap_or(0);
    let lab_rooms: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM rooms WHERE room_type='lab' {}", org_clause),
        [], |r| r.get(0),
    ).unwrap_or(0);
    let courses_without_matching_rooms = if lab_courses > 0 && lab_rooms == 0 { lab_courses } else { 0 };

    let no_courses_clause = match org {
        Some(id) => format!("WHERE b.org_id={} AND bc.batch_id IS NULL", id),
        None => "WHERE bc.batch_id IS NULL".into(),
    };
    let batches_without_courses: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM batches b LEFT JOIN batch_courses bc ON b.id=bc.batch_id {}", no_courses_clause),
        [], |r| r.get(0),
    ).unwrap_or(0);

    let lecturers_unavailable: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM lecturers WHERE (available_days='' OR available_days IS NULL) {}", org_clause),
        [], |r| r.get(0),
    ).unwrap_or(0);

    let total_warnings = courses_without_lecturers + courses_without_matching_rooms
        + batches_without_courses + lecturers_unavailable;

    Ok(DataHealth {
        courses_without_lecturers,
        courses_without_matching_rooms,
        batches_without_courses,
        lecturers_unavailable,
        total_warnings,
    })
}

#[tauri::command]
pub fn update_schedule_description(
    db: State<DbState>,
    session: State<SessionState>,
    id: i64,
    description: Option<String>,
) -> Result<(), String> {
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute("UPDATE schedules SET description=?1 WHERE id=?2", params![description, id])
        .map_err(db_err)?;
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// PASSWORD RECOVERY
// ══════════════════════════════════════════════════════════════════════════════

fn generate_recovery_code() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

#[tauri::command]
pub fn setup_recovery(
    db: State<DbState>,
    session: State<SessionState>,
    req: SetupRecoveryRequest,
) -> Result<RecoverySetup, String> {
    let s = require_super_admin(&session)?;
    let conn = db.0.lock().map_err(db_err)?;

    let recovery_code = generate_recovery_code();
    let code_hash = bcrypt::hash(&recovery_code, bcrypt::DEFAULT_COST).map_err(db_err)?;
    let answer_hash = bcrypt::hash(req.security_answer.trim().to_lowercase(), bcrypt::DEFAULT_COST).map_err(db_err)?;

    conn.execute(
        "UPDATE users SET recovery_code_hash=?1, security_question=?2, security_answer_hash=?3 WHERE id=?4",
        params![code_hash, req.security_question, answer_hash, s.user_id],
    ).map_err(db_err)?;

    log_audit(&conn, &s, "setup", "recovery", Some(s.user_id), Some("recovery code + security question configured"));

    Ok(RecoverySetup {
        recovery_code: recovery_code.clone(),
    })
}

#[tauri::command]
pub fn reset_password_with_recovery_code(
    db: State<DbState>,
    req: ResetPasswordWithCodeRequest,
) -> Result<(), String> {
    if req.new_password.len() < 6 {
        return Err("Password must be at least 6 characters".into());
    }

    let conn = db.0.lock().map_err(db_err)?;

    // Find super-admin user
    let user_result: Result<(i64, Option<String>), _> = conn.query_row(
        "SELECT id, recovery_code_hash FROM users WHERE role='super_admin'",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    );

    let (user_id, code_hash) = match user_result {
        Ok((id, Some(hash))) => (id, hash),
        _ => return Err("Recovery code is not set up".into()),
    };

    // Verify recovery code
    if !bcrypt::verify(&req.recovery_code, &code_hash).unwrap_or(false) {
        return Err("Invalid recovery code".into());
    }

    let new_hash = bcrypt::hash(&req.new_password, bcrypt::DEFAULT_COST).map_err(db_err)?;
    conn.execute("UPDATE users SET password_hash=?1 WHERE id=?2", params![new_hash, user_id])
        .map_err(db_err)?;

    Ok(())
}

#[tauri::command]
pub fn reset_password_with_security_answer(
    db: State<DbState>,
    req: ResetPasswordWithAnswerRequest,
) -> Result<(), String> {
    if req.new_password.len() < 6 {
        return Err("Password must be at least 6 characters".into());
    }

    let conn = db.0.lock().map_err(db_err)?;

    // Find super-admin user
    let user_result: Result<(i64, Option<String>), _> = conn.query_row(
        "SELECT id, security_answer_hash FROM users WHERE role='super_admin'",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    );

    let (user_id, answer_hash) = match user_result {
        Ok((id, Some(hash))) => (id, hash),
        _ => return Err("Security answer is not set up".into()),
    };

    // Verify answer (case-insensitive)
    let answer_lower = req.security_answer.trim().to_lowercase();
    if !bcrypt::verify(&answer_lower, &answer_hash).unwrap_or(false) {
        return Err("Incorrect answer".into());
    }

    let new_hash = bcrypt::hash(&req.new_password, bcrypt::DEFAULT_COST).map_err(db_err)?;
    conn.execute("UPDATE users SET password_hash=?1 WHERE id=?2", params![new_hash, user_id])
        .map_err(db_err)?;

    Ok(())
}

#[tauri::command]
pub fn get_security_question(db: State<DbState>) -> Result<String, String> {
    let conn = db.0.lock().map_err(db_err)?;
    conn.query_row(
        "SELECT security_question FROM users WHERE role='super_admin'",
        [],
        |r| r.get(0),
    ).map_err(|_| "Security question not set up".into())
}

// ══════════════════════════════════════════════════════════════════════════════
// APPROVAL REQUESTS
// ══════════════════════════════════════════════════════════════════════════════

/// Create a password-reset or account-unlock request. Callable without login.
#[tauri::command]
pub fn create_approval_request(
    db: State<DbState>,
    req: CreateApprovalReq,
) -> Result<i64, String> {
    let conn = db.0.lock().map_err(db_err)?;
    let (user_id, display_name): (i64, String) = conn.query_row(
        "SELECT id, display_name FROM users WHERE username=?1",
        params![req.username],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).map_err(|_| "User not found".to_string())?;

    let existing: i64 = conn.query_row(
        "SELECT COUNT(*) FROM approval_requests WHERE requester_user_id=?1 AND request_type=?2 AND status='pending'",
        params![user_id, req.request_type],
        |r| r.get(0),
    ).unwrap_or(0);
    if existing > 0 {
        return Err("A pending request of this type already exists. Please wait for super admin approval.".into());
    }

    let payload = if req.request_type == "password_reset" {
        let pw = req.new_password.as_deref().unwrap_or("");
        if pw.len() < 8 { return Err("Password must be at least 8 characters".into()); }
        let hash = bcrypt::hash(pw, bcrypt::DEFAULT_COST).map_err(db_err)?;
        Some(serde_json::json!({"new_password_hash": hash}).to_string())
    } else {
        None
    };

    conn.execute(
        "INSERT INTO approval_requests
         (requester_user_id, requester_username, requester_display_name, request_type, payload_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![user_id, req.username, display_name, req.request_type, payload],
    ).map_err(db_err)?;
    Ok(conn.last_insert_rowid())
}

/// Check status of a user's recent approval requests (no login required).
#[tauri::command]
pub fn get_my_approval_status(
    db: State<DbState>,
    username: String,
) -> Result<Vec<ApprovalRequest>, String> {
    let conn = db.0.lock().map_err(db_err)?;
    let mut stmt = conn.prepare(
        "SELECT ar.id, ar.requester_user_id, ar.requester_username, ar.requester_display_name,
                ar.request_type, ar.status, ar.rejection_reason,
                u2.display_name,
                ar.created_at, ar.resolved_at, ar.expires_at
         FROM approval_requests ar
         LEFT JOIN users u2 ON u2.id = ar.resolver_user_id
         WHERE ar.requester_username=?1
         ORDER BY ar.created_at DESC LIMIT 10"
    ).map_err(db_err)?;
    let rows: Result<Vec<ApprovalRequest>, _> = stmt.query_map(params![username], |row| Ok(ApprovalRequest {
        id: row.get(0)?,
        requester_user_id: row.get(1)?,
        requester_username: row.get(2)?,
        requester_display_name: row.get(3)?,
        request_type: row.get(4)?,
        status: row.get(5)?,
        rejection_reason: row.get(6)?,
        resolver_display_name: row.get(7)?,
        created_at: row.get(8)?,
        resolved_at: row.get(9)?,
        expires_at: row.get(10)?,
    })).map_err(db_err)?.collect();
    rows.map_err(db_err)
}

/// Get all approval requests — super admin only.
#[tauri::command]
pub fn get_pending_approvals(
    db: State<DbState>,
    session: State<SessionState>,
) -> Result<Vec<ApprovalRequest>, String> {
    require_super_admin(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    let mut stmt = conn.prepare(
        "SELECT ar.id, ar.requester_user_id, ar.requester_username, ar.requester_display_name,
                ar.request_type, ar.status, ar.rejection_reason,
                u2.display_name,
                ar.created_at, ar.resolved_at, ar.expires_at
         FROM approval_requests ar
         LEFT JOIN users u2 ON u2.id = ar.resolver_user_id
         ORDER BY ar.created_at DESC"
    ).map_err(db_err)?;
    let rows: Result<Vec<ApprovalRequest>, _> = stmt.query_map([], |row| Ok(ApprovalRequest {
        id: row.get(0)?,
        requester_user_id: row.get(1)?,
        requester_username: row.get(2)?,
        requester_display_name: row.get(3)?,
        request_type: row.get(4)?,
        status: row.get(5)?,
        rejection_reason: row.get(6)?,
        resolver_display_name: row.get(7)?,
        created_at: row.get(8)?,
        resolved_at: row.get(9)?,
        expires_at: row.get(10)?,
    })).map_err(db_err)?.collect();
    rows.map_err(db_err)
}

/// Count pending approvals for sidebar badge — super admin only.
#[tauri::command]
pub fn get_approval_count(
    db: State<DbState>,
    session: State<SessionState>,
) -> Result<i64, String> {
    require_super_admin(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.query_row(
        "SELECT COUNT(*) FROM approval_requests WHERE status='pending'",
        [], |r| r.get(0),
    ).map_err(db_err)
}

/// Approve or reject a request — super admin only.
#[tauri::command]
pub fn resolve_approval(
    db: State<DbState>,
    session: State<SessionState>,
    id: i64,
    approved: bool,
    rejection_reason: Option<String>,
) -> Result<(), String> {
    let s = require_super_admin(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    let (request_type, payload_json, requester_user_id): (String, Option<String>, Option<i64>) =
        conn.query_row(
            "SELECT request_type, payload_json, requester_user_id FROM approval_requests WHERE id=?1 AND status='pending'",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ).map_err(|_| "Request not found or already resolved".to_string())?;

    if approved {
        if let Some(uid) = requester_user_id {
            match request_type.as_str() {
                "password_reset" => {
                    if let Some(payload) = &payload_json {
                        let v: Value = serde_json::from_str(payload).map_err(db_err)?;
                        if let Some(hash) = v["new_password_hash"].as_str() {
                            conn.execute(
                                "UPDATE users SET password_hash=?1 WHERE id=?2",
                                params![hash, uid],
                            ).map_err(db_err)?;
                        }
                    }
                }
                "account_unlock" => {
                    conn.execute("UPDATE users SET is_active=1 WHERE id=?1", params![uid])
                        .map_err(db_err)?;
                }
                _ => {}
            }
        }
        conn.execute(
            "UPDATE approval_requests SET status='approved', resolver_user_id=?1, resolved_at=datetime('now') WHERE id=?2",
            params![s.user_id, id],
        ).map_err(db_err)?;
        log_audit(&conn, &s, "approve", "approval_request", Some(id), Some(&request_type));
    } else {
        conn.execute(
            "UPDATE approval_requests SET status='rejected', resolver_user_id=?1, resolved_at=datetime('now'), rejection_reason=?2 WHERE id=?3",
            params![s.user_id, rejection_reason, id],
        ).map_err(db_err)?;
        log_audit(&conn, &s, "reject", "approval_request", Some(id), Some(&request_type));
    }
    Ok(())
}

fn count_scoped(conn: &Connection, table: &str, org_id: Option<i64>) -> i64 {
    let sql = match org_id {
        Some(id) => format!("SELECT COUNT(*) FROM {} WHERE org_id={}", table, id),
        None => format!("SELECT COUNT(*) FROM {}", table),
    };
    conn.query_row(&sql, [], |r| r.get(0)).unwrap_or(0)
}

// ─── Private DB loaders ──────────────────────────────────────────────────────

// Helper: org filter trait
trait OrgFilter {
    fn org_id_filter(&self) -> Option<i64>;
}
impl OrgFilter for SessionPayload {
    fn org_id_filter(&self) -> Option<i64> {
        if self.role == "super_admin" { None } else { self.org_id }
    }
}

fn load_courses_scoped(conn: &Connection, org_id: Option<i64>) -> Result<Vec<Course>, String> {
    let where_clause = org_id.map_or(String::new(), |id| format!(" WHERE c.org_id={}", id));
    let sql = format!(
        "SELECT c.id, c.code, c.name, c.hours_per_week, c.room_type, c.class_type, c.frequency, c.lecturer_id, l.name, c.org_id
         FROM courses c LEFT JOIN lecturers l ON l.id=c.lecturer_id{} ORDER BY c.code",
        where_clause
    );
    let mut stmt = conn.prepare(&sql).map_err(db_err)?;
    let rows: Result<Vec<Course>, _> = stmt.query_map([], |row| Ok(Course {
        id: row.get(0)?,
        code: row.get(1)?,
        name: row.get(2)?,
        hours_per_week: row.get(3)?,
        room_type: row.get(4)?,
        class_type: row.get(5)?,
        frequency: row.get(6)?,
        lecturer_id: row.get(7)?,
        lecturer_name: row.get(8)?,
        org_id: row.get(9)?,
    })).map_err(db_err)?.collect();
    rows.map_err(db_err)
}

fn load_lecturers_scoped(conn: &Connection, org_id: Option<i64>) -> Result<Vec<Lecturer>, String> {
    let where_clause = org_id.map_or(String::new(), |id| format!(" WHERE org_id={}", id));
    let sql = format!(
        "SELECT id, name, email, available_days, max_hours_per_day, max_hours_per_week, org_id,
                preferred_slots_json, blackout_json, max_consecutive_hours
         FROM lecturers{} ORDER BY name",
        where_clause
    );
    let mut stmt = conn.prepare(&sql).map_err(db_err)?;
    let rows: Result<Vec<Lecturer>, _> = stmt.query_map([], |row| Ok(Lecturer {
        id: row.get(0)?,
        name: row.get(1)?,
        email: row.get(2)?,
        available_days: row.get(3)?,
        max_hours_per_day: row.get(4)?,
        max_hours_per_week: row.get(5)?,
        org_id: row.get(6)?,
        preferred_slots_json: row.get(7)?,
        blackout_json: row.get(8)?,
        max_consecutive_hours: row.get::<_, Option<i64>>(9)?.unwrap_or(3),
    })).map_err(db_err)?.collect();
    rows.map_err(db_err)
}

fn load_rooms_scoped(conn: &Connection, org_id: Option<i64>) -> Result<Vec<Room>, String> {
    let where_clause = org_id.map_or(String::new(), |id| format!(" WHERE org_id={}", id));
    let sql = format!(
        "SELECT id, name, capacity, room_type, available_days, org_id FROM rooms{} ORDER BY name",
        where_clause
    );
    let mut stmt = conn.prepare(&sql).map_err(db_err)?;
    let rows: Result<Vec<Room>, _> = stmt.query_map([], |row| Ok(Room {
        id: row.get(0)?,
        name: row.get(1)?,
        capacity: row.get(2)?,
        room_type: row.get(3)?,
        available_days: row.get(4)?,
        org_id: row.get(5)?,
    })).map_err(db_err)?.collect();
    rows.map_err(db_err)
}

fn load_batches_scoped(conn: &Connection, org_id: Option<i64>) -> Result<Vec<Batch>, String> {
    let where_clause = org_id.map_or(String::new(), |id| format!(" WHERE org_id={}", id));
    let sql = format!(
        "SELECT id, name, department, semester, size, org_id, semester_id FROM batches{} ORDER BY department, semester, name",
        where_clause
    );
    let mut stmt = conn.prepare(&sql).map_err(db_err)?;
    let mut batches: Vec<Batch> = {
        let rows: Result<Vec<Batch>, _> = stmt.query_map([], |row| Ok(Batch {
            id: row.get(0)?, name: row.get(1)?, department: row.get(2)?,
            semester: row.get(3)?, size: row.get(4)?, course_ids: vec![],
            org_id: row.get(5)?, semester_id: row.get(6)?,
        })).map_err(db_err)?.collect();
        rows.map_err(db_err)?
    };
    for batch in &mut batches {
        let mut cs = conn.prepare("SELECT course_id FROM batch_courses WHERE batch_id=?1").map_err(db_err)?;
        let ids: Result<Vec<i64>, _> = cs.query_map(params![batch.id], |row| row.get(0)).map_err(db_err)?.collect();
        batch.course_ids = ids.map_err(db_err)?;
    }
    Ok(batches)
}

fn load_batches_for_semester(conn: &Connection, semester_id: i64) -> Result<Vec<Batch>, String> {
    let mut stmt = conn.prepare(
        "SELECT id, name, department, semester, size, org_id, semester_id FROM batches WHERE semester_id=?1"
    ).map_err(db_err)?;
    let mut batches: Vec<Batch> = {
        let rows: Result<Vec<Batch>, _> = stmt.query_map(params![semester_id], |row| Ok(Batch {
            id: row.get(0)?, name: row.get(1)?, department: row.get(2)?,
            semester: row.get(3)?, size: row.get(4)?, course_ids: vec![],
            org_id: row.get(5)?, semester_id: row.get(6)?,
        })).map_err(db_err)?.collect();
        rows.map_err(db_err)?
    };
    for batch in &mut batches {
        let mut cs = conn.prepare("SELECT course_id FROM batch_courses WHERE batch_id=?1").map_err(db_err)?;
        let ids: Result<Vec<i64>, _> = cs.query_map(params![batch.id], |row| row.get(0)).map_err(db_err)?.collect();
        batch.course_ids = ids.map_err(db_err)?;
    }
    Ok(batches)
}
