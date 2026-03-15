<!--
  Semester Calendar: renders the full semester timeline with teaching weeks,
  exam/study blocks, and the weekly schedule pattern applied to actual dates.

  Props:
    semester  — full Semester object
    entries   — ScheduleEntry[] from the linked schedule
-->
<script>
  export let semester = null
  export let entries = []

  // ─── Date helpers ───────────────────────────────────────────────────────────

  function parseDate(s) { return s ? new Date(s + 'T00:00:00') : null }

  function isoDate(d) {
    return d.toISOString().slice(0, 10)
  }

  function addDays(d, n) {
    const r = new Date(d)
    r.setDate(r.getDate() + n)
    return r
  }

  // Monday of the week containing date d
  function mondayOf(d) {
    const r = new Date(d)
    const day = r.getDay() // 0=Sun
    const diff = day === 0 ? -6 : 1 - day
    r.setDate(r.getDate() + diff)
    return r
  }

  // ─── Block detection ────────────────────────────────────────────────────────

  function getWeekType(weekStart, s) {
    const ws = isoDate(weekStart)
    const we = isoDate(addDays(weekStart, 4)) // Friday

    if (s.midterm_start && s.midterm_end) {
      if (ws <= s.midterm_end && we >= s.midterm_start)
        return 'midterm'
    }
    if (s.study_break_start && s.study_break_end) {
      if (ws <= s.study_break_end && we >= s.study_break_start)
        return 'study'
    }
    if (s.final_start && s.final_end) {
      if (ws <= s.final_end && we >= s.final_start)
        return 'finals'
    }
    // Extra breaks from breaks_json
    try {
      const breaks = JSON.parse(s.breaks_json || '[]')
      for (const b of breaks) {
        if (b.start && b.end && ws <= b.end && we >= b.start)
          return 'break'
      }
    } catch {}
    return 'teaching'
  }

  const WEEK_TYPE_META = {
    teaching: { label: 'Teaching',   color: 'rgba(108,99,255,.12)', border: 'var(--accent)',   text: 'var(--accent2)' },
    midterm:  { label: 'Midterm Exam', color: 'rgba(245,158,11,.12)', border: '#f59e0b', text: '#fbbf24' },
    study:    { label: 'Study Break', color: 'rgba(6,182,212,.12)', border: '#06b6d4',  text: '#22d3ee' },
    finals:   { label: 'Final Exams', color: 'rgba(239,68,68,.12)',  border: '#ef4444',  text: '#f87171' },
    break:    { label: 'Break',       color: 'rgba(148,163,184,.08)', border: 'var(--border)', text: 'var(--text-muted)' },
  }

  const CLASS_TYPE_COLOR = {
    lecture:  { bg: 'rgba(108,99,255,.2)',  border: '#6c63ff', text: '#a5a0ff' },
    lab:      { bg: 'rgba(6,182,212,.2)',   border: '#06b6d4', text: '#22d3ee' },
    tutorial: { bg: 'rgba(34,197,94,.2)',   border: '#22c55e', text: '#4ade80' },
  }

  const DAY_NAMES = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri']

  // ─── Compute weeks ──────────────────────────────────────────────────────────

  $: weeks = (() => {
    if (!semester) return []
    const start = parseDate(semester.start_date)
    const end   = parseDate(semester.end_date)
    if (!start || !end) return []

    const result = []
    let cur = mondayOf(start)
    let weekNum = 0

    while (cur <= end) {
      const type = getWeekType(cur, semester)
      const days = DAY_NAMES.map((dayName, i) => {
        const date = addDays(cur, i)
        const dateStr = isoDate(date)
        const dayEntries = entries.filter(e => {
          if (e.day !== dayName) return false
          // Biweekly: only odd teaching weeks
          if (e.week_parity === 1 && type === 'teaching') {
            const teachingWeekIndex = result.filter(w => w.type === 'teaching').length
            return teachingWeekIndex % 2 === 0
          }
          return true
        })
        return { dayName, date: dateStr, entries: type === 'teaching' ? dayEntries : [] }
      })

      result.push({ weekStart: isoDate(cur), type, days, weekNum: ++weekNum })
      cur = addDays(cur, 7)
    }
    return result
  })()

  $: teachingWeekCount = weeks.filter(w => w.type === 'teaching').length

  // Build slot time label
  const SLOT_TIMES = ['08:00','09:00','10:00','11:00','13:00','14:00','15:00','16:00']

  // Batch color index map
  $: batchIds = [...new Set(entries.map(e => e.batch_id))]
  $: batchColorIdx = Object.fromEntries(batchIds.map((id, i) => [id, i % 8]))
  const BATCH_COLORS = ['#6c63ff','#06b6d4','#22c55e','#f59e0b','#ec4899','#8b5cf6','#14b8a6','#f97316']

  // Collapse teaching weeks toggle
  let collapsed = false
