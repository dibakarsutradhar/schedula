# Contributing to Schedula

Thank you for your interest in contributing to Schedula! We welcome contributions from the community. This guide will help you get started.

## 🎯 Code of Conduct

Be respectful, inclusive, and professional. We're building this for academics — let's set a good example.

## 🚀 Getting Started

### 1. Fork & Clone

```bash
git clone https://github.com/yourusername/schedula.git
cd schedula
```

### 2. Install Dependencies

```bash
npm install
```

### 3. Start Development

```bash
npm run tauri dev
```

This launches:
- Vite dev server at `http://localhost:5173`
- Tauri desktop app with hot reload
- Both frontend and backend changes rebuild automatically

### 4. Make Your Changes

## 📝 Commit Guidelines

We use **conventional commits** for clear, scoped changes. Format:

```
<type>(<scope>): <subject>

<body>

Co-Authored-By: Your Name <your.email@example.com>
```

### Types

- **feat** — New feature (e.g., `feat(ui): add dark theme toggle`)
- **fix** — Bug fix (e.g., `fix(scheduler): resolve biweekly placement`)
- **docs** — Documentation (e.g., `docs(readme): add API reference`)
- **refactor** — Code restructuring (e.g., `refactor(commands): extract org filter logic`)
- **test** — Testing (e.g., `test(scheduler): add constraint validation tests`)
- **chore** — Internal (e.g., `chore(deps): update tauri to 2.0`)
- **style** — Code style (e.g., `style(css): improve dark theme colors`)
- **perf** — Performance (e.g., `perf(scheduler): optimize sort algorithm`)

### Scope

Prefix with the area:
- `ui` — Frontend/UI changes
- `scheduler` — Scheduling algorithm
- `db` — Database operations
- `api` — API commands
- `settings` — Settings functionality
- `docs` — Documentation
- `deps` — Dependencies

### Examples

```
feat(scheduler): add biweekly course support

- Added week_parity field to schedule_entries
- Implemented alternating week logic in SemesterCalendar
- Updated slot calculation for biweekly courses (ceil(hours/2))

Co-Authored-By: Jane Doe <jane@example.com>
```

```
fix(api): correct org_id filtering for non-super-admins

Previously admins could see all organizations. Now properly
scoped to their assigned organization.

Co-Authored-By: John Smith <john@example.com>
```

## 🔍 Code Review Guidelines

When submitting a pull request:

1. **Test your changes** — Run `npm run tauri dev` and manually test
2. **Check for errors** — Run `cargo check` in `src-tauri/`
3. **Follow conventions** — Match existing code style
4. **Update docs** — If adding features, update README
5. **Keep commits clean** — Use conventional format, no merge commits
6. **Explain your approach** — Write a clear PR description

### What reviewers look for:

- ✅ **Correctness** — Does it solve the problem?
- ✅ **Tests** — Are scheduling constraints properly validated?
- ✅ **Performance** — Does it handle large datasets?
- ✅ **Code quality** — Is it readable and maintainable?
- ✅ **Documentation** — Are complex algorithms explained?

## 🎨 Frontend Changes

### Svelte Components

- Use `<script>`, `<template>`, and `<style>` blocks
- Prefer reactive declarations (`$:`) over computed properties
- Keep components focused on one responsibility
- Use stores (`session`, `prefs`) for shared state

### Styling

- Use CSS variables for theming (`--bg`, `--accent`, etc.)
- Support both dark and light themes
- Mobile-responsive is nice but desktop-first is fine

### State Management

- `src/lib/stores/session.js` — Authentication
- `src/lib/stores/prefs.js` — User preferences (theme, colors)
- Svelte writable stores with localStorage persistence

## 🦀 Backend Changes

### Rust Code

- Run `cargo check` before committing
- Follow Rust naming conventions (snake_case)
- Comment complex constraint logic
- Use type-safe abstractions (Result, Option)

### Adding Commands

1. Define the function in `commands.rs` with `#[tauri::command]`
2. Add input/output types to `models.rs`
3. Register in `lib.rs` via `tauri::generate_handler![]`
4. Add Tauri invoke wrapper in `src/lib/api.js`
5. Update `src/views/*.svelte` to call the command

