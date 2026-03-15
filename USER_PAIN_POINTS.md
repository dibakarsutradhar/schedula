## 🧨 Problem Detail 1: Student Class Clashes

### 🔴 Pain Point

Students are often enrolled in multiple courses across departments (e.g., electives or shared foundation courses). Manual scheduling doesn’t account for:

- Overlapping classes at the same time
- Back-to-back classes in distant buildings
- Missing prerequisite courses due to time conflicts

### ❗ Results in:

- Missed or skipped classes
- Poor academic performance
- Student dissatisfaction
- Additional burden on academic departments to revise schedules

---

## ✅ Solution in Schedula

### 🧠 1. **Student-Centric Conflict Detection**

During routine generation:

- System checks if any batch or group of students is assigned to multiple classes in the same time slot
- Conflicts are flagged as hard constraints and resolved during optimization

> ✅ Hard Constraint: A student group can only be in one class at a time.
> 

---

### ⚙️ 2. **Batch-Course Mapping Input**

In your data upload or form:

- Allow admins to map **which batches/students are enrolled in which courses**
- Support overlapping student groups across departments or electives

```
Course Code, Enrolled Batches
CSE101, CSE-2A, CSE-2B
EEE201, CSE-2A, EEE-2A

```

---

### 🧩 3. **Constraint Engine Handles:**

- Prevent same students from being scheduled in overlapping classes
- Prioritize minimizing idle gaps while maintaining no clashes
- Allow soft preferences like keeping student’s day under 5 hours

---

### 🛠 4. **Manual Override + Real-time Validation**

When admins manually adjust schedules:

- System warns about newly introduced student-level conflicts
- Option to auto-resolve or suggest alternative slots

---

### 📊 5. **Clash Report View**

- Visual report of:
    - Which students/batches are affected
    - Which courses conflict
    - Suggested alternative arrangements

---

## ✨ Future Enhancement (Post-MVP)

- Smart elective conflict resolver
- Class travel time between buildings
- Student feedback loop: flag conflicts they face after initial schedule release

---

## 🧨 Problem Detail 2: Lecturer Overload and Availability Conflicts

### 🔴 Pain Point

Academic departments often assign courses to lecturers without fully accounting for:

- Their **daily/weekly workload limits**
- **Availability windows** (some may only be available on certain days)
- Faculty teaching **multiple departments or programs**
- No enforcement of **maximum classes per day**

This leads to:

- Burnout and dissatisfaction among faculty
- Unbalanced workloads (some overworked, others underutilized)
- Class cancellations or frequent rescheduling
- Poor teaching quality due to fatigue or lack of prep time

---

## ✅ Solution in Schedula

### 🧠 1. **Lecturer Availability & Load Constraints**

During data input:

- Admins define each lecturer’s **available days/times**
- Set **maximum classes per day/week**
- Mark cross-department responsibilities if applicable

Example:

```
Lecturer, Availability, Max Per Day, Max Per Week
Dr. Ayesha, Mon-Wed (9am–2pm), 2, 6

```

---

### ⚙️ 2. **Balanced Load Distribution in Engine**

- Scheduling engine respects lecturer constraints as **hard rules**
- Soft preference to spread load across the week instead of back-to-back
- Suggests auto-reassignment if limits are breached

---

### 🛠 3. **Workload Report & Calendar View**

- Visual overview of each lecturer’s week
- Color-coded indicators: balanced, overloaded, under-assigned
- Alerts for any manual override that breaks a constraint

---

### 📊 4. **Smart Suggestions**

- Recommend alternate time slots or available substitute lecturers
- Enable efficient redistribution of underutilized teaching capacity

---

## 🧨 Problem Detail 3: Inefficient Room Utilization & Resource Wastage

### 🔴 Pain Point

Universities often struggle with poor room utilization due to:

- Lack of visibility into room usage across departments
- Double-booking or leaving large rooms underused
- Scheduling lab sessions in regular classrooms (or vice versa)
- Wasting premium spaces for small groups

This leads to:

- Inefficient space usage
- Increased administrative burden to reshuffle rooms
- Students attending lectures in inappropriate environments

---

## ✅ Solution in Schedula

### 🧠 1. **Room Metadata Input**

Rooms can be categorized by:

- **Type** (Lab, Lecture, Seminar, Auditorium)
- **Capacity**
- **Availability** (days/times)

Example:

```
Room, Type, Capacity, Availability
R101, Lecture, 50, Mon-Fri
L203, Lab, 30, Mon-Thu

```

---

### ⚙️ 2. **Room Assignment Optimization**

- The engine automatically:
    - Matches room type with course requirement
    - Selects rooms with the most appropriate capacity (not too large, not too small)
    - Prevents double-booking

> ✅ Hard constraint: One room = One class per time slot
> 

---

### 📊 3. **Room Utilization Dashboard**

- View room usage stats: occupancy rate, idle hours, overused slots
- Heatmaps to visualize space efficiency
- Suggestions for maximizing underused rooms or adjusting schedule layout

---

### 🛠 4. **Conflict Alerts & Reallocation**

- Warn if lab classes are scheduled in non-lab rooms
- Suggest available alternatives automatically
- Optional priority rules (e.g., prioritize core courses for best rooms)
