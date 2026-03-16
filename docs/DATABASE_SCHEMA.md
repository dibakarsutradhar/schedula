# Database Schema — Schedula

**Version:** v8 (8 migrations, backward compatible)
**Location:** `src-tauri/src/db.rs`
**Database:** SQLite 3.37+ (WAL mode, foreign keys enabled)

---

## Schema Evolution (v1 → v8)

### v1 — Core Scheduling
Added foundational tables for scheduling:
```sql
CREATE TABLE lecturers (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT,
    available_days TEXT DEFAULT 'Mon,Tue,Wed,Thu,Fri',
    max_hours_per_day INTEGER DEFAULT 4,
    max_hours_per_week INTEGER DEFAULT 16
);

CREATE TABLE courses (
    id INTEGER PRIMARY KEY,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    hours_per_week INTEGER DEFAULT 3,
    room_type TEXT CHECK(room_type IN ('lab','lecture'))
    lecturer_id INTEGER REFERENCES lecturers(id)
);

CREATE TABLE rooms (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    capacity INTEGER DEFAULT 30,
    room_type TEXT CHECK(room_type IN ('lab','lecture')),
    available_days TEXT DEFAULT 'Mon,Tue,Wed,Thu,Fri'
);

CREATE TABLE batches (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    department TEXT NOT NULL,
    semester INTEGER DEFAULT 1,
    size INTEGER DEFAULT 30
);

CREATE TABLE batch_courses (
    batch_id INTEGER REFERENCES batches(id) ON DELETE CASCADE,
    course_id INTEGER REFERENCES courses(id) ON DELETE CASCADE,
    PRIMARY KEY (batch_id, course_id)
);

CREATE TABLE schedules (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL,
    is_active INTEGER DEFAULT 0
);

CREATE TABLE schedule_entries (
    id INTEGER PRIMARY KEY,
    schedule_id INTEGER REFERENCES schedules(id) ON DELETE CASCADE,
    course_id INTEGER REFERENCES courses(id),
    lecturer_id INTEGER REFERENCES lecturers(id),
    room_id INTEGER REFERENCES rooms(id),
    batch_id INTEGER REFERENCES batches(id),
    day TEXT NOT NULL,
    time_slot INTEGER NOT NULL
);
```

### v2 — Multi-Tenancy & Rich Metadata
Added organizations, users, semesters. Made most tables multi-tenant:
```sql
CREATE TABLE organizations (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    org_type TEXT CHECK(org_type IN ('university','college','school','institute')),
    address TEXT,
    created_at TEXT DEFAULT datetime('now')
);

CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT CHECK(role IN ('super_admin','admin')),
    org_id INTEGER REFERENCES organizations(id) ON DELETE SET NULL,
    created_at TEXT DEFAULT datetime('now')
);

CREATE TABLE semesters (
    id INTEGER PRIMARY KEY,
    org_id INTEGER NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    start_date TEXT NOT NULL,
    end_date TEXT NOT NULL,
    student_capacity INTEGER,
    teaching_weeks INTEGER DEFAULT 14,
    midterm_start TEXT,
    midterm_end TEXT,
    study_break_start TEXT,
    study_break_end TEXT,
    final_start TEXT,
    final_end TEXT,
    breaks_json TEXT DEFAULT '[]',
    status TEXT CHECK(status IN ('planning','active','completed')),
    created_at TEXT DEFAULT datetime('now')
);

-- ALTER all v1 tables to add org_id, class_type, frequency columns
ALTER TABLE lecturers ADD COLUMN org_id INTEGER REFERENCES organizations(id);
ALTER TABLE courses ADD COLUMN org_id INTEGER REFERENCES organizations(id);
ALTER TABLE courses ADD COLUMN class_type TEXT DEFAULT 'lecture';
ALTER TABLE courses ADD COLUMN frequency TEXT DEFAULT 'weekly';
ALTER TABLE rooms ADD COLUMN org_id INTEGER REFERENCES organizations(id);
ALTER TABLE batches ADD COLUMN org_id INTEGER REFERENCES organizations(id);
ALTER TABLE batches ADD COLUMN semester_id INTEGER REFERENCES semesters(id);
ALTER TABLE schedules ADD COLUMN org_id INTEGER REFERENCES organizations(id);
ALTER TABLE schedules ADD COLUMN semester_id INTEGER REFERENCES semesters(id);
ALTER TABLE schedule_entries ADD COLUMN class_type TEXT DEFAULT 'lecture';
ALTER TABLE schedule_entries ADD COLUMN week_parity INTEGER DEFAULT 0;
```

