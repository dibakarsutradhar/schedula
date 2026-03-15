<script>
  import { onMount } from 'svelte'
  import Modal from '../lib/components/Modal.svelte'
  import { getBatches, createBatch, updateBatch, deleteBatch, getCourses, getSemesters } from '../lib/api.js'
  import { session } from '../lib/stores/session.js'
  import { toast } from '../lib/toast.js'

  let batches = []
  let courses = []
  let semesters = []
  let showModal = false
  let editing = null

  let form = emptyForm()
  function emptyForm() {
    return { name: '', department: '', semester: 1, size: 30, course_ids: [], org_id: $session?.org_id ?? null, semester_id: null }
  }

  onMount(async () => {
    ;[batches, courses, semesters] = await Promise.all([getBatches(), getCourses(), getSemesters()])
  })

  async function load() { batches = await getBatches() }

  function openCreate() { editing = null; form = emptyForm(); showModal = true }
  function openEdit(b) {
    editing = b
    form = { name: b.name, department: b.department, semester: b.semester, size: b.size, course_ids: [...b.course_ids], org_id: b.org_id ?? $session?.org_id ?? null, semester_id: b.semester_id ?? null }
    showModal = true
  }

  function toggleCourse(id) {
    const idx = form.course_ids.indexOf(id)
    if (idx >= 0) form.course_ids = form.course_ids.filter(x => x !== id)
    else form.course_ids = [...form.course_ids, id]
  }

  async function save() {
    const payload = { ...form, semester: +form.semester, size: +form.size, org_id: form.org_id ? +form.org_id : null, semester_id: form.semester_id ? +form.semester_id : null }
    if (editing) {
      await updateBatch(editing.id, payload)
      toast('Batch updated')
    } else {
      await createBatch(payload)
      toast('Batch added')
    }
    showModal = false
    await load()
  }

  async function remove(b) {
    if (!confirm(`Delete batch "${b.name}"?`)) return
    await deleteBatch(b.id)
    toast('Batch deleted', 'error')
    await load()
  }

  function courseName(id) {
    return courses.find(c => c.id === id)?.code ?? id
  }
</script>

<div class="page">
  <div class="page-header">
    <div class="page-header-left">
      <h1>Batches</h1>
      <p class="page-subtitle">{batches.length} total</p>
    </div>
    <button class="btn btn-primary" on:click={openCreate}>+ Add Batch</button>
  </div>

  <div class="card table-wrap">
    {#if batches.length === 0}
      <div class="empty-state">No batches yet.</div>
    {:else}
      <table>
        <thead>
          <tr>
            <th>Name</th><th>Department</th><th>Semester</th>
            <th>Size</th><th>Courses</th><th>Actions</th>
          </tr>
        </thead>
        <tbody>
          {#each batches as b}
            <tr>
              <td><strong>{b.name}</strong></td>
              <td>{b.department}</td>
              <td>{b.semester}</td>
              <td>{b.size}</td>
              <td>
                <div class="chip-row">
                  {#each b.course_ids as cid}
                    <span class="chip">{courseName(cid)}</span>
                  {/each}
                  {#if b.course_ids.length === 0}
                    <span style="color:var(--text-muted);font-size:12px">None</span>
                  {/if}
                </div>
              </td>
              <td>
                <button class="btn btn-secondary btn-sm" on:click={() => openEdit(b)}>Edit</button>
                <button class="btn btn-danger btn-sm" on:click={() => remove(b)}>Delete</button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</div>

{#if showModal}
  <Modal title={editing ? 'Edit Batch' : 'Add Batch'} onClose={() => (showModal = false)}>
    <div class="modal-body">
      <div class="form-group">
        <label class="form-label">Batch Name *</label>
        <input class="form-input" bind:value={form.name} placeholder="CSE-2A" />
      </div>
      <div class="form-group">
        <label class="form-label">Department *</label>
        <input class="form-input" bind:value={form.department} placeholder="Computer Science & Engineering" />
      </div>
      <div class="row">
        <div class="form-group">
          <label class="form-label">Semester *</label>
          <input class="form-input" type="number" min="1" max="12" bind:value={form.semester} />
        </div>
        <div class="form-group">
          <label class="form-label">Student Count *</label>
          <input class="form-input" type="number" min="1" bind:value={form.size} />
        </div>
      </div>
      <div class="form-group">
        <label class="form-label">Semester</label>
        <select class="form-select" bind:value={form.semester_id}>
          <option value={null}>— Not linked —</option>
          {#each semesters as s}
            <option value={s.id}>{s.name}</option>
          {/each}
        </select>
      </div>
      <div class="form-group">
        <label class="form-label">Enrolled Courses</label>
        <div class="course-picker">
          {#each courses as c}
            <label class="course-check">
              <input
                type="checkbox"
                checked={form.course_ids.includes(c.id)}
                on:change={() => toggleCourse(c.id)}
              />
              <span class="badge badge-{c.room_type}" style="margin-left:6px">{c.code}</span>
              <span style="margin-left:6px;font-size:13px">{c.name}</span>
            </label>
          {/each}
          {#if courses.length === 0}
            <p style="color:var(--text-muted);font-size:13px">No courses available. Add courses first.</p>
          {/if}
        </div>
      </div>
    </div>
    <div class="modal-footer">
      <button class="btn btn-secondary" on:click={() => (showModal = false)}>Cancel</button>
      <button class="btn btn-primary" on:click={save} disabled={!form.name || !form.department}>
        {editing ? 'Update' : 'Add Batch'}
      </button>
    </div>
  </Modal>
{/if}

<style>
  .course-picker {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-height: 200px;
    overflow-y: auto;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--bg);
  }
  .course-check {
    display: flex;
    align-items: center;
    cursor: pointer;
    padding: 4px 0;
  }
  .course-check:hover { opacity: .85; }
</style>
