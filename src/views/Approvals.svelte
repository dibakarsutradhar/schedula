<script>
  import { onMount } from 'svelte'
  import { getPendingApprovals, resolveApproval } from '../lib/api.js'
  import { toast } from '../lib/toast.js'

  let requests = []
  let loading = true
  let filter = 'pending'   // 'pending' | 'all'

  let rejectModal = null   // { id, requester_username, request_type }
  let rejectReason = ''
  let resolving = false

  async function load() {
    loading = true
    try {
      requests = await getPendingApprovals()
    } catch (e) {
      toast(e?.toString() ?? 'Failed to load approvals', 'error')
    } finally {
      loading = false
    }
  }

  onMount(load)

  $: filtered = filter === 'pending'
    ? requests.filter(r => r.status === 'pending')
    : requests

  async function approve(id) {
    resolving = true
    try {
      await resolveApproval(id, true, null)
      toast('Request approved', 'success')
      await load()
    } catch (e) {
      toast(e?.toString() ?? 'Failed to approve', 'error')
    } finally {
      resolving = false
    }
  }

  function openRejectModal(req) {
    rejectModal = req
    rejectReason = ''
  }

  async function confirmReject() {
    if (!rejectReason.trim()) {
      toast('Please provide a rejection reason', 'error')
      return
    }
    resolving = true
    try {
      await resolveApproval(rejectModal.id, false, rejectReason.trim())
      toast('Request rejected', 'success')
      rejectModal = null
      await load()
    } catch (e) {
      toast(e?.toString() ?? 'Failed to reject', 'error')
    } finally {
      resolving = false
    }
  }

  function typeLabel(t) {
    return t === 'password_reset' ? 'Password Reset'
         : t === 'account_unlock' ? 'Account Unlock'
         : t
  }

  function statusClass(s) {
    return s === 'pending'  ? 'status-pending'
         : s === 'approved' ? 'status-approved'
         : s === 'rejected' ? 'status-rejected'
         : 'status-expired'
  }

  function formatDate(d) {
    if (!d) return '—'
    return new Date(d + 'Z').toLocaleString()
  }
</script>

