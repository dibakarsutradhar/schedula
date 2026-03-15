<script>
  import { onMount } from 'svelte'
  import Modal from '../lib/components/Modal.svelte'
  import DaySelect from '../lib/components/DaySelect.svelte'
  import { getLecturers, createLecturer, updateLecturer, deleteLecturer } from '../lib/api.js'
  import { session } from '../lib/stores/session.js'
  import { toast } from '../lib/toast.js'

  let lecturers = []
  let showModal = false
  let editing = null

  let form = emptyForm()
  function emptyForm() {
    return { name: '', email: '', available_days: 'Mon,Tue,Wed,Thu,Fri', max_hours_per_day: 4, max_hours_per_week: 16, org_id: $session?.org_id ?? null }
  }

  onMount(load)
  async function load() { lecturers = await getLecturers() }

  function openCreate() { editing = null; form = emptyForm(); showModal = true }
  function openEdit(l) {
    editing = l
    form = { name: l.name, email: l.email ?? '', available_days: l.available_days, max_hours_per_day: l.max_hours_per_day, max_hours_per_week: l.max_hours_per_week }
    showModal = true
  }

  async function save() {
    const payload = { ...form, max_hours_per_day: +form.max_hours_per_day, max_hours_per_week: +form.max_hours_per_week, email: form.email || null, org_id: form.org_id ? +form.org_id : null }
    if (editing) {
      await updateLecturer(editing.id, payload)
      toast('Lecturer updated')
    } else {
      await createLecturer(payload)
      toast('Lecturer added')
    }
    showModal = false
    await load()
  }

  async function remove(l) {
    if (!confirm(`Delete lecturer "${l.name}"?`)) return
    await deleteLecturer(l.id)
    toast('Lecturer deleted', 'error')
    await load()
  }
</script>

<div class="page">
  <div class="page-header">
    <div class="page-header-left">
      <h1>Lecturers</h1>
      <p class="page-subtitle">{lecturers.length} total</p>
    </div>
    <button class="btn btn-primary" on:click={openCreate}>+ Add Lecturer</button>
  </div>

  <div class="card table-wrap">
    {#if lecturers.length === 0}
      <div class="empty-state">No lecturers yet. Add one to get started.</div>
    {:else}
      <table>
        <thead>
          <tr>
            <th>Name</th><th>Email</th><th>Available Days</th>
            <th>Max / Day</th><th>Max / Week</th><th>Actions</th>
          </tr>
        </thead>
        <tbody>
          {#each lecturers as l}
            <tr>
              <td><strong>{l.name}</strong></td>
              <td>{l.email ?? '—'}</td>
              <td>{l.available_days}</td>
              <td>{l.max_hours_per_day} h</td>
              <td>{l.max_hours_per_week} h</td>
              <td>
                <button class="btn btn-secondary btn-sm" on:click={() => openEdit(l)}>Edit</button>
                <button class="btn btn-danger btn-sm" on:click={() => remove(l)}>Delete</button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</div>

{#if showModal}
  <Modal title={editing ? 'Edit Lecturer' : 'Add Lecturer'} onClose={() => (showModal = false)}>
    <div class="modal-body">
      <div class="form-group">
        <label class="form-label">Full Name *</label>
        <input class="form-input" bind:value={form.name} placeholder="Dr. Sarah Ahmed" />
      </div>
      <div class="form-group">
        <label class="form-label">Email</label>
        <input class="form-input" type="email" bind:value={form.email} placeholder="sarah@university.edu" />
      </div>
      <div class="form-group">
        <label class="form-label">Available Days</label>
        <DaySelect bind:value={form.available_days} />
      </div>
      <div class="row">
        <div class="form-group">
          <label class="form-label">Max Hours / Day</label>
          <input class="form-input" type="number" min="1" max="8" bind:value={form.max_hours_per_day} />
        </div>
        <div class="form-group">
          <label class="form-label">Max Hours / Week</label>
          <input class="form-input" type="number" min="1" max="40" bind:value={form.max_hours_per_week} />
        </div>
      </div>
    </div>
    <div class="modal-footer">
      <button class="btn btn-secondary" on:click={() => (showModal = false)}>Cancel</button>
      <button class="btn btn-primary" on:click={save} disabled={!form.name}>
        {editing ? 'Update' : 'Add Lecturer'}
      </button>
    </div>
  </Modal>
{/if}
