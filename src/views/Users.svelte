<script>
  import { onMount } from 'svelte'
  import Modal from '../lib/components/Modal.svelte'
  import { getUsers, createUser, deleteUser, getOrganizations, changePassword } from '../lib/api.js'
  import { session, isSuperAdmin } from '../lib/stores/session.js'
  import { toast } from '../lib/toast.js'

  let users = []
  let orgs = []
  let showModal = false
  let showPwdModal = false
  let form = emptyForm()
  let pwdForm = { old: '', new1: '', new2: '' }

  function emptyForm() {
    return { username: '', display_name: '', password: '', role: 'admin', org_id: null }
  }

  onMount(async () => {
    ;[users, orgs] = await Promise.all([getUsers(), getOrganizations()])
  })
  async function load() { users = await getUsers() }

  async function save() {
    await createUser({ ...form, org_id: form.org_id ? +form.org_id : null })
    toast('User created'); showModal = false; await load()
  }

  async function remove(u) {
    if (!confirm(`Delete user "${u.username}"?`)) return
    await deleteUser(u.id); toast('User deleted', 'error'); await load()
  }

  async function savePwd() {
    if (pwdForm.new1 !== pwdForm.new2) { toast('Passwords do not match', 'error'); return }
    await changePassword(pwdForm.old, pwdForm.new1)
    toast('Password changed'); showPwdModal = false
    pwdForm = { old: '', new1: '', new2: '' }
  }

  const roleColor = { super_admin: 'var(--accent)', admin: 'var(--success)' }
</script>

<div class="page">
  <div class="page-header">
    <div class="page-header-left">
      <h1>Users</h1>
      <p class="page-subtitle">{users.length} total</p>
    </div>
    <div style="display:flex;gap:10px">
      <button class="btn btn-secondary" on:click={() => (showPwdModal = true)}>🔑 Change My Password</button>
      {#if isSuperAdmin($session)}
        <button class="btn btn-primary" on:click={() => { form = emptyForm(); showModal = true }}>+ Add User</button>
      {/if}
    </div>
  </div>

  <div class="card table-wrap">
    {#if users.length === 0}
      <div class="empty-state">No users found.</div>
    {:else}
      <table>
        <thead>
          <tr><th>Username</th><th>Display Name</th><th>Role</th><th>Organization</th><th>Actions</th></tr>
        </thead>
        <tbody>
          {#each users as u}
            <tr>
              <td><code style="font-size:13px">{u.username}</code></td>
              <td>{u.display_name}</td>
              <td><span style="color:{roleColor[u.role]};font-weight:600;text-transform:capitalize">{u.role.replace('_', ' ')}</span></td>
              <td>{u.org_name ?? '— Global —'}</td>
              <td>
                {#if isSuperAdmin($session) && u.id !== $session?.user_id}
                  <button class="btn btn-danger btn-sm" on:click={() => remove(u)}>Delete</button>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</div>

{#if showModal}
  <Modal title="Add User" onClose={() => (showModal = false)}>
    <div class="modal-body">
      <div class="row">
        <div class="form-group">
          <label class="form-label">Username *</label>
          <input class="form-input" bind:value={form.username} placeholder="jsmith" />
        </div>
        <div class="form-group">
          <label class="form-label">Display Name *</label>
          <input class="form-input" bind:value={form.display_name} placeholder="John Smith" />
        </div>
      </div>
      <div class="form-group">
        <label class="form-label">Password *</label>
        <input class="form-input" type="password" bind:value={form.password} />
      </div>
      <div class="row">
        <div class="form-group">
          <label class="form-label">Role *</label>
          <select class="form-select" bind:value={form.role}>
            <option value="admin">Admin</option>
            <option value="super_admin">Super Admin</option>
          </select>
        </div>
        <div class="form-group">
          <label class="form-label">Organization</label>
          <select class="form-select" bind:value={form.org_id}>
            <option value={null}>— Global —</option>
            {#each orgs as o}<option value={o.id}>{o.name}</option>{/each}
          </select>
        </div>
      </div>
    </div>
    <div class="modal-footer">
      <button class="btn btn-secondary" on:click={() => (showModal = false)}>Cancel</button>
      <button class="btn btn-primary" on:click={save} disabled={!form.username || !form.password || !form.display_name}>Create User</button>
    </div>
  </Modal>
{/if}

{#if showPwdModal}
  <Modal title="Change Password" onClose={() => (showPwdModal = false)}>
    <div class="modal-body">
      <div class="form-group">
        <label class="form-label">Current Password</label>
        <input class="form-input" type="password" bind:value={pwdForm.old} />
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
