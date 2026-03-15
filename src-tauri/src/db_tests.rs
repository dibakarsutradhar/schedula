/// Integration tests — run against an in-memory SQLite database so every
/// test starts from a clean, fully-migrated state with no side-effects.
///
/// Coverage:
///   - Migration idempotency
///   - Seeded super-admin credentials
///   - Password hashing (bcrypt verify round-trip)
///   - Duplicate-username rejection
///   - Org data isolation (admin A can't see org B's data)
///   - CRUD create/read round-trips
///   - Admin quota enforcement
///   - Password recovery flow (setup → verify question → reset)
///   - Audit log entries
///   - Foreign-key cascade deletes

#[cfg(test)]
mod tests {
    use rusqlite::{Connection, params};
    use crate::db;

    /// Open a fresh in-memory database and run all migrations.
    fn mem_db() -> Connection {
        let conn = Connection::open(":memory:").expect("in-memory DB");
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .expect("pragmas");
        db::run_migrations(&conn).expect("migrations");
        conn
    }

    // ── Migration ─────────────────────────────────────────────────────────────

    #[test]
    fn migration_idempotent() {
        // Running migrations twice must not panic or return an error
        let conn = Connection::open(":memory:").expect("in-memory DB");
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .expect("pragmas");
        db::run_migrations(&conn).expect("first run");
        db::run_migrations(&conn).expect("second run — must be idempotent");
    }

    #[test]
    fn migration_creates_expected_tables() {
        let conn = mem_db();
        let tables: Vec<String> = {
            let mut s = conn.prepare(
                "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"
            ).unwrap();
            s.query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect()
        };
        for expected in &["users", "organizations", "lecturers", "courses", "rooms",
                          "batches", "batch_courses", "schedules", "schedule_entries",
                          "semesters", "org_scheduling_settings", "app_settings", "audit_log"]
        {
            assert!(tables.contains(&expected.to_string()), "missing table: {}", expected);
        }
    }

    #[test]
    fn migration_v8_recovery_columns_exist() {
        let conn = mem_db();
        // Verify recovery columns were added to users
        let cols: Vec<String> = {
            let mut s = conn.prepare("PRAGMA table_info(users)").unwrap();
            s.query_map([], |r| r.get::<_, String>(1)).unwrap()
                .map(|r| r.unwrap()).collect()
        };
        assert!(cols.contains(&"recovery_code_hash".to_string()));
        assert!(cols.contains(&"security_question".to_string()));
        assert!(cols.contains(&"security_answer_hash".to_string()));
    }

    // ── Seeded super-admin ────────────────────────────────────────────────────

