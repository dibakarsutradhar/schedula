<script>
  export let active = 'dashboard'

  import { onMount, onDestroy } from 'svelte'
  import { session, isSuperAdmin } from '../stores/session.js'
  import { logout, getApprovalCount } from '../api.js'
  import { toast } from '../toast.js'

  let pendingApprovals = 0
  let pollInterval = null

  async function refreshApprovalCount() {
    if (!isSuperAdmin($session)) return
    try {
      pendingApprovals = await getApprovalCount()
    } catch (_) {
      pendingApprovals = 0
    }
  }

  $: if ($session) {
    refreshApprovalCount()
  }

  onMount(() => {
    refreshApprovalCount()
    pollInterval = setInterval(refreshApprovalCount, 30000)
  })

  onDestroy(() => {
    if (pollInterval) clearInterval(pollInterval)
  })

  $: nav = [
    { id: 'dashboard',  label: 'Dashboard',      icon: '⬡' },
    ...(isSuperAdmin($session) ? [{ id: 'orgs', label: 'Organization', icon: '🏫' }] : []),
    { id: 'semesters',  label: 'Semesters',       icon: '📆' },
    { id: 'lecturers',  label: 'Lecturers',        icon: '👤' },
    { id: 'courses',    label: 'Courses',          icon: '📚' },
    { id: 'rooms',      label: 'Rooms',            icon: '🏛' },
    { id: 'batches',    label: 'Batches',          icon: '🎓' },
    { id: 'schedule',   label: 'Schedule',         icon: '📅' },
    { id: 'import',     label: 'Import',           icon: '⬆' },
    { id: 'users',      label: 'Users',            icon: '👥' },
    ...(isSuperAdmin($session) ? [{ id: 'approvals', label: 'Approvals', icon: '✅', badge: pendingApprovals }] : []),
    { id: 'settings',   label: 'Settings',         icon: '⚙️' },
  ]

  async function handleLogout() {
    await logout()
    session.set(null)
    toast('Logged out')
  }
</script>

<aside class="sidebar">
  <div class="brand">
    <span class="brand-icon">◈</span>
    <span class="brand-name">Schedula</span>
  </div>

  <nav class="nav">
    {#each nav as item}
      <button
        class="nav-item"
        class:active={active === item.id}
        on:click={() => { active = item.id; refreshApprovalCount() }}
      >
        <span class="nav-icon">{item.icon}</span>
        <span>{item.label}</span>
        {#if item.badge > 0}
          <span class="nav-badge">{item.badge}</span>
        {/if}
      </button>
    {/each}
  </nav>

  <div class="sidebar-footer">
    {#if $session}
      <div class="user-info">
        <div class="user-name">{$session.display_name}</div>
        <div class="user-role">{$session.role.replace('_', ' ')}</div>
      </div>
      <button class="btn-logout" on:click={handleLogout} title="Sign out">⎋ Sign out</button>
    {/if}
  </div>
</aside>

<style>
  .sidebar {
    width: var(--sidebar-w);
    background: var(--surface);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
    overflow: hidden;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 20px 16px 16px;
    border-bottom: 1px solid var(--border);
    margin-bottom: 8px;
  }
  .brand-icon { font-size: 22px; color: var(--accent); }
  .brand-name { font-size: 16px; font-weight: 700; letter-spacing: -0.02em; }

  .nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 0 8px;
    flex: 1;
    overflow-y: auto;
  }

  .nav-item {
    display: flex; align-items: center; gap: 10px;
    padding: 9px 12px; border-radius: 8px; border: none;
    background: none; color: var(--text-muted);
    font-size: 13px; font-weight: 500; cursor: pointer;
    text-align: left; width: 100%; transition: all .15s;
  }
  .nav-item:hover { background: var(--surface2); color: var(--text); }
  .nav-item.active { background: rgba(108,99,255,.15); color: var(--accent2); }
  .nav-icon { font-size: 15px; width: 20px; text-align: center; }
  .nav-badge {
    margin-left: auto;
    background: var(--danger, #e05260);
    color: #fff;
    font-size: 10px;
    font-weight: 700;
    min-width: 18px;
    height: 18px;
    border-radius: 9px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0 5px;
  }

  .sidebar-footer {
    padding: 12px;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .user-info { padding: 4px 0; }
  .user-name { font-size: 13px; font-weight: 600; color: var(--text); }
  .user-role { font-size: 11px; color: var(--text-muted); text-transform: capitalize; }

  .btn-logout {
    background: none; border: 1px solid var(--border);
    border-radius: 6px; padding: 6px 10px;
    color: var(--text-muted); font-size: 12px; cursor: pointer;
    transition: all .15s; text-align: left;
  }
  .btn-logout:hover { border-color: var(--danger); color: var(--danger); }
</style>
