<script>
  import { onMount } from 'svelte'
  import { getOrganizations, createOrganization, updateOrganization } from '../lib/api.js'
  import { toast } from '../lib/toast.js'

  let org = null
  let loading = true
  let saving = false

  let form = { name: '', org_type: 'university', address: '', contact_email: '' }

  onMount(load)

  async function load() {
    loading = true
    const orgs = await getOrganizations()
    org = orgs[0] ?? null
    if (org) {
      form = {
        name:          org.name,
        org_type:      org.org_type,
        address:       org.address ?? '',
        contact_email: org.contact_email ?? '',
      }
    }
    loading = false
  }

  async function save() {
    if (!form.name.trim()) return
    saving = true
    try {
      const payload = {
        name:          form.name.trim(),
        org_type:      form.org_type,
        address:       form.address.trim() || null,
        contact_email: form.contact_email.trim() || null,
      }
      if (org) {
        await updateOrganization(org.id, payload)
        org = { ...org, ...payload }
        toast('Organization updated')
      } else {
        const id = await createOrganization(payload)
        org = { id, ...payload }
        toast('Organization created')
      }
    } catch (e) {
      toast(String(e), 'error')
    } finally {
      saving = false
    }
  }

  const typeLabel = {
    university: '🎓 University',
    college:    '🏫 College',
    school:     '📖 School',
    institute:  '🔬 Institute',
  }
</script>

<div class="page">
  <div class="page-header">
    <div class="page-header-left">
      <h1>Organization</h1>
      <p class="page-subtitle">
        {#if org}{typeLabel[org.org_type] ?? org.org_type}{:else}Not configured yet{/if}
      </p>
    </div>
    {#if org}
      <span class="badge badge-active">Active</span>
    {/if}
  </div>

  {#if loading}
    <div class="empty-state">Loading…</div>
  {:else}
    <div class="card">
      {#if !org}
        <div class="ob-hint" style="margin-bottom:20px">
          No organization is configured yet. Fill in the details below to set up your institution.
        </div>
      {/if}

      <div class="form-group" style="margin-bottom:16px">
        <label class="form-label">Institution Name *</label>
        <input class="form-input" bind:value={form.name} placeholder="University of Dhaka" />
      </div>

      <div class="row" style="margin-bottom:16px">
        <div class="form-group">
          <label class="form-label">Type</label>
          <select class="form-select" bind:value={form.org_type}>
            <option value="university">University</option>
            <option value="college">College</option>
            <option value="school">School</option>
            <option value="institute">Institute</option>
          </select>
        </div>
        <div class="form-group">
          <label class="form-label">Contact Email</label>
          <input class="form-input" type="email" bind:value={form.contact_email} placeholder="admin@university.edu" />
        </div>
      </div>

      <div class="form-group" style="margin-bottom:20px">
        <label class="form-label">Address</label>
        <textarea class="form-textarea" bind:value={form.address} rows="2" placeholder="123 Campus Road, City, Country"></textarea>
      </div>

      <div style="display:flex;justify-content:space-between;align-items:center">
        <p style="font-size:12px;color:var(--text-muted)">
          One organization per app instance.
        </p>
        <button class="btn btn-primary" on:click={save} disabled={saving || !form.name.trim()}>
          {saving ? 'Saving…' : (org ? 'Update Organization' : 'Create Organization')}
        </button>
      </div>
    </div>
  {/if}
</div>

<style>
  .ob-hint {
    font-size: 13px;
    color: var(--text-muted);
    background: var(--surface2);
    border-radius: 8px;
    padding: 10px 14px;
    border-left: 3px solid var(--accent);
    line-height: 1.6;
  }
</style>
