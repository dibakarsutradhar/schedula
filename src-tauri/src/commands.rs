use rusqlite::{params, Connection};
use serde_json::Value;
use std::sync::Mutex;
use tauri::State;

use crate::models::*;
use crate::scheduler::{self, SchedulerInput};

pub struct DbState(pub Mutex<Connection>);
pub struct SessionState(pub Mutex<Option<SessionPayload>>);

fn db_err(e: impl std::fmt::Display) -> String { e.to_string() }

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
    require_super_admin(&session)?;
    let conn = db.0.lock().map_err(db_err)?;

    // Enforce: only one super_admin ever
    if user.role == "super_admin" {
        return Err("Only one super admin is allowed per app instance.".into());
    }

    // Enforce max_admins quota
    if user.role == "admin" {
        let max: i64 = conn.query_row(
            "SELECT CAST(value AS INTEGER) FROM app_settings WHERE key='max_admins'",
            [], |r| r.get(0),
        ).unwrap_or(2);
        let current: i64 = conn.query_row(
            "SELECT COUNT(*) FROM users WHERE role='admin' AND is_active=1",
            [], |r| r.get(0),
        ).unwrap_or(0);
        if current >= max {
            return Err(format!(
                "Admin limit reached ({}/{}). Increase Max Admins in Settings → System.",
                current, max
            ).into());
        }
    }

    let hash = bcrypt::hash(&user.password, bcrypt::DEFAULT_COST).map_err(db_err)?;
    conn.execute(
        "INSERT INTO users (username, display_name, password_hash, role, org_id) VALUES (?1,?2,?3,?4,?5)",
        params![user.username, user.display_name, hash, user.role, user.org_id],
    ).map_err(db_err)?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn delete_user(db: State<DbState>, session: State<SessionState>, id: i64) -> Result<(), String> {
    let s = require_super_admin(&session)?;
    if s.user_id == id { return Err("Cannot delete yourself".into()); }
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute("DELETE FROM users WHERE id=?1", params![id]).map_err(db_err)?;
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
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute(
        "INSERT INTO courses (code, name, hours_per_week, room_type, class_type, frequency, lecturer_id, org_id)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![course.code, course.name, course.hours_per_week, course.room_type, course.class_type, course.frequency, course.lecturer_id, course.org_id],
    ).map_err(db_err)?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_course(db: State<DbState>, session: State<SessionState>, id: i64, course: NewCourse) -> Result<(), String> {
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute(
        "UPDATE courses SET code=?1, name=?2, hours_per_week=?3, room_type=?4, class_type=?5, frequency=?6, lecturer_id=?7, org_id=?8 WHERE id=?9",
        params![course.code, course.name, course.hours_per_week, course.room_type, course.class_type, course.frequency, course.lecturer_id, course.org_id, id],
    ).map_err(db_err)?;
    Ok(())
}

#[tauri::command]
pub fn delete_course(db: State<DbState>, session: State<SessionState>, id: i64) -> Result<(), String> {
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute("DELETE FROM courses WHERE id=?1", params![id]).map_err(db_err)?;
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
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute(
        "INSERT INTO lecturers (name, email, available_days, max_hours_per_day, max_hours_per_week, org_id)
         VALUES (?1,?2,?3,?4,?5,?6)",
        params![lecturer.name, lecturer.email, lecturer.available_days, lecturer.max_hours_per_day, lecturer.max_hours_per_week, lecturer.org_id],
    ).map_err(db_err)?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_lecturer(db: State<DbState>, session: State<SessionState>, id: i64, lecturer: NewLecturer) -> Result<(), String> {
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute(
        "UPDATE lecturers SET name=?1, email=?2, available_days=?3, max_hours_per_day=?4, max_hours_per_week=?5, org_id=?6 WHERE id=?7",
        params![lecturer.name, lecturer.email, lecturer.available_days, lecturer.max_hours_per_day, lecturer.max_hours_per_week, lecturer.org_id, id],
    ).map_err(db_err)?;
    Ok(())
}

#[tauri::command]
pub fn delete_lecturer(db: State<DbState>, session: State<SessionState>, id: i64) -> Result<(), String> {
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute("DELETE FROM lecturers WHERE id=?1", params![id]).map_err(db_err)?;
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
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute(
        "INSERT INTO rooms (name, capacity, room_type, available_days, org_id) VALUES (?1,?2,?3,?4,?5)",
        params![room.name, room.capacity, room.room_type, room.available_days, room.org_id],
    ).map_err(db_err)?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_room(db: State<DbState>, session: State<SessionState>, id: i64, room: NewRoom) -> Result<(), String> {
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute(
        "UPDATE rooms SET name=?1, capacity=?2, room_type=?3, available_days=?4, org_id=?5 WHERE id=?6",
        params![room.name, room.capacity, room.room_type, room.available_days, room.org_id, id],
    ).map_err(db_err)?;
    Ok(())
}

#[tauri::command]
pub fn delete_room(db: State<DbState>, session: State<SessionState>, id: i64) -> Result<(), String> {
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute("DELETE FROM rooms WHERE id=?1", params![id]).map_err(db_err)?;
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
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
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
    Ok(id)
}

#[tauri::command]
pub fn update_batch(db: State<DbState>, session: State<SessionState>, id: i64, batch: NewBatch) -> Result<(), String> {
    require_session(&session)?;
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
    Ok(())
}

#[tauri::command]
pub fn delete_batch(db: State<DbState>, session: State<SessionState>, id: i64) -> Result<(), String> {
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute("DELETE FROM batches WHERE id=?1", params![id]).map_err(db_err)?;
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

    let input = SchedulerInput { courses, lecturers, rooms, batches };
    let result = scheduler::generate(&input);

    let now = chrono::Local::now().to_rfc3339();
    conn.execute("UPDATE schedules SET is_active=0 WHERE org_id IS ?1 OR org_id=?1", params![s.org_id]).map_err(db_err)?;
    conn.execute(
        "INSERT INTO schedules (name, created_at, is_active, org_id, semester_id) VALUES (?1,?2,1,?3,?4)",
        params![schedule_name, now, s.org_id, semester_id],
    ).map_err(db_err)?;
    let schedule_id = conn.last_insert_rowid();

    let tuples: Vec<(i64, i64, i64, i64, &str, i64, &str, i64)> = result.entries.iter().map(|e| {
        (e.course_id, e.lecturer_id, e.room_id, e.batch_id, e.day.as_str(), e.time_slot, e.class_type.as_str(), e.week_parity)
    }).collect();
    crate::db::replace_schedule_entries(&conn, schedule_id, &tuples).map_err(db_err)?;

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
        "SELECT sch.id, sch.name, sch.created_at, sch.is_active,
                (SELECT COUNT(*) FROM schedule_entries WHERE schedule_id=sch.id),
                sch.semester_id, sem.name
         FROM schedules sch LEFT JOIN semesters sem ON sem.id=sch.semester_id
         ORDER BY sch.id DESC".to_string()
    } else {
        format!(
            "SELECT sch.id, sch.name, sch.created_at, sch.is_active,
                    (SELECT COUNT(*) FROM schedule_entries WHERE schedule_id=sch.id),
                    sch.semester_id, sem.name
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
        entry_count: row.get(4)?,
        semester_id: row.get(5)?,
        semester_name: row.get(6)?,
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
    conn.execute("UPDATE schedules SET is_active=0 WHERE org_id IS ?1 OR org_id=?1", params![s.org_id]).map_err(db_err)?;
    conn.execute("UPDATE schedules SET is_active=1 WHERE id=?1", params![id]).map_err(db_err)?;
    Ok(())
}

#[tauri::command]
pub fn delete_schedule(db: State<DbState>, session: State<SessionState>, id: i64) -> Result<(), String> {
    require_session(&session)?;
    let conn = db.0.lock().map_err(db_err)?;
    conn.execute("DELETE FROM schedules WHERE id=?1", params![id]).map_err(db_err)?;
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
        "SELECT id, name, email, available_days, max_hours_per_day, max_hours_per_week, org_id FROM lecturers{} ORDER BY name",
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
