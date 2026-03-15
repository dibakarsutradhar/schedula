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
    publishSchedule, revertScheduleToDraft,
    getPreflightWarnings, updateScheduleDescription,
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
  let scheduleDescription = ''
  let showConflicts = false
  let lastResult = null

  // Pre-flight
  let preflightWarnings = []
  let showPreflight = false
  let loadingPreflight = false
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

  // Conflict detection: find entry ids where lecturer, room, or batch is double-booked at same day+slot
  $: conflictKeys = (() => {
    const keys = new Set()
    const bySlot = {}
    for (const e of entries) {
      const k = `${e.day}-${e.time_slot}`
      if (!bySlot[k]) bySlot[k] = []
      bySlot[k].push(e)
    }
    for (const group of Object.values(bySlot)) {
      if (group.length < 2) continue
      const lecIds = group.map(e => e.lecturer_id)
      const roomIds = group.map(e => e.room_id)
      const batchIds = group.map(e => e.batch_id)
      const hasDup = (arr) => arr.length !== new Set(arr).size
      if (hasDup(lecIds) || hasDup(roomIds) || hasDup(batchIds)) {
        for (const e of group) keys.add(e.id)
      }
    }
    return keys
  })()
  $: conflictCount = conflictKeys.size

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

  async function runPreflight() {
    loadingPreflight = true
    try {
      preflightWarnings = await getPreflightWarnings()
      showPreflight = true
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      loadingPreflight = false
    }
  }

  async function generate() {
    if (!scheduleName.trim()) { toast('Enter a schedule name', 'error'); return }
    generating = true
    showPreflight = false
    try {
      lastResult = await generateSchedule(scheduleName.trim(), selectedSemesterId, scheduleDescription.trim() || null)
      scheduleName = ''
      scheduleDescription = ''
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

  // Inline notes edit
  let editingDescription = false
  let descriptionDraft = ''

  function startEditDescription() {
    descriptionDraft = activeSchedule?.description ?? ''
    editingDescription = true
  }

  async function saveDescription() {
    await updateScheduleDescription(selectedId, descriptionDraft.trim() || null)
    schedules = await getSchedules()
    editingDescription = false
    toast('Notes saved')
  }

  async function activate(id) {
    await activateSchedule(id)
    schedules = await getSchedules()
    toast('Schedule activated')
  }

  async function publish(id) {
    await publishSchedule(id)
    schedules = await getSchedules()
    toast('Schedule published')
  }

  async function revertDraft(id) {
    await revertScheduleToDraft(id)
    schedules = await getSchedules()
    toast('Schedule reverted to draft')
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

  // PDF print (full or filtered)
  function printPdf(filtered = false) {
    const prev = document.title
    let suffix = ''
    if (filtered && filterType === 'batch' && filterBatch) {
      const b = batches.find(b => b.id === filterBatch)
      suffix = b ? ` — ${b.name}` : ''
    } else if (filtered && filterType === 'lecturer' && filterLecturer) {
      const l = lecturers.find(l => l.id === filterLecturer)
      suffix = l ? ` — ${l.name}` : ''
    }
    document.title = (activeSchedule?.name ?? 'Schedule') + suffix
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

  // HTML timetable export
  function exportHtml() {
    if (!selectedId || !entries.length) return
    const DAYS = ['Mon','Tue','Wed','Thu','Fri']
    const SLOTS = [0,1,2,3,4,5,6,7]
    const slotL = ['08:00–09:00','09:00–10:00','10:00–11:00','11:00–12:00','13:00–14:00','14:00–15:00','15:00–16:00','16:00–17:00']

    // Group unique batches
    const batchSet = [...new Map(entries.map(e => [e.batch_id, e.batch_name])).entries()]

    let tables = ''
    for (const [batchId, batchName] of batchSet) {
      const bEntries = entries.filter(e => e.batch_id === batchId)
      let rows = ''
      for (const slot of SLOTS) {
        let cells = `<td style="background:#1a1a2e;color:#888;font-size:11px;padding:6px 10px;white-space:nowrap">${slotL[slot]}</td>`
        for (const day of DAYS) {
          const cell = bEntries.filter(e => e.day === day && e.time_slot === slot)
          if (cell.length) {
            cells += `<td style="background:#2a2a4a;border:1px solid #333;padding:6px 8px;vertical-align:top">${
              cell.map(e => `<div style="font-size:12px;font-weight:600;color:#a5a0ff">${e.course_code}</div><div style="font-size:11px;color:#ccc">${e.lecturer_name}</div><div style="font-size:10px;color:#888">${e.room_name}</div>`).join('')
            }</td>`
          } else {
            cells += `<td style="background:#111122;border:1px solid #222"></td>`
          }
        }
        rows += `<tr>${cells}</tr>`
      }
      tables += `<div style="margin-bottom:40px">
        <h2 style="color:#a5a0ff;margin-bottom:12px;font-family:sans-serif">${batchName}</h2>
        <table style="border-collapse:collapse;width:100%;font-family:sans-serif">
          <thead><tr>
            <th style="background:#111;color:#666;padding:8px 10px;font-size:11px;text-align:left">Time</th>
            ${DAYS.map(d => `<th style="background:#111;color:#aaa;padding:8px 14px;font-size:12px">${d}</th>`).join('')}
          </tr></thead>
          <tbody>${rows}</tbody>
        </table>
      </div>`
    }

    const html = `<!DOCTYPE html><html><head><meta charset="utf-8">
<title>${activeSchedule?.name ?? 'Schedule'}</title>
<style>body{background:#0d0d1a;color:#eee;padding:32px;}</style>
</head><body>
<h1 style="color:#fff;font-family:sans-serif;margin-bottom:6px">${activeSchedule?.name ?? 'Schedule'}</h1>
${activeSchedule?.description ? `<p style="color:#888;font-family:sans-serif;margin-bottom:24px">${activeSchedule.description}</p>` : ''}
<p style="color:#666;font-family:sans-serif;font-size:12px;margin-bottom:32px">Generated: ${activeSchedule?.created_at?.slice(0,10) ?? ''} · ${entries.length} slots</p>
${tables}
</body></html>`

    const blob = new Blob([html], { type: 'text/html' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `${(activeSchedule?.name ?? 'schedule').replace(/[^\w\s-]/g, '')}.html`
    a.click()
    URL.revokeObjectURL(url)
    toast('HTML timetable exported')
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
    </div>
    <div class="gen-bar" style="margin-top:8px">
      <input
        class="form-input"
        bind:value={scheduleDescription}
        placeholder="Optional notes (e.g. 'First draft, pending room confirmation')"
        style="flex:1"
      />
      <button class="btn btn-secondary" on:click={runPreflight} disabled={loadingPreflight} title="Check data quality before generating">
        {loadingPreflight ? '…' : '🔍 Pre-flight'}
      </button>
      <button class="btn btn-primary" on:click={generate} disabled={generating || !scheduleName.trim()}>
        {generating ? '⏳ Generating…' : '⚡ Generate'}
      </button>
    </div>

    {#if showPreflight && preflightWarnings.length > 0}
      <div class="preflight-panel" style="margin-top:14px">
        <div class="preflight-header">
          {#if preflightWarnings.some(w => w.severity === 'error')}
            <span style="color:var(--danger);font-weight:700">⛔ {preflightWarnings.filter(w=>w.severity==='error').length} error(s)</span>
          {/if}
          {#if preflightWarnings.some(w => w.severity === 'warning')}
            <span style="color:var(--warning,#fbbf24);font-weight:700;margin-left:12px">⚠ {preflightWarnings.filter(w=>w.severity==='warning').length} warning(s)</span>
          {/if}
          <button class="btn-icon" style="margin-left:auto" on:click={() => showPreflight = false}>✕</button>
        </div>
        {#each preflightWarnings as w}
          <div class="preflight-item preflight-{w.severity}">
            <span class="preflight-badge">{w.category}</span>
            {w.message}
          </div>
        {/each}
      </div>
    {:else if showPreflight && preflightWarnings.length === 0}
      <div class="preflight-panel" style="margin-top:14px">
        <div style="color:#4ade80;font-size:13px;font-weight:600">✓ All checks passed — ready to generate</div>
      </div>
    {/if}

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
            {#if s.description}
              <div style="font-size:11px;color:var(--text-muted);font-style:italic;line-height:1.4;margin-top:2px">{s.description}</div>
            {/if}
            <div class="sched-meta">{s.entry_count} slots · {s.created_at.slice(0,10)}</div>
            <div style="display:flex;gap:6px;align-items:center;margin-top:3px;flex-wrap:wrap">
              {#if s.status === 'published'}
                <span class="badge badge-active" style="font-size:10px">published</span>
              {:else}
                <span class="badge" style="font-size:10px;background:rgba(100,100,120,.2);color:var(--text-muted)">draft</span>
              {/if}
              {#if s.is_active}<span class="badge badge-active" style="font-size:10px;background:rgba(34,197,94,.15);color:#4ade80">active</span>{/if}
            </div>
            <div class="sched-actions">
              {#if s.status === 'draft'}
                <button class="btn btn-secondary btn-sm" on:click|stopPropagation={() => publish(s.id)}>Publish</button>
              {:else}
                <button class="btn btn-secondary btn-sm" on:click|stopPropagation={() => revertDraft(s.id)}>Revert</button>
              {/if}
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
            <button class="tab-btn" class:active={tab === 'timetable'} on:click={() => switchTab('timetable')}>
              Weekly Grid
              {#if conflictCount > 0}<span class="conflict-count">{conflictCount}</span>{/if}
            </button>
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
            <button class="btn btn-secondary btn-sm" on:click={exportHtml} title="Export shareable HTML timetable">🌐 HTML</button>
            <button class="btn btn-secondary btn-sm" on:click={() => printPdf(filterType !== 'all')} title="Print / Save as PDF">🖨 Print</button>
          </div>
        </div>

        <!-- Inline description editor -->
        <div class="desc-row">
          {#if editingDescription}
            <input
              class="form-input desc-input"
              bind:value={descriptionDraft}
              placeholder="Add notes about this schedule version…"
              on:keydown={e => { if (e.key === 'Enter') saveDescription(); if (e.key === 'Escape') editingDescription = false }}
            />
            <button class="btn btn-primary btn-sm" on:click={saveDescription}>Save</button>
            <button class="btn btn-secondary btn-sm" on:click={() => editingDescription = false}>Cancel</button>
          {:else}
            <span class="desc-text" on:click={startEditDescription} title="Click to edit notes">
              {activeSchedule?.description || '+ Add notes…'}
            </span>
          {/if}
        </div>

        {#if tab === 'timetable'}
          <Timetable entries={filteredEntries} editable={true} conflictKeys={conflictKeys} on:editEntry={e => openEditModal(e.detail)} />

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
  .conflict-count {
    display: inline-flex; align-items: center; justify-content: center;
    background: var(--danger); color: #fff;
    font-size: 10px; font-weight: 700; border-radius: 99px;
    padding: 1px 5px; margin-left: 5px; line-height: 1.4;
  }

  /* Pre-flight panel */
  .preflight-panel {
    border: 1px solid var(--border); border-radius: 8px;
    padding: 12px 14px; background: var(--surface2);
    display: flex; flex-direction: column; gap: 8px;
  }
  .preflight-header { display: flex; align-items: center; font-size: 13px; }
  .preflight-item {
    display: flex; align-items: flex-start; gap: 10px;
    font-size: 12px; padding: 6px 0; border-top: 1px solid var(--border);
  }
  .preflight-badge {
    font-size: 10px; font-weight: 700; text-transform: uppercase;
    border-radius: 4px; padding: 2px 6px; white-space: nowrap;
    background: var(--surface); border: 1px solid var(--border); color: var(--text-muted);
  }
  .preflight-error  { color: var(--danger); }
  .preflight-warning { color: #fbbf24; }
  .btn-icon {
    background: none; border: none; color: var(--text-muted);
    cursor: pointer; font-size: 14px; padding: 2px 6px;
  }
  .btn-icon:hover { color: var(--text); }

  /* Inline description editor */
  .desc-row {
    display: flex; align-items: center; gap: 8px;
    margin-bottom: 12px; min-height: 28px;
  }
  .desc-text {
    font-size: 12px; color: var(--text-muted); cursor: pointer;
    font-style: italic; flex: 1;
  }
  .desc-text:hover { color: var(--text); }
  .desc-input { flex: 1; font-size: 12px; padding: 4px 10px; height: 30px; }
</style>