<div class="page">
  <div class="page-header">
    <div>
      <h1>Approval Requests</h1>
      <p class="sub">Review password reset and account unlock requests from admins</p>
    </div>
    <button class="btn btn-secondary" on:click={load} disabled={loading}>
      {loading ? 'Loading…' : '↺ Refresh'}
    </button>
  </div>

  <div class="filter-row">
    <button class="filter-btn" class:active={filter === 'pending'} on:click={() => filter = 'pending'}>
      Pending
      {#if requests.filter(r => r.status === 'pending').length > 0}
        <span class="count-badge">{requests.filter(r => r.status === 'pending').length}</span>
      {/if}
    </button>
    <button class="filter-btn" class:active={filter === 'all'} on:click={() => filter = 'all'}>
      All Requests
    </button>
  </div>

  {#if loading}
    <div class="empty-state">Loading…</div>
  {:else if filtered.length === 0}
    <div class="empty-state">
      {#if filter === 'pending'}
        <span class="empty-icon">✅</span>
        <p>No pending requests</p>
        <p class="empty-sub">All caught up! Admins have no outstanding requests.</p>
      {:else}
        <span class="empty-icon">📋</span>
        <p>No requests yet</p>
        <p class="empty-sub">Requests will appear here when admins submit them.</p>
      {/if}
    </div>
  {:else}
    <div class="requests-list">
      {#each filtered as req (req.id)}
        <div class="request-card" class:resolved={req.status !== 'pending'}>
          <div class="request-header">
            <div class="request-info">
              <span class="type-tag">{typeLabel(req.request_type)}</span>
              <span class="requester">
                <strong>{req.requester_display_name}</strong>
                <span class="username">@{req.requester_username}</span>
              </span>
            </div>
            <span class="status-badge {statusClass(req.status)}">{req.status}</span>
          </div>

          <div class="request-meta">
            <span>Submitted: {formatDate(req.created_at)}</span>
            <span>Expires: {formatDate(req.expires_at)}</span>
            {#if req.resolved_at}
              <span>Resolved: {formatDate(req.resolved_at)}</span>
            {/if}
          </div>

          {#if req.rejection_reason}
            <div class="rejection-reason">
              <strong>Rejection reason:</strong> {req.rejection_reason}
            </div>
          {/if}

          {#if req.resolver_display_name}
            <div class="resolver">Resolved by: {req.resolver_display_name}</div>
          {/if}

          {#if req.status === 'pending'}
            <div class="request-actions">
              <button
                class="btn btn-success"
                disabled={resolving}
                on:click={() => approve(req.id)}
              >
                ✓ Approve
              </button>
              <button
                class="btn btn-danger"
                disabled={resolving}
                on:click={() => openRejectModal(req)}
              >
                ✕ Reject
              </button>
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<!-- Reject modal -->
{#if rejectModal}
  <div class="modal-backdrop" on:click|self={() => rejectModal = null}>
    <div class="modal-card">
      <div class="modal-header">
        <h2>Reject Request</h2>
        <button class="close-btn" on:click={() => rejectModal = null}>✕</button>
      </div>
      <div class="modal-body">
        <p>
          Rejecting <strong>{typeLabel(rejectModal.request_type)}</strong> request
          from <strong>@{rejectModal.requester_username}</strong>.
        </p>
        <div class="form-group">
          <label class="form-label">Reason (required)</label>
          <textarea
            class="form-input"
            rows="3"
            placeholder="Explain why this request is being rejected…"
            bind:value={rejectReason}
          ></textarea>
        </div>
      </div>
      <div class="modal-footer">
        <button class="btn btn-secondary" on:click={() => rejectModal = null}>Cancel</button>
        <button
          class="btn btn-danger"
          disabled={resolving || !rejectReason.trim()}
          on:click={confirmReject}
        >
          {resolving ? 'Rejecting…' : 'Confirm Reject'}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .page { padding: 32px; }
  .page-header {
    display: flex; justify-content: space-between; align-items: flex-start;
    margin-bottom: 24px;
  }
  .page-header h1 { font-size: 1.5rem; margin: 0 0 4px; }
  .sub { color: var(--text-muted); font-size: 13px; margin: 0; }

  .filter-row {
    display: flex; gap: 8px; margin-bottom: 20px;
    border-bottom: 1px solid var(--border); padding-bottom: 0;
  }
  .filter-btn {
    background: none; border: none; padding: 8px 16px;
    border-bottom: 2px solid transparent; color: var(--text-muted);
    font-size: 13px; font-weight: 500; cursor: pointer;
    display: flex; align-items: center; gap: 6px; margin-bottom: -1px;
  }
  .filter-btn:hover { color: var(--text); }
  .filter-btn.active { color: var(--accent); border-bottom-color: var(--accent); }
  .count-badge {
    background: var(--danger, #e05260); color: #fff;
    font-size: 10px; font-weight: 700; border-radius: 9px;
    padding: 1px 6px;
  }

  .empty-state {
    text-align: center; padding: 60px 20px; color: var(--text-muted);
  }
  .empty-icon { font-size: 40px; display: block; margin-bottom: 12px; }
  .empty-state p { margin: 4px 0; font-size: 15px; font-weight: 500; color: var(--text); }
  .empty-sub { font-size: 13px !important; font-weight: 400 !important; color: var(--text-muted) !important; }

  .requests-list { display: flex; flex-direction: column; gap: 12px; }

  .request-card {
    background: var(--surface); border: 1px solid var(--border);
    border-radius: 10px; padding: 16px;
  }
  .request-card.resolved { opacity: 0.7; }

  .request-header {
    display: flex; justify-content: space-between; align-items: center;
    margin-bottom: 10px;
  }
  .request-info { display: flex; align-items: center; gap: 10px; }

  .type-tag {
    background: rgba(108,99,255,.15); color: var(--accent2);
    font-size: 11px; font-weight: 600; padding: 3px 8px;
    border-radius: 4px; white-space: nowrap;
  }
  .requester { font-size: 14px; }
  .username { color: var(--text-muted); font-size: 12px; margin-left: 4px; }

  .status-badge {
    font-size: 11px; font-weight: 600; padding: 3px 10px;
    border-radius: 12px; text-transform: uppercase;
  }
  .status-pending  { background: rgba(250,200,80,.2);  color: #e8a800; }
  .status-approved { background: rgba(60,200,100,.2);  color: #2ecc71; }
  .status-rejected { background: rgba(224,82,96,.2);   color: #e05260; }
  .status-expired  { background: rgba(120,120,120,.2); color: var(--text-muted); }

  .request-meta {
    display: flex; gap: 16px; flex-wrap: wrap;
    font-size: 12px; color: var(--text-muted); margin-bottom: 10px;
  }

  .rejection-reason {
    font-size: 13px; color: var(--text-muted);
    background: var(--bg); border-radius: 6px;
    padding: 8px 12px; margin-bottom: 10px;
  }
  .resolver { font-size: 12px; color: var(--text-muted); margin-bottom: 8px; }

  .request-actions { display: flex; gap: 8px; margin-top: 12px; }

  .btn-success {
    background: #2ecc71; border: none; color: #fff;
    padding: 7px 16px; border-radius: 6px; font-size: 13px;
    font-weight: 600; cursor: pointer; transition: opacity .15s;
  }
  .btn-success:hover:not(:disabled) { opacity: .85; }
  .btn-success:disabled { opacity: .5; cursor: default; }

  .btn-danger {
    background: none; border: 1px solid var(--danger, #e05260);
    color: var(--danger, #e05260);
    padding: 7px 16px; border-radius: 6px; font-size: 13px;
    font-weight: 600; cursor: pointer; transition: all .15s;
  }
  .btn-danger:hover:not(:disabled) { background: var(--danger, #e05260); color: #fff; }
  .btn-danger:disabled { opacity: .5; cursor: default; }

  .modal-backdrop {
    position: fixed; inset: 0; background: rgba(0,0,0,.5);
    display: flex; align-items: center; justify-content: center; z-index: 1000;
  }
  .modal-card {
    background: var(--surface); border: 1px solid var(--border);
    border-radius: 12px; width: 90%; max-width: 440px;
    box-shadow: 0 24px 80px rgba(0,0,0,.4);
  }
  .modal-header {
    display: flex; justify-content: space-between; align-items: center;
    padding: 20px; border-bottom: 1px solid var(--border);
  }
  .modal-header h2 { margin: 0; font-size: 1.1rem; }
  .close-btn {
    background: none; border: none; color: var(--text-muted);
    font-size: 18px; cursor: pointer; width: 28px; height: 28px;
    display: flex; align-items: center; justify-content: center;
  }
  .modal-body { padding: 20px; display: flex; flex-direction: column; gap: 16px; }
  .modal-body p { margin: 0; font-size: 14px; color: var(--text-muted); }
  .modal-footer {
    padding: 16px 20px; border-top: 1px solid var(--border);
    display: flex; justify-content: flex-end; gap: 8px;
  }
</style>
