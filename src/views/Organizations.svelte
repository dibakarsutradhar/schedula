<script>
  import { onMount } from 'svelte'
  import Modal from '../lib/components/Modal.svelte'
  import { getOrganizations, createOrganization, updateOrganization, deleteOrganization } from '../lib/api.js'
  import { toast } from '../lib/toast.js'

  let orgs = []
  let showModal = false
  let editing = null
  let form = emptyForm()

  function emptyForm() {
    return { name: '', org_type: 'university', address: '' }
  }

  onMount(load)
  async function load() { orgs = await getOrganizations() }

  function openCreate() { editing = null; form = emptyForm(); showModal = true }
  function openEdit(o) { editing = o; form = { name: o.name, org_type: o.org_type, address: o.address ?? '' }; showModal = true }

  async function save() {
    const payload = { ...form, address: form.address || null }
    if (editing) { await updateOrganization(editing.id, payload); toast('Organization updated') }
    else { await createOrganization(payload); toast('Organization created') }
    showModal = false; await load()
  }

  async function remove(o) {
    if (!confirm(`Delete "${o.name}"? This removes all associated semesters and data.`)) return
    await deleteOrganization(o.id)
    toast('Organization deleted', 'error'); await load()
  }

  const typeLabel = { university: '🎓 University', college: '🏫 College', school: '📖 School', institute: '🔬 Institute' }
</script>

<div class="page">
  <div class="page-header">
    <div class="page-header-left">
      <h1>Organizations</h1>
      <p class="page-subtitle">{orgs.length} registered</p>
    </div>
    <button class="btn btn-primary" on:click={openCreate}>+ Add Organization</button>
  </div>

  <div class="card table-wrap">
    {#if orgs.length === 0}
      <div class="empty-state">No organizations yet. Add one to get started.</div>
    {:else}
      <table>
        <thead>
          <tr><th>Name</th><th>Type</th><th>Address</th><th>Actions</th></tr>
        </thead>
        <tbody>
          {#each orgs as o}
            <tr>
              <td><strong>{o.name}</strong></td>
              <td>{typeLabel[o.org_type] ?? o.org_type}</td>
              <td>{o.address ?? '—'}</td>
              <td>
                <button class="btn btn-secondary btn-sm" on:click={() => openEdit(o)}>Edit</button>
                <button class="btn btn-danger btn-sm" on:click={() => remove(o)}>Delete</button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</div>

{#if showModal}
  <Modal title={editing ? 'Edit Organization' : 'Add Organization'} onClose={() => (showModal = false)}>
    <div class="modal-body">
      <div class="form-group">
        <label class="form-label">Name *</label>
        <input class="form-input" bind:value={form.name} placeholder="University of Dhaka" />
      </div>
      <div class="form-group">
        <label class="form-label">Type *</label>
        <select class="form-select" bind:value={form.org_type}>
          <option value="university">University</option>
          <option value="college">College</option>
          <option value="school">School</option>
          <option value="institute">Institute</option>
        </select>
      </div>
      <div class="form-group">
        <label class="form-label">Address</label>
        <textarea class="form-textarea" bind:value={form.address} placeholder="City, Country" rows="2"></textarea>
      </div>
    </div>
    <div class="modal-footer">
      <button class="btn btn-secondary" on:click={() => (showModal = false)}>Cancel</button>
      <button class="btn btn-primary" on:click={save} disabled={!form.name}>{editing ? 'Update' : 'Create'}</button>
    </div>
  </Modal>
{/if}