### v3 — Scheduling Settings & User Status
Added per-org scheduling settings, user active flag, org contact email:
```sql
CREATE TABLE org_scheduling_settings (
    id INTEGER PRIMARY KEY,
    org_id INTEGER NOT NULL UNIQUE REFERENCES organizations(id) ON DELETE CASCADE,
    working_days TEXT DEFAULT 'Mon,Tue,Wed,Thu,Fri',
    day_start_slot INTEGER DEFAULT 0,
    day_end_slot INTEGER DEFAULT 7,
    slot_duration INTEGER DEFAULT 60,
    updated_at TEXT DEFAULT datetime('now')
);

ALTER TABLE users ADD COLUMN is_active INTEGER DEFAULT 1;
ALTER TABLE organizations ADD COLUMN contact_email TEXT;
```

### v4 — App-Wide Settings
Added global config table for admin quota:
```sql
CREATE TABLE app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Seed default max_admins = 2
INSERT OR IGNORE INTO app_settings (key, value) VALUES ('max_admins', '2');
```

### v5 — Lecturer Soft Constraints
Added preferred time-of-day, blackout slots, max consecutive hours:
```sql
ALTER TABLE lecturers ADD COLUMN preferred_slots_json TEXT;
-- {"Mon":"morning", "Tue":"afternoon", ...}

ALTER TABLE lecturers ADD COLUMN blackout_json TEXT;
-- [{"day":"Mon","slot":null}, {"day":"Fri","slot":7}, ...]

ALTER TABLE lecturers ADD COLUMN max_consecutive_hours INTEGER DEFAULT 3;
```

### v6 — Schedule Status & Audit Log
Added draft/published states to schedules, audit logging:
```sql
ALTER TABLE schedules ADD COLUMN status TEXT DEFAULT 'draft';
-- Backfill: UPDATE schedules SET status='published' WHERE is_active=1;

CREATE TABLE audit_log (
    id INTEGER PRIMARY KEY,
    user_id INTEGER,
    username TEXT DEFAULT 'system',
    action TEXT NOT NULL,  -- 'create', 'update', 'delete', 'generate', 'publish', 'import'
    entity_type TEXT NOT NULL,  -- 'lecturer', 'course', 'room', 'batch', 'user', 'schedule'
    entity_id INTEGER,
    details_json TEXT,
    created_at TEXT DEFAULT datetime('now')
);
```

### v7 — Schedule Notes
Added description field to schedules:
```sql
ALTER TABLE schedules ADD COLUMN description TEXT;
```

### v8 — Password Recovery (Latest)
Added recovery code and security question for admin account recovery:
```sql
ALTER TABLE users ADD COLUMN recovery_code_hash TEXT;
-- Stores bcrypt hash of 32-char alphanumeric recovery code

ALTER TABLE users ADD COLUMN security_question TEXT;
-- Stores the security question (e.g., "What is the name of your first pet?")

ALTER TABLE users ADD COLUMN security_answer_hash TEXT;
-- Stores bcrypt hash of the security answer (case-insensitive)
```

---

## Current Schema (v8)

### Organizations
```
organizations
├── id (INTEGER PRIMARY KEY)
├── name (TEXT NOT NULL)
├── org_type (TEXT) — 'university', 'college', 'school', 'institute'
├── address (TEXT)
├── contact_email (TEXT)
└── created_at (TEXT DEFAULT datetime('now'))
```

**Unique:** (none)
**Indexes:** name (recommended)

---

