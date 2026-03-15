<script>
  export let title = ''
  export let onClose = () => {}

  function handleKey(e) {
    if (e.key === 'Escape') onClose()
  }
</script>

<svelte:window on:keydown={handleKey} />

<div class="modal-backdrop" on:click|self={onClose}>
  <div class="modal">
    <div class="modal-header">
      <h2>{title}</h2>
      <button class="modal-close" on:click={onClose}>✕</button>
    </div>
    <slot />
  </div>
</div>
