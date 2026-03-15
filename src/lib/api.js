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
export const generateSchedule    = (scheduleName, semesterId = null) => invoke('generate_schedule', { scheduleName, semesterId })
export const getSchedules        = ()              => invoke('get_schedules')
export const getScheduleEntries  = (scheduleId)   => invoke('get_schedule_entries', { scheduleId })
export const activateSchedule    = (id)            => invoke('activate_schedule', { id })
export const deleteSchedule      = (id)            => invoke('delete_schedule', { id })
export const exportScheduleCsv   = (scheduleId)   => invoke('export_schedule_csv', { scheduleId })

// Dashboard
export const getStats = () => invoke('get_stats')
