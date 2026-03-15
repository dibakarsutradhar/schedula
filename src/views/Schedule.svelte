<script>
  import { onMount } from 'svelte'
  import Timetable from '../lib/components/Timetable.svelte'
  import SemesterCalendar from '../lib/components/SemesterCalendar.svelte'
  import Reports from '../lib/components/Reports.svelte'
  import Modal from '../lib/components/Modal.svelte'
  import {
    generateSchedule, getSchedules, getScheduleEntries,
    activateSchedule, deleteSchedule, exportScheduleCsv,
    getBatches, getSemesters, getRooms,
    getUtilizationReport, updateScheduleEntry,
  } from '../lib/api.js'
  import { toast } from '../lib/toast.js'

  let schedules = []
  let entries = []
  let batches = []
  let semesters = []
  let rooms = []
  let selectedId = null
  let selectedSemesterId = null  // for generate
  let filterBatch = null
  let generating = false
  let scheduleName = ''
  let showConflicts = false
  let lastResult = null
  let tab = 'timetable'   // 'timetable' | 'list' | 'calendar' | 'reports'
  let filterType = 'all'
  let filterLecturer = null

  // Utilization report
  let report = null
  let loadingReport = false

  // Edit entry modal
  let showEditModal = false
  let editingEntry = null
  let editForm = { day: 'Mon', time_slot: 0, room_id: null }
  let savingEdit = false

  $: activeSchedule = schedules.find(s => s.id === selectedId)
  $: activeSemester  = semesters.find(s => s.id === activeSchedule?.semester_id) ?? null

  $: lecturers = [...new Map(entries.map(e => [e.lecturer_id, { id: e.lecturer_id, name: e.lecturer_name }])).values()]
  $: filteredEntries = applyFilter(entries, filterType, filterBatch, filterLecturer)

  function applyFilter(es, type, batchId, lecId) {
    if (type === 'batch' && batchId) return es.filter(e => e.batch_id === batchId)
    if (type === 'lecturer' && lecId) return es.filter(e => e.lecturer_id === lecId)
    return es
  }

  onMount(async () => {
    ;[schedules, batches, semesters, rooms] = await Promise.all([getSchedules(), getBatches(), getSemesters(), getRooms()])
    const active = schedules.find(s => s.is_active)
    if (active) await selectSchedule(active.id)
  })

  async function selectSchedule(id) {
    selectedId = id
    entries = await getScheduleEntries(id)
  }

  async function generate() {
    if (!scheduleName.trim()) { toast('Enter a schedule name', 'error'); return }
    generating = true
    try {
      lastResult = await generateSchedule(scheduleName.trim(), selectedSemesterId)
      scheduleName = ''
      toast(`Schedule generated — ${lastResult.entry_count} slots placed`)
      ;[schedules, semesters] = await Promise.all([getSchedules(), getSemesters()])
      await selectSchedule(lastResult.schedule_id)
      if (lastResult.unscheduled?.length > 0) showConflicts = true
    } catch (e) {
      toast('Generation failed: ' + e, 'error')
    } finally {
      generating = false
    }
  }

  async function activate(id) {
    await activateSchedule(id)
    schedules = await getSchedules()
    toast('Schedule activated')
  }

  async function remove(id) {
    if (!confirm('Delete this schedule?')) return
    await deleteSchedule(id)
    schedules = await getSchedules()
    if (selectedId === id) { selectedId = null; entries = [] }
    toast('Schedule deleted', 'error')
  }

  async function exportCsv() {
    if (!selectedId) return
    const csv = await exportScheduleCsv(selectedId)
    const blob = new Blob([csv], { type: 'text/csv' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url; a.download = `schedule-${selectedId}.csv`; a.click()
    URL.revokeObjectURL(url)
    toast('CSV exported')
  }

  // Tab switching — load report on demand
  async function switchTab(t) {
    tab = t
    if (t === 'reports' && selectedId && !report) {
      loadingReport = true
      try { report = await getUtilizationReport(selectedId) } catch(e) { toast(String(e), 'error') } finally { loadingReport = false }
    }
  }

  // Manual entry editing
  function openEditModal(entry) {
    editingEntry = entry
    editForm = { day: entry.day, time_slot: entry.time_slot, room_id: entry.room_id }
    showEditModal = true
  }

  async function saveEdit() {
    savingEdit = true
    try {
      await updateScheduleEntry(editingEntry.id, editForm)
      entries = await getScheduleEntries(selectedId)
      if (tab === 'reports') report = await getUtilizationReport(selectedId)
      toast('Entry moved')
      showEditModal = false
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      savingEdit = false
    }
  }

  $: editableRooms = editingEntry
    ? rooms.filter(r => r.room_type === (editingEntry.class_type === 'lab' ? 'lab' : 'lecture'))
    : rooms

  // PDF print
  function printPdf() {
    const prev = document.title
    document.title = activeSchedule?.name ?? 'Schedule'
    window.print()
    document.title = prev
  }

  // iCal export
  function getMonday(dateStr) {
    const d = new Date(dateStr + 'T00:00:00')
    const day = d.getDay()
    d.setDate(d.getDate() + (day === 0 ? -6 : 1 - day))
    return d.toISOString().slice(0, 10)
  }
  function addDays(dateStr, n) {
    const d = new Date(dateStr + 'T00:00:00')
    d.setDate(d.getDate() + n)
    return d.toISOString().slice(0, 10)
  }

  function exportIcal() {
    if (!selectedId) return
    const dayMap = { Mon: 'MO', Tue: 'TU', Wed: 'WE', Thu: 'TH', Fri: 'FR' }
    const slotStartIcal = ['080000','090000','100000','110000','130000','140000','150000','160000']
    const slotEndIcal   = ['090000','100000','110000','120000','140000','150000','160000','170000']
    const dayOrder = ['Mon','Tue','Wed','Thu','Fri']
    const anchor = getMonday(activeSemester?.start_date ?? new Date().toISOString().slice(0, 10))
    const lines = ['BEGIN:VCALENDAR','VERSION:2.0','PRODID:-//Schedula//EN','CALSCALE:GREGORIAN']
    for (const e of entries) {
      const off = dayOrder.indexOf(e.day)
      if (off < 0) continue
      const dateStr = addDays(anchor, off).replace(/-/g, '')
      const interval = e.week_parity === 0 ? 1 : 2
      lines.push('BEGIN:VEVENT')
      lines.push(`UID:schedula-${e.id}@app`)
      lines.push(`DTSTART:${dateStr}T${slotStartIcal[e.time_slot]}`)
      lines.push(`DTEND:${dateStr}T${slotEndIcal[e.time_slot]}`)
      lines.push(`SUMMARY:${e.course_code} \u2013 ${e.batch_name}`)
      lines.push(`DESCRIPTION:${e.course_name} | ${e.lecturer_name} | ${e.room_name}`)
      lines.push(`LOCATION:${e.room_name}`)
      lines.push(`RRULE:FREQ=WEEKLY;INTERVAL=${interval};BYDAY=${dayMap[e.day]}`)
      lines.push('END:VEVENT')
    }
    lines.push('END:VCALENDAR')
    const blob = new Blob([lines.join('\r\n')], { type: 'text/calendar' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `${(activeSchedule?.name ?? 'schedule').replace(/[^\w\s-]/g, '')}.ics`
    a.click()
    URL.revokeObjectURL(url)
    toast('iCal exported')
  }

  const slotStart = ['08:00','09:00','10:00','11:00','13:00','14:00','15:00','16:00']
  const slotEnd   = ['09:00','10:00','11:00','12:00','14:00','15:00','16:00','17:00']

  const classTypeStyle = {
    lecture:  { badge: 'background:rgba(108,99,255,.15);color:#a5a0ff' },
    lab:      { badge: 'background:rgba(6,182,212,.15);color:#22d3ee' },
    tutorial: { badge: 'background:rgba(34,197,94,.15);color:#4ade80' },
  }
</script>

<div class="page">
  <div class="page-header">
    <div class="page-header-left">
      <h1>Schedule</h1>
      <p class="page-subtitle">Generate and view conflict-free timetables</p>
    </div>
  </div>

  <!-- Generator bar -->
  <div class="card section">
    <h3 style="margin-bottom:14px">Generate New Schedule</h3>
    <div class="gen-bar">
      <input
        class="form-input"
        bind:value={scheduleName}
        placeholder="e.g. Spring 2025 — CSE Department"
        on:keydown={e => e.key === 'Enter' && generate()}
        style="flex:1"
      />
      <select class="form-select" style="width:220px" bind:value={selectedSemesterId}>
        <option value={null}>— No semester link —</option>
        {#each semesters as s}
          <option value={s.id}>{s.name} ({s.org_name})</option>
        {/each}
      </select>
      <button class="btn btn-primary" on:click={generate} disabled={generating || !scheduleName.trim()}>
        {generating ? '⏳ Generating…' : '⚡ Generate'}
      </button>
    </div>

    {#if lastResult?.unscheduled?.length > 0}
      <button class="btn btn-secondary btn-sm" style="margin-top:10px" on:click={() => (showConflicts = !showConflicts)}>
        ⚠ {lastResult.unscheduled.length} unscheduled — view details
      </button>
      {#if showConflicts}
        <div class="conflict-list" style="margin-top:14px">
          {#each lastResult.unscheduled as u}
            <div class="conflict-item">
              <strong>{u.batch_name} — {u.course_code}: {u.course_name}</strong>
              <div class="conflict-reason">{u.reason} ({u.hours_needed} h unplaced)</div>
            </div>
          {/each}
        </div>
      {/if}
    {/if}
  </div>

  <div class="schedule-layout">
    <!-- History sidebar -->
    <div class="schedule-list card">
      <h3 style="margin-bottom:14px">History</h3>
      {#if schedules.length === 0}
        <p style="color:var(--text-muted);font-size:13px">No schedules yet.</p>
      {:else}
        {#each schedules as s}
          <button class="sched-item" class:selected={selectedId === s.id} on:click={() => selectSchedule(s.id)}>
            <div class="sched-name">{s.name}</div>
            {#if s.semester_name}
              <div style="font-size:11px;color:var(--accent2)">📆 {s.semester_name}</div>
            {/if}
            <div class="sched-meta">{s.entry_count} slots · {s.created_at.slice(0,10)}</div>
            {#if s.is_active}<span class="badge badge-active" style="font-size:10px">active</span>{/if}
            <div class="sched-actions">
              {#if !s.is_active}
                <button class="btn btn-secondary btn-sm" on:click|stopPropagation={() => activate(s.id)}>Activate</button>
              {/if}
              <button class="btn btn-danger btn-sm" on:click|stopPropagation={() => remove(s.id)}>✕</button>
            </div>
          </button>
        {/each}
      {/if}
    </div>

    <!-- Viewer panel -->
    <div class="timetable-panel card">
      {#if selectedId}
        <div class="tt-toolbar">
          <div class="tab-group">
            <button class="tab-btn" class:active={tab === 'timetable'} on:click={() => switchTab('timetable')}>Weekly Grid</button>
            <button class="tab-btn" class:active={tab === 'list'} on:click={() => switchTab('list')}>List</button>
            {#if activeSemester}
              <button class="tab-btn" class:active={tab === 'calendar'} on:click={() => switchTab('calendar')}>
                📆 Calendar
              </button>
            {/if}
            <button class="tab-btn" class:active={tab === 'reports'} on:click={() => switchTab('reports')}>
              📊 Reports
            </button>
          </div>

          <div class="filter-group">
            {#if tab === 'timetable' || tab === 'list'}
              <select class="form-select" style="width:130px" bind:value={filterType} on:change={() => { filterBatch = null; filterLecturer = null }}>
                <option value="all">All</option>
                <option value="batch">By Batch</option>
                <option value="lecturer">By Lecturer</option>
              </select>
              {#if filterType === 'batch'}
                <select class="form-select" style="width:160px" bind:value={filterBatch}>
                  <option value={null}>— select batch —</option>
                  {#each batches as b}<option value={b.id}>{b.name}</option>{/each}
                </select>
              {/if}
              {#if filterType === 'lecturer'}
                <select class="form-select" style="width:160px" bind:value={filterLecturer}>
                  <option value={null}>— select lecturer —</option>
                  {#each lecturers as l}<option value={l.id}>{l.name}</option>{/each}
                </select>
              {/if}
            {/if}
            <button class="btn btn-secondary btn-sm" on:click={exportCsv} title="Export CSV">⬇ CSV</button>
            <button class="btn btn-secondary btn-sm" on:click={exportIcal} title="Export to Google/Outlook Calendar">📅 iCal</button>
            <button class="btn btn-secondary btn-sm" on:click={printPdf} title="Print / Save as PDF">🖨 Print</button>
          </div>
        </div>

        {#if tab === 'timetable'}
          <Timetable entries={filteredEntries} editable={true} on:editEntry={e => openEditModal(e.detail)} />

        {:else if tab === 'list'}
          <div class="table-wrap">
            <table>
              <thead>
                <tr><th>Day</th><th>Time</th><th>Type</th><th>Batch</th><th>Course</th><th>Lecturer</th><th>Room</th><th>Freq</th></tr>
              </thead>
              <tbody>
                {#each filteredEntries.sort((a,b) => { const di = ['Mon','Tue','Wed','Thu','Fri']; return di.indexOf(a.day) - di.indexOf(b.day) || a.time_slot - b.time_slot }) as e}
                  <tr>
                    <td>{e.day}</td>
                    <td style="white-space:nowrap;font-size:12px">{slotStart[e.time_slot]}–{slotEnd[e.time_slot]}</td>
                    <td>
                      <span class="badge" style={classTypeStyle[e.class_type]?.badge ?? ''}>
                        {e.class_type}
                      </span>
                    </td>
                    <td>{e.batch_name}</td>
                    <td><strong>{e.course_code}</strong> {e.course_name}</td>
                    <td>{e.lecturer_name}</td>
                    <td>{e.room_name}</td>
                    <td style="font-size:11px;color:var(--text-muted)">{e.week_parity === 0 ? 'weekly' : 'biweekly'}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>

        {:else if tab === 'calendar'}
          <SemesterCalendar semester={activeSemester} entries={entries} />

        {:else if tab === 'reports'}
          {#if loadingReport}
            <div class="empty-state">Loading report…</div>
          {:else}
            <Reports report={report} />
          {/if}
        {/if}

      {:else}
        <div class="empty-state" style="padding:60px">
          <div style="font-size:40px;margin-bottom:12px">📅</div>
          <p>Generate or select a schedule to view the timetable</p>
        </div>
      {/if}
    </div>
  </div>
</div>

<!-- Edit Entry Modal -->
{#if showEditModal && editingEntry}
  <Modal title="Move Entry" onClose={() => (showEditModal = false)}>
    <div class="modal-body">
      <p style="font-size:13px;color:var(--text-muted);margin-bottom:16px">
        <strong>{editingEntry.course_code}</strong> — {editingEntry.batch_name}
        <span style="margin-left:8px;font-size:11px">({editingEntry.class_type})</span>
      </p>
      <div class="row">
        <div class="form-group">
          <label class="form-label">Day</label>
          <select class="form-select" bind:value={editForm.day}>
            {#each ['Mon','Tue','Wed','Thu','Fri'] as d}
              <option value={d}>{d}</option>
            {/each}
          </select>
        </div>
        <div class="form-group">
          <label class="form-label">Time Slot</label>
          <select class="form-select" bind:value={editForm.time_slot}>
            {#each slotStart as label, i}
              <option value={i}>{label}–{slotEnd[i]}</option>
            {/each}
          </select>
        </div>
      </div>
      <div class="form-group">
        <label class="form-label">Room</label>
        <select class="form-select" bind:value={editForm.room_id}>
          {#each editableRooms as r}
            <option value={r.id}>{r.name} ({r.capacity} seats)</option>
          {/each}
        </select>
      </div>
    </div>
    <div class="modal-footer">
      <button class="btn btn-secondary" on:click={() => (showEditModal = false)}>Cancel</button>
      <button class="btn btn-primary" on:click={saveEdit} disabled={savingEdit || !editForm.room_id}>
        {savingEdit ? 'Saving…' : 'Move Entry'}
      </button>
    </div>
  </Modal>
{/if}

<style>
  .gen-bar { display: flex; gap: 12px; align-items: center; flex-wrap: wrap; }
  .schedule-layout { display: flex; gap: 20px; min-height: 500px; }
  .schedule-list { width: 220px; flex-shrink: 0; display: flex; flex-direction: column; overflow-y: auto; }
  .sched-item {
    display: flex; flex-direction: column; gap: 3px;
    padding: 12px; border-radius: 8px; border: 1px solid transparent;
    background: none; color: var(--text); text-align: left;
    cursor: pointer; transition: all .15s; width: 100%; margin-bottom: 4px;
  }
  .sched-item:hover { background: var(--surface2); }
  .sched-item.selected { border-color: var(--accent); background: rgba(108,99,255,.08); }
  .sched-name { font-weight: 600; font-size: 13px; }
  .sched-meta { font-size: 11px; color: var(--text-muted); }
  .sched-actions { display: flex; gap: 6px; margin-top: 6px; }
  .tt-toolbar {
    display: flex; align-items: center; justify-content: space-between;
    gap: 12px; margin-bottom: 16px; flex-wrap: wrap;
  }
  .tab-group { display: flex; gap: 4px; }
  .tab-btn {
    padding: 6px 14px; border-radius: 8px; border: 1px solid var(--border);
    background: none; color: var(--text-muted); cursor: pointer; font-size: 13px; transition: all .15s;
  }
  .tab-btn.active { background: var(--accent); color: #fff; border-color: var(--accent); }
  .filter-group { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
  .timetable-panel { display: flex; flex-direction: column; flex: 1; overflow: auto; }
</style>
