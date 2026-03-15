<script>
  import { bulkImportLecturers, bulkImportRooms, bulkImportCourses } from '../lib/api.js'
  import { toast } from '../lib/toast.js'

  // ── state ───────────────────────────────────────────────────────────────────
  let importType = 'lecturers'   // 'lecturers' | 'rooms' | 'courses'
  let rawText = ''
  let parsed = []
  let parseError = ''
  let importing = false
  let result = null

  // ── CSV templates ────────────────────────────────────────────────────────────
  const templates = {
    lecturers: `name,email,available_days,max_hours_per_day,max_hours_per_week
Dr. Alice Smith,alice@uni.edu,Mon;Tue;Wed;Thu;Fri,4,16
Prof. Bob Jones,bob@uni.edu,Mon;Tue;Thu,3,12`,
    rooms: `name,capacity,room_type,available_days
LH-101,60,lecture,Mon;Tue;Wed;Thu;Fri
LAB-201,30,lab,Mon;Tue;Wed;Thu;Fri`,
    courses: `code,name,hours_per_week,room_type,class_type,frequency,lecturer_email
CS101,Introduction to Programming,3,lecture,lecture,weekly,alice@uni.edu
CS102,Data Structures,3,lecture,lecture,weekly,
CS103,Operating Systems Lab,2,lab,lab,biweekly,bob@uni.edu`,
  }

  const columnDefs = {
    lecturers: ['name','email','available_days','max_hours_per_day','max_hours_per_week'],
    rooms:     ['name','capacity','room_type','available_days'],
    courses:   ['code','name','hours_per_week','room_type','class_type','frequency','lecturer_email'],
  }

  function loadTemplate() {
    rawText = templates[importType]
    parsed = []
    parseError = ''
    result = null
  }

  // ── Parse CSV ────────────────────────────────────────────────────────────────
  function parseCsv(text) {
    const lines = text.trim().split('\n').map(l => l.trim()).filter(Boolean)
    if (lines.length < 2) return { rows: [], error: 'Need at least a header row and one data row.' }
    const headers = lines[0].split(',').map(h => h.trim().toLowerCase())
    const cols = columnDefs[importType]
    const missing = cols.filter(c => !headers.includes(c))
    if (missing.length) return { rows: [], error: `Missing columns: ${missing.join(', ')}` }

    const rows = []
    for (let i = 1; i < lines.length; i++) {
      const vals = lines[i].split(',').map(v => v.trim())
      const row = {}
      headers.forEach((h, idx) => row[h] = vals[idx] ?? '')
      // Coerce types
      if (importType === 'lecturers') {
        row.max_hours_per_day = parseInt(row.max_hours_per_day) || 4
        row.max_hours_per_week = parseInt(row.max_hours_per_week) || 16
        row.available_days = (row.available_days || 'Mon,Tue,Wed,Thu,Fri').replace(/;/g, ',')
        if (!row.email) row.email = null
      } else if (importType === 'rooms') {
        row.capacity = parseInt(row.capacity) || 30
        row.available_days = (row.available_days || 'Mon,Tue,Wed,Thu,Fri').replace(/;/g, ',')
      } else if (importType === 'courses') {
        row.hours_per_week = parseInt(row.hours_per_week) || 3
        if (!row.lecturer_email) row.lecturer_email = null
      }
      rows.push(row)
    }
    return { rows, error: '' }
  }

  function handleParse() {
    result = null
    const { rows, error } = parseCsv(rawText)
    parseError = error
    parsed = rows
  }

  async function doImport() {
    if (!parsed.length) return
    importing = true
    result = null
    try {
      if (importType === 'lecturers') result = await bulkImportLecturers(parsed)
      else if (importType === 'rooms') result = await bulkImportRooms(parsed)
      else result = await bulkImportCourses(parsed)
      toast(`Imported ${result.inserted} record(s)${result.skipped ? `, ${result.skipped} skipped` : ''}`)
      rawText = ''
      parsed = []
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      importing = false
    }
  }

  function handleChange() {
    if (rawText) handleParse()
  }

  $: if (importType) { rawText = ''; parsed = []; parseError = ''; result = null }
</script>

