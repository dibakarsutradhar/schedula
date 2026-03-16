<!-- Chip-based day picker; bind:value = "Mon,Tue,..." -->
<script>
  export let value = 'Mon,Tue,Wed,Thu,Fri'

  const days = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']

  $: selected = new Set(value ? value.split(',').map(d => d.trim()).filter(Boolean) : [])

  function toggle(day) {
    const s = new Set(selected)
    s.has(day) ? s.delete(day) : s.add(day)
    value = days.filter(d => s.has(d)).join(',')
  }
</script>

<div class="chip-row">
  {#each days as day}
    <button
      type="button"
      class="chip chip-toggle"
      class:active={selected.has(day)}
      on:click={() => toggle(day)}
    >{day}</button>
  {/each}
</div>
