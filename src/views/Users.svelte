<script>
  import { onMount } from 'svelte'
  import Modal from '../lib/components/Modal.svelte'
  import { getUsers, createUser, deleteUser, getOrganizations, changePassword } from '../lib/api.js'
  import { getMaxAdmins, getAdminCount } from '../lib/api.js'
  import { session, isSuperAdmin } from '../lib/stores/session.js'
  import { toast } from '../lib/toast.js'

  let users = []
  let org = null
  let maxAdmins = 2
  let adminCount = 0
  let showModal = false
  let showPwdModal = false
  let form = emptyForm()
  let pwdForm = { old: '', new1: '', new2: '' }

  function emptyForm() {
    return { username: '', display_name: '', password: '' }
  }

  onMount(load)

  async function load() {
    const [u, orgs, max, count] = await Promise.all([
      getUsers(),
      getOrganizations(),
      getMaxAdmins(),
      getAdminCount(),
    ])
    users = u
    org = orgs[0] ?? null
    maxAdmins = max
    adminCount = count
  }

  async function save() {
    try {
      await createUser({
        username:     form.username.trim(),
        display_name: form.display_name.trim(),
        password:     form.password,
        role:         'admin',
        org_id:       org?.id ?? null,
      })
      toast('Admin created')
      showModal = false
      form = emptyForm()
      await load()
    } catch (e) {
      toast(String(e), 'error')
    }
  }

  async function remove(u) {
    if (!confirm(`Delete user "${u.username}"? This cannot be undone.`)) return
    try {
      await deleteUser(u.id)
      toast('User deleted', 'error')
      await load()
    } catch (e) {
      toast(String(e), 'error')
    }
  }

  async function savePwd() {
    if (pwdForm.new1 !== pwdForm.new2) { toast('Passwords do not match', 'error'); return }
    if (pwdForm.new1.length < 6) { toast('Password must be at least 6 characters', 'error'); return }
    try {
      await changePassword(pwdForm.old, pwdForm.new1)
      toast('Password changed')
      showPwdModal = false
      pwdForm = { old: '', new1: '', new2: '' }
    } catch (e) {
      toast(String(e), 'error')
    }
  }

  $: atLimit = adminCount >= maxAdmins
  $: superAdmin = users.find(u => u.role === 'super_admin')
  $: admins = users.filter(u => u.role === 'admin')
</script>