### Users
```
users
├── id (INTEGER PRIMARY KEY)
├── username (TEXT UNIQUE NOT NULL)
├── display_name (TEXT NOT NULL)
├── password_hash (TEXT NOT NULL) — bcrypt, cost=12
├── role (TEXT) — 'super_admin', 'admin' [CHECK]
├── org_id (INTEGER) → organizations(id) [SET NULL on delete]
├── is_active (INTEGER DEFAULT 1) — 0=deactivated
├── recovery_code_hash (TEXT) — bcrypt hash of 32-char code (or NULL)
├── security_question (TEXT) — question text (or NULL)
├── security_answer_hash (TEXT) — bcrypt hash of answer (or NULL)
└── created_at (TEXT DEFAULT datetime('now'))
```

**Constraints:**
- username UNIQUE
- role CHECK
- is_active: 0 or 1 (disabled/enabled)

**Recovery flow:**
- On first super-admin onboarding: all three recovery columns set
- On password reset via code: recovery_code_hash cleared (not reusable)
- On password reset via answer: nothing cleared (answer reusable)

---

### Semesters
```
semesters
├── id (INTEGER PRIMARY KEY)
├── org_id (INTEGER NOT NULL) → organizations(id) [CASCADE on delete]
├── name (TEXT NOT NULL) — e.g., "Fall 2025"
├── start_date (TEXT NOT NULL) — YYYY-MM-DD
├── end_date (TEXT NOT NULL)
├── student_capacity (INTEGER)
├── teaching_weeks (INTEGER DEFAULT 14)
├── midterm_start (TEXT)
├── midterm_end (TEXT)
├── study_break_start (TEXT)
├── study_break_end (TEXT)
├── final_start (TEXT)
├── final_end (TEXT)
├── breaks_json (TEXT DEFAULT '[]') — arbitrary break blocks
├── status (TEXT) — 'planning', 'active', 'completed' [CHECK]
└── created_at (TEXT)
```

**Indexes:** org_id, status

---

### Courses
```
courses
├── id (INTEGER PRIMARY KEY)
├── code (TEXT NOT NULL) — e.g., "CS-201"
├── name (TEXT NOT NULL)
├── hours_per_week (INTEGER DEFAULT 3)
├── room_type (TEXT) — 'lab', 'lecture' [CHECK]
├── class_type (TEXT DEFAULT 'lecture') — 'lecture', 'lab', 'tutorial'
├── frequency (TEXT DEFAULT 'weekly') — 'weekly', 'biweekly'
├── lecturer_id (INTEGER) → lecturers(id) [SET NULL on delete]
├── org_id (INTEGER) → organizations(id)
└── (no timestamp)
```

**Constraints:**
- room_type CHECK
- frequency: 'weekly' or 'biweekly'
- lecturer_id optional (NULL = unassigned)

---

### Lecturers
```
lecturers
├── id (INTEGER PRIMARY KEY)
├── name (TEXT NOT NULL)
├── email (TEXT)
├── available_days (TEXT DEFAULT 'Mon,Tue,Wed,Thu,Fri') — comma-separated
├── max_hours_per_day (INTEGER DEFAULT 4)
├── max_hours_per_week (INTEGER DEFAULT 16)
├── org_id (INTEGER)
├── preferred_slots_json (TEXT) — {"Mon":"morning", "Tue":"afternoon", ...}
├── blackout_json (TEXT) — [{"day":"Mon","slot":null}, {"day":"Fri","slot":7}]
├── max_consecutive_hours (INTEGER DEFAULT 3) — 0 = unlimited
└── (no timestamp)
```

**JSON formats:**
- **preferred_slots_json:** `{"Mon":"morning", "Tue":"afternoon", "Wed":"morning"}` — values are "morning" or "afternoon"
- **blackout_json:** `[{"day":"Mon","slot":null}, {"day":"Fri","slot":7}]` — slot:null = entire day

---

### Rooms
```
rooms
├── id (INTEGER PRIMARY KEY)
├── name (TEXT NOT NULL) — e.g., "A-101"
├── capacity (INTEGER DEFAULT 30)
├── room_type (TEXT) — 'lab', 'lecture' [CHECK]
├── available_days (TEXT DEFAULT 'Mon,Tue,Wed,Thu,Fri')
├── org_id (INTEGER)
└── (no timestamp)
```

