<script>
  import { createEventDispatcher } from 'svelte'

  // Pass the raw error string from the API; if it contains plan_limit_exceeded JSON, we parse it.
  export let error = ''
  export let inline = false   // true = compact inline bar; false = card

  const dispatch = createEventDispatcher()

  const FEATURE_LABELS = {
    batches:          'batch limit',
    admins:           'admin limit',
    csp_algorithm:    'CSP scheduling algorithm',
    bulk_import:      'bulk CSV import',
    utilization_reports: 'utilization reports',
    multi_org:        'multiple organizations',
    multi_machine_sync: 'multi-machine sync',
  }

  const PLAN_NAMES = { free: 'Free', pro: 'Pro', institution: 'Institution' }

  $: parsed = (() => {
    if (!error) return null
    try {
      const obj = typeof error === 'string' ? JSON.parse(error) : error
      if (obj?.code === 'plan_limit_exceeded') return obj
    } catch {}
    return null
  })()

  $: featureLabel = parsed ? (FEATURE_LABELS[parsed.feature] ?? parsed.feature) : ''
  $: planName     = parsed ? (PLAN_NAMES[parsed.plan] ?? parsed.plan) : ''
  $: limitMsg     = parsed?.limit > 0
    ? `${parsed.current}/${parsed.limit} ${featureLabel} used`
    : `${featureLabel} not available on ${planName} plan`
</script>

{#if parsed}
  {#if inline}
    <div class="upgrade-inline">
      <span class="upgrade-icon">🔒</span>
      <span>{limitMsg} on your <strong>{planName}</strong> plan.</span>
      <a class="upgrade-link" href={parsed.upgrade_url} target="_blank" rel="noopener">
        Upgrade →
      </a>
    </div>
  {:else}
    <div class="upgrade-card">
      <div class="upgrade-card-icon">🔒</div>
      <div class="upgrade-card-body">
        <p class="upgrade-card-title">Plan limit reached</p>
        <p class="upgrade-card-msg">{limitMsg} on your <strong>{planName}</strong> plan.</p>
        <p class="upgrade-card-sub">Upgrade to unlock {featureLabel} and more.</p>
      </div>
      <div class="upgrade-card-actions">
        <a class="btn btn-primary" href={parsed.upgrade_url} target="_blank" rel="noopener">
          Upgrade plan
        </a>
        <button class="btn btn-secondary" on:click={() => dispatch('dismiss')}>Dismiss</button>
      </div>
    </div>
  {/if}
{/if}

<style>
  .upgrade-inline {
    display: flex; align-items: center; gap: 8px;
    padding: 8px 14px; border-radius: 8px;
    background: color-mix(in srgb, var(--warning, #f59e0b) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--warning, #f59e0b) 40%, transparent);
    font-size: 13px;
  }
  .upgrade-icon { font-size: 14px; flex-shrink: 0; }
  .upgrade-link { color: var(--accent); font-weight: 600; text-decoration: none; white-space: nowrap; }
  .upgrade-link:hover { text-decoration: underline; }

  .upgrade-card {
    display: flex; align-items: flex-start; gap: 16px;
    padding: 18px 20px; border-radius: 12px;
    background: color-mix(in srgb, var(--warning, #f59e0b) 8%, var(--surface));
    border: 1px solid color-mix(in srgb, var(--warning, #f59e0b) 35%, transparent);
  }
  .upgrade-card-icon { font-size: 24px; line-height: 1; margin-top: 2px; flex-shrink: 0; }
  .upgrade-card-body { flex: 1; }
  .upgrade-card-title { font-weight: 700; font-size: 14px; margin: 0 0 4px; }
  .upgrade-card-msg   { font-size: 13px; margin: 0 0 4px; }
  .upgrade-card-sub   { font-size: 12px; color: var(--text-muted); margin: 0; }
  .upgrade-card-actions { display: flex; flex-direction: column; gap: 6px; flex-shrink: 0; }
</style>
