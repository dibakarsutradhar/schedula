use serde::{Deserialize, Serialize};

// ─── Organization ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub id: i64,
    pub name: String,
    pub org_type: String,
    pub address: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewOrganization {
    pub name: String,
    pub org_type: String,
    pub address: Option<String>,
}

// ─── User / Auth ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub org_id: Option<i64>,
    pub org_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewUser {
    pub username: String,
    pub display_name: String,
    pub password: String,
    pub role: String,
    pub org_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPayload {
    pub user_id: i64,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub org_id: Option<i64>,
}

// ─── Semester ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Semester {
    pub id: i64,
    pub org_id: i64,
    pub org_name: Option<String>,
    pub name: String,
    pub start_date: String,
    pub end_date: String,
    pub student_capacity: Option<i64>,
    pub teaching_weeks: i64,
    pub midterm_start: Option<String>,
    pub midterm_end: Option<String>,
    pub study_break_start: Option<String>,
    pub study_break_end: Option<String>,
    pub final_start: Option<String>,
    pub final_end: Option<String>,
    pub breaks_json: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct NewSemester {
    pub org_id: i64,
    pub name: String,
    pub start_date: String,
    pub end_date: String,
    pub student_capacity: Option<i64>,
    pub teaching_weeks: i64,
    pub midterm_start: Option<String>,
    pub midterm_end: Option<String>,
    pub study_break_start: Option<String>,
    pub study_break_end: Option<String>,
    pub final_start: Option<String>,
    pub final_end: Option<String>,
    pub breaks_json: String,
    pub status: String,
}

// ─── Course ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Course {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub hours_per_week: i64,
    pub room_type: String,
    pub class_type: String,   // lecture | lab | tutorial
    pub frequency: String,    // weekly | biweekly
    pub lecturer_id: Option<i64>,
    pub lecturer_name: Option<String>,
    pub org_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct NewCourse {
    pub code: String,
    pub name: String,
    pub hours_per_week: i64,
    pub room_type: String,
    pub class_type: String,
    pub frequency: String,
    pub lecturer_id: Option<i64>,
    pub org_id: Option<i64>,
}

// ─── Lecturer ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lecturer {
    pub id: i64,
    pub name: String,
    pub email: Option<String>,
    pub available_days: String,
    pub max_hours_per_day: i64,
    pub max_hours_per_week: i64,
    pub org_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct NewLecturer {
    pub name: String,
    pub email: Option<String>,
    pub available_days: String,
    pub max_hours_per_day: i64,
    pub max_hours_per_week: i64,
    pub org_id: Option<i64>,
}

// ─── Room ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub id: i64,
    pub name: String,
    pub capacity: i64,
    pub room_type: String,
    pub available_days: String,
    pub org_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct NewRoom {
    pub name: String,
    pub capacity: i64,
    pub room_type: String,
    pub available_days: String,
    pub org_id: Option<i64>,
}

// ─── Batch ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Batch {
    pub id: i64,
    pub name: String,
    pub department: String,
    pub semester: i64,
    pub size: i64,
    pub course_ids: Vec<i64>,
    pub org_id: Option<i64>,
    pub semester_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct NewBatch {
    pub name: String,
    pub department: String,
    pub semester: i64,
    pub size: i64,
    pub course_ids: Vec<i64>,
    pub org_id: Option<i64>,
    pub semester_id: Option<i64>,
}

// ─── Schedule ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleEntry {
    pub id: i64,
    pub schedule_id: i64,
    pub course_id: i64,
    pub course_code: String,
    pub course_name: String,
    pub class_type: String,
    pub frequency: String,
    pub week_parity: i64,
    pub lecturer_id: i64,
    pub lecturer_name: String,
    pub room_id: i64,
    pub room_name: String,
    pub batch_id: i64,
    pub batch_name: String,
    pub department: String,
    pub day: String,
    pub time_slot: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    pub id: i64,
    pub name: String,
    pub created_at: String,
    pub is_active: bool,
    pub entry_count: i64,
    pub semester_id: Option<i64>,
    pub semester_name: Option<String>,
}

// ─── Scheduling constants ─────────────────────────────────────────────────────

pub const DAYS: &[&str] = &["Mon", "Tue", "Wed", "Thu", "Fri"];
pub const TIME_SLOTS: &[i64] = &[0, 1, 2, 3, 4, 5, 6, 7];

pub fn slot_label(slot: i64) -> &'static str {
    match slot {
        0 => "08:00–09:00",
        1 => "09:00–10:00",
        2 => "10:00–11:00",
        3 => "11:00–12:00",
        4 => "13:00–14:00",
        5 => "14:00–15:00",
        6 => "15:00–16:00",
        7 => "16:00–17:00",
        _ => "Unknown",
    }
}

// ─── Conflict / unscheduled report ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnscheduledItem {
    pub batch_name: String,
    pub course_code: String,
    pub course_name: String,
    pub hours_needed: i64,
    pub reason: String,
}
