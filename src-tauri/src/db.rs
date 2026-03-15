use rusqlite::{Connection, Result, params};
use std::path::Path;

pub fn open(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    migrate_v1(&conn)?;
    migrate_v2(&conn)?;
    migrate_v3(&conn)?;
    migrate_v4(&conn)?;
    seed_super_admin(&conn);
    Ok(conn)
}

// ─── V1: original schema ──────────────────────────────────────────────────────
fn migrate_v1(conn: &Connection) -> Result<()> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS lecturers (
            id                INTEGER PRIMARY KEY AUTOINCREMENT,
            name              TEXT    NOT NULL,
            email             TEXT,
            available_days    TEXT    NOT NULL DEFAULT 'Mon,Tue,Wed,Thu,Fri',
            max_hours_per_day INTEGER NOT NULL DEFAULT 4,
            max_hours_per_week INTEGER NOT NULL DEFAULT 16
        );

        CREATE TABLE IF NOT EXISTS courses (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            code            TEXT    NOT NULL,
            name            TEXT    NOT NULL,
            hours_per_week  INTEGER NOT NULL DEFAULT 3,
            room_type       TEXT    NOT NULL DEFAULT 'lecture'
                            CHECK(room_type IN ('lab','lecture')),
            lecturer_id     INTEGER REFERENCES lecturers(id) ON DELETE SET NULL
        );

        CREATE TABLE IF NOT EXISTS rooms (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            name           TEXT    NOT NULL,
            capacity       INTEGER NOT NULL DEFAULT 30,
            room_type      TEXT    NOT NULL DEFAULT 'lecture'
                           CHECK(room_type IN ('lab','lecture')),
            available_days TEXT    NOT NULL DEFAULT 'Mon,Tue,Wed,Thu,Fri'
        );

        CREATE TABLE IF NOT EXISTS batches (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            name       TEXT    NOT NULL,
            department TEXT    NOT NULL,
            semester   INTEGER NOT NULL DEFAULT 1,
            size       INTEGER NOT NULL DEFAULT 30
        );

        CREATE TABLE IF NOT EXISTS batch_courses (
            batch_id  INTEGER NOT NULL REFERENCES batches(id)  ON DELETE CASCADE,
            course_id INTEGER NOT NULL REFERENCES courses(id)  ON DELETE CASCADE,
            PRIMARY KEY (batch_id, course_id)
        );

        CREATE TABLE IF NOT EXISTS schedules (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            name       TEXT    NOT NULL,
            created_at TEXT    NOT NULL,
            is_active  INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS schedule_entries (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            schedule_id INTEGER NOT NULL REFERENCES schedules(id) ON DELETE CASCADE,
            course_id   INTEGER NOT NULL REFERENCES courses(id),
            lecturer_id INTEGER NOT NULL REFERENCES lecturers(id),
            room_id     INTEGER NOT NULL REFERENCES rooms(id),
            batch_id    INTEGER NOT NULL REFERENCES batches(id),
            day         TEXT    NOT NULL,
            time_slot   INTEGER NOT NULL
        );
    ")?;
    Ok(())
}

