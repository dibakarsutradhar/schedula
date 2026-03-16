# System Architecture — Schedula

**Last Updated:** March 16, 2025
**Version:** 0.1.0
**Status:** Production Ready

---

## Executive Summary

Schedula is a **constraint-based timetable generation system** built as a cross-platform desktop application. The system decouples concern cleanly:

- **Frontend:** Svelte 4 reactive UI with Vite bundler
- **IPC Layer:** Tauri 2.x type-safe command system
- **Backend:** Rust business logic (scheduler, auth, CRUD)
- **Persistence:** SQLite with 8 schema migrations

**Key metrics:**
- Schedule generation: <10ms for 200-course datasets
- Memory footprint: ~50 MB runtime
- Database size: 0.5–2 MB depending on data
- Test coverage: 75 unit + integration tests + criterion benchmarks

---

## 1. System Architecture Diagram

```
┌───────────────────────────────────────────────────────────────┐
│                    BROWSER / DESKTOP APP                      │
├───────────────────────────────────────────────────────────────┤
│  Frontend (Svelte 4 + Vite)                                  │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │ App Router                                              │ │
│  │ ├─ Dashboard (stats, data health, getting started)     │ │
│  │ ├─ Lecturers (CRUD + soft constraints)                 │ │
│  │ ├─ Courses (CRUD + class type, frequency)              │ │
│  │ ├─ Rooms (CRUD + capacity, type)                       │ │
│  │ ├─ Batches (CRUD + course enrollment)                  │ │
│  │ ├─ Schedule (generation, viewing, editing, export)     │ │
│  │ ├─ Settings (org, user, scheduling, theme)             │ │
│  │ ├─ Users (admin management, quota)                     │ │
│  │ └─ Import (bulk CSV upload)                            │ │
│  │                                                         │ │
│  │ Stores (Svelte reactive)                               │ │
│  │ ├─ session: User auth state + org context              │ │
│  │ └─ prefs: Theme, accent color (localStorage)           │ │
│  │                                                         │ │
│  │ API Layer (api.js)                                      │ │
│  │ └─ 40+ invoke() wrappers for Tauri commands             │ │
│  └─────────────────────────────────────────────────────────┘ │
└──────────────────────┬───────────────────────────────────────┘
                       │
            ┌──────────▼──────────┐
            │  Tauri 2.x IPC      │
            │  ────────────────── │
            │  invoke(cmd, args)  │
            │  Serde JSON         │
            │  Bidirectional      │
            └──────────┬──────────┘
                       │
┌──────────────────────▼──────────────────────────────────────────┐
│               Backend (Rust + Tauri)                            │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Command Handlers (commands.rs, ~1,730 LOC)                    │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ #[tauri::command] functions                             │  │
│  │ ├─ auth: login, logout, get_session, change_password    │  │
│  │ ├─ users: create, delete, get, reset_password           │  │
│  │ ├─ orgs: CRUD + org isolation check                      │  │
│  │ ├─ scheduler: generate_schedule, publish, revert         │  │
│  │ ├─ data: bulk import, pre-flight check, data health      │  │
│  │ └─ recovery: setup_recovery, reset_with_code/answer      │  │
│  └──────────────────────────────────────────────────────────┘  │
│                          ▲                                      │
│  ┌─────────────────────────┴──────────────────┐               │
│  │                                            │                │
│  │  Scheduler (scheduler.rs, ~324 LOC)       │  Models         │
│  │  ┌─────────────────────────────────────┐  │  (~430 LOC)    │
│  │  │ pub fn generate()                   │  │                │
│  │  │ - Input: courses, lecturers, rooms │  │  - 30+ structs  │
│  │  │ - Constraint checking loop         │  │  - DAYS, SLOTS  │
│  │  │ - Returns: entries + unscheduled   │  │  - Constants    │
│  │  │                                     │  │                │
│  │  │ Hard constraints (9):              │  │                │
│  │  │ 1. No room double-booking         │  │                │
│  │  │ 2. No lecturer conflicts          │  │                │
│  │  │ 3. No batch double-booking        │  │                │
│  │  │ 4. Room type matching             │  │                │
│  │  │ 5. Room capacity >= batch size    │  │                │
│  │  │ 6. Lecturer available on day      │  │                │
│  │  │ 7. max_hours_per_day respected    │  │                │
│  │  │ 8. max_hours_per_week cap         │  │                │
│  │  │ 9. max_consecutive_hours (gap)    │  │                │
│  │  │                                     │  │                │
│  │  │ Soft constraints:                  │  │                │
│  │  │ - Preferred time-of-day            │  │                │
│  │  │ - Blackout slots/days              │  │                │
│  │  │                                     │  │                │
│  │  │ Diversity heuristics:              │  │                │
│  │  │ - Labs → afternoon (slot ≥4)      │  │                │
│  │  │ - Tutorials → morning (slot <4)   │  │                │
│  │  │ - Spread batches across days      │  │                │
│  │  │ - Biweekly ceil(N/2) sessions     │  │                │
│  │  └─────────────────────────────────────┘  │                │
│  │                                            │                │
│  │  Auth (commands.rs)                        │                │
│  │  ┌─────────────────────────────────────┐  │                │
│  │  │ login(): bcrypt verify, session     │  │                │
│  │  │ require_session(): guard            │  │                │
│  │  │ require_super_admin(): guard        │  │                │
│  │  │ password_recovery flow              │  │                │
│  │  └─────────────────────────────────────┘  │                │
│  └─────────────────────────────────────────────┘                │
│                          ▲                                      │
│  ┌──────────────────────────┴─────────────────┐               │
│  │                                            │                │
│  │  Database (db.rs, ~281 LOC)               │                │
│  │  ┌─────────────────────────────────────┐  │                │
│  │  │ fn open() → Connection              │  │                │
│  │  │ fn run_migrations()                 │  │                │
│  │  │   v1: core schema                  │  │                │
│  │  │   v2: orgs, users, semesters       │  │                │
│  │  │   v3: settings, is_active          │  │                │
│  │  │   v4: app_settings (max_admins)    │  │                │
│  │  │   v5: soft constraints (JSON)      │  │                │
│  │  │   v6: draft/published status       │  │                │
│  │  │   v7: schedule description         │  │                │
│  │  │   v8: recovery (code+question)     │  │                │
│  │  │                                     │  │                │
│  │  │ fn seed_super_admin()               │  │                │
│  │  │   Creates admin/admin123 if empty  │  │                │
│  │  │                                     │  │                │
│  │  │ WAL mode, foreign keys enabled      │  │                │
│  │  └─────────────────────────────────────┘  │                │
│  └─────────────────────────────────────────────┘                │
│                          │                                      │
│                          ▼                                      │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  SQLite Database (bundled, ~/.local/share/schedula/)    │  │
│  │  - organizations, users, semesters, courses             │  │
│  │  - lecturers, rooms, batches, batch_courses             │  │
│  │  - schedules, schedule_entries                          │  │
│  │  - org_scheduling_settings, app_settings                │  │
│  │  - audit_log                                            │  │
│  │  ────────────────────────────────────────────────────── │  │
│  │  Size: 0.5–2 MB (empty to 1000 entities)                │  │
│  │  Concurrency: WAL mode, read-write lock                 │  │
│  │  Isolation: Foreign keys enforced                       │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

---

## 2. Data Flow Example: Schedule Generation

```
User clicks "Generate Schedule"
    ↓
