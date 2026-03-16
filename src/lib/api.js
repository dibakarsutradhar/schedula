import { invoke }   from '@tauri-apps/api/core'
import { get }      from 'svelte/store'
import { syncMode } from './stores/syncMode.js'

// ─── Core dispatcher ──────────────────────────────────────────────────────────
// In standalone mode  → invoke the Tauri command directly.
// In server mode      → HTTP fetch to the hub server with JWT bearer auth.

async function call(tauriCmd, tauriArgs, method, path, body) {
  const { mode, serverUrl, token } = get(syncMode)

  if (mode === 'server' && serverUrl) {
    const base = serverUrl.replace(/\/$/, '')
    const headers = { 'Content-Type': 'application/json' }
    if (token) headers['Authorization'] = `Bearer ${token}`

    const res = await fetch(`${base}${path}`, {
      method,
      headers,
      body: (body !== undefined && body !== null) ? JSON.stringify(body) : undefined,
    })

    if (!res.ok) {
      const err = await res.json().catch(() => ({ error: `HTTP ${res.status}` }))
      throw new Error(err.error || `Request failed: ${res.status}`)
    }

    const ct = res.headers.get('content-type') || ''
    if (ct.includes('application/json')) return res.json()
    if (ct.includes('text/'))            return res.text()
    return null
  }

  return invoke(tauriCmd, tauriArgs)
}

// ── Auth ──────────────────────────────────────────────────────────────────────

export async function login(req) {
  const { mode, serverUrl } = get(syncMode)

  if (mode === 'server' && serverUrl) {
    const base = serverUrl.replace(/\/$/, '')
    const res = await fetch(`${base}/api/auth/login`, {
      method:  'POST',
      headers: { 'Content-Type': 'application/json' },
      body:    JSON.stringify(req),
    })
    if (!res.ok) {
      const err = await res.json().catch(() => ({ error: 'Login failed' }))
      throw new Error(err.error || 'Login failed')
    }
    const data = await res.json()
    // Persist JWT so subsequent calls are authenticated
    syncMode.setToken(data.token)
    return data.session
  }

  return invoke('login', { req })
}

export function logout() {
  // Clear server token too
  syncMode.clearToken()
  return call('logout', {}, 'POST', '/api/auth/logout', null)
}

export const getSession  = ()  => call('get_session',  {}, 'GET',  '/api/auth/session',    undefined)
export const hasUsers    = ()  => call('has_users',    {}, 'GET',  '/api/auth/has-users',  undefined)

export const changePassword = (oldPassword, newPassword) =>
  call('change_password', { oldPassword, newPassword }, 'POST', '/api/users/change-password',
       { old_password: oldPassword, new_password: newPassword })

// ── Users ─────────────────────────────────────────────────────────────────────

export const getUsers   = ()          => call('get_users',    {},       'GET',    '/api/users',        undefined)
export const createUser = (user)      => call('create_user',  { user }, 'POST',   '/api/users',        user)
export const deleteUser = (id)        => call('delete_user',  { id },   'DELETE', `/api/users/${id}`,  undefined)

export const adminResetPassword = (userId, newPassword) =>
  call('admin_reset_password', { userId, newPassword }, 'POST', `/api/users/${userId}/password`,
       { new_password: newPassword })

export const setUserActive = (userId, active) =>
  call('set_user_active', { userId, active }, 'PUT', `/api/users/${userId}/active`, { active })

// ── Organizations ─────────────────────────────────────────────────────────────

export const getOrganizations   = ()          => call('get_organizations',   {},       'GET',    '/api/orgs',        undefined)
export const createOrganization = (org)       => call('create_organization', { org },  'POST',   '/api/orgs',        org)
export const updateOrganization = (id, org)   => call('update_organization', { id, org }, 'PUT', `/api/orgs/${id}`, org)
export const deleteOrganization = (id)        => call('delete_organization', { id },   'DELETE', `/api/orgs/${id}`,  undefined)

// ── Semesters ─────────────────────────────────────────────────────────────────

export const getSemesters   = (orgIdFilter = null) =>
  call('get_semesters', { orgIdFilter }, 'GET',
       orgIdFilter ? `/api/semesters?org_id=${orgIdFilter}` : '/api/semesters', undefined)

