<script>
  import { onMount } from 'svelte'
  import Modal from '../lib/components/Modal.svelte'
  import { getCourses, createCourse, updateCourse, deleteCourse, getLecturers } from '../lib/api.js'
  import { toast } from '../lib/toast.js'

  let courses = []
  let lecturers = []
  let showModal = false
  let editing = null

  import { session } from '../lib/stores/session.js'

  let form = emptyForm()
  function emptyForm() {
    return { code: '', name: '', hours_per_week: 3, room_type: 'lecture', class_type: 'lecture', frequency: 'weekly', lecturer_id: null, org_id: $session?.org_id ?? null }
  }

  onMount(async () => {
    [courses, lecturers] = await Promise.all([getCourses(), getLecturers()])
  })

  async function load() {
    courses = await getCourses()
  }

  function openCreate() { editing = null; form = emptyForm(); showModal = true }
  function openEdit(c) {
    editing = c
    form = { code: c.code, name: c.name, hours_per_week: c.hours_per_week, room_type: c.room_type, class_type: c.class_type ?? 'lecture', frequency: c.frequency ?? 'weekly', lecturer_id: c.lecturer_id ?? null, org_id: c.org_id ?? $session?.org_id ?? null }
    showModal = true
  }

  async function save() {
    const payload = { ...form, hours_per_week: +form.hours_per_week, lecturer_id: form.lecturer_id ? +form.lecturer_id : null, org_id: form.org_id ? +form.org_id : null }
    if (editing) {
      await updateCourse(editing.id, payload)
      toast('Course updated')
    } else {
      await createCourse(payload)
      toast('Course added')
    }
    showModal = false
    await load()
  }

  async function remove(c) {
    if (!confirm(`Delete course "${c.code}"?`)) return
    await deleteCourse(c.id)
    toast('Course deleted', 'error')
    await load()
  }
</script>

<div class="page">
  <div class="page-header">
    <div class="page-header-left">
      <h1>Courses</h1>
      <p class="page-subtitle">{courses.length} total</p>
    </div>
    <button class="btn btn-primary" on:click={openCreate}>+ Add Course</button>
  </div>

  <div class="card table-wrap">
    {#if courses.length === 0}
      <div class="empty-state">No courses yet.</div>
    {:else}
      <table>
        <thead>
          <tr>
            <th>Code</th><th>Name</th><th>Hrs/Week</th>
            <th>Room Type</th><th>Lecturer</th><th>Actions</th>
          </tr>
        </thead>
        <tbody>
          {#each courses as c}
            <tr>
              <td><strong>{c.code}</strong></td>
              <td>{c.name}</td>
              <td>{c.hours_per_week}</td>
              <td>
                <span class="badge badge-{c.room_type}">{c.room_type}</span>
              </td>
              <td>
                {#if c.lecturer_name}
                  {c.lecturer_name}
                {:else}
                  <span style="color:var(--warning)">⚠ Unassigned</span>
                {/if}
              </td>
              <td>
                <button class="btn btn-secondary btn-sm" on:click={() => openEdit(c)}>Edit</button>
                <button class="btn btn-danger btn-sm" on:click={() => remove(c)}>Delete</button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</div>

{#if showModal}
  <Modal title={editing ? 'Edit Course' : 'Add Course'} onClose={() => (showModal = false)}>
    <div class="modal-body">
      <div class="row">
        <div class="form-group">
          <label class="form-label">Course Code *</label>
          <input class="form-input" bind:value={form.code} placeholder="CSE101" />
        </div>
        <div class="form-group">
          <label class="form-label">Hours / Week *</label>
          <input class="form-input" type="number" min="1" max="8" bind:value={form.hours_per_week} />
        </div>
      </div>
      <div class="form-group">
        <label class="form-label">Course Name *</label>
        <input class="form-input" bind:value={form.name} placeholder="Introduction to Programming" />
      </div>
      <div class="row">
        <div class="form-group">
          <label class="form-label">Class Type *</label>
          <select class="form-select" bind:value={form.class_type} on:change={() => { if (form.class_type === 'lab') { form.room_type = 'lab'; form.frequency = 'biweekly' } else { form.room_type = 'lecture'; form.frequency = 'weekly' } }}>
            <option value="lecture">Lecture</option>
            <option value="lab">Lab</option>
            <option value="tutorial">Tutorial</option>
          </select>
        </div>
        <div class="form-group">
          <label class="form-label">Frequency *</label>
          <select class="form-select" bind:value={form.frequency}>
            <option value="weekly">Weekly</option>
            <option value="biweekly">Bi-weekly (alt. weeks)</option>
          </select>
        </div>
      </div>
      <div class="row">
        <div class="form-group">
          <label class="form-label">Room Type *</label>
          <select class="form-select" bind:value={form.room_type}>
            <option value="lecture">Lecture Room</option>
            <option value="lab">Lab</option>
          </select>
        </div>
        <div class="form-group">
          <label class="form-label">Assign Lecturer</label>
          <select class="form-select" bind:value={form.lecturer_id}>
            <option value={null}>— None —</option>
            {#each lecturers as l}
              <option value={l.id}>{l.name}</option>
            {/each}
          </select>
        </div>
      </div>
    </div>
    <div class="modal-footer">
      <button class="btn btn-secondary" on:click={() => (showModal = false)}>Cancel</button>
      <button class="btn btn-primary" on:click={save} disabled={!form.code || !form.name}>
        {editing ? 'Update' : 'Add Course'}
      </button>
    </div>
  </Modal>
{/if}