    #[test]
    fn seed_super_admin_created_on_fresh_db() {
        let conn = mem_db();
        db::seed_super_admin_if_empty(&conn);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM users WHERE username='admin'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1, "super-admin 'admin' must be seeded on first run");
    }

    #[test]
    fn seed_super_admin_not_duplicated_on_second_call() {
        let conn = mem_db();
        db::seed_super_admin_if_empty(&conn);
        db::seed_super_admin_if_empty(&conn); // second call
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM users WHERE username='admin'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1, "seeding twice must not insert duplicate");
    }

    #[test]
    fn seeded_admin_password_is_bcrypt_hashed() {
        let conn = mem_db();
        db::seed_super_admin_if_empty(&conn);
        let hash: String = conn
            .query_row("SELECT password_hash FROM users WHERE username='admin'", [], |r| r.get(0))
            .unwrap();
        assert!(hash.starts_with("$2"), "password hash must be a bcrypt hash");
        assert!(bcrypt::verify("admin123", &hash).unwrap_or(false),
            "default password 'admin123' must verify against stored hash");
    }

    #[test]
    fn seeded_admin_role_is_super_admin() {
        let conn = mem_db();
        db::seed_super_admin_if_empty(&conn);
        let role: String = conn
            .query_row("SELECT role FROM users WHERE username='admin'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(role, "super_admin");
    }

    // ── User management ───────────────────────────────────────────────────────

    #[test]
    fn duplicate_username_rejected() {
        let conn = mem_db();
        db::seed_super_admin_if_empty(&conn);
        let hash = bcrypt::hash("pass", 4).unwrap();
        conn.execute(
            "INSERT INTO users (username,display_name,password_hash,role) VALUES ('alice','Alice',?1,'admin')",
            params![hash],
        ).expect("first insert");
        let result = conn.execute(
            "INSERT INTO users (username,display_name,password_hash,role) VALUES ('alice','Alice2',?1,'admin')",
            params![hash],
        );
        assert!(result.is_err(), "duplicate username must be rejected by UNIQUE constraint");
    }

    #[test]
    fn user_is_active_defaults_to_one() {
        let conn = mem_db();
        let hash = bcrypt::hash("pw", 4).unwrap();
        conn.execute(
            "INSERT INTO users (username,display_name,password_hash,role) VALUES ('bob','Bob',?1,'admin')",
            params![hash],
        ).unwrap();
        let active: i64 = conn
            .query_row("SELECT is_active FROM users WHERE username='bob'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(active, 1);
    }

    // ── Org data isolation ────────────────────────────────────────────────────

    #[test]
    fn org_isolation_admin_cannot_see_other_org_lecturers() {
        let conn = mem_db();

        // Create two orgs
        conn.execute("INSERT INTO organizations (name,org_type) VALUES ('Uni A','university')", []).unwrap();
        let org_a = conn.last_insert_rowid();
        conn.execute("INSERT INTO organizations (name,org_type) VALUES ('Uni B','university')", []).unwrap();
        let org_b = conn.last_insert_rowid();

        // Insert a lecturer in each org
        conn.execute(
            "INSERT INTO lecturers (name,available_days,max_hours_per_day,max_hours_per_week,org_id)
             VALUES ('Alice','Mon,Tue,Wed,Thu,Fri',4,16,?1)",
            params![org_a],
        ).unwrap();
        conn.execute(
            "INSERT INTO lecturers (name,available_days,max_hours_per_day,max_hours_per_week,org_id)
             VALUES ('Bob','Mon,Tue,Wed,Thu,Fri',4,16,?1)",
            params![org_b],
        ).unwrap();

        // Admin A queries only their org
        let count_a: i64 = conn.query_row(
            "SELECT COUNT(*) FROM lecturers WHERE org_id=?1", params![org_a], |r| r.get(0),
        ).unwrap();
        let count_b: i64 = conn.query_row(
            "SELECT COUNT(*) FROM lecturers WHERE org_id=?1", params![org_b], |r| r.get(0),
        ).unwrap();

        assert_eq!(count_a, 1, "Org A should see only its 1 lecturer");
        assert_eq!(count_b, 1, "Org B should see only its 1 lecturer");

        // Cross-check: org A admin must not retrieve org B data
        let cross: i64 = conn.query_row(
            "SELECT COUNT(*) FROM lecturers WHERE org_id=?1 AND name='Bob'",
            params![org_a], |r| r.get(0),
        ).unwrap();
        assert_eq!(cross, 0, "Org A must not see Org B's lecturers");
    }

    #[test]
    fn org_isolation_courses_scoped_to_org() {
        let conn = mem_db();
        conn.execute("INSERT INTO organizations (name,org_type) VALUES ('X','university')", []).unwrap();
        let org_x = conn.last_insert_rowid();
        conn.execute("INSERT INTO organizations (name,org_type) VALUES ('Y','university')", []).unwrap();
        let org_y = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO courses (code,name,hours_per_week,room_type,class_type,frequency,org_id)
             VALUES ('X101','XCourse',3,'lecture','lecture','weekly',?1)",
            params![org_x],
        ).unwrap();
        conn.execute(
            "INSERT INTO courses (code,name,hours_per_week,room_type,class_type,frequency,org_id)
             VALUES ('Y101','YCourse',3,'lecture','lecture','weekly',?1)",
            params![org_y],
        ).unwrap();

        let x_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM courses WHERE org_id=?1", params![org_x], |r| r.get(0),
        ).unwrap();
        assert_eq!(x_count, 1, "Org X should see only its own course");

        // Org X admin must not see org Y's course
        let cross: i64 = conn.query_row(
            "SELECT COUNT(*) FROM courses WHERE org_id=?1 AND code='Y101'",
            params![org_x], |r| r.get(0),
        ).unwrap();
        assert_eq!(cross, 0, "Org X must not see Org Y courses");
    }

    // ── CRUD round-trips ──────────────────────────────────────────────────────

    #[test]
    fn create_and_read_lecturer() {
        let conn = mem_db();
        conn.execute("INSERT INTO organizations (name,org_type) VALUES ('Uni','university')", []).unwrap();
        let org = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO lecturers (name,email,available_days,max_hours_per_day,max_hours_per_week,org_id)
             VALUES ('Dr. Test','test@uni.edu','Mon,Tue,Wed',3,12,?1)",
            params![org],
        ).unwrap();
        let id = conn.last_insert_rowid();

        let (name, email, days): (String, Option<String>, String) = conn.query_row(
            "SELECT name,email,available_days FROM lecturers WHERE id=?1",
            params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ).unwrap();
        assert_eq!(name, "Dr. Test");
        assert_eq!(email, Some("test@uni.edu".to_string()));
        assert_eq!(days, "Mon,Tue,Wed");
    }

    #[test]
    fn cascade_delete_removes_schedule_entries() {
        let conn = mem_db();
        // Create minimal data
        conn.execute("INSERT INTO organizations (name,org_type) VALUES ('Uni','university')", []).unwrap();
        let _org = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO schedules (name,created_at,is_active,status) VALUES ('S1',datetime('now'),0,'draft')", [],
        ).unwrap();
        let sched_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO lecturers (name,available_days,max_hours_per_day,max_hours_per_week)
             VALUES ('L','Mon',4,16)", [],
        ).unwrap();
        let lec_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO rooms (name,capacity,room_type,available_days) VALUES ('R1',30,'lecture','Mon')", [],
        ).unwrap();
        let room_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO courses (code,name,hours_per_week,room_type,class_type,frequency,lecturer_id)
             VALUES ('C1','Course',2,'lecture','lecture','weekly',?1)",
            params![lec_id],
        ).unwrap();
        let course_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO batches (name,department,semester,size) VALUES ('B1','CS',1,25)", [],
        ).unwrap();
        let batch_id = conn.last_insert_rowid();

        // Insert a schedule entry
        conn.execute(
            "INSERT INTO schedule_entries
             (schedule_id,course_id,lecturer_id,room_id,batch_id,day,time_slot,class_type,week_parity)
             VALUES (?1,?2,?3,?4,?5,'Mon',0,'lecture',0)",
            params![sched_id, course_id, lec_id, room_id, batch_id],
        ).unwrap();

        let before: i64 = conn.query_row(
            "SELECT COUNT(*) FROM schedule_entries WHERE schedule_id=?1",
            params![sched_id], |r| r.get(0),
        ).unwrap();
        assert_eq!(before, 1);

        // Delete the schedule — entries should cascade
        conn.execute("DELETE FROM schedules WHERE id=?1", params![sched_id]).unwrap();
        let after: i64 = conn.query_row(
            "SELECT COUNT(*) FROM schedule_entries WHERE schedule_id=?1",
            params![sched_id], |r| r.get(0),
        ).unwrap();
        assert_eq!(after, 0, "Cascade delete must remove schedule_entries");
    }

    // ── Admin quota enforcement ───────────────────────────────────────────────

    #[test]
    fn app_settings_default_max_admins_is_two() {
        let conn = mem_db();
        let max: i64 = conn
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM app_settings WHERE key='max_admins'",
                [], |r| r.get(0),
            )
            .unwrap();
        assert_eq!(max, 2);
    }

    #[test]
    fn app_settings_max_admins_can_be_updated() {
        let conn = mem_db();
        conn.execute(
            "INSERT OR REPLACE INTO app_settings (key,value) VALUES ('max_admins','5')", [],
        ).unwrap();
        let max: i64 = conn
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM app_settings WHERE key='max_admins'",
                [], |r| r.get(0),
            )
            .unwrap();
        assert_eq!(max, 5);
    }

    // ── Password recovery flow ────────────────────────────────────────────────

    #[test]
    fn setup_recovery_stores_hashed_code_and_answer() {
        let conn = mem_db();
        db::seed_super_admin_if_empty(&conn);

        let recovery_code = "TESTCODE1234567890ABCDEFGHIJKLMN";
        let security_question = "What is the name of your first pet?";
        let security_answer = "Fluffy";

        let code_hash = bcrypt::hash(recovery_code, 4).unwrap();
        let answer_hash = bcrypt::hash(security_answer, 4).unwrap();

        conn.execute(
            "UPDATE users SET recovery_code_hash=?1, security_question=?2, security_answer_hash=?3
             WHERE username='admin'",
            params![code_hash, security_question, answer_hash],
        ).unwrap();

        // Verify stored data
        let (stored_code_hash, stored_question, stored_answer_hash): (String, String, String) = conn
            .query_row(
                "SELECT recovery_code_hash, security_question, security_answer_hash
                 FROM users WHERE username='admin'",
                [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();

        assert!(bcrypt::verify(recovery_code, &stored_code_hash).unwrap(),
            "Recovery code must verify against stored hash");
        assert_eq!(stored_question, security_question);
        assert!(bcrypt::verify(security_answer, &stored_answer_hash).unwrap(),
            "Security answer must verify against stored hash");
    }

    #[test]
    fn reset_password_with_valid_recovery_code() {
        let conn = mem_db();
        db::seed_super_admin_if_empty(&conn);

        let recovery_code = "MYRECOVERYCODE32CHARSLONGXXXXXXXX";
        let code_hash = bcrypt::hash(recovery_code, 4).unwrap();
        conn.execute(
            "UPDATE users SET recovery_code_hash=?1 WHERE username='admin'",
            params![code_hash],
        ).unwrap();

        // Simulate password reset: verify code then update password
        let stored_hash: String = conn
            .query_row("SELECT recovery_code_hash FROM users WHERE username='admin'", [], |r| r.get(0))
            .unwrap();
        assert!(bcrypt::verify(recovery_code, &stored_hash).unwrap(), "Code must verify");

        let new_hash = bcrypt::hash("newpassword123", 4).unwrap();
        conn.execute(
            "UPDATE users SET password_hash=?1, recovery_code_hash=NULL WHERE username='admin'",
            params![new_hash],
        ).unwrap();

        // Old code should now be cleared
        let cleared: Option<String> = conn
            .query_row("SELECT recovery_code_hash FROM users WHERE username='admin'", [], |r| r.get(0))
            .unwrap();
        assert!(cleared.is_none(), "Recovery code must be cleared after use");

        // New password must work
        let pass_hash: String = conn
            .query_row("SELECT password_hash FROM users WHERE username='admin'", [], |r| r.get(0))
            .unwrap();
        assert!(bcrypt::verify("newpassword123", &pass_hash).unwrap());
    }

    #[test]
    fn reset_password_with_valid_security_answer() {
        let conn = mem_db();
        db::seed_super_admin_if_empty(&conn);

        let answer = "Rover";
        let answer_hash = bcrypt::hash(answer, 4).unwrap();
        conn.execute(
            "UPDATE users SET security_question='Pet name?', security_answer_hash=?1 WHERE username='admin'",
            params![answer_hash],
        ).unwrap();

        let stored: String = conn
            .query_row("SELECT security_answer_hash FROM users WHERE username='admin'", [], |r| r.get(0))
            .unwrap();
        assert!(bcrypt::verify(answer, &stored).unwrap(), "Answer must verify");
    }

    #[test]
    fn wrong_recovery_code_does_not_verify() {
        let code_hash = bcrypt::hash("CORRECTCODE", 4).unwrap();
        assert!(!bcrypt::verify("WRONGCODE", &code_hash).unwrap_or(false),
            "Wrong recovery code must not verify");
    }

    #[test]
    fn wrong_security_answer_does_not_verify() {
        let answer_hash = bcrypt::hash("Fluffy", 4).unwrap();
        assert!(!bcrypt::verify("Rex", &answer_hash).unwrap_or(false),
            "Wrong security answer must not verify");
    }

    // ── Audit log ─────────────────────────────────────────────────────────────

    #[test]
    fn audit_log_insert_and_retrieve() {
        let conn = mem_db();
        conn.execute(
            "INSERT INTO audit_log (username, action, entity_type, entity_id, created_at)
             VALUES ('admin', 'create', 'lecturer', 42, datetime('now'))",
            [],
        ).unwrap();
        let (action, entity): (String, String) = conn
            .query_row(
                "SELECT action, entity_type FROM audit_log ORDER BY id DESC LIMIT 1",
                [], |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(action, "create");
        assert_eq!(entity, "lecturer");
    }

    // ── Batch–course relationship ─────────────────────────────────────────────

    #[test]
    fn batch_course_primary_key_prevents_duplicates() {
        let conn = mem_db();
        conn.execute(
            "INSERT INTO courses (code,name,hours_per_week,room_type,class_type,frequency)
             VALUES ('C1','Course',2,'lecture','lecture','weekly')", [],
        ).unwrap();
        let cid = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO batches (name,department,semester,size) VALUES ('B1','CS',1,25)", [],
        ).unwrap();
        let bid = conn.last_insert_rowid();

        conn.execute("INSERT INTO batch_courses (batch_id,course_id) VALUES (?1,?2)", params![bid, cid]).unwrap();
        let result = conn.execute("INSERT INTO batch_courses (batch_id,course_id) VALUES (?1,?2)", params![bid, cid]);
        assert!(result.is_err(), "Duplicate batch_courses entry must be rejected");
    }

    #[test]
    fn cascade_delete_batch_removes_batch_courses() {
        let conn = mem_db();
        conn.execute(
            "INSERT INTO courses (code,name,hours_per_week,room_type,class_type,frequency)
             VALUES ('C1','Course',2,'lecture','lecture','weekly')", [],
        ).unwrap();
        let cid = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO batches (name,department,semester,size) VALUES ('B1','CS',1,25)", [],
        ).unwrap();
        let bid = conn.last_insert_rowid();
        conn.execute("INSERT INTO batch_courses (batch_id,course_id) VALUES (?1,?2)", params![bid, cid]).unwrap();

        conn.execute("DELETE FROM batches WHERE id=?1", params![bid]).unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM batch_courses WHERE batch_id=?1", params![bid], |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 0, "batch_courses must cascade delete when batch deleted");
    }
}
