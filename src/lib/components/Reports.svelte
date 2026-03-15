<script>
  export let report = null
</script>

{#if !report}
  <div class="empty-state">Select a schedule to view its utilization report.</div>
{:else}
  <div class="report-summary">
    <div class="stat-chip">{report.total_entries} total slots</div>
    <div class="stat-chip">{report.rooms.length} rooms</div>
    <div class="stat-chip">{report.lecturer_loads.filter(l => l.scheduled_hours > 0).length} lecturers scheduled</div>
  </div>

  <!-- Room Utilization -->
  <div class="report-section">
    <h3 class="section-label">Room Utilization</h3>
    <div class="report-table">
      {#each report.rooms as r}
        <div class="report-row">
          <div class="report-name">
            <strong>{r.room_name}</strong>
            <span class="sub">{r.room_type} · {r.capacity} seats</span>
          </div>
          <div class="bar-cell">
            <div class="bar-track">
              <div
                class="bar-fill"
                class:bar-high={r.utilization_pct >= 80}
                class:bar-med={r.utilization_pct >= 50 && r.utilization_pct < 80}
                style="width:{Math.min(100, r.utilization_pct)}%"
              ></div>
            </div>
          </div>
          <div class="report-pct" class:pct-high={r.utilization_pct >= 80}>
            {r.utilization_pct.toFixed(1)}%
          </div>
          <div class="report-detail">{r.booked_slots} / {r.total_available_slots} slots</div>
        </div>
      {/each}
    </div>
  </div>

  <!-- Lecturer Load -->
  <div class="report-section">
    <h3 class="section-label">Lecturer Load</h3>
    <div class="report-table">
      {#each report.lecturer_loads as l}
        <div class="report-row">
          <div class="report-name">
            <strong>{l.lecturer_name}</strong>
            <span class="sub">max {l.max_hours_per_week} h/week</span>
          </div>
          <div class="bar-cell">
            <div class="bar-track">
              <div
                class="bar-fill"
                class:bar-high={l.load_pct >= 90}
                class:bar-med={l.load_pct >= 60 && l.load_pct < 90}
                style="width:{Math.min(100, l.load_pct)}%"
              ></div>
            </div>
          </div>
          <div class="report-pct" class:pct-high={l.load_pct >= 90}>
            {l.load_pct.toFixed(1)}%
          </div>
          <div class="report-detail">{l.scheduled_hours} / {l.max_hours_per_week} h</div>
        </div>
      {/each}
    </div>
  </div>
{/if}

<style>
  .report-summary {
    display: flex; gap: 10px; flex-wrap: wrap;
    margin-bottom: 24px;
  }
  .stat-chip {
    background: var(--surface2); border: 1px solid var(--border);
    border-radius: 99px; padding: 4px 14px;
    font-size: 12px; font-weight: 600; color: var(--text-muted);
  }

  .report-section { margin-bottom: 28px; }
  .section-label {
    font-size: 11px; font-weight: 700; text-transform: uppercase;
    letter-spacing: .08em; color: var(--text-muted); margin-bottom: 10px;
  }
  .report-table { display: flex; flex-direction: column; gap: 8px; }

  .report-row {
    display: grid;
    grid-template-columns: 200px 1fr 56px 80px;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    background: var(--surface2);
    border-radius: 8px;
    font-size: 13px;
  }
  .report-name { display: flex; flex-direction: column; gap: 2px; }
  .sub { font-size: 11px; color: var(--text-muted); }

  .bar-cell { flex: 1; }
  .bar-track {
    height: 6px; background: var(--border);
    border-radius: 3px; overflow: hidden;
  }
  .bar-fill {
    height: 100%; background: var(--accent);
    border-radius: 3px; transition: width .3s ease;
  }
  .bar-fill.bar-med  { background: var(--warning); }
  .bar-fill.bar-high { background: var(--danger); }

  .report-pct  { font-weight: 700; text-align: right; }
  .pct-high    { color: var(--danger); }
  .report-detail { font-size: 11px; color: var(--text-muted); text-align: right; }
</style>
