<script>
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import DaySelect from '../lib/components/DaySelect.svelte'
  import { session } from '../lib/stores/session.js'
  import { isSuperAdmin } from '../lib/stores/session.js'
  import { prefs, ACCENT_PRESETS } from '../lib/stores/prefs.js'
  import { toast } from '../lib/toast.js'
  import { syncMode } from '../lib/stores/syncMode.js'
  import { wsConnected, pingHub, connectWs, disconnectWs } from '../lib/stores/ws.js'
  import {
    changePassword, updateDisplayName, adminResetPassword, setUserActive,
    getUsers, getOrganizations, updateOrganization,
    getSchedulingSettings, upsertSchedulingSettings,
    clearSchedules, backupDatabase, getAppInfo,
    getStats, getMaxAdmins, setMaxAdmins, getAdminCount,
    getAuditLog,
  } from '../lib/api.js'

  let tab = 'appearance'

  // ── Sync ─────────────────────────────────────────────────────────────────────
  let syncUrlInput = $syncMode.serverUrl || ''
  let syncTesting  = false
  let syncSaving   = false
  let pingResult   = null   // null | true | false

  async function testConnection() {
    if (!syncUrlInput.trim()) { toast('Enter a server URL first', 'error'); return }
    syncTesting = true
    pingResult  = null
    try {
      pingResult = await pingHub(syncUrlInput.trim())
      if (!pingResult) toast('Could not reach the hub server', 'error')
      else             toast('Hub server reachable ✓', 'success')
    } finally {
      syncTesting = false
    }
  }

  function connectToServer() {
    const url = syncUrlInput.trim()
    if (!url) { toast('Enter a server URL', 'error'); return }
    syncMode.setServer(url)
    toast('Server mode enabled. Please log in again to authenticate with the hub.', 'info')
  }

  function disconnectFromServer() {
    disconnectWs()
    syncMode.setStandalone()
    toast('Switched to standalone mode')
  }

  // ── Hub Mode (sidecar) ────────────────────────────────────────────────────────
  let hubRunning  = false
  let hubUrl      = null
  let hubLoading  = false

  // ── License ───────────────────────────────────────────────────────────────────
  let licenseInfo    = null   // { active, plan, org_name, expires_at, status }
  let licenseToken   = ''
  let licenseLoading = false

  function activeHubUrl() {
    if (hubRunning) return 'http://localhost:7878'
    if ($syncMode.mode === 'server' && $syncMode.serverUrl) return $syncMode.serverUrl.replace(/\/$/, '')
    return null
  }

  async function fetchLicenseInfo() {
    const url = activeHubUrl()
    if (!url) { licenseInfo = null; return }
    try {
      const res = await fetch(`${url}/api/license`)
      if (res.ok) licenseInfo = await res.json()
    } catch { licenseInfo = null }
  }

  async function activateLicense() {
    const token = licenseToken.trim()
    if (!token) { toast('Paste your license token first', 'error'); return }
    const url = activeHubUrl()
    if (!url) { toast('Enable Hub Mode or connect to a hub first', 'error'); return }
    licenseLoading = true
    try {
      const res = await fetch(`${url}/api/license/activate`, {
        method:  'POST',
        headers: { 'Content-Type': 'application/json' },
        body:    JSON.stringify({ token }),
      })
      const data = await res.json()
      if (!res.ok) throw new Error(data.error || 'Activation failed')
      licenseInfo  = data
      licenseToken = ''
      toast(`${capitalize(data.plan)} plan activated ✓`, 'success')
    } catch (e) {
      toast(e.message, 'error')
    } finally {
      licenseLoading = false
    }
  }

  async function deactivateLicense() {
    const url = activeHubUrl()
    if (!url) return
    licenseLoading = true
    try {
      const res = await fetch(`${url}/api/license/deactivate`, {
        method:  'POST',
        headers: { 'Content-Type': 'application/json', 'Authorization': `JWT ${$syncMode.token || ''}` },
      })
      if (res.ok) { licenseInfo = await fetchLicenseInfo(); toast('License deactivated') }
      else { const d = await res.json(); toast(d.error || 'Failed', 'error') }
    } catch (e) { toast(e.message, 'error') }
    finally { licenseLoading = false }
  }

  function capitalize(s) { return s ? s.charAt(0).toUpperCase() + s.slice(1) : s }

  function planBadgeClass(plan) {
    if (!plan || plan === 'free') return 'plan-badge-free'
    if (plan === 'pro')          return 'plan-badge-pro'
    return 'plan-badge-institution'
  }

  async function checkHubStatus() {
    try {
      const status = await invoke('get_hub_status')
      hubRunning = status.running
      hubUrl     = status.url ?? null
    } catch (e) {
      // sidecar not available in dev without binary — ignore silently
    }
  }

  async function startHubMode() {
    hubLoading = true
    try {
      const status = await invoke('start_hub_mode')
      hubRunning = status.running
      hubUrl     = status.url ?? null
      toast('Hub mode enabled — share the address with other admins', 'success')
      await fetchLicenseInfo()
    } catch (e) {
      toast('Failed to start hub: ' + e, 'error')
    } finally {
      hubLoading = false
    }
  }

  async function stopHubMode() {
    hubLoading = true
    try {
      await invoke('stop_hub_mode')
      hubRunning = false
      hubUrl     = null
      toast('Hub mode stopped')
    } catch (e) {
      toast('Failed to stop hub: ' + e, 'error')
    } finally {
      hubLoading = false
    }
  }

  async function copyHubUrl() {
    if (!hubUrl) return
    await navigator.clipboard.writeText(hubUrl)
    toast('Hub address copied ✓')
  }

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

  // ── System ───────────────────────────────────────────────────────────────────
  let maxAdmins = 2
  let adminCount = 0
  let savingMaxAdmins = false

  async function loadSystemSettings() {
    const [max, count] = await Promise.all([getMaxAdmins(), getAdminCount()])
    maxAdmins = max
    adminCount = count
  }

  async function saveMaxAdmins() {
    if (maxAdmins < 1) { toast('Max admins must be at least 1', 'error'); return }
    savingMaxAdmins = true
    try {
      await setMaxAdmins(maxAdmins)
      toast('Admin limit updated')
      await loadSystemSettings()
    } catch (e) {
      toast(e, 'error')
    } finally {
      savingMaxAdmins = false
    }
  }

  // ── Audit Log ────────────────────────────────────────────────────────────────
  let auditLog = []
  let loadingAudit = false

  async function loadAuditLog() {
    loadingAudit = true
    try { auditLog = await getAuditLog(200) } catch(e) { toast(e, 'error') } finally { loadingAudit = false }
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
    if (isSuperAdmin($session)) await loadSystemSettings()
    await checkHubStatus()
    await fetchLicenseInfo()
  })

  function switchTab(t) {
    tab = t
    if (t === 'users') loadUsers()
    if (t === 'org') loadOrgs()
    if (t === 'scheduling') loadSchedSettings()
    if (t === 'system') loadSystemSettings()
    if (t === 'audit') loadAuditLog()
    if (t === 'about') getAppInfo().then(i => appInfo = i)
    if (t === 'sync') syncUrlInput = $syncMode.serverUrl || ''
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
        { id: 'sync',       icon: '🔄', label: 'Sync' },
        ...(isSuperAdmin($session) ? [{ id: 'system', icon: '🔧', label: 'System' }] : []),
        { id: 'audit',      icon: '📋', label: 'Audit Log' },
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
            <label class="form-label" for="custom-accent-color">Custom Color</label>
            <input id="custom-accent-color" type="color" value={currentAccent} class="form-input" style="height:40px; padding:4px"
              on:input={e => prefs.setAccent(e.target.value)} />
          </div>
        </div>

      <!-- ── Profile ── -->
      {:else if tab === 'profile'}
        <div class="card settings-section">
          <h2>Display Name</h2>
          <div class="row" style="align-items:flex-end">
            <div class="form-group">
              <label class="form-label" for="profile-display-name">Your Name</label>
              <input id="profile-display-name" class="form-input" bind:value={displayName} placeholder="Enter your name" />
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
            <label class="form-label" for="password-old">Current Password</label>
            <input id="password-old" class="form-input" type="password" bind:value={oldPassword} />
          </div>
          <div class="row">
            <div class="form-group">
              <label class="form-label" for="password-new">New Password</label>
              <input id="password-new" class="form-input" type="password" bind:value={newPassword} />
            </div>
            <div class="form-group">
              <label class="form-label" for="password-confirm">Confirm New Password</label>
              <input id="password-confirm" class="form-input" type="password" bind:value={confirmPassword} />
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
              <label class="form-label" for="org-name">Organization Name *</label>
              <input id="org-name" class="form-input" bind:value={orgForm.name} />
            </div>
            <div class="row" style="margin-bottom:14px">
              <div class="form-group">
                <label class="form-label" for="org-type">Type</label>
                <select id="org-type" class="form-select" bind:value={orgForm.org_type}>
                  <option value="university">University</option>
                  <option value="college">College</option>
                  <option value="school">School</option>
                  <option value="institute">Institute</option>
                </select>
              </div>
              <div class="form-group">
                <label class="form-label" for="org-email">Contact Email</label>
                <input id="org-email" class="form-input" type="email" bind:value={orgForm.contact_email} placeholder="admin@university.edu" />
              </div>
            </div>
            <div class="form-group" style="margin-bottom:16px">
              <label class="form-label" for="org-address">Address</label>
              <textarea id="org-address" class="form-textarea" bind:value={orgForm.address} rows="2" placeholder="123 Campus Road..."></textarea>
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
            <span class="form-label">Working Days</span>
            <DaySelect bind:value={schedSettings.working_days} />
          </div>
          <div class="row" style="margin-bottom:14px">
            <div class="form-group">
              <label class="form-label" for="sched-start-slot">Day Start Slot</label>
              <select id="sched-start-slot" class="form-select" bind:value={schedSettings.day_start_slot}>
                {#each slotLabels.slice(0,8) as label, i}
                  <option value={i}>{label}</option>
                {/each}
              </select>
            </div>
            <div class="form-group">
              <label class="form-label" for="sched-end-slot">Day End Slot</label>
              <select id="sched-end-slot" class="form-select" bind:value={schedSettings.day_end_slot}>
                {#each slotLabels.slice(1) as label, i}
                  <option value={i}>{label}</option>
                {/each}
              </select>
            </div>
          </div>
          <div class="form-group" style="margin-bottom:16px; max-width:200px">
            <label class="form-label" for="sched-slot-duration">Slot Duration (minutes)</label>
            <select id="sched-slot-duration" class="form-select" bind:value={schedSettings.slot_duration}>
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

      <!-- ── System ── -->
      {:else if tab === 'system'}
        {#if isSuperAdmin($session)}
          <div class="card settings-section">
            <h2>Admin Quota</h2>
            <p style="font-size:13px;color:var(--text-muted);margin-bottom:16px">
              Control how many admin accounts can exist. Current usage: <strong>{adminCount} / {maxAdmins}</strong>.
            </p>
            <div class="row" style="align-items:flex-end;max-width:320px">
              <div class="form-group">
                <label class="form-label" for="system-max-admins">Max Admins</label>
                <input
                  id="system-max-admins"
                  class="form-input"
                  type="number"
                  min="1"
                  max="50"
                  bind:value={maxAdmins}
                  style="max-width:100px"
                />
              </div>
              <button class="btn btn-primary" on:click={saveMaxAdmins} disabled={savingMaxAdmins}>
                {savingMaxAdmins ? 'Saving…' : 'Save'}
              </button>
            </div>
            <p style="margin-top:10px;font-size:12px;color:var(--text-muted)">
              Minimum 1. Super admin does not count toward this limit.
            </p>
          </div>

          <div class="card settings-section">
            <h2>Instance Constraints</h2>
            <div class="info-grid">
              <div class="info-item">
                <div class="info-label">Organizations</div>
                <div class="info-value">1 (fixed)</div>
              </div>
              <div class="info-item">
                <div class="info-label">Super Admins</div>
                <div class="info-value">1 (fixed)</div>
              </div>
              <div class="info-item">
                <div class="info-label">Max Admins</div>
                <div class="info-value">{maxAdmins}</div>
              </div>
              <div class="info-item">
                <div class="info-label">Current Admins</div>
                <div class="info-value" style={adminCount >= maxAdmins ? 'color:var(--danger)' : ''}>{adminCount}</div>
              </div>
            </div>
          </div>
        {/if}

      <!-- ── Audit Log ── -->
      {:else if tab === 'audit'}
        <div class="card settings-section">
          <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:16px">
            <h2 style="margin:0">Audit Log</h2>
            <button class="btn btn-secondary btn-sm" on:click={loadAuditLog}>↻ Refresh</button>
          </div>
          {#if loadingAudit}
            <div style="color:var(--text-muted);font-size:13px">Loading…</div>
          {:else if auditLog.length === 0}
            <div style="color:var(--text-muted);font-size:13px">No audit entries yet.</div>
          {:else}
            <div class="table-wrap">
              <table>
                <thead>
                  <tr>
                    <th>Time</th>
                    <th>User</th>
                    <th>Action</th>
                    <th>Entity</th>
                    <th>Details</th>
                  </tr>
                </thead>
                <tbody>
                  {#each auditLog as entry}
                    <tr>
                      <td style="white-space:nowrap;font-size:11px;color:var(--text-muted)">{entry.created_at.slice(0,16).replace('T',' ')}</td>
                      <td style="font-size:12px">{entry.username}</td>
                      <td>
                        <span class="audit-action audit-{entry.action}">{entry.action}</span>
                      </td>
                      <td style="font-size:12px">{entry.entity_type}{entry.entity_id ? ` #${entry.entity_id}` : ''}</td>
                      <td style="font-size:11px;color:var(--text-muted);max-width:200px;overflow:hidden;text-overflow:ellipsis">{entry.details_json ?? ''}</td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {/if}
        </div>

      <!-- ── Sync ── -->
      {:else if tab === 'sync'}
        <div class="card settings-section">
          <h2>Real-Time Sync</h2>
          <p class="section-desc">
            Connect every admin machine to a shared <strong>hub</strong> for real-time updates.
            One machine runs the hub — all others connect to it. No extra software needed.
          </p>

          <div class="sync-status-bar" class:connected={$syncMode.mode === 'server'}>
            <span class="sync-dot" class:on={$syncMode.mode === 'server' && !!$syncMode.token}></span>
            {#if $syncMode.mode === 'server' && $syncMode.token}
              <span>Connected to <strong>{$syncMode.serverUrl}</strong></span>
              {#if $wsConnected}
                <span class="ws-badge">● Live</span>
              {:else}
                <span class="ws-badge offline">○ Reconnecting…</span>
              {/if}
            {:else if $syncMode.mode === 'server'}
              <span>Server mode set — <em>log in to authenticate</em></span>
            {:else}
              <span>Standalone mode (local database)</span>
            {/if}
          </div>
        </div>

        <div class="card settings-section">
          <h2>Hub Server URL</h2>
          <p class="section-desc">Enter the hub server address (e.g. <code>http://192.168.1.100:7878</code>)</p>
          <div class="form-row">
            <input
              class="form-input"
              bind:value={syncUrlInput}
              placeholder="http://192.168.1.100:7878"
              style="flex:1"
            />
            <button class="btn btn-secondary" disabled={syncTesting} on:click={testConnection}>
              {syncTesting ? 'Testing…' : 'Test Connection'}
            </button>
          </div>
          {#if pingResult === true}
            <p class="ping-ok">✓ Hub server is reachable</p>
          {:else if pingResult === false}
            <p class="ping-fail">✗ Cannot reach hub server — check the URL and ensure the server is running</p>
          {/if}

          <div class="sync-actions">
            {#if $syncMode.mode !== 'server'}
              <button class="btn btn-primary" on:click={connectToServer}
                      disabled={!syncUrlInput.trim()}>
                Connect to Hub
              </button>
            {:else}
              <button class="btn btn-secondary" style="color:var(--danger)" on:click={disconnectFromServer}>
                Disconnect (switch to standalone)
              </button>
            {/if}
          </div>
        </div>

        <div class="card settings-section">
          <h2>Hub Mode</h2>
          <p class="section-desc">
            Turn this machine into the hub. All other Schedula machines on your network
            will connect to the address shown below.
          </p>

          {#if hubRunning}
            <div class="hub-mode-active">
              <div class="hub-status-indicator">
                <span class="hub-dot on"></span>
                <span class="hub-status-label">Hub is running</span>
              </div>
              <div class="hub-url-row">
                <code class="hub-url-display">{hubUrl}</code>
                <button class="btn btn-secondary btn-sm" on:click={copyHubUrl}>Copy</button>
              </div>
              <p class="hub-hint">Share this address with every other admin — paste it into their Hub Server URL field above.</p>
              <div style="margin-top: 14px">
                <button class="btn btn-danger-outline" disabled={hubLoading} on:click={stopHubMode}>
                  {hubLoading ? 'Stopping…' : 'Stop Hub Mode'}
                </button>
              </div>
            </div>
          {:else}
            <div class="sync-how" style="margin-bottom: 16px">
              <div class="how-step"><span class="step-num">1</span><span>On the designated hub machine: enable Hub Mode below</span></div>
              <div class="how-step"><span class="step-num">2</span><span>Copy the address it shows</span></div>
              <div class="how-step"><span class="step-num">3</span><span>On every other machine: paste that address in Hub Server URL above and click Connect</span></div>
            </div>
            <button class="btn btn-primary" disabled={hubLoading} on:click={startHubMode}>
              {hubLoading ? 'Starting…' : 'Enable Hub Mode'}
            </button>
          {/if}
        </div>

        <!-- ── License ── -->
        <div class="card settings-section">
          <h2>License</h2>
          <p class="section-desc">
            Activate your Pro or Institution license token here.
            You receive the token by email after purchasing.
          </p>

          {#if activeHubUrl()}
            <!-- Current license status -->
            {#if licenseInfo}
              <div class="license-status-row">
                <span class="plan-badge {planBadgeClass(licenseInfo.plan)}">{capitalize(licenseInfo.plan)}</span>
                {#if licenseInfo.active}
                  <span class="license-state active">Active</span>
                  {#if licenseInfo.org_name}
                    <span class="license-org">{licenseInfo.org_name}</span>
                  {/if}
                  {#if licenseInfo.expires_at}
                    <span class="license-expiry">Expires {new Date(licenseInfo.expires_at * 1000).toLocaleDateString()}</span>
                  {:else}
                    <span class="license-expiry">Perpetual</span>
                  {/if}
                {:else}
                  <span class="license-state inactive">{licenseInfo.status === 'none' ? 'No license' : licenseInfo.status}</span>
                {/if}
              </div>
            {/if}

            <!-- Activation form -->
            {#if !licenseInfo?.active}
              <div class="license-form">
                <textarea
                  class="form-input license-token-input"
                  bind:value={licenseToken}
                  placeholder="Paste your license token (eyJ…)"
                  rows="3"
                  spellcheck="false"
                ></textarea>
                <button class="btn btn-primary" disabled={licenseLoading || !licenseToken.trim()} on:click={activateLicense}>
                  {licenseLoading ? 'Activating…' : 'Activate License'}
                </button>
              </div>
            {:else}
              <div style="margin-top:12px">
                <button class="btn btn-danger-outline" disabled={licenseLoading} on:click={deactivateLicense}>
                  {licenseLoading ? 'Working…' : 'Deactivate'}
                </button>
              </div>
            {/if}
          {:else}
            <div class="license-no-hub">
              <span>Enable Hub Mode above, or connect to a hub, to manage your license.</span>
            </div>
          {/if}
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
  <div
    class="modal-backdrop"
    on:click|self={() => showResetModal = false}
    on:keydown|self={(e) => { if (e.key === 'Enter' || e.key === ' ') showResetModal = false; }}
    role="button"
    tabindex="-1"
  >
    <div class="modal">
      <div class="modal-header">
        <h2>Reset Password</h2>
        <button class="modal-close" on:click={() => showResetModal = false}>×</button>
      </div>
      <div class="modal-body">
        <div class="form-group">
          <label class="form-label" for="reset-password">New Password</label>
          <input id="reset-password" class="form-input" type="password" bind:value={resetPwd} placeholder="Min 6 characters" />
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

<style>
  .audit-action {
    display: inline-block; padding: 2px 8px; border-radius: 4px;
    font-size: 11px; font-weight: 600; text-transform: uppercase;
  }
  .audit-create  { background: rgba(34,197,94,.15);   color: #4ade80; }
  .audit-update  { background: rgba(251,191,36,.15);  color: #fbbf24; }
  .audit-delete  { background: rgba(239,68,68,.15);   color: #f87171; }
  .audit-generate { background: rgba(108,99,255,.15); color: #a5a0ff; }
  .audit-publish { background: rgba(34,197,94,.15);   color: #4ade80; }
  .audit-revert  { background: rgba(100,100,120,.2);  color: var(--text-muted); }
  .audit-import  { background: rgba(6,182,212,.15);   color: #22d3ee; }

  /* ── Sync tab ── */
  .sync-status-bar {
    display: flex; align-items: center; gap: 10px;
    padding: 12px 16px; border-radius: 8px;
    background: var(--bg); border: 1px solid var(--border);
    font-size: 13px; color: var(--text-muted);
  }
  .sync-dot {
    width: 8px; height: 8px; border-radius: 50%;
    background: var(--text-muted); flex-shrink: 0;
  }
  .sync-dot.on { background: #2ecc71; box-shadow: 0 0 6px #2ecc71; }
  .ws-badge {
    margin-left: auto; font-size: 11px; font-weight: 600;
    color: #2ecc71; background: rgba(46,204,113,.15);
    padding: 2px 8px; border-radius: 10px;
  }
  .ws-badge.offline { color: var(--text-muted); background: transparent; }
  .form-row { display: flex; gap: 10px; align-items: stretch; }
  .ping-ok   { font-size: 12px; color: #2ecc71; margin: 8px 0 0; }
  .ping-fail { font-size: 12px; color: var(--danger); margin: 8px 0 0; }
  .sync-actions { margin-top: 16px; }
  .code-block {
    background: var(--bg); border: 1px solid var(--border);
    border-radius: 6px; padding: 10px 14px; font-family: monospace;
    font-size: 13px; color: var(--text);
  }
  .sync-how { display: flex; flex-direction: column; gap: 10px; margin-top: 12px; }
  .how-step { display: flex; align-items: flex-start; gap: 12px; font-size: 13px; color: var(--text-muted); }
  .step-num {
    width: 22px; height: 22px; border-radius: 50%; flex-shrink: 0;
    background: rgba(108,99,255,.2); color: var(--accent2);
    font-size: 11px; font-weight: 700; display: flex; align-items: center; justify-content: center;
  }
  /* Hub Mode */
  .hub-mode-active { display: flex; flex-direction: column; gap: 10px; }
  .hub-status-indicator { display: flex; align-items: center; gap: 8px; font-size: 13px; font-weight: 600; color: #2ecc71; }
  .hub-dot { width: 8px; height: 8px; border-radius: 50%; background: #2ecc71; flex-shrink: 0; }
  .hub-dot.on { box-shadow: 0 0 6px rgba(46,204,113,.6); }
  .hub-url-row { display: flex; align-items: center; gap: 10px; }
  .hub-url-display {
    flex: 1; padding: 8px 12px; background: var(--bg); border: 1px solid var(--border);
    border-radius: 6px; font-size: 13px; color: var(--text); white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .btn-sm { padding: 6px 12px; font-size: 12px; }
  .hub-hint { font-size: 12px; color: var(--text-muted); margin: 0; }
  .btn-danger-outline {
    background: transparent; border: 1px solid var(--danger); color: var(--danger);
    padding: 8px 16px; border-radius: 6px; cursor: pointer; font-size: 13px;
    transition: background .15s;
  }
  .btn-danger-outline:hover { background: rgba(231,76,60,.1); }
  .btn-danger-outline:disabled { opacity: .5; cursor: not-allowed; }
  /* License */
  .license-status-row { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; margin-bottom: 14px; }
  .plan-badge { font-size: 11px; font-weight: 700; text-transform: uppercase; letter-spacing: .05em; padding: 3px 9px; border-radius: 100px; }
  .plan-badge-free        { background: rgba(255,255,255,.08); color: var(--text-muted); }
  .plan-badge-pro         { background: rgba(59,130,246,.18); color: #93c5fd; }
  .plan-badge-institution { background: rgba(139,92,246,.18); color: #c4b5fd; }
  .license-state { font-size: 12px; font-weight: 600; }
  .license-state.active   { color: #2ecc71; }
  .license-state.inactive { color: var(--text-muted); }
  .license-org    { font-size: 12px; color: var(--text); }
  .license-expiry { font-size: 12px; color: var(--text-muted); }
  .license-form { display: flex; flex-direction: column; gap: 10px; }
  .license-token-input { font-family: monospace; font-size: 11px; resize: vertical; }
  .license-no-hub { font-size: 13px; color: var(--text-muted); padding: 12px 0; }
</style>
