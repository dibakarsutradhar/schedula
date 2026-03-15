<script>
  import { onMount } from 'svelte'
  import DaySelect from '../lib/components/DaySelect.svelte'
  import { session } from '../lib/stores/session.js'
  import { isSuperAdmin } from '../lib/stores/session.js'
  import { prefs, ACCENT_PRESETS } from '../lib/stores/prefs.js'
  import { toast } from '../lib/toast.js'
  import {
    changePassword, updateDisplayName, adminResetPassword, setUserActive,
    getUsers, getOrganizations, updateOrganization,
    getSchedulingSettings, upsertSchedulingSettings,
    clearSchedules, backupDatabase, getAppInfo,
    getStats,
  } from '../lib/api.js'

  let tab = 'appearance'

  // ── Appearance ──────────────────────────────────────────────────────────────
  $: currentTheme = $prefs.theme
  $: currentAccent = $prefs.accentColor

  // ── Profile ─────────────────────────────────────────────────────────────────
  let displayName = $session?.display_name ?? ''
  let oldPassword = ''
  let newPassword = ''
  let confirmPassword = ''
  let savingProfile = false

  async function saveDisplayName() {
    if (!displayName.trim()) return
    savingProfile = true
    try {
      await updateDisplayName(displayName.trim())
      session.set($session ? { ...$session, display_name: displayName.trim() } : null)
      toast('Display name updated')
    } catch (e) {
      toast(e, 'error')
    } finally {
      savingProfile = false
    }
  }

  async function savePassword() {
    if (newPassword !== confirmPassword) { toast('Passwords do not match', 'error'); return }
    if (newPassword.length < 6) { toast('Password must be at least 6 characters', 'error'); return }
    try {
      await changePassword(oldPassword, newPassword)
      oldPassword = ''; newPassword = ''; confirmPassword = ''
      toast('Password changed')
    } catch (e) {
      toast(e, 'error')
    }
  }

  // ── Users ───────────────────────────────────────────────────────────────────
  let users = []
  let resetUserId = null
  let resetPwd = ''
  let showResetModal = false

  async function loadUsers() { users = await getUsers() }

  async function toggleActive(u) {
    try {
      await setUserActive(u.id, !u.is_active)
      toast(u.is_active ? 'User deactivated' : 'User reactivated')
      await loadUsers()
    } catch (e) {
      toast(e, 'error')
    }
  }

  function openResetPwd(u) { resetUserId = u.id; resetPwd = ''; showResetModal = true }

  async function doResetPwd() {
    if (resetPwd.length < 6) { toast('Password must be at least 6 characters', 'error'); return }
    try {
      await adminResetPassword(resetUserId, resetPwd)
      showResetModal = false
      toast('Password reset')
    } catch (e) {
      toast(e, 'error')
    }
  }

  // ── Organization ─────────────────────────────────────────────────────────────
  let orgs = []
  let orgForm = { name: '', org_type: 'university', address: '', contact_email: '' }
  let editingOrg = null
  let savingOrg = false

  async function loadOrgs() {
    orgs = await getOrganizations()
    // For admin, pre-select their org
    if (!isSuperAdmin($session) && $session?.org_id) {
      const org = orgs.find(o => o.id === $session.org_id)
      if (org) selectOrg(org)
    }
  }

  function selectOrg(org) {
    editingOrg = org
    orgForm = {
      name: org.name,
      org_type: org.org_type,
      address: org.address ?? '',
      contact_email: org.contact_email ?? '',
    }
  }

  async function saveOrg() {
    if (!editingOrg) return
    savingOrg = true
    try {
      await updateOrganization(editingOrg.id, {
        name: orgForm.name,
        org_type: orgForm.org_type,
        address: orgForm.address || null,
        contact_email: orgForm.contact_email || null,
      })
      toast('Organization updated')
      await loadOrgs()
    } catch (e) {
      toast(e, 'error')
    } finally {
      savingOrg = false
    }
  }

  // ── Scheduling ───────────────────────────────────────────────────────────────
  let schedSettings = { org_id: 0, working_days: 'Mon,Tue,Wed,Thu,Fri', day_start_slot: 0, day_end_slot: 7, slot_duration: 60 }
  let savingScheds = false
  const slotLabels = ['08:00','09:00','10:00','11:00','13:00','14:00','15:00','16:00','17:00']

  async function loadSchedSettings() {
    const orgId = $session?.org_id
    if (!orgId && !isSuperAdmin($session)) return
    const id = orgId ?? (orgs[0]?.id ?? null)
    if (!id) return
    schedSettings = await getSchedulingSettings(id)
  }

  async function saveSchedSettings() {
    savingScheds = true
    try {
      await upsertSchedulingSettings(schedSettings)
      toast('Scheduling settings saved')
    } catch (e) {
      toast(e, 'error')
    } finally {
      savingScheds = false
    }
  }

  // ── Data ─────────────────────────────────────────────────────────────────────
  let stats = {}
  let clearing = false
  let backingUp = false

  async function doBackup() {
    backingUp = true
    try {
      const b64 = await backupDatabase()
      const bytes = Uint8Array.from(atob(b64), c => c.charCodeAt(0))
      const blob = new Blob([bytes], { type: 'application/json' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `schedula-backup-${new Date().toISOString().slice(0,10)}.json`
      a.click()
      URL.revokeObjectURL(url)
      toast('Backup downloaded')
    } catch (e) {
      toast(e, 'error')
    } finally {
      backingUp = false
    }
  }

  async function doClearSchedules() {
    if (!confirm('Delete ALL schedules for your organization? This cannot be undone.')) return
    clearing = true
    try {
      const count = await clearSchedules()
      toast(`Cleared ${count} schedule(s)`)
      stats = await getStats()
    } catch (e) {
      toast(e, 'error')
    } finally {
      clearing = false
    }
  }

  // ── About ────────────────────────────────────────────────────────────────────
  let appInfo = null

  // ── Init ─────────────────────────────────────────────────────────────────────
  onMount(async () => {
    if (tab === 'users' || isSuperAdmin($session)) await loadUsers()
    await loadOrgs()
    await loadSchedSettings()
    stats = await getStats()
    appInfo = await getAppInfo()
  })

  function switchTab(t) {
    tab = t
    if (t === 'users') loadUsers()
    if (t === 'org') loadOrgs()
    if (t === 'scheduling') loadSchedSettings()
    if (t === 'about') getAppInfo().then(i => appInfo = i)
  }

  function fmtBytes(b) {
    if (b < 1024) return b + ' B'
    if (b < 1024*1024) return (b/1024).toFixed(1) + ' KB'
    return (b/1024/1024).toFixed(1) + ' MB'
  }
</script>

<div class="page">
  <div class="page-header">
    <div class="page-header-left">
      <h1>Settings</h1>
      <p class="page-subtitle">Customize Schedula to your preferences</p>
    </div>
  </div>

  <div class="settings-layout">
    <!-- Tab sidebar -->
    <nav class="settings-tabs">
      {#each [
        { id: 'appearance', icon: '🎨', label: 'Appearance' },
        { id: 'profile',    icon: '👤', label: 'My Profile' },
        { id: 'users',      icon: '👥', label: 'Users' },
        { id: 'org',        icon: '🏫', label: 'Organization' },
        { id: 'scheduling', icon: '⚙️', label: 'Scheduling' },
        { id: 'data',       icon: '💾', label: 'Data' },
        { id: 'about',      icon: 'ℹ️', label: 'About' },
      ] as t}
        <button class="settings-tab" class:active={tab === t.id} on:click={() => switchTab(t.id)}>
          <span class="tab-icon">{t.icon}</span>
          {t.label}
        </button>
      {/each}
    </nav>

    <!-- Content panel -->
    <div class="settings-panel">

      <!-- ── Appearance ── -->
      {#if tab === 'appearance'}
        <div class="card settings-section">
          <h2>Theme</h2>
          <div class="theme-options">
            {#each [['dark','🌙','Dark'],['light','☀️','Light'],['system','💻','System']] as [val, icon, label]}
              <button class="theme-btn" class:active={currentTheme === val} on:click={() => prefs.setTheme(val)}>
                <span class="theme-preview">{icon}</span>
                {label}
              </button>
            {/each}
          </div>
        </div>

        <div class="card settings-section">
          <h2>Accent Color</h2>
          <div class="accent-swatches">
            {#each ACCENT_PRESETS as color}
              <button
                class="swatch"
                class:active={currentAccent === color}
                style="background:{color}"
                title={color}
                on:click={() => prefs.setAccent(color)}
              ></button>
            {/each}
          </div>
          <div class="form-group" style="margin-top:16px; max-width:200px">
            <label class="form-label">Custom Color</label>
            <input type="color" value={currentAccent} class="form-input" style="height:40px; padding:4px"
              on:input={e => prefs.setAccent(e.target.value)} />
          </div>
        </div>

      <!-- ── Profile ── -->
      {:else if tab === 'profile'}
        <div class="card settings-section">
          <h2>Display Name</h2>
          <div class="row" style="align-items:flex-end">
            <div class="form-group">
              <label class="form-label">Your Name</label>
              <input class="form-input" bind:value={displayName} placeholder="Enter your name" />
            </div>
            <button class="btn btn-primary" on:click={saveDisplayName} disabled={savingProfile || !displayName.trim()}>
              Save
            </button>
          </div>
          <p style="margin-top:8px; font-size:12px; color:var(--text-muted)">
            Username: <strong>{$session?.username}</strong> · Role: <strong>{$session?.role.replace('_',' ')}</strong>
          </p>
        </div>

        <div class="card settings-section">
          <h2>Change Password</h2>
          <div class="form-group">
            <label class="form-label">Current Password</label>
            <input class="form-input" type="password" bind:value={oldPassword} />
          </div>
          <div class="row">
            <div class="form-group">
              <label class="form-label">New Password</label>
              <input class="form-input" type="password" bind:value={newPassword} />
            </div>
            <div class="form-group">
              <label class="form-label">Confirm New Password</label>
              <input class="form-input" type="password" bind:value={confirmPassword} />
            </div>
          </div>
          <div style="display:flex;justify-content:flex-end">
            <button class="btn btn-primary" on:click={savePassword}
              disabled={!oldPassword || !newPassword || !confirmPassword}>
              Update Password
            </button>
          </div>
        </div>

      <!-- ── Users ── -->
      {:else if tab === 'users'}
        <div class="card">
          <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:16px">
            <h2 style="margin:0;border:none;padding:0">{users.length} Users</h2>
          </div>
          {#if users.length === 0}
            <div class="empty-state">No users found.</div>
          {:else}
            <table>
              <thead>
                <tr>
                  <th>Name</th><th>Username</th><th>Role</th><th>Org</th><th>Status</th>
                  {#if isSuperAdmin($session)}<th>Actions</th>{/if}
                </tr>
              </thead>
              <tbody>
                {#each users as u}
                  <tr>
                    <td><strong>{u.display_name}</strong></td>
                    <td style="color:var(--text-muted)">{u.username}</td>
                    <td><span class="badge badge-lecture">{u.role.replace('_',' ')}</span></td>
                    <td>{u.org_name ?? '—'}</td>
                    <td>
                      {#if u.is_active}
                        <span class="badge badge-active">Active</span>
                      {:else}
                        <span class="badge" style="background:rgba(239,68,68,.15);color:#f87171">Inactive</span>
                      {/if}
                    </td>
                    {#if isSuperAdmin($session)}
                      <td>
                        {#if u.id !== $session?.user_id}
                          <button class="btn btn-secondary btn-sm" on:click={() => toggleActive(u)}>
                            {u.is_active ? 'Deactivate' : 'Activate'}
                          </button>
                          <button class="btn btn-secondary btn-sm" on:click={() => openResetPwd(u)}
                            style="margin-left:6px">
                            Reset Pwd
                          </button>
                        {:else}
                          <span style="font-size:12px;color:var(--text-muted)">You</span>
                        {/if}
                      </td>
                    {/if}
                  </tr>
                {/each}
              </tbody>
            </table>
          {/if}
        </div>

      <!-- ── Organization ── -->
      {:else if tab === 'org'}
        {#if isSuperAdmin($session) && orgs.length > 1}
          <div class="card settings-section">
            <h2>Select Organization</h2>
            <div class="chip-row">
              {#each orgs as org}
                <button
                  class="chip chip-toggle"
                  class:active={editingOrg?.id === org.id}
                  on:click={() => selectOrg(org)}
                >{org.name}</button>
              {/each}
            </div>
          </div>
        {/if}
        {#if editingOrg}
          <div class="card">
            <h2 style="margin-bottom:16px">{editingOrg.name}</h2>
            <div class="form-group" style="margin-bottom:14px">
              <label class="form-label">Organization Name *</label>
              <input class="form-input" bind:value={orgForm.name} />
            </div>
            <div class="row" style="margin-bottom:14px">
              <div class="form-group">
                <label class="form-label">Type</label>
                <select class="form-select" bind:value={orgForm.org_type}>
                  <option value="university">University</option>
                  <option value="college">College</option>
                  <option value="school">School</option>
                  <option value="institute">Institute</option>
                </select>
              </div>
              <div class="form-group">
                <label class="form-label">Contact Email</label>
                <input class="form-input" type="email" bind:value={orgForm.contact_email} placeholder="admin@university.edu" />
              </div>
            </div>
            <div class="form-group" style="margin-bottom:16px">
              <label class="form-label">Address</label>
              <textarea class="form-textarea" bind:value={orgForm.address} rows="2" placeholder="123 Campus Road..."></textarea>
            </div>
            <div style="display:flex;justify-content:flex-end">
              <button class="btn btn-primary" on:click={saveOrg} disabled={savingOrg || !orgForm.name}>
                Save Organization
              </button>
            </div>
          </div>
        {:else}
          <div class="card"><div class="empty-state">No organization linked to your account.</div></div>
        {/if}

      <!-- ── Scheduling ── -->
      {:else if tab === 'scheduling'}
        <div class="card">
          <h2 style="margin-bottom:16px">Scheduling Defaults</h2>
          <div class="form-group" style="margin-bottom:14px">
            <label class="form-label">Working Days</label>
            <DaySelect bind:value={schedSettings.working_days} />
          </div>
          <div class="row" style="margin-bottom:14px">
            <div class="form-group">
              <label class="form-label">Day Start Slot</label>
              <select class="form-select" bind:value={schedSettings.day_start_slot}>
                {#each slotLabels.slice(0,8) as label, i}
                  <option value={i}>{label}</option>
                {/each}
              </select>
            </div>
            <div class="form-group">
              <label class="form-label">Day End Slot</label>
              <select class="form-select" bind:value={schedSettings.day_end_slot}>
                {#each slotLabels.slice(1) as label, i}
                  <option value={i}>{label}</option>
                {/each}
              </select>
            </div>
          </div>
          <div class="form-group" style="margin-bottom:16px; max-width:200px">
            <label class="form-label">Slot Duration (minutes)</label>
            <select class="form-select" bind:value={schedSettings.slot_duration}>
              <option value={45}>45 min</option>
              <option value={60}>60 min</option>
              <option value={90}>90 min</option>
            </select>
          </div>
          <p style="font-size:12px;color:var(--text-muted);margin-bottom:16px">
            These defaults inform schedule generation for your organization.
          </p>
          <div style="display:flex;justify-content:flex-end">
            <button class="btn btn-primary" on:click={saveSchedSettings} disabled={savingScheds}>
              Save Defaults
            </button>
          </div>
        </div>

      <!-- ── Data ── -->
      {:else if tab === 'data'}
        <div class="card settings-section">
          <h2>Database Backup</h2>
          <p style="font-size:13px;color:var(--text-muted);margin-bottom:16px">
            Download a JSON backup of all your data including courses, lecturers, rooms, batches, and schedules.
          </p>
          <button class="btn btn-primary" on:click={doBackup} disabled={backingUp}>
            {backingUp ? 'Preparing…' : '⬇ Download Backup'}
          </button>
        </div>

        <div class="card settings-section">
          <h2>Clear Schedules</h2>
          <p style="font-size:13px;color:var(--text-muted);margin-bottom:8px">
            Delete all generated schedules for your organization.
            <strong>Courses, lecturers, rooms, and batches are not affected.</strong>
          </p>
          <div class="info-grid" style="margin-bottom:16px">
            <div class="info-item">
              <div class="info-label">Schedules</div>
              <div class="info-value">{stats.schedules ?? '—'}</div>
            </div>
            <div class="info-item">
              <div class="info-label">Active Entries</div>
              <div class="info-value">{stats.active_entries ?? '—'}</div>
            </div>
          </div>
          <button class="btn btn-danger" on:click={doClearSchedules} disabled={clearing}>
            {clearing ? 'Clearing…' : 'Clear All Schedules'}
          </button>
        </div>

      <!-- ── About ── -->
      {:else if tab === 'about'}
        <div class="card settings-section">
          <h2>About Schedula</h2>
          <div class="info-grid">
            <div class="info-item">
              <div class="info-label">Version</div>
              <div class="info-value">{appInfo?.version ?? '—'}</div>
            </div>
            <div class="info-item">
              <div class="info-label">Database Size</div>
              <div class="info-value">{appInfo ? fmtBytes(appInfo.db_size_bytes) : '—'}</div>
            </div>
            <div class="info-item">
              <div class="info-label">Users</div>
              <div class="info-value">{appInfo?.user_count ?? '—'}</div>
            </div>
            <div class="info-item">
              <div class="info-label">Organizations</div>
              <div class="info-value">{appInfo?.org_count ?? '—'}</div>
            </div>
            <div class="info-item">
              <div class="info-label">Total Schedules</div>
              <div class="info-value">{appInfo?.schedule_count ?? '—'}</div>
            </div>
          </div>
        </div>

        <div class="card">
          <h2 style="margin-bottom:12px">Keyboard Shortcuts</h2>
          <table>
            <tbody>
              {#each [
                ['Cmd/Ctrl + ,', 'Open Settings'],
                ['Escape', 'Close modal'],
              ] as [key, desc]}
                <tr>
                  <td style="width:160px"><kbd style="background:var(--surface2);border:1px solid var(--border);border-radius:4px;padding:2px 7px;font-size:12px">{key}</kbd></td>
                  <td style="color:var(--text-muted)">{desc}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </div>
  </div>
</div>

<!-- Password Reset Modal -->
{#if showResetModal}
  <div class="modal-backdrop" on:click|self={() => showResetModal = false}>
    <div class="modal">
      <div class="modal-header">
        <h2>Reset Password</h2>
        <button class="modal-close" on:click={() => showResetModal = false}>×</button>
      </div>
      <div class="modal-body">
        <div class="form-group">
          <label class="form-label">New Password</label>
          <input class="form-input" type="password" bind:value={resetPwd} placeholder="Min 6 characters" />
        </div>
      </div>
      <div class="modal-footer">
        <button class="btn btn-secondary" on:click={() => showResetModal = false}>Cancel</button>
        <button class="btn btn-primary" on:click={doResetPwd} disabled={resetPwd.length < 6}>
          Reset Password
        </button>
      </div>
    </div>
  </div>
{/if}