<div class="page">
  <div class="page-header">
    <div class="page-header-left">
      <h1>Bulk Import</h1>
      <p class="page-subtitle">Import lecturers, rooms, or courses from CSV</p>
    </div>
  </div>

  <div class="import-layout">
    <!-- Left: type selector + instructions -->
    <div class="import-sidebar card">
      <h3 style="margin-bottom:14px">Import Type</h3>
      {#each [['lecturers','👨‍🏫','Lecturers'],['rooms','🏫','Rooms'],['courses','📚','Courses']] as [id, icon, label]}
        <button
          class="type-btn"
          class:active={importType === id}
          on:click={() => { importType = id }}
        >
          <span style="font-size:18px">{icon}</span>
          {label}
        </button>
      {/each}

      <div class="template-hint" style="margin-top:20px">
        <p style="font-size:12px;color:var(--text-muted);margin-bottom:8px">
          Required columns for <strong>{importType}</strong>:
        </p>
        <ul style="font-size:11px;color:var(--text-muted);padding-left:16px;line-height:1.8">
          {#each columnDefs[importType] as col}
            <li><code>{col}</code></li>
          {/each}
        </ul>
        <p style="font-size:11px;color:var(--text-muted);margin-top:8px">
          Use semicolons (;) for day lists, or commas.
          Duplicate names/codes for the same org will be skipped.
        </p>
        <button class="btn btn-secondary btn-sm" style="margin-top:10px" on:click={loadTemplate}>
          Load Example
        </button>
      </div>
    </div>

    <!-- Right: editor + preview -->
    <div class="import-main">
      <div class="card" style="margin-bottom:16px">
        <h3 style="margin-bottom:12px">Paste CSV</h3>
        <textarea
          class="csv-editor"
          bind:value={rawText}
          on:input={handleChange}
          placeholder="Paste CSV here or click 'Load Example'…"
          rows="12"
          spellcheck="false"
        ></textarea>
        {#if parseError}
          <div class="error-msg">{parseError}</div>
        {/if}
        <div style="display:flex;gap:10px;margin-top:12px;align-items:center">
          <button class="btn btn-secondary" on:click={handleParse} disabled={!rawText.trim()}>
            Preview
          </button>
          <button class="btn btn-primary" on:click={doImport} disabled={importing || !parsed.length || !!parseError}>
            {importing ? 'Importing…' : `Import ${parsed.length || ''} row(s)`}
          </button>
        </div>
      </div>

      {#if result}
        <div class="card result-card">
          <div class="result-row">
            <span class="result-stat result-ok">{result.inserted} inserted</span>
            <span class="result-stat result-skip">{result.skipped} skipped</span>
          </div>
          {#if result.errors.length > 0}
            <div style="margin-top:10px">
              <p style="font-size:12px;font-weight:600;color:var(--danger);margin-bottom:6px">Errors:</p>
              {#each result.errors as err}
                <div style="font-size:11px;color:var(--danger);margin-bottom:3px">• {err}</div>
              {/each}
            </div>
          {/if}
        </div>
      {/if}

      {#if parsed.length > 0 && !parseError}
        <div class="card">
          <h3 style="margin-bottom:12px">Preview ({parsed.length} rows)</h3>
          <div class="table-wrap">
            <table>
              <thead>
                <tr>
                  {#each columnDefs[importType] as col}
                    <th>{col}</th>
                  {/each}
                </tr>
              </thead>
              <tbody>
                {#each parsed.slice(0, 20) as row}
                  <tr>
                    {#each columnDefs[importType] as col}
                      <td style="font-size:12px;max-width:160px;overflow:hidden;text-overflow:ellipsis">
                        {row[col] ?? '—'}
                      </td>
                    {/each}
                  </tr>
                {/each}
                {#if parsed.length > 20}
                  <tr>
                    <td colspan={columnDefs[importType].length} style="text-align:center;color:var(--text-muted);font-size:11px">
                      …and {parsed.length - 20} more rows
                    </td>
                  </tr>
                {/if}
              </tbody>
            </table>
          </div>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .import-layout { display: flex; gap: 20px; align-items: flex-start; }
  .import-sidebar { width: 220px; flex-shrink: 0; }
  .import-main { flex: 1; min-width: 0; }

  .type-btn {
    display: flex; align-items: center; gap: 10px;
    width: 100%; padding: 10px 12px; border-radius: 8px;
    border: 1px solid transparent; background: none;
    color: var(--text); cursor: pointer; font-size: 13px;
    transition: all .15s; margin-bottom: 4px; text-align: left;
  }
  .type-btn:hover { background: var(--surface2); }
  .type-btn.active { border-color: var(--accent); background: rgba(108,99,255,.08); }

  .csv-editor {
    width: 100%; resize: vertical;
    background: var(--surface2); border: 1px solid var(--border);
    border-radius: 8px; color: var(--text); font-family: monospace;
    font-size: 12px; padding: 12px; line-height: 1.6;
    box-sizing: border-box;
  }
  .csv-editor:focus { outline: none; border-color: var(--accent); }

  .error-msg {
    margin-top: 8px; padding: 8px 12px;
    background: rgba(239,68,68,.1); border: 1px solid var(--danger);
    border-radius: 6px; font-size: 12px; color: var(--danger);
  }

  .result-card { padding: 16px; }
  .result-row { display: flex; gap: 16px; }
  .result-stat { font-size: 16px; font-weight: 700; }
  .result-ok   { color: #4ade80; }
  .result-skip { color: var(--text-muted); }
</style>