<div class="page">
  <div class="page-header">
    <div class="page-header-left">
      <h1>Users</h1>
      <p class="page-subtitle">{users.length} total</p>
    </div>
    <div style="display:flex;gap:10px;align-items:center">
      <button class="btn btn-secondary" on:click={() => (showPwdModal = true)}>🔑 Change My Password</button>
      {#if isSuperAdmin($session)}
        <button
          class="btn btn-primary"
          on:click={() => { form = emptyForm(); showModal = true }}
          disabled={atLimit}
          title={atLimit ? `Admin limit reached (${adminCount}/${maxAdmins}). Increase in Settings → System.` : ''}
        >
          + Add Admin
        </button>
      {/if}
    </div>
  </div>

  <!-- Admin quota bar -->
  {#if isSuperAdmin($session)}
    <div class="quota-bar">
      <div class="quota-label">
        <span>Admin accounts</span>
        <span class="quota-count" class:at-limit={atLimit}>
          {adminCount} / {maxAdmins}
        </span>
      </div>
      <div class="quota-track">
        <div
          class="quota-fill"
          class:at-limit={atLimit}
          style="width: {Math.min(100, (adminCount / maxAdmins) * 100)}%"
        ></div>
      </div>
      {#if atLimit}
        <p class="quota-hint">
          Admin limit reached. Go to <strong>Settings → System</strong> to increase the maximum.
        </p>
      {/if}
    </div>
  {/if}

  <!-- Super Admin section -->
  {#if superAdmin}
    <div class="section-label">Super Admin</div>
    <div class="card table-wrap" style="margin-bottom:20px">
      <table>
        <thead>
          <tr><th>Username</th><th>Name</th><th>Role</th><th>Status</th></tr>
        </thead>
        <tbody>
          <tr>
            <td><code style="font-size:13px">{superAdmin.username}</code></td>
            <td><strong>{superAdmin.display_name}</strong> {#if superAdmin.id === $session?.user_id}<span class="you-badge">you</span>{/if}</td>
            <td><span class="role-badge role-super">super admin</span></td>
            <td><span class="badge badge-active">Active</span></td>
          </tr>
        </tbody>
      </table>
    </div>
  {/if}

  <!-- Admins section -->
  <div class="section-label">Admins</div>
  <div class="card table-wrap">
    {#if admins.length === 0}
      <div class="empty-state">
        No admins yet.
        {#if isSuperAdmin($session)}
          <br><small>Click "+ Add Admin" to create one.</small>
        {/if}
      </div>
    {:else}
      <table>
        <thead>
          <tr>
            <th>Username</th><th>Name</th><th>Status</th>
            {#if isSuperAdmin($session)}<th>Actions</th>{/if}
          </tr>
        </thead>
        <tbody>
          {#each admins as u}
            <tr>
              <td><code style="font-size:13px">{u.username}</code></td>
              <td>
                <strong>{u.display_name}</strong>
                {#if u.id === $session?.user_id}<span class="you-badge">you</span>{/if}
              </td>
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
                    <button class="btn btn-danger btn-sm" on:click={() => remove(u)}>Delete</button>
                  {:else}
                    <span style="font-size:12px;color:var(--text-muted)">—</span>
                  {/if}
                </td>
              {/if}
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</div>

<!-- Add Admin Modal -->
{#if showModal}
  <Modal title="Add Admin" onClose={() => (showModal = false)}>
    <div class="modal-body">
      <div class="row">
        <div class="form-group">
          <label class="form-label">Username *</label>
          <input class="form-input" bind:value={form.username} placeholder="jsmith" autofocus />
        </div>
        <div class="form-group">
          <label class="form-label">Display Name *</label>
          <input class="form-input" bind:value={form.display_name} placeholder="John Smith" />
        </div>
      </div>
      <div class="form-group">
        <label class="form-label">Password *</label>
        <input class="form-input" type="password" bind:value={form.password} placeholder="Min 6 characters" />
      </div>
      {#if org}
        <p style="font-size:12px;color:var(--text-muted)">
          Will be linked to: <strong>{org.name}</strong>
        </p>
      {/if}
    </div>
    <div class="modal-footer">
      <button class="btn btn-secondary" on:click={() => (showModal = false)}>Cancel</button>
      <button
        class="btn btn-primary"
        on:click={save}
        disabled={!form.username.trim() || !form.password || !form.display_name.trim()}
      >Create Admin</button>
    </div>
  </Modal>
{/if}

<!-- Change Password Modal -->
{#if showPwdModal}
  <Modal title="Change My Password" onClose={() => (showPwdModal = false)}>
    <div class="modal-body">
      <div class="form-group">
        <label class="form-label">Current Password</label>
        <input class="form-input" type="password" bind:value={pwdForm.old} autofocus />
      </div>
      <div class="form-group">
        <label class="form-label">New Password</label>
        <input class="form-input" type="password" bind:value={pwdForm.new1} />
      </div>
      <div class="form-group">
        <label class="form-label">Confirm New Password</label>
        <input class="form-input" type="password" bind:value={pwdForm.new2} />
      </div>
    </div>
    <div class="modal-footer">
      <button class="btn btn-secondary" on:click={() => (showPwdModal = false)}>Cancel</button>
      <button class="btn btn-primary" on:click={savePwd} disabled={!pwdForm.old || !pwdForm.new1}>Update Password</button>
    </div>
  </Modal>
{/if}

<style>
  .section-label {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: .08em;
    color: var(--text-muted);
    margin: 0 0 8px;
  }

  .role-badge {
    display: inline-flex;
    align-items: center;
    padding: 2px 9px;
    border-radius: 99px;
    font-size: 11px;
    font-weight: 600;
    text-transform: capitalize;
  }
  .role-super {
    background: rgba(108,99,255,.15);
    color: var(--accent2);
  }

  .you-badge {
    display: inline-flex;
    align-items: center;
    padding: 1px 7px;
    border-radius: 99px;
    font-size: 10px;
    font-weight: 600;
    background: rgba(34,197,94,.12);
    color: var(--success);
    margin-left: 6px;
    vertical-align: middle;
  }

  /* ── Quota bar ──────────────────────────────────────────────────────────── */
  .quota-bar {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 14px 16px;
    margin-bottom: 20px;
  }
  .quota-label {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 13px;
    margin-bottom: 8px;
    color: var(--text-muted);
  }
  .quota-count {
    font-weight: 700;
    color: var(--text);
    font-size: 14px;
  }
  .quota-count.at-limit { color: var(--danger); }

  .quota-track {
    height: 6px;
    background: var(--surface2);
    border-radius: 3px;
    overflow: hidden;
  }
  .quota-fill {
    height: 100%;
    background: var(--accent);
    border-radius: 3px;
    transition: width .4s ease;
  }
  .quota-fill.at-limit { background: var(--danger); }

  .quota-hint {
    font-size: 12px;
    color: var(--danger);
    margin-top: 8px;
  }
  .quota-hint strong { color: var(--danger); }
</style>