export const createSemester = (sem)       => call('create_semester', { sem },       'POST',   '/api/semesters',      sem)
export const updateSemester = (id, sem)   => call('update_semester', { id, sem },   'PUT',    `/api/semesters/${id}`, sem)
export const deleteSemester = (id)        => call('delete_semester', { id },        'DELETE', `/api/semesters/${id}`, undefined)

// ── Courses ───────────────────────────────────────────────────────────────────

export const getCourses   = ()              => call('get_courses',    {},              'GET',    '/api/courses',        undefined)
export const createCourse = (course)        => call('create_course',  { course },      'POST',   '/api/courses',        course)
export const updateCourse = (id, course)    => call('update_course',  { id, course },  'PUT',    `/api/courses/${id}`,  course)
export const deleteCourse = (id)            => call('delete_course',  { id },          'DELETE', `/api/courses/${id}`,  undefined)

// ── Lecturers ─────────────────────────────────────────────────────────────────

export const getLecturers   = ()                  => call('get_lecturers',    {},                 'GET',    '/api/lecturers',        undefined)
export const createLecturer = (lecturer)          => call('create_lecturer',  { lecturer },       'POST',   '/api/lecturers',        lecturer)
export const updateLecturer = (id, lecturer)      => call('update_lecturer',  { id, lecturer },   'PUT',    `/api/lecturers/${id}`,  lecturer)
export const deleteLecturer = (id)                => call('delete_lecturer',  { id },             'DELETE', `/api/lecturers/${id}`,  undefined)

// ── Rooms ─────────────────────────────────────────────────────────────────────

export const getRooms   = ()          => call('get_rooms',    {},           'GET',    '/api/rooms',       undefined)
export const createRoom = (room)      => call('create_room',  { room },     'POST',   '/api/rooms',       room)
export const updateRoom = (id, room)  => call('update_room',  { id, room }, 'PUT',    `/api/rooms/${id}`, room)
export const deleteRoom = (id)        => call('delete_room',  { id },       'DELETE', `/api/rooms/${id}`, undefined)

// ── Batches ───────────────────────────────────────────────────────────────────

export const getBatches   = ()              => call('get_batches',    {},              'GET',    '/api/batches',       undefined)
export const createBatch  = (batch)         => call('create_batch',   { batch },       'POST',   '/api/batches',       batch)
export const updateBatch  = (id, batch)     => call('update_batch',   { id, batch },   'PUT',    `/api/batches/${id}`, batch)
export const deleteBatch  = (id)            => call('delete_batch',   { id },          'DELETE', `/api/batches/${id}`, undefined)

// ── Scheduler ─────────────────────────────────────────────────────────────────

export const generateSchedule   = (scheduleName, semesterId = null, description = null) =>
  call('generate_schedule', { scheduleName, semesterId, description }, 'POST', '/api/schedules/generate',
       { schedule_name: scheduleName, semester_id: semesterId, description })

export const getSchedules       = ()             => call('get_schedules',       {},             'GET',    '/api/schedules',                  undefined)
export const getScheduleEntries = (scheduleId)   => call('get_schedule_entries', { scheduleId }, 'GET',   `/api/schedules/${scheduleId}/entries`, undefined)
export const activateSchedule   = (id)           => call('activate_schedule',   { id },         'PUT',    `/api/schedules/${id}/activate`,   null)
export const deleteSchedule     = (id)           => call('delete_schedule',     { id },         'DELETE', `/api/schedules/${id}`,            undefined)
export const exportScheduleCsv  = (scheduleId)   => call('export_schedule_csv', { scheduleId }, 'GET',   `/api/schedules/${scheduleId}/csv`, undefined)
export const publishSchedule    = (id)           => call('publish_schedule',    { id },         'PUT',    `/api/schedules/${id}/publish`,    null)
export const revertScheduleToDraft = (id)        => call('revert_schedule_to_draft', { id },    'PUT',    `/api/schedules/${id}/draft`,      null)

export const updateScheduleDescription = (id, description) =>
  call('update_schedule_description', { id, description }, 'PUT', `/api/schedules/${id}/description`, { description })

export const updateScheduleEntry = (entryId, req) =>
  call('update_schedule_entry', { entryId, req }, 'PUT', `/api/schedule-entries/${entryId}`, req)

