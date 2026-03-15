---

**Product Name**: SchedulaAI

**Version**: MVP v1

**Owner**: @dibakar 

**Date**: 30 July 2025

---

## 📌 1. Overview

### Purpose

Schedula is a SaaS platform that enables universities, colleges, and schools to generate **conflict-free, optimized semester routines** for departments, lecturers, and students — minimizing manual work, errors, and inefficiencies.

### Target Users

- University academic administrators
- Department heads / schedulers
- Lecturers (view-only in MVP)
- Students (view-only in MVP)

### MVP Goal

Allow an academic admin to upload basic semester data and generate optimized class routines within minutes, with conflict detection and export features.

---

## 🚀 2. Goals & Success Metrics

| Goal | Success Metric |
| --- | --- |
| Generate optimized, conflict-free routines | 95% of test schedules are conflict-free |
| Simplify routine creation for admins | Reduce time from hours/days to under 10 minutes |
| Ensure usability with real university data | Onboard 3 institutions and get routine feedback |

---

## 🧩 3. Features (MVP Scope)

### 3.1. User Management & Roles

- **Admin**: Can upload data, generate, view, edit, and export routines
- **Lecturer**: View only
- **Student**: View only

> Authentication via email/password or magic link (e.g., Supabase Auth)
> 

---

### 3.2. Data Upload & Management

- Upload course, teacher, room, and student batch data via:
    - CSV
    - Manual form input (basic)
- Field mapping & validation step

**Data Types**:

- Courses (code, name, credit hours/week)
- Teachers (name, availability, max hours/day)
- Rooms (name, capacity, lab/theory type)
- Student batches (dept, semester, enrolled courses)

---

### 3.3. Scheduler Engine

- Generates weekly routine based on constraints:
    - No overlapping use of rooms or lecturers
    - Match course hours per week
    - Match room type (lab vs lecture)
    - Room capacity ≥ batch size
    - Honor teacher availability
    - Soft: minimize idle gaps for students
- Generates:
    - Department-level routine
    - Teacher-wise routine
    - Batch-wise routine

> Engine: Google OR-Tools or custom constraint solver
> 

---

### 3.4. Conflict Detection

- Detect conflicts:
    - Room double-booked
    - Teacher in two places at once
    - Batch scheduled for multiple classes at same time
- Display clear error messages and suggestions

---

### 3.5. Routine Viewer

- Interactive calendar-style display:
    - Per department, batch, and lecturer
- Weekly view (Mon–Fri/Sat)
- Hover to view class/course details

---

### 3.6. Export & Share

- Download as:
    - PDF
    - Excel
- Sharable links (read-only viewer mode)

---

## 🚫 4. Out of Scope (for MVP)

- Mobile app
- Student registration/attendance
- Exam routine scheduler
- Notifications
- AI learning from overrides
- Integration with external LMS or calendars

---

## 🛠️ 5. Tech Stack

| Layer | Tech |
| --- | --- |
| Frontend | React + Tailwind + Next.js |
| Backend | Node.js or Python (FastAPI) |
| Scheduler | Google OR-Tools / Custom optimizer |
| Database | PostgreSQL / Supabase DB |
| Auth | Supabase Auth or Clerk |
| File Upload | CSV parser (Papaparse) |
| Hosting | Vercel / Railway / Supabase |

---

## 🧪 6. Validation & Testing

- Test with real datasets from 2–3 departments
- Manual overrides to stress test engine
- Measure:
    - % of conflict-free routines
    - Admin feedback on ease of use
    - Time saved compared to manual process

---

## 🗓️ 7. Timeline (MVP Build)

| Phase | Duration | Tasks |
| --- | --- | --- |
| Week 1–2 | Planning | Finalize schema, define constraints, mock UI |
| Week 3–4 | Core Dev | Build uploader, scheduler, calendar UI |
| Week 5 | QA & Testing | Try with sample university data, fix issues |
| Week 6 | Feedback | Pilot test with real users, iterate based on input |
| Week 7 | Launch | Launch MVP on cloud, onboard initial users |

---

## 🔮 8. Future Enhancements (Post-MVP)

- Mobile app for students and teachers
- Auto-rescheduling with real-time conflict updates
- AI-based optimization learning from overrides
- Exam & invigilation routine generator
- Calendar integration (Google, Outlook)

---