// ─── V2: organisations, semesters, users, richer course metadata ──────────────
fn migrate_v2(conn: &Connection) -> Result<()> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS organizations (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            name       TEXT NOT NULL,
            org_type   TEXT NOT NULL DEFAULT 'university'
                       CHECK(org_type IN ('university','college','school','institute')),
            address    TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS users (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            username      TEXT NOT NULL UNIQUE,
            display_name  TEXT NOT NULL,
            password_hash TEXT NOT NULL,
            role          TEXT NOT NULL DEFAULT 'admin'
                          CHECK(role IN ('super_admin','admin')),
            org_id        INTEGER REFERENCES organizations(id) ON DELETE SET NULL,
            created_at    TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS semesters (
            id                INTEGER PRIMARY KEY AUTOINCREMENT,
            org_id            INTEGER NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
            name              TEXT    NOT NULL,
            start_date        TEXT    NOT NULL,
            end_date          TEXT    NOT NULL,
            student_capacity  INTEGER,
            teaching_weeks    INTEGER NOT NULL DEFAULT 14,
            midterm_start     TEXT,
            midterm_end       TEXT,
            study_break_start TEXT,
            study_break_end   TEXT,
            final_start       TEXT,
            final_end         TEXT,
            breaks_json       TEXT    NOT NULL DEFAULT '[]',
            status            TEXT    NOT NULL DEFAULT 'planning'
                              CHECK(status IN ('planning','active','completed')),
            created_at        TEXT    NOT NULL DEFAULT (datetime('now'))
        );
    ")?;

    // Alter existing tables — silently ignore duplicate-column errors
    let alters = [
        "ALTER TABLE lecturers      ADD COLUMN org_id      INTEGER REFERENCES organizations(id)",
        "ALTER TABLE courses        ADD COLUMN org_id      INTEGER REFERENCES organizations(id)",
        "ALTER TABLE courses        ADD COLUMN class_type  TEXT NOT NULL DEFAULT 'lecture'",
        "ALTER TABLE courses        ADD COLUMN frequency   TEXT NOT NULL DEFAULT 'weekly'",
        "ALTER TABLE rooms          ADD COLUMN org_id      INTEGER REFERENCES organizations(id)",
        "ALTER TABLE batches        ADD COLUMN org_id      INTEGER REFERENCES organizations(id)",
        "ALTER TABLE batches        ADD COLUMN semester_id INTEGER REFERENCES semesters(id)",
        "ALTER TABLE schedules      ADD COLUMN org_id      INTEGER REFERENCES organizations(id)",
        "ALTER TABLE schedules      ADD COLUMN semester_id INTEGER REFERENCES semesters(id)",
        "ALTER TABLE schedule_entries ADD COLUMN class_type  TEXT NOT NULL DEFAULT 'lecture'",
        "ALTER TABLE schedule_entries ADD COLUMN week_parity INTEGER NOT NULL DEFAULT 0",
    ];
    for sql in &alters {
        try_alter(conn, sql);
    }
    Ok(())
}

// ─── V3: scheduling settings, user active flag, org contact email ─────────────
fn migrate_v3(conn: &Connection) -> Result<()> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS org_scheduling_settings (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            org_id          INTEGER NOT NULL UNIQUE REFERENCES organizations(id) ON DELETE CASCADE,
            working_days    TEXT    NOT NULL DEFAULT 'Mon,Tue,Wed,Thu,Fri',
            day_start_slot  INTEGER NOT NULL DEFAULT 0,
            day_end_slot    INTEGER NOT NULL DEFAULT 7,
            slot_duration   INTEGER NOT NULL DEFAULT 60,
            updated_at      TEXT    NOT NULL DEFAULT (datetime('now'))
        );
    ")?;
    let alters = [
        "ALTER TABLE users         ADD COLUMN is_active     INTEGER NOT NULL DEFAULT 1",
        "ALTER TABLE organizations ADD COLUMN contact_email TEXT",
    ];
    for sql in &alters {
        try_alter(conn, sql);
    }
    Ok(())
}

// ─── V4: app-wide settings (single-org, admin quota) ─────────────────────────
fn migrate_v4(conn: &Connection) -> Result<()> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS app_settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
    ")?;
    // Seed default max_admins = 2 (idempotent)
    let _ = conn.execute(
        "INSERT OR IGNORE INTO app_settings (key, value) VALUES ('max_admins', '2')",
        [],
    );
    Ok(())
}

fn try_alter(conn: &Connection, sql: &str) {
    let _ = conn.execute_batch(sql);
}

// ─── Seed default super-admin on first run ────────────────────────────────────
fn seed_super_admin(conn: &Connection) {
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
        .unwrap_or(0);
    if count == 0 {
        let hash = bcrypt::hash("admin123", bcrypt::DEFAULT_COST).unwrap_or_default();
        let _ = conn.execute(
            "INSERT INTO users (username, display_name, password_hash, role, org_id)
             VALUES ('admin', 'Super Admin', ?1, 'super_admin', NULL)",
            params![hash],
        );
    }
}

// ─── Bulk-insert schedule entries ─────────────────────────────────────────────
pub fn replace_schedule_entries(
    conn: &Connection,
    schedule_id: i64,
    entries: &[(i64, i64, i64, i64, &str, i64, &str, i64)],
    // (course, lecturer, room, batch, day, slot, class_type, week_parity)
) -> Result<()> {
    conn.execute("DELETE FROM schedule_entries WHERE schedule_id = ?1", params![schedule_id])?;
    let mut stmt = conn.prepare(
        "INSERT INTO schedule_entries
         (schedule_id, course_id, lecturer_id, room_id, batch_id, day, time_slot, class_type, week_parity)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
    )?;
    for (cid, lid, rid, bid, day, slot, ct, wp) in entries {
        stmt.execute(params![schedule_id, cid, lid, rid, bid, day, slot, ct, wp])?;
    }
    Ok(())
}
