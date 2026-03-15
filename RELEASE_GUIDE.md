# 🚀 Release Guide — Publishing Schedula to GitHub

This guide explains how to build the app and publish it as a downloadable DMG file on GitHub Releases.

## Prerequisites

1. **GitHub account** with the `schedula` repository
2. **Git** installed locally
3. **All changes committed** to the `main` branch

## Step-by-Step Release Process

### 1. Update Version Numbers

Before releasing, update the version in two places:

**File**: `src-tauri/Cargo.toml`
```toml
[package]
version = "0.2.0"  # Bump from 0.1.0
```

**File**: `src-tauri/tauri.conf.json`
```json
{
  "productVersion": "0.2.0"
}
```

**File**: `package.json` (optional, for consistency)
```json
{
  "version": "0.2.0"
}
```

### 2. Commit Version Bump

```bash
git add src-tauri/Cargo.toml src-tauri/tauri.conf.json package.json
git commit -m "chore(release): bump version to 0.2.0"
```

### 3. Create Git Tag

Tags trigger the GitHub Actions release workflow:

```bash
git tag v0.2.0
git push origin main --tags
```

Or push just the tag:
```bash
git push origin v0.2.0
```

### 4. GitHub Actions Runs Automatically

When you push the tag:

1. ✅ GitHub Actions workflow starts (`.github/workflows/release.yml`)
2. ✅ Builds macOS DMG in the cloud
3. ✅ Creates GitHub Release with the DMG file
4. ✅ Generates release notes

**Monitor the build**:
- Go to: `https://github.com/yourusername/schedula/actions`
- Click the `v0.2.0` workflow run
- Watch build progress in real-time

### 5. Download & Test the DMG

Once the workflow completes:

1. Go to: `https://github.com/yourusername/schedula/releases`
2. Find your new release (e.g., "Schedula v0.2.0")
3. Download the `Schedula-v0.2.0.dmg` file
4. Test installation:
   ```bash
   open Schedula-v0.2.0.dmg
   # Drag app to Applications, test functionality
   ```

### 6. Publish Release Notes

The workflow auto-generates release notes, but you can edit them:

1. Go to the release page
2. Click "Edit" (pencil icon)
3. Update description if needed
4. Check "Latest release" checkbox
5. Click "Update release"

---

## Manual Build (Optional)

If you prefer to build locally and upload manually:

### Build DMG Locally

```bash
npm run tauri build
```

Output: `src-tauri/target/release/bundle/macos/Schedula.dmg`

### Upload to Existing Release

```bash
# Create a draft release (manual)
gh release create v0.2.0 --draft --title "Schedula v0.2.0"

# Upload the DMG
gh release upload v0.2.0 \
  src-tauri/target/release/bundle/macos/Schedula.dmg

# Publish it
gh release edit v0.2.0 --draft=false
```

Or via GitHub web UI:
1. Go to Releases page
2. Click "Create a new release"
3. Enter tag: `v0.2.0`
4. Fill title and description
5. Drag & drop DMG file
6. Click "Publish release"

---

## Workflow Troubleshooting

### Build Failed
- Check Actions tab for error logs
- Common issues:
  - Missing Rust toolchain (workflow installs automatically)
  - Node.js version mismatch (should be ≥18)
  - Icon files missing (check `src-tauri/icons/`)

**Fix**: Commit the fix and retry:
```bash
git tag --delete v0.2.0 origin/v0.2.0  # Remove failed tag
git push origin main
# Fix the issue
git commit -m "fix: resolve build error"
git tag v0.2.0
git push origin v0.2.0
```

### Release Not Created
- Ensure tag format is `v*.*.*` (e.g., `v0.2.0`)
- Check Actions tab to confirm build succeeded
- Verify you have write permissions to the repository

### DMG File is Empty
- The build likely failed (check Actions logs)
- Try building locally: `npm run tauri build`
- Upload manually if local build succeeds

---

## Version Numbering

Use **Semantic Versioning**: `MAJOR.MINOR.PATCH`

- **0.1.0** → First release (breaking changes okay)
- **0.1.1** → Bug fix
- **0.2.0** → New feature (scheduling settings added)
- **1.0.0** → Stable release (API stable)

Examples:
```
v0.1.0  # Initial release
v0.1.1  # Bug fixes
v0.2.0  # Settings feature
v0.3.0  # Theme customization
v1.0.0  # Stable release
v1.0.1  # Critical hotfix
v1.1.0  # New features
v2.0.0  # Major redesign
```

---

## Release Checklist

Before each release:

- [ ] All changes merged to `main`
- [ ] Tests passing locally (`npm run tauri dev`)
- [ ] No console errors in dev tools
- [ ] Version numbers updated (Cargo.toml, tauri.conf.json, package.json)
- [ ] Version change committed
- [ ] Git tag created with format `v*.*.*`
- [ ] Tag pushed to GitHub
- [ ] GitHub Actions workflow completes successfully
- [ ] DMG file uploaded and accessible
- [ ] Release notes are clear and helpful
- [ ] Tested the downloaded DMG

---

## Announcing a Release

After releasing, share it:

1. **GitHub**: Release page has download link
2. **Readme**: Update "Download" section with latest version
3. **Social media**: Tweet, post in academic forums
4. **Email**: If you have an early user list

Example announcement:

```
🎉 Schedula v0.2.0 is now available!

New features:
✨ Dark/light theme toggle
🎨 Custom accent colors
⚙️ Organization-specific scheduling settings
💾 Database backup and restore
🗑️ Clear schedules safely

Download: https://github.com/yourusername/schedula/releases/tag/v0.2.0

Updated docs: https://github.com/yourusername/schedula#readme
```

---

## Continuous Distribution

Users can always download the latest release from:
```
https://github.com/yourusername/schedula/releases
```

Each release includes:
- DMG file (ready to drag into Applications)
- Release notes with what's new
- Installation instructions
- Link to full documentation

---

## Future: App Store Distribution

Eventually, you may want to distribute via:
- **Mac App Store** (requires Apple Developer account, $99/year)
- **Homebrew** (`brew install schedula`)
- **Direct download** from schedula.app website

For now, GitHub Releases is the best distribution method for an open-source app.

---

## Questions?

- 🐛 **Build issues**: Check [GitHub Actions logs](https://github.com/yourusername/schedula/actions)
- 📚 **Reference**: See [README.md](README.md) for full documentation
- 💬 **Discuss**: Open a [GitHub Discussion](https://github.com/yourusername/schedula/discussions)

Happy releasing! 🚀