### Database Changes

1. Create a new migration function (e.g., `migrate_v4`)
2. Add to `db::open()` in proper sequence
3. Use `try_alter()` for backward compatibility
4. Document schema changes in `models.rs` comments

### Scheduler Constraints

The constraint solver enforces 7 hard constraints:

1. No student conflicts
2. Room capacity matches batch size
3. Room type matches course type (lab courses → lab rooms)
4. Lecturer availability (day + max hours)
5. Teaching hours fulfilled per course
6. Lecturer max hours/day and /week
7. Class type time preference (labs → afternoon, tutorials → morning)

If modifying the scheduler, ensure:
- All constraints remain enforceable
- Performance stays acceptable (< 5s for typical inputs)
- Unscheduled items with clear error reasons
- Diversity heuristics still spread classes across weekdays

## 📋 Testing Checklist

Before submitting a PR, test:

- [ ] App builds with `npm run tauri build`
- [ ] Dev server runs with `npm run tauri dev`
- [ ] Scheduling works with test data
- [ ] Conflict detection catches overlaps
- [ ] Settings save and persist
- [ ] Theme toggle works
- [ ] Logout and re-login works
- [ ] CSV export has correct data
- [ ] No console errors in dev tools

## 🐛 Bug Reports

Include:

1. **Description** — What's the bug?
2. **Reproduce** — Steps to trigger it
3. **Expected** — What should happen
4. **Actual** — What actually happens
5. **Environment** — macOS version, Schedula version
6. **Screenshots** — If applicable

Example:

```
## Bug: Biweekly courses show on all weeks

When I generate a schedule with biweekly courses,
the Semester Calendar view shows them on every week
instead of alternating weeks.

### Reproduce
1. Create a biweekly course (Freq: Biweekly)
2. Generate a schedule
3. Click "Semester Calendar" tab

### Expected
Biweekly classes appear on alternating teaching weeks

### Actual
Biweekly classes appear on every week

### Environment
- macOS 14.3
- Schedula 0.1.0

### Screenshots
[Attached: screenshot.png]
```

## 💡 Feature Requests

Include:

1. **Problem** — What pain point does this solve?
2. **Solution** — How should it work?
3. **Alternatives** — Other approaches considered?
4. **Impact** — Who benefits?

Example:

```
## Feature: Room scheduling preferences

### Problem
Some rooms are better for labs (tables, sinks) but we have
no way to strongly prefer them. Right now the scheduler
treats all labs the same, leading to inefficient room use.

### Solution
Add a "preferred_room_type" field to courses:
- Lecture rooms: default
- Lab tables (for chemistry)
- Lab sinks (for biology)
- Outdoor (for PE)

### Impact
Better room utilization, happier lecturers
```

## 📚 Documentation Updates

When adding features:

1. **README** — Add to Features section or Usage Guide
2. **Inline comments** — Explain complex logic
3. **API reference** — Document new commands
4. **Examples** — Show how to use if not obvious

## 🚢 Release Process

Only maintainers can create releases, but here's how it works:

```bash
# 1. Update version in src-tauri/Cargo.toml
# 2. Update version in src-tauri/tauri.conf.json
# 3. Commit version bump
# 4. Create git tag and push
git tag v0.1.0
git push origin v0.1.0

# GitHub Actions automatically:
# - Builds macOS DMG
# - Creates GitHub release
# - Uploads artifacts
```

Users download from [Releases](https://github.com/yourusername/schedula/releases)

## 📖 Architecture Resources

- `README.md` — Overview and usage
- `CONTRIBUTING.md` — This file
- Inline comments in `src-tauri/src/scheduler.rs` — Constraint logic
- `src-tauri/src/models.rs` — Database schema documentation

## 🤔 Need Help?

- **Questions** — Start a [discussion](https://github.com/yourusername/schedula/discussions)
- **Bugs** — [Open an issue](https://github.com/yourusername/schedula/issues)
- **Chat** — Check existing discussions first

## ✨ Recognition

Contributors will be:
- Listed in `CONTRIBUTORS.md` (coming soon)
- Credited in release notes
- Our eternal gratitude 🙏

---

Happy coding! 🚀
