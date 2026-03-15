# 📚 Schedula — AI-Powered University Timetable Generator

> Automated, conflict-free class schedule generation for universities and colleges. Built with Rust, Tauri, Svelte, and SQLite.

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![macOS](https://img.shields.io/badge/macOS-13.0+-brightgreen)

## 🎯 What is Schedula?

Schedula is a desktop application that intelligently generates semester class routines for academic institutions. Instead of spending weeks manually resolving scheduling conflicts, admins input their courses, lecturers, rooms, and student batches — and Schedula generates an optimized, conflict-free timetable in seconds.

### Problem It Solves
- ❌ Manual scheduling is time-consuming (dozens of hours per semester)
- ❌ Human errors lead to classroom/lecturer conflicts
- ❌ Student schedule clashes cause frustration
- ❌ No visibility across departments and roles

### Solution
✅ **Fully automated schedule generation** using constraint-based optimization algorithms
✅ **7 hard constraints** ensuring no conflicts
✅ **Smart diversification** spreading classes across weekdays
✅ **Support for diverse class types** (lectures, labs, tutorials)
✅ **Biweekly scheduling** for intensive courses
✅ **Semester calendars** with exam/study blocks
✅ **Multi-organization** support with role-based access

---

## ✨ Features

### Core Scheduling Engine
- 🤖 **Constraint-based optimization** with diversity heuristics
- 📅 **7 hard constraints**: no student conflicts, room capacity, lecturer availability, room type matching, teaching hours, lecturer max loads, class-type time preferences
- 🎲 **Diversity sorting** to spread classes throughout the week (Mon–Fri)
- 🔄 **Biweekly course support** with alternating week placement
- 📊 **Detailed conflict reports** showing unscheduled items with reasons

### User Interface
- 🎨 **Dark/Light theme toggle** with custom accent colors
- 📅 **Multiple schedule views**:
  - Weekly grid (Mon–Fri × 8 time slots)
  - Detailed list with class type badges
  - Semester calendar with teaching/exam/study blocks and biweekly visualization
- 📋 **Entity management** — Create and edit courses, lecturers, rooms, batches
- 💾 **CSV export** of generated schedules
- 🔍 **Semester management** with teaching weeks and exam/study block dates

### Multi-Tenancy & Roles
- 🏢 **Organizations** (universities, colleges, schools, institutes)
- 👤 **Role-based access**:
  - **Super Admin** — manage all organizations, create admin users
  - **Admin** — manage own organization's data and schedules
- 🔒 **Data isolation** — admins see only their org's data
- 👥 **User management** — create, deactivate, password reset

### Settings & Customization
- 🎨 **Appearance** — dark/light theme, accent color
- 👤 **Profile** — display name, password
- 👥 **User management** — activate/deactivate, admin password reset
- 🏢 **Organization settings** — contact email, address
- ⚙️ **Scheduling defaults** — working days, start/end slots, slot duration
- 💾 **Data management** — JSON backup download, clear all schedules
- ℹ️ **About** — app version, DB size, entity counts

### Data Management
- 📤 **CSV export** of entire schedules
- 💾 **JSON backup** of all data (downloadable)
- 🗑️ **Safe data clearing** with confirmation

---

## 🏗️ Architecture

### Tech Stack
- **Backend**: Rust + Tauri 2.x (desktop framework)
- **Frontend**: Svelte 4 + Vite 5
- **Database**: SQLite with bundled distribution
- **Authentication**: bcrypt password hashing
- **IPC**: Tauri command invocation system

### Layers

```
┌─────────────────────────────────────────┐
│  Frontend (Svelte Components)           │
│  - App.svelte (main router)             │
│  - Views (Dashboard, Settings, etc.)    │
│  - Stores (session, prefs)              │
│  - API layer (Tauri invoke wrappers)    │
└────────────────┬────────────────────────┘
                 │
        ┌────────▼─────────┐
        │  Tauri IPC       │
        │  invoke()        │
        └────────┬─────────┘
                 │
┌────────────────▼────────────────────────┐
│  Backend (Rust Commands)                │
│  - commands.rs (35+ handler functions)  │
│  - scheduler.rs (constraint solver)     │
│  - models.rs (domain types)             │
│  - db.rs (migrations, helpers)          │
└────────────────┬────────────────────────┘
                 │
        ┌────────▼──────────┐
        │  SQLite Database  │
        │  (bundled)        │
        └───────────────────┘
```

### Database Schema

**v3 (Current)**

Tables:
- `organizations` — schools, universities, institutes
- `users` — admin and super_admin accounts with bcrypt passwords
- `semesters` — semester definitions with teaching/exam/study blocks
- `courses` — courses with class type (lecture/lab/tutorial) and frequency (weekly/biweekly)
- `lecturers` — faculty with availability and load constraints
- `rooms` — classrooms and labs with capacities
- `batches` — student groups linked to semesters
- `batch_courses` — courses for each batch
- `schedules` — generated timetables (active/archived)
- `schedule_entries` — individual class slots with week parity (for biweekly)
- `org_scheduling_settings` — per-org scheduling defaults

---

## 📦 Installation & Setup

### System Requirements
- **macOS** 13.0 or later
- **Disk space**: ~200 MB

### Download & Run

1. **Download the DMG file** from [Releases](https://github.com/yourusername/schedula/releases)
2. **Mount the DMG**:
   ```bash
   open Schedula-0.1.0.dmg
   ```
3. **Drag Schedula.app to Applications**
4. **Launch** from Applications or Spotlight

### First Login
- **Username**: `admin`
- **Password**: `admin123`
- ⚠️ **Change this password immediately** in Settings → My Profile

---

## 🚀 Usage Guide

### 1. Set Up Your Organization

1. **Login** with admin credentials
2. **Navigate** to Settings → Organization
3. **Enter**:
   - Organization name (required)
   - Type (University, College, School, Institute)
   - Contact email
   - Address
4. **Save**

### 2. Define a Semester

1. **Go to** Semesters tab
2. **Click** "+ Add Semester"
3. **Fill in**:
   - Semester name (e.g., "Fall 2025")
   - Start and end dates
   - Teaching weeks (typically 14)
   - Exam block dates (midterm, finals)
   - Study break dates
4. **Save**

### 3. Add Lecturers

1. **Go to** Lecturers tab
2. **Click** "+ Add Lecturer"
3. **Enter**:
   - Full name
   - Email (optional)
   - Available days (Mon–Sat, clickable chips)
   - Max hours/day (e.g., 4)
   - Max hours/week (e.g., 16)
4. **Save**

### 4. Create Rooms

1. **Go to** Rooms tab
2. **Click** "+ Add Room"
3. **Configure**:
   - Room name (e.g., "A-101")
   - Type: Lecture or Lab
   - Capacity (e.g., 30 students)
   - Available days
4. **Save**

### 5. Define Courses

1. **Go to** Courses tab
2. **Click** "+ Add Course"
3. **Set**:
   - Course code (e.g., "CS-201")
   - Course name
   - Hours per week (e.g., 3)
   - Class type: Lecture, Lab, or Tutorial
   - Frequency: Weekly or Biweekly
   - Assigned lecturer (optional)
4. **Save**

Note: Labs auto-link to lab rooms; biweekly courses get half the weekly hours per placement

### 6. Organize Student Batches

1. **Go to** Batches tab
2. **Click** "+ Add Batch"
3. **Enter**:
   - Batch name (e.g., "CSE-2A")
   - Department
   - Semester year
   - Student count
   - Link to a Semester (optional)
   - Select enrolled courses
4. **Save**

### 7. Generate Schedule

1. **Go to** Schedule tab
2. **Enter** a schedule name (e.g., "Fall 2025 - Draft 1")
3. **Optionally** select a semester (filters batches)
4. **Click** "Generate Schedule"
5. **View results**:
   - Success: Shows entry count and any unscheduled items
   - Errors: Lists which courses/hours couldn't be scheduled and why

### 8. Review & Export

**Weekly Grid Tab**:
- Click days/times to see class details
- Color-coded by batch
- Shows lecturer, room, course code

**List Tab**:
- Sortable by batch, day, time
- Shows class type (lecture/lab/tutorial)
- Biweekly indicator

**Semester Calendar Tab** (if linked):
- Visual week-by-week view
- Color-coded by class type
- Biweekly classes shown on alternating weeks
- Exam/study blocks highlighted

**Export**:
- Click "📋 Export as CSV" to download timetable

---

## ⌨️ Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| **Cmd + ,** | Open Settings |
| **Escape** | Close any modal |

---

## 🎨 Customization

### Theme Preferences

**Settings → Appearance**:
- **Dark** — Default dark theme
- **Light** — Light theme for daytime use
- **System** — Follow macOS system preferences

### Accent Colors

8 preset colors + custom color picker:
- 🔵 Blue, Cyan, Green, Amber
- 🔴 Red, Pink, Purple, Orange

Preferences auto-save to `~/Library/Application Support/Schedula/`

---

## 🔐 Security & Privacy

- **Local-first**: All data stored in encrypted SQLite file on your machine
- **No cloud**: No data leaves your computer
- **Passwords**: Hashed with bcrypt, never stored in plain text
- **Sessions**: In-memory only, cleared on logout

---

## 🛠️ Development Setup

### Prerequisites
- **Rust** 1.70+ (install via [rustup](https://rustup.rs))
- **Node.js** 18+ and npm
- **Xcode Command Line Tools** (for macOS)
  ```bash
  xcode-select --install
  ```

### Clone & Install

```bash
git clone https://github.com/yourusername/schedula.git
cd schedula

# Install frontend dependencies
npm install

# Backend builds automatically with Tauri
```

### Development Server

```bash
npm run tauri dev
```

This launches:
- **Vite dev server** at `http://localhost:5173`
- **Tauri desktop app** pointing to it
- **Hot reload** for both frontend and backend changes

### Build Project

```bash
# Release build (optimized)
npm run tauri build

# Output: src-tauri/target/release/bundle/macos/Schedula.dmg
```

### Project Structure

```
schedula/
├── src/                    # Frontend (Svelte)
│   ├── App.svelte
│   ├── app.css            # Dark/light theme system
│   ├── views/             # Route components
│   ├── lib/
│   │   ├── api.js         # Tauri invoke wrappers
│   │   ├── stores/        # Svelte stores (session, prefs)
│   │   └── components/    # Reusable UI components
│   └── index.html
├── src-tauri/             # Backend (Rust)
│   ├── src/
│   │   ├── main.rs        # App entry (mobile boilerplate)
│   │   ├── lib.rs         # Tauri setup, command registration
│   │   ├── commands.rs    # 35+ command handlers
│   │   ├── db.rs          # Database, migrations
│   │   ├── models.rs      # Domain types, constants
│   │   └── scheduler.rs   # Constraint solver
│   ├── tauri.conf.json    # Tauri config
│   └── Cargo.toml         # Rust dependencies
├── vite.config.js         # Vite bundler config
├── package.json           # Node.js dependencies
└── README.md              # This file
```

---

## 📚 API Commands Reference

### Authentication
| Command | Params | Returns | Notes |
|---------|--------|---------|-------|
| `login` | `{username, password}` | `SessionPayload` | Sets in-memory session |
| `logout` | - | - | Clears session |
| `get_session` | - | `SessionPayload \| null` | Sync with Rust state |
| `change_password` | `{oldPassword, newPassword}` | - | Must be logged in |

### Users (Super Admin)
| Command | Params | Returns |
|---------|--------|---------|
| `get_users` | - | `User[]` |
| `create_user` | `NewUser` | `i64` (id) |
| `delete_user` | `{id}` | - |
| `admin_reset_password` | `{userId, newPassword}` | - |
| `set_user_active` | `{userId, active}` | - |

### Organizations (Super Admin)
| Command | Params | Returns |
|---------|--------|---------|
| `get_organizations` | - | `Organization[]` |
| `create_organization` | `NewOrganization` | `i64` (id) |
| `update_organization` | `{id, org}` | - |
| `delete_organization` | `{id}` | - |

### Semesters
| Command | Params | Returns |
|---------|--------|---------|
| `get_semesters` | `{orgIdFilter?}` | `Semester[]` |
| `create_semester` | `NewSemester` | `i64` (id) |
| `update_semester` | `{id, sem}` | - |
| `delete_semester` | `{id}` | - |

### Courses, Lecturers, Rooms, Batches
Standard CRUD operations:
- `get_*` → `*[]`
- `create_*` → `i64` (id)
- `update_*` → `-`
- `delete_*` → `-`

### Scheduling
| Command | Params | Returns |
|---------|--------|---------|
| `generate_schedule` | `{scheduleName, semesterId?}` | `{schedule_id, entry_count, unscheduled}` |
| `get_schedules` | - | `Schedule[]` |
| `get_schedule_entries` | `{scheduleId}` | `ScheduleEntry[]` |
| `activate_schedule` | `{id}` | - |
| `delete_schedule` | `{id}` | - |
| `export_schedule_csv` | `{scheduleId}` | `CSV string` |

### Settings
| Command | Params | Returns |
|---------|--------|---------|
| `update_display_name` | `{newName}` | - |
| `get_scheduling_settings` | `{orgId}` | `OrgSchedulingSettings` |
| `upsert_scheduling_settings` | `{settings}` | - |
| `clear_schedules` | - | `i64` (count cleared) |
| `backup_database` | - | `base64 string` |
| `get_app_info` | - | `AppInfo` |

### Dashboard
| Command | Params | Returns |
|---------|--------|---------|
| `get_stats` | - | `{courses, lecturers, rooms, batches, ...}` |

---

## 🐛 Troubleshooting

### App won't open
- Check that macOS version is 13.0+
- Try moving Schedula.app to `/Applications`
- Reset permissions: `sudo xattr -rd com.apple.quarantine /Applications/Schedula.app`

### Database locked error
- Ensure only one instance of Schedula is open
- Restart the app if you see this error

### Schedule generation fails
- Verify you have at least one course, lecturer, room, and batch
- Check that lecturers have compatible availability with course needs
- Review conflict report to see which items couldn't be scheduled and why

### Can't login
- Username and password are case-sensitive
- Default is `admin` / `admin123`
- Change in Settings → My Profile if forgotten (requires access to Settings by super admin)

---

## 📝 Git Commit History

The project uses conventional commits for clear, scoped changes:

```bash
# View recent changes
git log --oneline | head -20

# See details of a feature
git show <commit-hash>
```

**Commit types**:
- `feat` — new feature
- `fix` — bug fix
- `docs` — documentation
- `chore` — internal cleanup, dependency updates
- `refactor` — code restructuring without behavior change
- `test` — testing

---

## 📦 Building for Distribution

### Create DMG Release

```bash
# Build optimized binary
npm run tauri build

# Output DMG is at:
# src-tauri/target/release/bundle/macos/Schedula.dmg

# Optionally sign and notarize for macOS Gatekeeper
# (requires Apple Developer account)
```

### Upload to GitHub Releases

1. **Create a GitHub release**:
   ```bash
   gh release create v0.1.0 \
     --title "Schedula v0.1.0" \
     --notes "Initial release: AI-powered timetable generator"
   ```

2. **Upload DMG**:
   ```bash
   gh release upload v0.1.0 \
     src-tauri/target/release/bundle/macos/Schedula.dmg
   ```

Users can then download from: `https://github.com/yourusername/schedula/releases`

---

## 🤝 Contributing

Contributions are welcome! To contribute:

1. **Fork** the repository
2. **Create a feature branch**: `git checkout -b feat/your-feature`
3. **Make changes** with conventional commits
4. **Test** with `npm run tauri dev`
5. **Submit a pull request**

Guidelines:
- Follow existing code style
- Add comments for complex logic
- Test scheduling edge cases thoroughly
- Update README if adding features

---

## 📄 License

This project is licensed under the **MIT License** — see [LICENSE](LICENSE) for details.

---

## 🎓 Academic Use

Schedula is designed for and freely available to academic institutions. If you're using it at a university or college, please let us know! We'd love to hear about your experience.

---

## 🙏 Acknowledgments

Built with:
- [Tauri](https://tauri.app/) — Lightweight desktop apps
- [Svelte](https://svelte.dev/) — Reactive UI framework
- [Rust](https://www.rust-lang.org/) — Systems language
- [SQLite](https://www.sqlite.org/) — Embedded database
- The open-source community

---

## 📧 Support

For issues, questions, or feature requests:
- **Issues**: [GitHub Issues](https://github.com/yourusername/schedula/issues)
- **Discussions**: [GitHub Discussions](https://github.com/yourusername/schedula/discussions)

---

**Made with ❤️ for academics worldwide**
