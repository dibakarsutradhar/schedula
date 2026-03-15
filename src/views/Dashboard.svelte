<script>
  import { onMount } from 'svelte'
  import { getStats, getDataHealth } from '../lib/api.js'

  export let navigate = () => {}

  let stats = null
  let health = null
  let loading = true

  onMount(async () => {
    ;[stats, health] = await Promise.all([getStats(), getDataHealth()])
    loading = false
  })

  const cards = [
    { key: 'lecturers', label: 'Lecturers',     icon: '👤', view: 'lecturers', color: '#06b6d4' },
    { key: 'courses',   label: 'Courses',        icon: '📚', view: 'courses',   color: '#6c63ff' },
    { key: 'rooms',     label: 'Rooms',          icon: '🏛', view: 'rooms',     color: '#22c55e' },
    { key: 'batches',   label: 'Batches',        icon: '🎓', view: 'batches',   color: '#f59e0b' },
    { key: 'schedules', label: 'Schedules',      icon: '📅', view: 'schedule',  color: '#ec4899' },
    { key: 'active_entries', label: 'Active Slots', icon: '✓', view: 'schedule', color: '#8b5cf6' },
  ]
</script>

<div class="page">
  <div class="page-header">
    <div class="page-header-left">
      <h1>Dashboard</h1>
      <p class="page-subtitle">AI-powered university timetable generator</p>
    </div>
    <button class="btn btn-primary" on:click={() => navigate('schedule')}>
      ⚡ Generate Schedule
    </button>
  </div>

  {#if loading}
    <p class="page-subtitle">Loading…</p>
  {:else if stats}
    <div class="stats-grid">
      {#each cards as c}
        <button class="stat-card stat-btn" on:click={() => navigate(c.view)} style="cursor:pointer; text-align:left;">
          <div class="stat-icon" style="color:{c.color}">{c.icon}</div>
          <div class="stat-value" style="color:{c.color}">{stats[c.key] ?? 0}</div>
          <div class="stat-label">{c.label}</div>
        </button>
      {/each}
    </div>
  {/if}

  {#if health && health.total_warnings > 0}
    <div class="health-panel card">
      <div style="display:flex;align-items:center;gap:10px;margin-bottom:12px">
        <span style="font-size:16px">⚠</span>
        <h3 style="margin:0;font-size:14px">Data Health Warnings</h3>
        <span class="health-badge">{health.total_warnings} issue{health.total_warnings !== 1 ? 's' : ''}</span>
      </div>
      <div class="health-items">
        {#if health.courses_without_lecturers > 0}
          <button type="button" class="health-item" on:click={() => navigate('courses')}>
            <span class="health-dot warning"></span>
            {health.courses_without_lecturers} course{health.courses_without_lecturers !== 1 ? 's' : ''} have no lecturer assigned
            <span class="health-link">Fix →</span>
          </button>
        {/if}
        {#if health.batches_without_courses > 0}
          <button type="button" class="health-item" on:click={() => navigate('batches')}>
            <span class="health-dot error"></span>
            {health.batches_without_courses} batch{health.batches_without_courses !== 1 ? 'es' : ''} have no courses enrolled
            <span class="health-link">Fix →</span>
          </button>
        {/if}
        {#if health.lecturers_unavailable > 0}
          <button type="button" class="health-item" on:click={() => navigate('lecturers')}>
            <span class="health-dot error"></span>
            {health.lecturers_unavailable} lecturer{health.lecturers_unavailable !== 1 ? 's' : ''} have no available days
            <span class="health-link">Fix →</span>
          </button>
        {/if}
        {#if health.courses_without_matching_rooms > 0}
          <button type="button" class="health-item" on:click={() => navigate('rooms')}>
            <span class="health-dot error"></span>
            {health.courses_without_matching_rooms} lab course{health.courses_without_matching_rooms !== 1 ? 's' : ''} but no lab rooms exist
            <span class="health-link">Add rooms →</span>
          </button>
        {/if}
      </div>
    </div>
  {/if}

  <div class="card">
    <h2 style="margin-bottom:16px">Getting Started</h2>
    <ol class="steps">
      <li>
        <strong>Add Lecturers</strong> — set availability and max load per day/week
      </li>
      <li>
        <strong>Add Courses</strong> — specify hours/week, room type, and assign a lecturer
      </li>
      <li>
        <strong>Add Rooms</strong> — set capacity, type (lab or lecture), and available days
      </li>
      <li>
        <strong>Add Batches</strong> — define student groups and enrol them in courses
      </li>
      <li>
        <strong>Generate Schedule</strong> — the engine creates a conflict-free timetable instantly
      </li>
    </ol>
  </div>
</div>

<style>
  .stat-btn {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 20px 20px 16px;
    transition: border-color .15s;
  }
  .stat-btn:hover { border-color: var(--accent); }
  .stat-icon { font-size: 20px; margin-bottom: 8px; }

  .steps {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding-left: 20px;
    color: var(--text-muted);
    line-height: 1.6;
  }
  .steps strong { color: var(--text); }

  /* Data health panel */
  .health-panel { margin-bottom: 0; }
  .health-badge {
    background: rgba(251,191,36,.15); color: #fbbf24;
    border-radius: 99px; padding: 2px 10px;
    font-size: 11px; font-weight: 700;
  }
  .health-items { display: flex; flex-direction: column; gap: 6px; }
  .health-item {
    display: flex; align-items: center; gap: 10px;
    font-size: 12px; color: var(--text-muted);
    cursor: pointer; padding: 6px 8px; border-radius: 6px;
    transition: background .15s;
    background: none; border: none; font: inherit; text-align: left; width: 100%;
  }
  .health-item:hover, .health-item:focus { background: var(--surface2); color: var(--text); outline: none; }
  .health-item:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .health-dot {
    width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0;
  }
  .health-dot.error   { background: var(--danger); }
  .health-dot.warning { background: #fbbf24; }
  .health-link {
    margin-left: auto; font-size: 11px; color: var(--accent);
    font-weight: 600;
  }
</style>
