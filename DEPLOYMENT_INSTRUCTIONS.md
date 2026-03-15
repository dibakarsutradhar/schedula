# 🚀 Deployment Instructions — Getting Schedula on GitHub

Everything is ready! Follow these steps to publish Schedula on GitHub with automated DMG releases.

---

## Step 1: Push to GitHub

### If you haven't created a GitHub repository yet:

```bash
# Create a new repository on GitHub (empty, no README)
# Then run:

git remote add origin https://github.com/yourusername/schedula.git
git branch -M main
git push -u origin main
```

### If you already have a GitHub repo:

```bash
# Update the remote if needed
git remote set-url origin https://github.com/yourusername/schedula.git

# Push all commits and tags
git push origin main
git push origin --tags
```

---

## Step 2: Verify GitHub Actions Workflow

1. Go to your repository: `https://github.com/yourusername/schedula`
2. Click **"Actions"** tab
3. You should see `.github/workflows/release.yml` is active
4. ✅ If you see it listed, GitHub Actions is configured correctly

---

## Step 3: Create Your First Release

```bash
# Create a version tag (format: v*.*.*) and push it
git tag v0.1.0
git push origin v0.1.0
```

### What happens next:

1. ✅ GitHub Actions workflow starts automatically
2. ✅ Installs dependencies (Node.js, Rust, Tauri)
3. ✅ Builds the Tauri app for macOS
4. ✅ Creates a GitHub Release with the DMG file
5. ✅ Generates release notes automatically

### Monitor the build:

- Go to: `https://github.com/yourusername/schedula/actions`
- Click the workflow run for `v0.1.0`
- Watch the progress in real-time
- Build typically takes 10–15 minutes

---

## Step 4: Download & Test

Once the build completes:

1. Go to: `https://github.com/yourusername/schedula/releases`
2. Find the **"Schedula v0.1.0"** release
3. Download the **`Schedula.dmg`** file
4. Test it locally:
   ```bash
   open Schedula.dmg
   # Drag app to Applications, test functionality
   ```

---

## Step 5: Edit Release Notes (Optional)

The workflow auto-generates nice release notes, but you can customize them:

1. Go to the release page
2. Click the **pencil icon** (Edit)
3. Update the description if needed
4. Check **"Latest release"** checkbox
5. Click **"Update release"**

Example note to add:

```markdown
## What's New in v0.1.0

🎉 **Initial Release** — MVP of Schedula

### Features
- AI-powered schedule generation with 7 hard constraints
- Multi-organization support with role-based access
- Diverse class scheduling (spread across weekdays)
- Biweekly course support
- Semester calendars with exam/study blocks
- Dark/light theme with custom accent colors
- Settings page for customization
- JSON backup and CSV export

### What You Can Do
- ✅ Create organizations, semesters, courses, lecturers, rooms
- ✅ Organize student batches
- ✅ Generate conflict-free schedules
- ✅ View in 3 formats (grid, list, semester calendar)
- ✅ Export to CSV
- ✅ Manage users and permissions

### Requirements
- macOS 13.0 or later
- ~200 MB disk space

### Getting Started
1. Download and mount the DMG
2. Drag Schedula.app to Applications
3. Launch and login (default: admin/admin123)
4. Follow the 7-step setup in the [README](README.md)

See [PROJECT_STATUS.md](PROJECT_STATUS.md) for complete feature list and stats.
```

---

## Step 6: Share the Release

Users can now download Schedula from:

```
https://github.com/yourusername/schedula/releases
```

You can share:
- **Direct DMG link**: `https://github.com/yourusername/schedula/releases/download/v0.1.0/Schedula.dmg`
- **Release page**: `https://github.com/yourusername/schedula/releases`
- **Repository**: `https://github.com/yourusername/schedula`

---

## Future Releases

For v0.2.0 and beyond:

```bash
# Update version in:
# - src-tauri/Cargo.toml (version = "0.2.0")
# - src-tauri/tauri.conf.json (productVersion: "0.2.0")

# Commit and tag:
git add src-tauri/Cargo.toml src-tauri/tauri.conf.json
git commit -m "chore(release): bump to v0.2.0"
git tag v0.2.0
git push origin main --tags

# GitHub Actions builds automatically! 🚀
```

See [RELEASE_GUIDE.md](RELEASE_GUIDE.md) for detailed instructions.

---

## Troubleshooting

### GitHub Actions workflow doesn't start

**Check**: Repository has `.github/workflows/release.yml` committed
```bash
git ls-files | grep workflows
# Should show: .github/workflows/release.yml
```

**Solution**: Make sure you've committed and pushed the file:
```bash
git add .github/workflows/release.yml
git commit -m "ci: add GitHub Actions release workflow"
git push origin main
```

### Build fails on GitHub Actions

1. Check the Actions tab for error logs
2. Common issues:
   - **Rust toolchain**: Workflow installs automatically (should be fine)
   - **Icon files**: Check `src-tauri/icons/` exists with PNG files
   - **Version syntax**: Tag must be `v*.*.*` format (e.g., `v0.1.0`)

**Solution**: Fix the issue locally, commit, and try the tag again:
```bash
git tag --delete v0.1.0
git push origin :v0.1.0
# Fix the issue...
git commit -m "fix: [what was broken]"
git tag v0.1.0
git push origin v0.1.0
```

### DMG file is too large or empty

If the file seems wrong:
1. Check Actions logs for build errors
2. Try building locally: `npm run tauri build`
3. Check output at: `src-tauri/target/release/bundle/macos/Schedula.dmg`
4. If it works locally, retry the GitHub Actions build

---

## What Users See

When they visit your release page, they'll see:

```
📦 Schedula v0.1.0

AI-Powered University Timetable Generator

[Release notes auto-generated]

Assets:
📥 Schedula.dmg (65 MB)
   ↳ Ready to download
```

They can:
1. Click **"Schedula.dmg"** to download
2. Follow installation instructions in README
3. Get a fully functional desktop app on their Mac

---

## Next Steps

1. ✅ **Push to GitHub**: `git push origin main --tags`
2. ✅ **Create tag**: `git tag v0.1.0 && git push origin v0.1.0`
3. ✅ **Wait for build**: Check Actions tab (10–15 min)
4. ✅ **Download & test**: Verify DMG works
5. ✅ **Share link**: `https://github.com/yourusername/schedula/releases`

---

## Success Checklist

- [ ] GitHub repository created
- [ ] Code pushed to `main` branch
- [ ] `.github/workflows/release.yml` exists in repo
- [ ] Tag created: `git tag v0.1.0`
- [ ] Tag pushed: `git push origin v0.1.0`
- [ ] GitHub Actions workflow completed successfully
- [ ] DMG file visible on release page
- [ ] DMG file tested locally
- [ ] Release notes visible and clear
- [ ] Users can download and run the app ✅

---

## Documentation for Users

Share these files with users:

- **README.md** — How to install and use Schedula
- **RELEASE_GUIDE.md** — For future releases
- **CONTRIBUTING.md** — For developers who want to contribute

All are in the repository and automatically included in releases.

---

## Questions?

- 📚 **Setup help**: See [README.md](README.md#-installation--setup)
- 🔧 **Build issues**: Check [Troubleshooting](#troubleshooting)
- 🤝 **Contributing**: See [CONTRIBUTING.md](CONTRIBUTING.md)
- 💬 **More help**: Open a GitHub Discussion

---

## Congratulations! 🎉

Schedula is ready for distribution. Users worldwide can now:

1. Download a single DMG file
2. Drag it to Applications
3. Start generating conflict-free timetables

**You've built something awesome.** Ship it! 🚀

---

**Made with ❤️ for academics worldwide**