</script>

<div class="sem-cal">
  <!-- Legend -->
  <div class="legend">
    {#each Object.entries(WEEK_TYPE_META) as [type, meta]}
      <div class="legend-item">
        <span class="legend-dot" style="background:{meta.border}"></span>
        {meta.label}
      </div>
    {/each}
    <div style="flex:1"></div>
    <div class="legend-item">
      <span class="legend-dot" style="background:#6c63ff"></span>Lecture
    </div>
    <div class="legend-item">
      <span class="legend-dot" style="background:#06b6d4"></span>Lab
    </div>
    <div class="legend-item">
      <span class="legend-dot" style="background:#22c55e"></span>Tutorial
    </div>
    <span style="color:var(--text-muted);font-size:12px">{teachingWeekCount} teaching weeks</span>
  </div>

  <!-- Weeks -->
  <div class="weeks-list">
    {#each weeks as week}
      {@const meta = WEEK_TYPE_META[week.type]}
      <div class="week-row" style="border-left:3px solid {meta.border};background:{meta.color}">
        <div class="week-header">
          <span class="week-num" style="color:{meta.text}">W{week.weekNum}</span>
          <span class="week-range">{week.weekStart}</span>
          <span class="week-type-badge" style="color:{meta.text};background:transparent;border:1px solid {meta.border}">
            {meta.label}
          </span>
        </div>

        {#if week.type === 'teaching'}
          <div class="week-days">
            {#each week.days as day}
              <div class="day-col">
                <div class="day-header">{day.dayName} <span class="day-date">{day.date.slice(5)}</span></div>
                <div class="day-slots">
                  {#each day.entries as entry}
                    {@const ct = CLASS_TYPE_COLOR[entry.class_type] ?? CLASS_TYPE_COLOR.lecture}
                    <div class="entry-chip"
                         style="background:{ct.bg};border-left:2px solid {ct.border};color:{ct.text}"
                         title="{entry.course_name} — {entry.lecturer_name} — {entry.room_name} @ {SLOT_TIMES[entry.time_slot]}">
                      <strong>{entry.course_code}</strong>
                      <span>{entry.batch_name}</span>
                      {#if entry.week_parity === 1}<span class="biweekly-tag">biweekly</span>{/if}
                    </div>
                  {/each}
                  {#if day.entries.length === 0}
                    <div class="empty-day">—</div>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        {:else}
          <div class="block-message" style="color:{meta.text}">
            {#if week.type === 'midterm'}📋 Midterm examination period — no regular classes
            {:else if week.type === 'study'}📚 Study break — no classes scheduled
            {:else if week.type === 'finals'}🎯 Final examination period — no regular classes
            {:else}🗓 Break — no classes scheduled{/if}
          </div>
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .sem-cal { display: flex; flex-direction: column; gap: 12px; }

  .legend {
    display: flex;
    align-items: center;
    gap: 16px;
    flex-wrap: wrap;
    padding: 10px 14px;
    background: var(--surface2);
    border-radius: 8px;
    font-size: 12px;
  }
  .legend-item { display: flex; align-items: center; gap: 6px; color: var(--text-muted); }
  .legend-dot { width: 10px; height: 10px; border-radius: 50%; display: inline-block; }

  .weeks-list { display: flex; flex-direction: column; gap: 6px; }

  .week-row {
    border-radius: 8px;
    padding: 10px 14px;
    transition: background .1s;
  }

  .week-header {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 8px;
    font-size: 12px;
  }
  .week-num { font-weight: 700; font-size: 13px; min-width: 30px; }
  .week-range { color: var(--text-muted); }
  .week-type-badge {
    padding: 1px 8px;
    border-radius: 99px;
    font-size: 11px;
    font-weight: 600;
  }

  .week-days {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 6px;
  }

  .day-col { display: flex; flex-direction: column; gap: 3px; }
  .day-header {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    padding-bottom: 4px;
    border-bottom: 1px solid var(--border);
  }
  .day-date { font-weight: 400; margin-left: 4px; }
  .day-slots { display: flex; flex-direction: column; gap: 2px; min-height: 32px; }
  .empty-day { font-size: 11px; color: var(--border); padding: 4px 0; }

  .entry-chip {
    padding: 3px 7px;
    border-radius: 4px;
    font-size: 11px;
    line-height: 1.4;
    cursor: default;
  }
  .entry-chip strong { display: block; }
  .entry-chip span { display: block; font-size: 10px; opacity: .8; }
  .biweekly-tag {
    background: rgba(255,255,255,.1);
    border-radius: 3px;
    padding: 0 4px;
    font-size: 9px !important;
    opacity: 1 !important;
  }

  .block-message {
    font-size: 13px;
    font-style: italic;
    padding: 6px 0;
  }
</style>