---

### Batches
```
batches
├── id (INTEGER PRIMARY KEY)
├── name (TEXT NOT NULL) — e.g., "CSE-2A"
├── department (TEXT NOT NULL)
├── semester (INTEGER DEFAULT 1) — legacy field (deprecated in favor of semester_id)
├── size (INTEGER DEFAULT 30) — student count
├── course_ids (Computed from batch_courses table)
├── org_id (INTEGER)
├── semester_id (INTEGER) → semesters(id)
└── (no timestamp)
```

---

### Batch-Courses (M:N relationship)
```
batch_courses
├── batch_id (INTEGER) → batches(id) [CASCADE on delete]
├── course_id (INTEGER) → courses(id) [CASCADE on delete]
└── PRIMARY KEY (batch_id, course_id)
```

---

### Schedules
```
schedules
├── id (INTEGER PRIMARY KEY)
├── name (TEXT NOT NULL) — e.g., "Fall 2025 - Draft 1"
├── created_at (TEXT NOT NULL) — datetime('now')
├── is_active (INTEGER DEFAULT 0) — 0 or 1 (legacy; see status)
├── status (TEXT DEFAULT 'draft') — 'draft', 'published' [CHECK]
├── entry_count (INTEGER) — count of schedule_entries
├── semester_id (INTEGER) → semesters(id) [optional, for filtering]
├── org_id (INTEGER)
└── description (TEXT) — optional notes by admin
```

**Backfill (v6):** `UPDATE schedules SET status='published' WHERE is_active=1;`

---

### Schedule Entries
```
schedule_entries
├── id (INTEGER PRIMARY KEY)
├── schedule_id (INTEGER NOT NULL) → schedules(id) [CASCADE on delete]
├── course_id (INTEGER NOT NULL) → courses(id)
├── lecturer_id (INTEGER NOT NULL) → lecturers(id)
├── room_id (INTEGER NOT NULL) → rooms(id)
├── batch_id (INTEGER NOT NULL) → batches(id)
├── day (TEXT NOT NULL) — 'Mon', 'Tue', ..., 'Fri'
├── time_slot (INTEGER NOT NULL) — 0–7 (8 one-hour slots)
├── class_type (TEXT DEFAULT 'lecture') — 'lecture', 'lab', 'tutorial'
└── week_parity (INTEGER DEFAULT 0) — 0 = every week, 1 = odd weeks, 2 = even weeks
```

**Constraints:**
- schedule_id required (no orphaned entries)
- day: Mon–Fri
- time_slot: 0–7 (08:00–09:00 through 16:00–17:00)
- week_parity: 0 for weekly, 1 for biweekly

**Compound key:** (schedule_id, course_id, lecturer_id, room_id, batch_id, day, time_slot) should be unique (by design, not constraint).

---

### Org Scheduling Settings
```
org_scheduling_settings
├── id (INTEGER PRIMARY KEY)
├── org_id (INTEGER UNIQUE NOT NULL) → organizations(id) [CASCADE on delete]
├── working_days (TEXT DEFAULT 'Mon,Tue,Wed,Thu,Fri')
├── day_start_slot (INTEGER DEFAULT 0)
├── day_end_slot (INTEGER DEFAULT 7)
├── slot_duration (INTEGER DEFAULT 60) — minutes per slot
└── updated_at (TEXT DEFAULT datetime('now'))
```

---

### App Settings (Global)
```
app_settings
├── key (TEXT PRIMARY KEY) — e.g., 'max_admins'
└── value (TEXT NOT NULL) — e.g., '2'
```

**Current settings:**
- **max_admins:** Max admin (non-super) accounts allowed (default: 2)

---

### Audit Log
```
audit_log
├── id (INTEGER PRIMARY KEY)
├── user_id (INTEGER)
├── username (TEXT DEFAULT 'system')
├── action (TEXT NOT NULL) — 'create', 'update', 'delete', 'generate', 'publish', 'import'
├── entity_type (TEXT NOT NULL) — 'lecturer', 'course', 'room', 'batch', 'user', 'schedule'
├── entity_id (INTEGER)
├── details_json (TEXT) — arbitrary JSON context
└── created_at (TEXT DEFAULT datetime('now'))
```

