<script>
  import './app.css'
  import { onMount, onDestroy } from 'svelte'
  import { toasts }             from './lib/toast.js'
  import { toast }              from './lib/toast.js'
  import { session }            from './lib/stores/session.js'
  import { syncMode }           from './lib/stores/syncMode.js'
  import { connectWs, disconnectWs, lastWsEvent, wsConnected } from './lib/stores/ws.js'

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
  import Import       from './views/Import.svelte'
  import Approvals    from './views/Approvals.svelte'
  import Onboarding   from './lib/components/Onboarding.svelte'
  import { prefs }    from './lib/stores/prefs.js'

  let active = 'dashboard'
  let loading = true
  let showOnboarding = false

  function checkOnboarding(sess) {
    if (!sess) return false
    const key = `schedula_onboarded_${sess.user_id}`
    return !localStorage.getItem(key)
  }

  onMount(async () => {
    await session.restore()
    loading = false
    // Connect WebSocket if server mode is configured
    if ($syncMode.mode === 'server' && $syncMode.serverUrl && $syncMode.token) {
      connectWs()
    }
  })

  onDestroy(() => disconnectWs())

  // Show a toast when the WS reports a data change from another user
  const ENTITY_LABELS = {
    users: 'Users', courses: 'Courses', lecturers: 'Lecturers', rooms: 'Rooms',
    batches: 'Batches', schedules: 'Schedules', semesters: 'Semesters',
    approval_request: 'Approvals',
  }
  let prevWsTs = 0
  $: if ($lastWsEvent && $lastWsEvent._ts !== prevWsTs) {
    prevWsTs = $lastWsEvent._ts
    const label = ENTITY_LABELS[$lastWsEvent.entity] || $lastWsEvent.entity
    if ($lastWsEvent.action !== 'login') {
      toast(`${label} updated by another user — refresh to see changes`, 'info')
    }
  }

  // Connect WS when a session becomes active in server mode
  $: if ($session && $syncMode.mode === 'server') connectWs()

  // Show onboarding whenever a session becomes active and user hasn't completed it
  $: if (!loading && $session && !showOnboarding) {
    if (checkOnboarding($session)) showOnboarding = true
  }

  function handleOnboardingComplete(e) {
    showOnboarding = false
    if (e?.detail?.navigateTo) active = e.detail.navigateTo
  }

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
      {:else if active === 'import'}
        <Import />
      {:else if active === 'approvals'}
        <Approvals />
      {/if}
    </div>
  </div>
{/if}

<!-- Onboarding wizard (first-time users) -->
{#if $session && showOnboarding}
  <Onboarding on:complete={handleOnboardingComplete} />
{/if}

<!-- Toast notifications -->
<div class="toast-container">
  {#each $toasts as t (t.id)}
    <div class="toast toast-{t.type}">{t.msg}</div>
  {/each}
</div>
