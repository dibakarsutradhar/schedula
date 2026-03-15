<script>
  import { createEventDispatcher } from 'svelte'
  export let entries = []
  export let filterBatch = null  // batch_id or null = all
  export let editable = false    // when true, clicking an entry fires 'editEntry'
  export let conflictKeys = new Set()  // Set of "day-slot-entryId" keys that have conflicts

  const dispatch = createEventDispatcher()

  const DAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri']
  const SLOTS = [0, 1, 2, 3, 4, 5, 6, 7]
  const SLOT_LABELS = [
    '08:00–09:00', '09:00–10:00', '10:00–11:00', '11:00–12:00',
    '13:00–14:00', '14:00–15:00', '15:00–16:00', '16:00–17:00',
  ]

  // Build a map of unique batches for coloring
  $: batchIds = [...new Set(entries.map(e => e.batch_id))]
  $: batchColorMap = Object.fromEntries(batchIds.map((id, i) => [id, i % 8]))

  $: filtered = filterBatch ? entries.filter(e => e.batch_id === filterBatch) : entries

  function cellEntries(day, slot) {
    return filtered.filter(e => e.day === day && e.time_slot === slot)
  }
</script>

<div class="tt-wrap">
  <table class="timetable">
    <thead>
      <tr>
        <th class="time-col">Time</th>
        {#each DAYS as day}
          <th>{day}</th>
        {/each}
      </tr>
    </thead>
    <tbody>
      {#each SLOTS as slot}
        <tr>
          <td class="time-col">{SLOT_LABELS[slot]}</td>
          {#each DAYS as day}
            <td>
              {#each cellEntries(day, slot) as entry}
                <div
                  class="slot-entry batch-color-{batchColorMap[entry.batch_id]}"
                  class:slot-editable={editable}
                  class:slot-conflict={conflictKeys.has(entry.id)}
                  on:click={() => editable && dispatch('editEntry', entry)}
                  title={conflictKeys.has(entry.id) ? '⚠ Conflict detected' : editable ? 'Click to move this entry' : ''}
                >
                  {#if conflictKeys.has(entry.id)}
                    <span class="conflict-badge">⚠</span>
                  {/if}
                  <strong>{entry.course_code}</strong>
                  <span>{entry.batch_name}</span>
                  <span style="color:var(--text-muted);font-size:10px">{entry.room_name}</span>
                  {#if editable}<span class="edit-hint">✎</span>{/if}
                </div>
              {/each}
            </td>
          {/each}
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .tt-wrap { overflow-x: auto; }
  .slot-editable { cursor: pointer; position: relative; }
  .slot-editable:hover { filter: brightness(1.15); }
  .edit-hint {
    position: absolute; top: 3px; right: 4px;
    font-size: 10px; opacity: 0; transition: opacity .15s;
  }
  .slot-editable:hover .edit-hint { opacity: 0.7; }
  .slot-conflict {
    outline: 2px solid var(--danger) !important;
    outline-offset: -2px;
  }
  .conflict-badge {
    position: absolute; top: 3px; right: 4px;
    font-size: 10px; color: var(--danger);
  }
</style>
