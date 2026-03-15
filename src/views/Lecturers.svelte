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
  let preferredSlots = {}   // {Mon: 'any', Tue: 'morning', ...}

  function emptyForm() {
    return { name: '', email: '', available_days: 'Mon,Tue,Wed,Thu,Fri', max_hours_per_day: 4, max_hours_per_week: 16, max_consecutive_hours: 3, org_id: $session?.org_id ?? null }
  }

  $: availDays = form.available_days.split(',').map(d => d.trim()).filter(Boolean)

  onMount(load)
  async function load() { lecturers = await getLecturers() }

  function openCreate() {
    editing = null
    form = emptyForm()
    preferredSlots = Object.fromEntries(['Mon','Tue','Wed','Thu','Fri'].map(d => [d, 'any']))
    showModal = true
  }

  function openEdit(l) {
    editing = l
    const parsed = l.preferred_slots_json ? JSON.parse(l.preferred_slots_json) : {}
    preferredSlots = Object.fromEntries(
      l.available_days.split(',').map(d => d.trim()).filter(Boolean).map(d => [d, parsed[d] ?? 'any'])
    )
    form = {
      name: l.name, email: l.email ?? '',
      available_days: l.available_days,
      max_hours_per_day: l.max_hours_per_day,
      max_hours_per_week: l.max_hours_per_week,
      max_consecutive_hours: l.max_consecutive_hours ?? 3,
      org_id: l.org_id ?? null,
    }
    showModal = true
  }

  async function save() {
    const hasPrefs = Object.values(preferredSlots).some(v => v !== 'any')
    const preferred_slots_json = hasPrefs
      ? JSON.stringify(Object.fromEntries(availDays.map(d => [d, preferredSlots[d] ?? 'any'])))
      : null
    const payload = {
      name: form.name, email: form.email || null,
      available_days: form.available_days,
      max_hours_per_day: +form.max_hours_per_day,
      max_hours_per_week: +form.max_hours_per_week,
      max_consecutive_hours: +form.max_consecutive_hours,
      preferred_slots_json,
      blackout_json: editing?.blackout_json ?? null,
      org_id: form.org_id ? +form.org_id : null,
    }
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
        <div class="form-group">
          <label class="form-label">Max Consecutive Hours</label>
          <input class="form-input" type="number" min="1" max="6" bind:value={form.max_consecutive_hours} style="max-width:90px" />
        </div>
      </div>

      {#if availDays.length > 0}
        <div class="form-group" style="margin-top:4px">
          <label class="form-label">Preferred Time per Day</label>
          <div class="day-pref-grid">
            {#each availDays as day}
              <div class="day-pref-row">
                <span class="day-lbl">{day}</span>
                <select class="form-select" style="flex:1" bind:value={preferredSlots[day]}>
                  <option value="any">Any time</option>
                  <option value="morning">Morning  (8–12)</option>
                  <option value="afternoon">Afternoon (1–5)</option>
                </select>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </div>
    <div class="modal-footer">
      <button class="btn btn-secondary" on:click={() => (showModal = false)}>Cancel</button>
      <button class="btn btn-primary" on:click={save} disabled={!form.name}>
        {editing ? 'Update' : 'Add Lecturer'}
      </button>
    </div>
  </Modal>
{/if}

<style>
  .day-pref-grid { display: flex; flex-direction: column; gap: 6px; margin-top: 6px; }
  .day-pref-row  { display: flex; align-items: center; gap: 10px; }
  .day-lbl       { width: 36px; font-size: 12px; font-weight: 600; color: var(--text-muted); }
</style>
