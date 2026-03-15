<script>
  import { onMount } from 'svelte'
  import Modal from '../lib/components/Modal.svelte'
  import DaySelect from '../lib/components/DaySelect.svelte'
  import { getRooms, createRoom, updateRoom, deleteRoom } from '../lib/api.js'
  import { session } from '../lib/stores/session.js'
  import { toast } from '../lib/toast.js'

  let rooms = []
  let showModal = false
  let editing = null

  let form = emptyForm()
  function emptyForm() {
    return { name: '', capacity: 30, room_type: 'lecture', available_days: 'Mon,Tue,Wed,Thu,Fri', org_id: $session?.org_id ?? null }
  }

  onMount(load)
  async function load() { rooms = await getRooms() }

  function openCreate() { editing = null; form = emptyForm(); showModal = true }
  function openEdit(r) {
    editing = r
    form = { name: r.name, capacity: r.capacity, room_type: r.room_type, available_days: r.available_days }
    showModal = true
  }

  async function save() {
    const payload = { ...form, capacity: +form.capacity, org_id: form.org_id ? +form.org_id : null }
    if (editing) {
      await updateRoom(editing.id, payload)
      toast('Room updated')
    } else {
      await createRoom(payload)
      toast('Room added')
    }
    showModal = false
    await load()
  }

  async function remove(r) {
    if (!confirm(`Delete room "${r.name}"?`)) return
    await deleteRoom(r.id)
    toast('Room deleted', 'error')
    await load()
  }
</script>

<div class="page">
  <div class="page-header">
    <div class="page-header-left">
      <h1>Rooms</h1>
      <p class="page-subtitle">{rooms.length} total</p>
    </div>
    <button class="btn btn-primary" on:click={openCreate}>+ Add Room</button>
  </div>

  <div class="card table-wrap">
    {#if rooms.length === 0}
      <div class="empty-state">No rooms yet.</div>
    {:else}
      <table>
        <thead>
          <tr>
            <th>Name</th><th>Type</th><th>Capacity</th><th>Available Days</th><th>Actions</th>
          </tr>
        </thead>
        <tbody>
          {#each rooms as r}
            <tr>
              <td><strong>{r.name}</strong></td>
              <td><span class="badge badge-{r.room_type}">{r.room_type}</span></td>
              <td>{r.capacity}</td>
              <td>{r.available_days}</td>
              <td>
                <button class="btn btn-secondary btn-sm" on:click={() => openEdit(r)}>Edit</button>
                <button class="btn btn-danger btn-sm" on:click={() => remove(r)}>Delete</button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</div>

{#if showModal}
  <Modal title={editing ? 'Edit Room' : 'Add Room'} onClose={() => (showModal = false)}>
    <div class="modal-body">
      <div class="form-group">
        <label class="form-label">Room Name *</label>
        <input class="form-input" bind:value={form.name} placeholder="R-101" />
      </div>
      <div class="row">
        <div class="form-group">
          <label class="form-label">Type *</label>
          <select class="form-select" bind:value={form.room_type}>
            <option value="lecture">Lecture Room</option>
            <option value="lab">Lab</option>
          </select>
        </div>
        <div class="form-group">
          <label class="form-label">Capacity *</label>
          <input class="form-input" type="number" min="1" bind:value={form.capacity} />
        </div>
      </div>
      <div class="form-group">
        <label class="form-label">Available Days</label>
        <DaySelect bind:value={form.available_days} />
      </div>
    </div>
    <div class="modal-footer">
      <button class="btn btn-secondary" on:click={() => (showModal = false)}>Cancel</button>
      <button class="btn btn-primary" on:click={save} disabled={!form.name}>
        {editing ? 'Update' : 'Add Room'}
      </button>
    </div>
  </Modal>
{/if}
