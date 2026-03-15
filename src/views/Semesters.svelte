<script>
  import { onMount } from 'svelte'
  import Modal from '../lib/components/Modal.svelte'
  import { getSemesters, createSemester, updateSemester, deleteSemester, getOrganizations } from '../lib/api.js'
  import { session, isSuperAdmin } from '../lib/stores/session.js'
  import { toast } from '../lib/toast.js'

  let semesters = []
  let orgs = []
  let showModal = false
  let editing = null
  let form = emptyForm()

  function emptyForm() {
    return {
      org_id: $session?.org_id ?? null,
      name: '',
      start_date: '',
      end_date: '',
      student_capacity: null,
      teaching_weeks: 14,
      midterm_start: '',
      midterm_end: '',
      study_break_start: '',
      study_break_end: '',
      final_start: '',
      final_end: '',
      breaks_json: '[]',
      status: 'planning',
    }
  }

  onMount(async () => {
    ;[semesters, orgs] = await Promise.all([getSemesters(), getOrganizations()])
  })
  async function load() { semesters = await getSemesters() }

  function openCreate() { editing = null; form = emptyForm(); showModal = true }
  function openEdit(s) {
    editing = s
    form = {
      org_id: s.org_id, name: s.name, start_date: s.start_date, end_date: s.end_date,
      student_capacity: s.student_capacity, teaching_weeks: s.teaching_weeks,
      midterm_start: s.midterm_start ?? '', midterm_end: s.midterm_end ?? '',
      study_break_start: s.study_break_start ?? '', study_break_end: s.study_break_end ?? '',
      final_start: s.final_start ?? '', final_end: s.final_end ?? '',
      breaks_json: s.breaks_json, status: s.status,
    }
    showModal = true
  }

  function nullEmpty(v) { return v === '' ? null : v }

  async function save() {
    const payload = {
      ...form,
      org_id: +form.org_id,
      teaching_weeks: +form.teaching_weeks,
      student_capacity: form.student_capacity ? +form.student_capacity : null,
      midterm_start: nullEmpty(form.midterm_start),
      midterm_end: nullEmpty(form.midterm_end),
      study_break_start: nullEmpty(form.study_break_start),
      study_break_end: nullEmpty(form.study_break_end),
      final_start: nullEmpty(form.final_start),
      final_end: nullEmpty(form.final_end),
    }
    if (editing) { await updateSemester(editing.id, payload); toast('Semester updated') }
    else { await createSemester(payload); toast('Semester created') }
    showModal = false; await load()
  }

  async function remove(s) {
    if (!confirm(`Delete semester "${s.name}"?`)) return
    await deleteSemester(s.id); toast('Semester deleted', 'error'); await load()
  }

  const statusColor = { planning: 'var(--text-muted)', active: 'var(--success)', completed: 'var(--warning)' }
</script>

