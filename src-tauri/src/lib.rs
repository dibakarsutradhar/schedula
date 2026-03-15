mod commands;
mod db;
mod models;
mod scheduler;

use commands::{DbState, SessionState};
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data dir");
            std::fs::create_dir_all(&data_dir).expect("Failed to create data dir");
            let db_path = data_dir.join("schedula.db");
            let conn = db::open(&db_path).expect("Failed to open database");
            app.manage(DbState(Mutex::new(conn)));
            app.manage(SessionState(Mutex::new(None)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Auth
            commands::login,
            commands::logout,
            commands::get_session,
            commands::has_users,
            // Users
            commands::get_users,
            commands::create_user,
            commands::delete_user,
            commands::change_password,
            // Organizations
            commands::get_organizations,
            commands::create_organization,
            commands::update_organization,
            commands::delete_organization,
            // Semesters
            commands::get_semesters,
            commands::create_semester,
            commands::update_semester,
            commands::delete_semester,
            // Courses
            commands::get_courses,
            commands::create_course,
            commands::update_course,
            commands::delete_course,
            // Lecturers
            commands::get_lecturers,
            commands::create_lecturer,
            commands::update_lecturer,
            commands::delete_lecturer,
            // Rooms
            commands::get_rooms,
            commands::create_room,
            commands::update_room,
            commands::delete_room,
            // Batches
            commands::get_batches,
            commands::create_batch,
            commands::update_batch,
            commands::delete_batch,
            // Scheduler
            commands::generate_schedule,
            commands::get_schedules,
            commands::get_schedule_entries,
            commands::activate_schedule,
            commands::delete_schedule,
            commands::export_schedule_csv,
            // Dashboard
            commands::get_stats,
            // Settings
            commands::update_display_name,
            commands::admin_reset_password,
            commands::set_user_active,
            commands::get_scheduling_settings,
            commands::upsert_scheduling_settings,
            commands::clear_schedules,
            commands::backup_database,
            commands::get_app_info,
            // Admin quota
            commands::get_max_admins,
            commands::set_max_admins,
            commands::get_admin_count,
            // Utilization & editing
            commands::get_utilization_report,
            commands::update_schedule_entry,
            // Schedule status
            commands::publish_schedule,
            commands::revert_schedule_to_draft,
            // Audit log
            commands::get_audit_log,
            // Bulk import
            commands::bulk_import_lecturers,
            commands::bulk_import_rooms,
            commands::bulk_import_courses,
            // Pre-flight / data health
            commands::get_preflight_warnings,
            commands::get_data_health,
            // Schedule notes
            commands::update_schedule_description,
            // Password recovery
            commands::setup_recovery,
            commands::reset_password_with_recovery_code,
            commands::reset_password_with_security_answer,
            commands::get_security_question,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