// ── Dashboard ─────────────────────────────────────────────────────────────────

export const getStats = () => call('get_stats', {}, 'GET', '/api/stats', undefined)

// ── Settings ──────────────────────────────────────────────────────────────────

export const updateDisplayName        = (newName)   =>
  call('update_display_name',        { newName },   'PUT',  '/api/settings/display-name', { new_name: newName })

export const getSchedulingSettings    = (orgId)     =>
  call('get_scheduling_settings',    { orgId },     'GET',  `/api/settings/scheduling/${orgId}`, undefined)

export const upsertSchedulingSettings = (settings)  =>
  call('upsert_scheduling_settings', { settings },  'PUT',  '/api/settings/scheduling', settings)

export const clearSchedules           = ()           =>
  call('clear_schedules',            {},             'POST', '/api/settings/clear-schedules', null)

export const backupDatabase           = ()           =>
  call('backup_database',            {},             'GET',  '/api/settings/backup', undefined)

export const getAppInfo               = ()           =>
  call('get_app_info',               {},             'GET',  '/api/settings/app-info', undefined)

// ── Admin quota ───────────────────────────────────────────────────────────────

export const getMaxAdmins   = ()      => call('get_max_admins',   {},      'GET', '/api/settings/max-admins',   undefined)
export const setMaxAdmins   = (max)   => call('set_max_admins',   { max }, 'PUT', '/api/settings/max-admins',   { max })
export const getAdminCount  = ()      => call('get_admin_count',  {},      'GET', '/api/settings/admin-count',  undefined)

// ── Reports ───────────────────────────────────────────────────────────────────

export const getUtilizationReport = (scheduleId) =>
  call('get_utilization_report', { scheduleId }, 'GET', `/api/reports/utilization/${scheduleId}`, undefined)

export const getAuditLog = (limit = 100) =>
  call('get_audit_log', { limit }, 'GET', `/api/audit-log?limit=${limit}`, undefined)

// ── Bulk import ───────────────────────────────────────────────────────────────

export const bulkImportLecturers = (rows) => call('bulk_import_lecturers', { rows }, 'POST', '/api/import/lecturers', { rows })
export const bulkImportRooms     = (rows) => call('bulk_import_rooms',     { rows }, 'POST', '/api/import/rooms',     { rows })
export const bulkImportCourses   = (rows) => call('bulk_import_courses',   { rows }, 'POST', '/api/import/courses',   { rows })

// ── Pre-flight / data health ──────────────────────────────────────────────────

export const getPreflightWarnings      = ()   => call('get_preflight_warnings',      {}, 'GET', '/api/preflight',   undefined)
export const getDataHealth             = ()   => call('get_data_health',             {}, 'GET', '/api/data-health', undefined)

// ── Password recovery ─────────────────────────────────────────────────────────

export const setupRecovery                   = (req) => call('setup_recovery',                    { req }, 'POST', '/api/recovery/setup',               req)
export const resetPasswordWithRecoveryCode   = (req) => call('reset_password_with_recovery_code', { req }, 'POST', '/api/recovery/reset-with-code',      req)
export const resetPasswordWithSecurityAnswer = (req) => call('reset_password_with_security_answer', { req }, 'POST', '/api/recovery/reset-with-answer', req)
export const getSecurityQuestion             = ()    => call('get_security_question',              {},      'GET',  '/api/recovery/question',             undefined)

// ── Approval workflow ─────────────────────────────────────────────────────────

export const createApprovalRequest = (req)                            =>
  call('create_approval_request', { req }, 'POST', '/api/approvals', req)

export const getMyApprovalStatus   = (username)                       =>
  call('get_my_approval_status', { username }, 'GET', `/api/approvals/my/${username}`, undefined)

export const getPendingApprovals   = ()                               =>
  call('get_pending_approvals', {}, 'GET', '/api/approvals', undefined)

export const getApprovalCount      = ()                               =>
  call('get_approval_count', {}, 'GET', '/api/approvals/count', undefined)

export const resolveApproval       = (id, approved, rejectionReason) =>
  call('resolve_approval', { id, approved, rejectionReason }, 'PUT', `/api/approvals/${id}/resolve`,
       { approved, rejection_reason: rejectionReason })