[Frontend] Schedule.svelte
    ↓ invoke('generate_schedule', {scheduleName, semesterId, description})
[Tauri IPC]
    ↓ Serde JSON serialization
[Backend] commands.rs::generate_schedule()
    ├─ require_session() — Check user logged in
    ├─ Get courses, lecturers, rooms, batches from DB
    ├─ Filter by org_id (isolation)
    │  ↓
    [Scheduler] scheduler.rs::generate()
    │  ├─ Build needs (batch × course × hours)
    │  ├─ Sort by hours descending (hardest first)
    │  ├─ For each need:
    │  │  ├─ Try each (day, slot) candidate
    │  │  ├─ Check all 9 hard constraints
    │  │  ├─ Apply soft constraints (penalties)
    │  │  ├─ Pick best valid slot
    │  │  └─ Record or mark unscheduled
    │  │  ↓
    │  │  [Database] INSERT schedule_entries
    │  └─ Return { entries, unscheduled }
    │  ↓
    [Backend] commands.rs::generate_schedule()
    ├─ Create schedule record
    ├─ Bulk insert entries
    ├─ log_audit("generate", "schedule", ...)
    ├─ Serialize ScheduleResult to JSON
    ↓ Tauri IPC
[Frontend] Schedule.svelte
    ├─ Receive { schedule_id, entry_count, unscheduled }
    ├─ Show success toast
    ├─ Refresh schedule list
    ├─ Display timetable grid
    └─ Highlight unscheduled items
