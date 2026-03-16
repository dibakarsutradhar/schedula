import { invoke } from '@tauri-apps/api/core'

// Auth
export const login           = (req)                => invoke('login', { req })
export const logout          = ()                   => invoke('logout')
export const getSession      = ()                   => invoke('get_session')
export const hasUsers        = ()                   => invoke('has_users')
export const changePassword  = (oldPassword, newPassword) => invoke('change_password', { oldPassword, newPassword })

// Users
export const getUsers    = ()          => invoke('get_users')
export const createUser  = (user)      => invoke('create_user', { user })
export const deleteUser  = (id)        => invoke('delete_user', { id })

// Organizations
export const getOrganizations   = ()          => invoke('get_organizations')
export const createOrganization = (org)       => invoke('create_organization', { org })
export const updateOrganization = (id, org)   => invoke('update_organization', { id, org })
export const deleteOrganization = (id)        => invoke('delete_organization', { id })

// Semesters
export const getSemesters   = (orgIdFilter = null) => invoke('get_semesters', { orgIdFilter })
export const createSemester = (sem)                => invoke('create_semester', { sem })
export const updateSemester = (id, sem)            => invoke('update_semester', { id, sem })
export const deleteSemester = (id)                 => invoke('delete_semester', { id })

// Courses
export const getCourses    = ()           => invoke('get_courses')
export const createCourse  = (course)     => invoke('create_course', { course })
export const updateCourse  = (id, course) => invoke('update_course', { id, course })
export const deleteCourse  = (id)         => invoke('delete_course', { id })

// Lecturers
export const getLecturers   = ()              => invoke('get_lecturers')
export const createLecturer = (lecturer)      => invoke('create_lecturer', { lecturer })
export const updateLecturer = (id, lecturer)  => invoke('update_lecturer', { id, lecturer })
export const deleteLecturer = (id)            => invoke('delete_lecturer', { id })

// Rooms
export const getRooms    = ()         => invoke('get_rooms')
export const createRoom  = (room)     => invoke('create_room', { room })
export const updateRoom  = (id, room) => invoke('update_room', { id, room })
export const deleteRoom  = (id)       => invoke('delete_room', { id })

// Batches
export const getBatches   = ()            => invoke('get_batches')
export const createBatch  = (batch)       => invoke('create_batch', { batch })
export const updateBatch  = (id, batch)   => invoke('update_batch', { id, batch })
export const deleteBatch  = (id)          => invoke('delete_batch', { id })

// Scheduler
export const generateSchedule    = (scheduleName, semesterId = null, description = null) => invoke('generate_schedule', { scheduleName, semesterId, description })
export const getSchedules        = ()              => invoke('get_schedules')
export const getScheduleEntries  = (scheduleId)   => invoke('get_schedule_entries', { scheduleId })
export const activateSchedule    = (id)            => invoke('activate_schedule', { id })
export const deleteSchedule      = (id)            => invoke('delete_schedule', { id })
export const exportScheduleCsv   = (scheduleId)   => invoke('export_schedule_csv', { scheduleId })

// Dashboard
export const getStats = () => invoke('get_stats')

// Settings
export const updateDisplayName        = (newName)             => invoke('update_display_name', { newName })
export const adminResetPassword       = (userId, newPassword) => invoke('admin_reset_password', { userId, newPassword })
export const setUserActive            = (userId, active)      => invoke('set_user_active', { userId, active })
export const getSchedulingSettings    = (orgId)               => invoke('get_scheduling_settings', { orgId })
export const upsertSchedulingSettings = (settings)            => invoke('upsert_scheduling_settings', { settings })
export const clearSchedules           = ()                    => invoke('clear_schedules')
export const backupDatabase           = ()                    => invoke('backup_database')
export const getAppInfo               = ()                    => invoke('get_app_info')

// Admin quota
export const getMaxAdmins   = ()      => invoke('get_max_admins')
export const setMaxAdmins   = (max)   => invoke('set_max_admins', { max })
export const getAdminCount  = ()      => invoke('get_admin_count')

// Utilization & editing
export const getUtilizationReport  = (scheduleId)       => invoke('get_utilization_report', { scheduleId })
export const updateScheduleEntry   = (entryId, req)     => invoke('update_schedule_entry', { entryId, req })

// Schedule status
export const publishSchedule         = (id) => invoke('publish_schedule', { id })
export const revertScheduleToDraft   = (id) => invoke('revert_schedule_to_draft', { id })

// Audit log
export const getAuditLog = (limit = 100) => invoke('get_audit_log', { limit })

// Bulk import
export const bulkImportLecturers = (rows) => invoke('bulk_import_lecturers', { rows })
export const bulkImportRooms     = (rows) => invoke('bulk_import_rooms', { rows })
export const bulkImportCourses   = (rows) => invoke('bulk_import_courses', { rows })

// Pre-flight / data health
export const getPreflightWarnings     = ()   => invoke('get_preflight_warnings')
export const getDataHealth            = ()   => invoke('get_data_health')
export const updateScheduleDescription = (id, description) => invoke('update_schedule_description', { id, description })

// Password recovery
export const setupRecovery                    = (req) => invoke('setup_recovery', { req })
export const resetPasswordWithRecoveryCode    = (req) => invoke('reset_password_with_recovery_code', { req })
export const resetPasswordWithSecurityAnswer  = (req) => invoke('reset_password_with_security_answer', { req })
export const getSecurityQuestion             = ()    => invoke('get_security_question')

// Approval workflow
export const createApprovalRequest  = (req)                           => invoke('create_approval_request', { req })
export const getMyApprovalStatus    = (username)                      => invoke('get_my_approval_status', { username })
export const getPendingApprovals    = ()                              => invoke('get_pending_approvals')
export const getApprovalCount       = ()                              => invoke('get_approval_count')
export const resolveApproval        = (id, approved, rejectionReason) => invoke('resolve_approval', { id, approved, rejectionReason })
