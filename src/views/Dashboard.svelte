<script>
  import { onMount } from 'svelte'
  import { getStats } from '../lib/api.js'

  export let navigate = () => {}

  let stats = null
  let loading = true

  onMount(async () => {
    stats = await getStats()
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
</style>