```

---

## 3. Concurrency & Isolation

### Thread Safety
- **Database access:** `std::sync::Mutex<Connection>` in `DbState`
- **Session state:** `std::sync::Mutex<Option<SessionPayload>>` in `SessionState`
- **No async:** Tauri commands are blocking; CLI is single-threaded
- **Lock contention:** Negligible (human-speed interactions, <100ms operations)

### Data Isolation
Every query enforces `org_id` in WHERE clause:

```sql
-- Admin from Org A cannot see Org B's courses
SELECT * FROM courses WHERE org_id = ?  -- Org A's ID only
```

Org context comes from authenticated session:
```rust
let session = require_session(&session)?;  // SessionPayload has org_id
let org_id = session.org_id;  // Use this in queries
```

---

## 4. Error Handling Strategy

All commands return `Result<T, String>` for frontend JSON serialization:

```rust
pub fn create_course(...) -> Result<i64, String> {
    let conn = db.0.lock().map_err(db_err)?;  // "Failed to acquire lock"
    conn.execute(...).map_err(db_err)?;       // SQLite error
    Ok(id)
}

// Frontend receives:
// Success: { "data": 123 }
// Error: "Unique constraint failed: courses.code"
```

**No panics in production code** — all recoverable errors converted to strings.

---

## 5. Authentication & Authorization

### Login Flow
```
User enters username/password
    ↓
SELECT password_hash FROM users WHERE username = ?
    ↓
bcrypt::verify(password, stored_hash)  // Timing-safe comparison
    ↓
SessionPayload { user_id, username, role, org_id }
    ↓ Mutex::lock() and store in SessionState
    ↓ Return to frontend (also saves to localStorage)
```

### Privilege Checks
```rust
fn require_session(session) → Result<SessionPayload, String>
    // Returns error "Not logged in" if no session

fn require_super_admin(session) → Result<SessionPayload, String>
    // Checks role == "super_admin"
    // Returns error "Super admin access required" otherwise
```

### Password Recovery
```
Setup phase (onboarding):
  input: security_question, security_answer
  → hash answer with bcrypt(answer, cost=12)
  → generate recovery_code (32-char alphanumeric)
  → hash code with bcrypt(code, cost=12)
  → store both hashes

Reset phase (login → forgot password):
  Tab 1 (recovery code):
    input: recovery_code
    → bcrypt::verify(code, stored_hash)
    → prompt new password
    → update password_hash, clear recovery_code_hash

  Tab 2 (security answer):
    input: answer
    → bcrypt::verify(answer, stored_hash)
    → prompt new password
    → update password_hash