**Example entries:**
```json
{
  "action": "create",
  "entity_type": "course",
  "entity_id": 42,
  "username": "admin",
  "details_json": "{\"code\": \"CS-201\", \"name\": \"Data Structures\"}"
}

{
  "action": "generate",
  "entity_type": "schedule",
  "entity_id": 5,
  "username": "admin",
  "details_json": "{\"entry_count\": 120, \"unscheduled\": 3}"
}
```

---

## Access Patterns

### Login
```sql
SELECT password_hash FROM users WHERE username = ?;
```

### Org Isolation (all queries scoped to org_id)
```sql
SELECT * FROM courses WHERE org_id = ?;
SELECT * FROM lecturers WHERE org_id = ?;
SELECT * FROM batches WHERE org_id = ?;
```

### Schedule Generation
```sql
SELECT courses.*, lecturers.* FROM courses
  LEFT JOIN lecturers ON courses.lecturer_id = lecturers.id
  WHERE courses.org_id = ?;

SELECT * FROM rooms WHERE org_id = ?;
SELECT * FROM batches WHERE org_id = ?;

-- After generation:
INSERT INTO schedules (name, created_at, is_active, status, org_id) ...;
INSERT INTO schedule_entries (schedule_id, ...) VALUES (...), (...), ...;  -- bulk
```

### Audit Log
```sql
INSERT INTO audit_log (user_id, username, action, entity_type, entity_id, details_json) ...;
SELECT * FROM audit_log ORDER BY created_at DESC LIMIT 100;
```

---

## Pragmas & Settings

```sql
PRAGMA journal_mode = WAL;         -- Write-Ahead Logging (better concurrency)
PRAGMA foreign_keys = ON;          -- Enforce referential integrity
PRAGMA busy_timeout = 5000;        -- Wait up to 5s if DB locked
```

---

## Migration Strategy

### Safe Alterations
```rust
fn try_alter(conn: &Connection, sql: &str) {
    let _ = conn.execute_batch(sql);  // Ignore "duplicate column" errors
}
```

Allows idempotent migrations:
```rust
fn migrate_v8(conn: &Connection) -> Result<()> {
    try_alter(conn, "ALTER TABLE users ADD COLUMN recovery_code_hash TEXT");
    // Safe to run multiple times
    Ok(())
}
```

### No Data Loss
All migrations use:
- `ALTER TABLE ... ADD COLUMN` (existing rows get NULL)
- `INSERT OR IGNORE` (skip if already exists)
- Backfill via `UPDATE` (preserve old data)

Never: DROP, TRUNCATE, or destructive ALTER.

---

## Indexes (Recommended, Not Applied)

For production deployments with 1000+ rows:

```sql
CREATE INDEX idx_users_org_id ON users(org_id);
CREATE INDEX idx_courses_org_id ON courses(org_id);
CREATE INDEX idx_lecturers_org_id ON lecturers(org_id);
CREATE INDEX idx_rooms_org_id ON rooms(org_id);
CREATE INDEX idx_batches_org_id ON batches(org_id);
CREATE INDEX idx_semesters_org_id ON semesters(org_id);
CREATE INDEX idx_schedules_org_id ON schedules(org_id);
CREATE INDEX idx_schedule_entries_schedule_id ON schedule_entries(schedule_id);
CREATE INDEX idx_batch_courses_course_id ON batch_courses(course_id);
CREATE INDEX idx_audit_log_created_at ON audit_log(created_at DESC);
```

Current codebase relies on query optimizer (few enough rows for table scans).

---

**See also:**
- [ARCHITECTURE.md](ARCHITECTURE.md) — System design
- [SCHEDULER_ALGORITHM.md](SCHEDULER_ALGORITHM.md) — How constraints stored/used
- [TESTING_GUIDE.md](TESTING_GUIDE.md) — DB test coverage
