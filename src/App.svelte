<script>
  import './app.css'
  import { onMount } from 'svelte'
  import { toasts } from './lib/toast.js'
  import { session } from './lib/stores/session.js'

  import Login        from './views/Login.svelte'
  import Sidebar      from './lib/components/Sidebar.svelte'
  import Dashboard    from './views/Dashboard.svelte'
  import Organizations from './views/Organizations.svelte'
  import Semesters    from './views/Semesters.svelte'
  import Lecturers    from './views/Lecturers.svelte'
  import Courses      from './views/Courses.svelte'
  import Rooms        from './views/Rooms.svelte'
  import Batches      from './views/Batches.svelte'
  import Schedule     from './views/Schedule.svelte'
  import Users        from './views/Users.svelte'
  import Settings     from './views/Settings.svelte'
  import { prefs }    from './lib/stores/prefs.js'

  let active = 'dashboard'
  let loading = true

  onMount(async () => {
    await session.restore()
    loading = false
  })

  function handleKeydown(e) {
    if ((e.metaKey || e.ctrlKey) && e.key === ',') {
      e.preventDefault()
      active = 'settings'
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

{#if loading}
  <div style="display:flex;align-items:center;justify-content:center;height:100vh;color:var(--text-muted)">Loading…</div>

{:else if !$session}
  <Login />

{:else}
  <div class="layout">
    <Sidebar bind:active />

    <div class="main-content">
      {#if active === 'dashboard'}
        <Dashboard navigate={v => (active = v)} />
      {:else if active === 'orgs'}
        <Organizations />
      {:else if active === 'semesters'}
        <Semesters />
      {:else if active === 'lecturers'}
        <Lecturers />
      {:else if active === 'courses'}
        <Courses />
      {:else if active === 'rooms'}
        <Rooms />
      {:else if active === 'batches'}
        <Batches />
      {:else if active === 'schedule'}
        <Schedule />
      {:else if active === 'users'}
        <Users />
      {:else if active === 'settings'}
        <Settings />
      {/if}
    </div>
  </div>
{/if}

<!-- Toast notifications -->
<div class="toast-container">
  {#each $toasts as t (t.id)}
    <div class="toast toast-{t.type}">{t.msg}</div>
  {/each}
</div>
