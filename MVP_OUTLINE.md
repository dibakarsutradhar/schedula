**Product Name**: *Schedula* (AI-Powered University Routine Generator)

**Owner**: @dibakar 

**Date**: 30 July 2025

**Stage**: MVP (Minimum Viable Product)

---

## 🎯 1. Core Objective

To build a SaaS platform that enables universities and colleges to automatically generate **conflict-free**, **optimized** semester routines for students, faculty, and departments — saving time, preventing scheduling clashes, and improving academic logistics. read more [**Project Overview**](https://www.notion.so/Project-Overview-240feb51244581e5bcb2f4f822dfe4f1?pvs=21) 

---

## ❗️2. Key Problems and Solutions

<aside>
💡

read more about the problems here [Pain Points](https://www.notion.so/Pain-Points-240feb51244580958e6ccb9b752b0a48?pvs=21) 

</aside>

### 🧨 Problem 1: Student Class Clashes

**Issue**: Students enrolled in multiple courses often face overlapping classes or timing conflicts, especially with electives or shared core subjects.

✅ **Solution**:

- System ensures no student is assigned to two classes at the same time.
- Batch-course mappings are processed to build a student-course graph.
- Conflicts are treated as **hard constraints** during schedule generation.

---

### 🧨 Problem 2: Lecturer Availability and Overload

**Issue**: Lecturers are often double-booked, over-assigned, or scheduled outside their availability, leading to cancellations, burnout, and uneven workload distribution.

✅ **Solution**:

- Allow input of lecturer availability and max class limits.
- Optimize schedules to balance teaching loads and avoid clashes.
- Visual reports show teaching distribution and alert on overload.

---

### 🧨 Problem 3: Room Misuse and Underutilization

**Issue**: Institutions often double-book rooms, assign labs to theory classes, or use large halls for small groups, wasting physical resources.

✅ **Solution**:

- Match rooms to class size and type (lab/theory).
- Prevent overlapping use of rooms.
- Show utilization heatmaps and suggest better room usage.

---

## 🧩 3. MVP Features

### 🎛️ 3.1 Admin Dashboard

- Add/manage semesters
- Upload data via CSV or manual form:
    - Courses (code, hours/week, room type)
    - Lecturers (availability, max load)
    - Rooms (type, capacity, availability)
    - Student batches (semester-wise, enrolled courses)

### ⚙️ 3.2 AI-Powered Scheduler Engine

- Conflict-free schedule generation using optimization algorithms (e.g. Google OR-Tools)
- Constraints enforced:
    - No student, teacher, or room overlap
    - Respect teacher availability
    - Room capacity/type match
    - Limit daily/weekly lecturer load
    - Minimize student idle time

### 📅 3.3 Timetable Viewer

- Calendar view (weekly) by:
    - Department
    - Lecturer
    - Student batch
- Color-coded, interactive, with filter/search

### 🚨 3.4 Conflict Detector

- Real-time validation of:
    - Student overlaps
    - Room double-bookings
    - Lecturer overloads or unavailability
- Reports with suggested auto-fixes

### 📤 3.5 Export & Sharing

- Export generated routines to PDF/Excel
- Shareable links for read-only public access (e.g. for students)

---

## 👥 4. User Roles

| Role | Permissions |
| --- | --- |
| Admin | Full access: upload, generate, edit, export |
| Lecturer | View only (personal timetable) |
| Student | View only (batch-level timetable) |

---

## 🛠️ 5. Tech Stack (Recommended)

| Layer | Stack |
| --- | --- |
| Frontend | Next.js + Tailwind CSS |
| Backend | Node.js or Python (FastAPI) |
| DB | PostgreSQL (via Supabase or self-hosted) |
| Scheduler | Google OR-Tools (constraint solver) |
| Auth | Supabase Auth or Clerk |
| Deployment | Vercel (frontend), Railway/Supabase (backend) |

---

## ✅ 6. Success Metrics

| Goal | KPI |
| --- | --- |
| Conflict-free schedule generation | 95%+ conflict-free on first run |
| Time savings | Reduce planning time by 90% |
| Accuracy of faculty load balancing | <5% deviation from set max loads |
| Room utilization | 80%+ average use of available slots |

---

## 📅 7. Timeline (6 Weeks MVP Build)

| Phase | Duration | Focus Area |
| --- | --- | --- |
| Week 1 | Planning | Data schema, constraints, UI mockups |
| Week 2–3 | Core Build | Uploader, database, scheduler engine |
| Week 4 | Viewer & Export | Calendar UI, conflict reports, PDF export |
| Week 5 | Testing | Real data from 1–2 universities |
| Week 6 | Launch Prep | Docs, demo, onboarding |

---

## 🔮 8. Post-MVP Feature Ideas

- Drag-and-drop timetable editor
- Exam routine scheduler
- Calendar sync (Google, Outlook)
- Real-time notification of changes
- Substitute teacher scheduling
- Student feedback loop to improve next-semester plans

---