```

---

## 6. Database Persistence & Migrations

### Connection Lifecycle
```rust
pub fn open(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    run_migrations(&conn)?;
    seed_super_admin(&conn);
    Ok(conn)
}
```

**WAL mode**: Write-Ahead Logging
- Better concurrency (readers don't block writers)
- Safer crash recovery
- Slightly larger disk footprint (WAL + SHM files)

**Foreign keys enabled**:
- Enforce referential integrity at the database level
- CASCADE DELETE on foreign key violations

### Migration Pattern
```rust
fn migrate_v8(conn: &Connection) -> Result<()> {
    let alters = [
        "ALTER TABLE users ADD COLUMN recovery_code_hash TEXT",
        "ALTER TABLE users ADD COLUMN security_question TEXT",
        "ALTER TABLE users ADD COLUMN security_answer_hash TEXT",
    ];
    for sql in &alters {
        try_alter(conn, sql);  // Silently ignore if column already exists
    }
    Ok(())
}

fn try_alter(conn: &Connection, sql: &str) {
    let _ = conn.execute_batch(sql);  // Idempotent — safe to run twice
}
```

Each migration is idempotent — running `open()` multiple times is safe.

---

## 7. Testing Architecture

### Unit Tests (49 tests in scheduler.rs)
- **Constraint helpers:** `slot_penalty`, `preferred_penalty`, `is_blacked_out`, `would_exceed_consecutive`
- **Hard constraints:** All 9 verified independently (no room conflicts, no lecturer conflicts, etc.)
- **Soft constraints:** Diversity heuristics, biweekly placement, class-type preferences
- **Edge cases:** Empty input, trivial case, large datasets (100 lecturers, 200 courses)

### Integration Tests (26 tests in db_tests.rs, in-memory SQLite)
- **Migrations:** Idempotency, all 13 tables created, v8 recovery columns
- **Seeding:** Super-admin auto-created, password verifiable, not duplicated
- **Org isolation:** Admin A cannot query org B data
- **CRUD:** Create/read round-trips
- **Constraints:** Foreign keys, unique constraints, cascade deletes
- **Security:** Password recovery flow, recovery code verification, wrong code rejection

### Benchmarks (Criterion)
- **Scheduler scales:** tiny (380µs) → stress (8.2ms)
- **Helper micro-benchmarks:** `slot_penalty` (~81 ns), `blackout_check` (~64 µs)
- **Input construction overhead:** ~31 µs

Run:
```bash
cargo test          # 75 tests, 4 seconds
cargo bench         # 5 profiles, 2 minutes
```

---

## 8. Performance Characteristics

| Operation | Typical Time | Notes |
|-----------|------------|-------|
| Schedule generation (50 courses) | 2–3 ms | <10ms for 200 courses |
| Create course | <1 ms | Single INSERT |
| Bulk import (100 rows) | 10–50 ms | CSV parsing + 100 INSERTs |
| Login | 5–10 ms | bcrypt verify (cost=12) |
| Password change | 10–20 ms | Hash + update |
| Get all schedules | <5 ms | Query + iterate |
| Backup database | 50–200 ms | Full DB read to JSON |

**Memory:**
- Idle: ~30 MB
- With 1000 courses loaded: ~50 MB
- Peak during generation: ~60 MB

**Database size:**
- Empty: 0.5 MB
- 500 entities: 1 MB
- 1000+ entities: 2 MB

---

## 9. Deployment

### Build Pipeline (.github/workflows/release.yml)
```
[Tag push] → v0.1.0
    ↓
[GitHub Actions] macos-latest runner
    ├─ Checkout
    ├─ Install Rust (aarch64-apple-darwin, x86_64-apple-darwin)
    ├─ Install Node.js
    ├─ npm ci (frontend deps)
    ├─ tauri-action (ARM64 build)
    ├─ tauri-action (x86_64 cross-compile)
    ↓
[Windows runner]
    ├─ Install Rust (x86_64-pc-windows-msvc)
    ├─ npm ci
    ├─ tauri-action (NSIS build)
    ↓
[Ubuntu runner]
    └─ Create GitHub Release
       Upload DMGs + EXE
