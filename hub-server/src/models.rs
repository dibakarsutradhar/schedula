use serde::{Deserialize, Serialize};

// ─── Organization ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub id: i64,
    pub name: String,
    pub org_type: String,
    pub address: Option<String>,
    pub contact_email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NewOrganization {
    pub name: String,
    pub org_type: String,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub contact_email: Option<String>,
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
    pub is_active: bool,
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
    // Soft constraints (v5)
    pub preferred_slots_json: Option<String>,   // {"Mon":"morning","Tue":"afternoon",...}
    pub blackout_json: Option<String>,          // [{"day":"Mon","slot":null},...]
    pub max_consecutive_hours: i64,
}

fn default_max_consecutive() -> i64 { 3 }

#[derive(Debug, Deserialize)]
pub struct NewLecturer {
    pub name: String,
    pub email: Option<String>,
    pub available_days: String,
    pub max_hours_per_day: i64,
    pub max_hours_per_week: i64,
    pub org_id: Option<i64>,
    #[serde(default)]
    pub preferred_slots_json: Option<String>,
    #[serde(default)]
    pub blackout_json: Option<String>,
    #[serde(default = "default_max_consecutive")]
    pub max_consecutive_hours: i64,
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
    pub status: String,          // 'draft' | 'published'
    pub entry_count: i64,
    pub semester_id: Option<i64>,
    pub semester_name: Option<String>,
    pub description: Option<String>,
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

// ─── Scheduling Settings ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgSchedulingSettings {
    pub org_id: i64,
    pub working_days: String,
    pub day_start_slot: i64,
    pub day_end_slot: i64,
    pub slot_duration: i64,
}

// ─── App Info ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    pub version: String,
    pub db_size_bytes: u64,
    pub user_count: i64,
    pub org_count: i64,
    pub schedule_count: i64,
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

// ─── Utilization report ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomUtilization {
    pub room_id: i64,
    pub room_name: String,
    pub room_type: String,
    pub capacity: i64,
    pub booked_slots: i64,
    pub total_available_slots: i64,
    pub utilization_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LecturerLoad {
    pub lecturer_id: i64,
    pub lecturer_name: String,
    pub scheduled_hours: i64,
    pub max_hours_per_week: i64,
    pub load_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtilizationReport {
    pub schedule_id: i64,
    pub schedule_name: String,
    pub rooms: Vec<RoomUtilization>,
    pub lecturer_loads: Vec<LecturerLoad>,
    pub total_entries: i64,
}

// ─── Manual schedule entry edit ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct UpdateScheduleEntryReq {
    pub day: String,
    pub time_slot: i64,
    pub room_id: i64,
}

// ─── Audit log ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: i64,
    pub user_id: Option<i64>,
    pub username: String,
    pub action: String,        // 'create' | 'update' | 'delete' | 'generate' | 'publish' | 'import'
    pub entity_type: String,   // 'lecturer' | 'course' | 'room' | 'batch' | 'user' | 'schedule'
    pub entity_id: Option<i64>,
    pub details_json: Option<String>,
    pub created_at: String,
}

// ─── Recovery / Password Reset ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoverySetup {
    pub recovery_code: String,  // displayed once to super-admin, must be written down
}

#[derive(Debug, Deserialize)]
pub struct SetupRecoveryRequest {
    pub security_question: String,
    pub security_answer: String,
}

#[derive(Debug, Deserialize)]
pub struct ResetPasswordWithCodeRequest {
    pub recovery_code: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
pub struct ResetPasswordWithAnswerRequest {
    pub security_answer: String,
    pub new_password: String,
}

// ─── Pre-flight / Data-health ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightWarning {
    pub severity: String,   // "error" | "warning"
    pub category: String,   // "courses" | "lecturers" | "rooms" | "batches"
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataHealth {
    pub courses_without_lecturers: i64,
    pub courses_without_matching_rooms: i64,
    pub batches_without_courses: i64,
    pub lecturers_unavailable: i64,   // available_days is empty
    pub total_warnings: i64,
}

// ─── Approval Requests ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: i64,
    pub requester_user_id: Option<i64>,
    pub requester_username: String,
    pub requester_display_name: String,
    pub request_type: String,              // 'password_reset' | 'account_unlock'
    pub status: String,                    // 'pending' | 'approved' | 'rejected' | 'expired'
    pub rejection_reason: Option<String>,
    pub resolver_display_name: Option<String>,
    pub created_at: String,
    pub resolved_at: Option<String>,
    pub expires_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateApprovalReq {
    pub username: String,
    pub request_type: String,
    pub new_password: Option<String>,      // required for 'password_reset'
}

// ─── Bulk CSV import ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CsvLecturer {
    pub name: String,
    pub email: Option<String>,
    pub available_days: String,
    pub max_hours_per_day: i64,
    pub max_hours_per_week: i64,
}

#[derive(Debug, Deserialize)]
pub struct CsvCourse {
    pub code: String,
    pub name: String,
    pub hours_per_week: i64,
    pub room_type: String,
    pub class_type: String,
    pub frequency: String,
    pub lecturer_email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CsvRoom {
    pub name: String,
    pub capacity: i64,
    pub room_type: String,
    pub available_days: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkImportResult {
    pub inserted: i64,
    pub skipped: i64,
    pub errors: Vec<String>,
}