<div class="page">
  <div class="page-header">
    <div class="page-header-left">
      <h1>Semesters</h1>
      <p class="page-subtitle">{semesters.length} total</p>
    </div>
    <button class="btn btn-primary" on:click={openCreate}>+ Add Semester</button>
  </div>

  <div class="card table-wrap">
    {#if semesters.length === 0}
      <div class="empty-state">No semesters yet.</div>
    {:else}
      <table>
        <thead>
          <tr>
            <th>Semester</th><th>Organization</th><th>Duration</th>
            <th>Weeks</th><th>Blocks</th><th>Status</th><th>Actions</th>
          </tr>
        </thead>
        <tbody>
          {#each semesters as s}
            <tr>
              <td><strong>{s.name}</strong></td>
              <td>{s.org_name}</td>
              <td style="font-size:12px">{s.start_date} → {s.end_date}</td>
              <td>{s.teaching_weeks} teaching</td>
              <td>
                <div style="display:flex;gap:6px;flex-wrap:wrap">
                  {#if s.midterm_start}<span class="badge" style="background:rgba(245,158,11,.15);color:#fbbf24">Midterm</span>{/if}
                  {#if s.study_break_start}<span class="badge" style="background:rgba(6,182,212,.15);color:#22d3ee">Study Break</span>{/if}
                  {#if s.final_start}<span class="badge" style="background:rgba(239,68,68,.15);color:#f87171">Finals</span>{/if}
                </div>
              </td>
              <td><span style="color:{statusColor[s.status]};font-weight:600;text-transform:capitalize">{s.status}</span></td>
              <td>
                <button class="btn btn-secondary btn-sm" on:click={() => openEdit(s)}>Edit</button>
                <button class="btn btn-danger btn-sm" on:click={() => remove(s)}>Delete</button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</div>

{#if showModal}
  <Modal title={editing ? 'Edit Semester' : 'New Semester'} onClose={() => (showModal = false)}>
    <div class="modal-body">
      <!-- Basic Info -->
      {#if isSuperAdmin($session)}
        <div class="form-group">
          <label class="form-label">Organization *</label>
          <select class="form-select" bind:value={form.org_id}>
            <option value={null}>— select —</option>
            {#each orgs as o}<option value={o.id}>{o.name}</option>{/each}
          </select>
        </div>
      {/if}
      <div class="form-group">
        <label class="form-label">Semester Name *</label>
        <input class="form-input" bind:value={form.name} placeholder="Spring 2025 — Sem 1" />
      </div>
      <div class="row">
        <div class="form-group">
          <label class="form-label">Start Date *</label>
          <input class="form-input" type="date" bind:value={form.start_date} />
        </div>
        <div class="form-group">
          <label class="form-label">End Date *</label>
          <input class="form-input" type="date" bind:value={form.end_date} />
        </div>
      </div>
      <div class="row">
        <div class="form-group">
          <label class="form-label">Teaching Weeks</label>
          <input class="form-input" type="number" min="1" max="30" bind:value={form.teaching_weeks} />
        </div>
        <div class="form-group">
          <label class="form-label">Student Capacity</label>
          <input class="form-input" type="number" min="1" bind:value={form.student_capacity} placeholder="optional" />
        </div>
      </div>

      <hr class="divider" />
      <h3 style="color:var(--text-muted);font-size:12px;text-transform:uppercase;letter-spacing:.06em">Exam & Study Blocks</h3>

      <div class="block-section">
        <div class="block-label" style="color:#fbbf24">📋 Midterm Exam</div>
        <div class="row">
          <div class="form-group">
            <label class="form-label">Start</label>
            <input class="form-input" type="date" bind:value={form.midterm_start} />
          </div>
          <div class="form-group">
            <label class="form-label">End</label>
            <input class="form-input" type="date" bind:value={form.midterm_end} />
          </div>
        </div>
      </div>

      <div class="block-section">
        <div class="block-label" style="color:#22d3ee">📚 Study Break</div>
        <div class="row">
          <div class="form-group">
            <label class="form-label">Start</label>
            <input class="form-input" type="date" bind:value={form.study_break_start} />
          </div>
          <div class="form-group">
            <label class="form-label">End</label>
            <input class="form-input" type="date" bind:value={form.study_break_end} />
          </div>
        </div>
      </div>

      <div class="block-section">
        <div class="block-label" style="color:#f87171">🎯 Final Exams</div>
        <div class="row">
          <div class="form-group">
            <label class="form-label">Start</label>
            <input class="form-input" type="date" bind:value={form.final_start} />
          </div>
          <div class="form-group">
            <label class="form-label">End</label>
            <input class="form-input" type="date" bind:value={form.final_end} />
          </div>
        </div>
      </div>

      <div class="form-group">
        <label class="form-label">Status</label>
        <select class="form-select" bind:value={form.status}>
          <option value="planning">Planning</option>
          <option value="active">Active</option>
          <option value="completed">Completed</option>
        </select>
      </div>
    </div>
    <div class="modal-footer">
      <button class="btn btn-secondary" on:click={() => (showModal = false)}>Cancel</button>
      <button class="btn btn-primary" on:click={save} disabled={!form.name || !form.start_date || !form.end_date}>
        {editing ? 'Update' : 'Create Semester'}
      </button>
    </div>
  </Modal>
{/if}

<style>
  .block-section { display: flex; flex-direction: column; gap: 8px; }
  .block-label { font-size: 12px; font-weight: 600; margin-bottom: 4px; }
</style>