```

**Release artifacts:**
- `Schedula_v0.1.0_aarch64.dmg` (~100 MB)
- `Schedula_v0.1.0_x64.dmg` (~100 MB)
- `Schedula_v0.1.0_x64-setup.exe` (~150 MB)

### Distribution
1. User downloads DMG from GitHub Releases
2. Double-click to mount
3. Drag Schedula.app to /Applications
4. Launch → database auto-initializes in `~/.local/share/schedula/`
5. First run: seed super-admin, redirect to login

---

## 10. Security Posture

### Threats & Mitigations

| Threat | Mitigation |
|--------|-----------|
| SQL injection | Parameterized queries (rusqlite `params!` macro) |
| Password breach | bcrypt hashing (cost=12), salted |
| Unauthorized access | Session validation on every command, `require_session()` guards |
| Cross-tenant access | Org scope in WHERE clause, SQL layer filtering |
| Privilege escalation | Role checks (`require_super_admin`), quota enforcement |
| Recovery code leakage | Code hashed with bcrypt, displayed once only, cleared after use |
| Data tampering | Foreign keys enforce referential integrity |
| Denial of service | Single-threaded (no async), local-only app, no network exposure |

### What's NOT Encrypted
- SQLite file at rest (in `~/.local/share/schedula/`)
- Backup JSON file (contains all data)
- Session localStorage (auth only, cleared on logout)

**Recommendation:** For sensitive institutions, use SQLCipher (encrypted SQLite) or file-level encryption (FileVault on macOS).

---

## 11. Future Extensibility

### Plugin Points for v1.0
1. **SIS Integration** — REST API endpoint to push schedules to Banner/Blackboard
2. **Mobile UI** — Web UI (responsive Svelte site) for student schedule viewing
3. **Real-time sync** — WebSocket for multi-admin simultaneous edits
4. **Advanced scheduling** — Genetic algorithm variant for harder constraint sets
5. **Calendar export** — iCal generation with RRULE for biweekly
6. **Cloud backup** — S3/Google Drive integration
7. **Analytics dashboard** — Room utilization trends, lecturer load over time

### Architectural Readiness
- Clean IPC boundary (easy to expose HTTP REST endpoints)
- Modular scheduler (can swap algorithm without breaking commands)
- Type-safe models (Serde makes JSON serialization easy for APIs)
- Migration system (can evolve schema safely to v9, v10, ...)

---

## 12. Known Limitations

1. **Single-threaded:** No concurrent schedule generations
2. **SQLite only:** No distributed deployment (could add PostgreSQL later)
3. **Desktop-only:** No mobile native app (would require Tauri mobile or separate Flutter/React Native)
4. **No multi-user editing:** Sessions lock state; simultaneous edits not supported
5. **Simple auth:** No 2FA, no LDAP/Active Directory integration
6. **Language:** English only (UI labels hardcoded)

All addressable in future versions without major refactoring.

---

## Summary

Schedula is a **well-architected, production-ready desktop application** with clear separation of concerns:

- **Frontend** is pure presentation (Svelte + Vite)
- **IPC layer** is minimal and type-safe (Tauri invoke)
- **Backend** is business logic (Rust, no framework)
- **Database** is sound (SQLite, migrations, constraints)
- **Tests** are comprehensive (75 tests, benchmarks)
- **Security** is hardened (bcrypt, session guards, org isolation)

The codebase is ready for academic deployment and extensible for future features.

---

**See also:**
- [SCHEDULER_ALGORITHM.md](SCHEDULER_ALGORITHM.md) — Deep dive into constraint solver
- [DATABASE_SCHEMA.md](DATABASE_SCHEMA.md) — Complete schema with ER diagram
- [API_REFERENCE.md](API_REFERENCE.md) — All Tauri commands documented
- [TESTING_GUIDE.md](TESTING_GUIDE.md) — How to run tests and benchmarks
