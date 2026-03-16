<script>
  import { fade, fly } from 'svelte/transition'
  import { backOut } from 'svelte/easing'
  import { login, getSecurityQuestion, resetPasswordWithRecoveryCode, resetPasswordWithSecurityAnswer, createApprovalRequest, getMyApprovalStatus } from '../lib/api.js'
  import { session } from '../lib/stores/session.js'
  import { toast } from '../lib/toast.js'

  let username = ''
  let password = ''
  let loading = false

  // Recovery modal state
  let showRecoveryModal = false
  let recoveryTab = 'code' // 'code' | 'question' | 'request'
  let recoveryUsername = ''
  let recoveryCode = ''
  let newPassword = ''
  let confirmPassword = ''
  let securityQuestion = ''
  let securityAnswer = ''
  let recoveryLoading = false

  // Request admin reset state
  let requestType = 'password_reset'
  let requestNewPassword = ''
  let requestConfirmPassword = ''
  let requestSubmitted = false
  let myRequests = []

  async function submit() {
    if (!username || !password) return
    loading = true
    try {
      const payload = await login({ username, password })
      session.set(payload)
    } catch (e) {
      toast(e?.toString() ?? 'Login failed', 'error')
    } finally {
      loading = false
    }
  }

  function openRecovery() {
    showRecoveryModal = true
    recoveryTab = 'code'
    recoveryUsername = ''
    recoveryCode = ''
    newPassword = ''
    confirmPassword = ''
    securityAnswer = ''
    securityQuestion = ''
    requestNewPassword = ''
    requestConfirmPassword = ''
    requestSubmitted = false
    myRequests = []
  }

  async function switchToRequest() {
    recoveryTab = 'request'
    if (recoveryUsername) await loadMyRequests()
  }

  async function loadMyRequests() {
    if (!recoveryUsername) return
    try {
      myRequests = await getMyApprovalStatus(recoveryUsername)
    } catch (_) {
      myRequests = []
    }
  }

  async function submitAdminRequest() {
    if (!recoveryUsername) { toast('Please enter your username', 'error'); return }
    if (requestType === 'password_reset') {
      if (!requestNewPassword || !requestConfirmPassword) {
        toast('Please fill all fields', 'error'); return
      }
      if (requestNewPassword !== requestConfirmPassword) {
        toast('Passwords do not match', 'error'); return
      }
      if (requestNewPassword.length < 8) {
        toast('Password must be at least 8 characters', 'error'); return
      }
    }
    try {
      recoveryLoading = true
      await createApprovalRequest({
        username: recoveryUsername,
        request_type: requestType,
        new_password: requestType === 'password_reset' ? requestNewPassword : null
      })
      requestSubmitted = true
      myRequests = await getMyApprovalStatus(recoveryUsername)
      toast('Request submitted. The super admin will review it shortly.', 'success')
    } catch (e) {
      toast(e?.toString() ?? 'Failed to submit request', 'error')
    } finally {
      recoveryLoading = false
    }
  }

  function statusLabel(s) {
    return s === 'pending' ? '⏳ Pending' : s === 'approved' ? '✅ Approved' : s === 'rejected' ? '❌ Rejected' : s
  }

  function closeRecovery() {
    showRecoveryModal = false
  }

  async function loadSecurityQuestion() {
    if (!recoveryUsername) {
      toast('Please enter username', 'error')
      return
    }
    try {
      recoveryLoading = true
      const question = await getSecurityQuestion()
      securityQuestion = question
    } catch (e) {
      toast(e?.toString() ?? 'Failed to load security question', 'error')
    } finally {
      recoveryLoading = false
    }
  }

  async function resetWithCode() {
    if (!recoveryUsername || !recoveryCode || !newPassword || !confirmPassword) {
      toast('Please fill all fields', 'error')
      return
    }
    if (newPassword !== confirmPassword) {
      toast('Passwords do not match', 'error')
      return
    }
    if (newPassword.length < 8) {
      toast('Password must be at least 8 characters', 'error')
      return
    }

    try {
      recoveryLoading = true
      await resetPasswordWithRecoveryCode({
        username: recoveryUsername,
        recovery_code: recoveryCode,
        new_password: newPassword
      })
      toast('Password reset successfully. You can now log in.', 'success')
      closeRecovery()
      username = recoveryUsername
      password = ''
    } catch (e) {
      toast(e?.toString() ?? 'Password reset failed', 'error')
    } finally {
      recoveryLoading = false
    }
  }

  async function resetWithQuestion() {
    if (!recoveryUsername || !securityAnswer || !newPassword || !confirmPassword) {
      toast('Please fill all fields', 'error')
      return
    }
    if (newPassword !== confirmPassword) {
      toast('Passwords do not match', 'error')
      return
    }
    if (newPassword.length < 8) {
      toast('Password must be at least 8 characters', 'error')
      return
    }

    try {
      recoveryLoading = true
      await resetPasswordWithSecurityAnswer({
        username: recoveryUsername,
        security_answer: securityAnswer,
        new_password: newPassword
      })
      toast('Password reset successfully. You can now log in.', 'success')
      closeRecovery()
      username = recoveryUsername
      password = ''
    } catch (e) {
      toast(e?.toString() ?? 'Password reset failed', 'error')
    } finally {
      recoveryLoading = false
    }
  }
</script>

<div class="login-page">
  <div class="login-card" in:fly={{ y: 20, duration: 600, delay: 150, easing: backOut }}>
    <div class="login-brand">
      <span class="login-icon">◈</span>
      <h1>Schedula</h1>
      <p>AI-Powered University Timetable Generator</p>
    </div>

    <form class="login-form" on:submit|preventDefault={submit}>
      <div class="form-group">
        <label class="form-label">Username</label>
        <input
          class="form-input"
          bind:value={username}
          placeholder="username"
          autocomplete="username"
          autofocus
        />
      </div>
      <div class="form-group">
        <label class="form-label">Password</label>
        <input
          class="form-input"
          type="password"
          bind:value={password}
          placeholder="••••••••"
          autocomplete="current-password"
        />
      </div>
      <button class="btn btn-primary" style="width:100%;justify-content:center;padding:10px" disabled={loading || !username || !password}>
        {loading ? 'Signing in…' : 'Sign In'}
      </button>
    </form>

    <div style="text-align: center; margin-top: 16px;">
      <button type="button" class="link-btn" on:click={openRecovery}>
        Forgot password?
      </button>
    </div>
  </div>
</div>

<!-- Password Recovery Modal -->
{#if showRecoveryModal}
  <div class="modal-backdrop" on:click|self={closeRecovery}>
    <div class="modal-card">
      <div class="modal-header">
        <h2>Recover Password</h2>
        <button type="button" class="close-btn" on:click={closeRecovery}>✕</button>
      </div>

      <div class="recovery-tabs">
        <button
          type="button"
          class="tab-btn"
          class:active={recoveryTab === 'code'}
          on:click={() => recoveryTab = 'code'}
        >
          Recovery Code
        </button>
        <button
          type="button"
          class="tab-btn"
          class:active={recoveryTab === 'question'}
          on:click={() => recoveryTab = 'question'}
        >
          Security Question
        </button>
        <button
          type="button"
          class="tab-btn"
          class:active={recoveryTab === 'request'}
          on:click={switchToRequest}
        >
          Request Reset
        </button>
      </div>

      <div class="recovery-content">
        {#if recoveryTab === 'code'}
          <div class="form-group">
            <label class="form-label">Username</label>
            <input
              class="form-input"
              bind:value={recoveryUsername}
              placeholder="Enter your username"
            />
          </div>
          <div class="form-group">
            <label class="form-label">Recovery Code</label>
            <input
              class="form-input"
              bind:value={recoveryCode}
              placeholder="Enter your recovery code"
              spellcheck="false"
            />
          </div>
          <div class="form-group">
            <label class="form-label">New Password</label>
            <input
              class="form-input"
              type="password"
              bind:value={newPassword}
              placeholder="••••••••"
            />
          </div>
          <div class="form-group">
            <label class="form-label">Confirm Password</label>
            <input
              class="form-input"
              type="password"
              bind:value={confirmPassword}
              placeholder="••••••••"
            />
          </div>
          <button
            type="button"
            class="btn btn-primary"
            style="width:100%;justify-content:center;padding:10px"
            disabled={recoveryLoading || !recoveryUsername || !recoveryCode || !newPassword || !confirmPassword}
            on:click={resetWithCode}
          >
            {recoveryLoading ? 'Resetting…' : 'Reset Password'}
          </button>
        {/if}

        {#if recoveryTab === 'question'}
          <div class="form-group">
            <label class="form-label">Username</label>
            <input
              class="form-input"
              bind:value={recoveryUsername}
              placeholder="Enter your username"
              on:change={loadSecurityQuestion}
            />
          </div>
          {#if securityQuestion}
            <div class="form-group">
              <label class="form-label">Security Question</label>
              <div class="security-question">
                {securityQuestion}
              </div>
            </div>
            <div class="form-group">
              <label class="form-label">Your Answer</label>
              <input
                class="form-input"
                bind:value={securityAnswer}
                placeholder="Enter your answer"
              />
            </div>
            <div class="form-group">
              <label class="form-label">New Password</label>
              <input
                class="form-input"
                type="password"
                bind:value={newPassword}
                placeholder="••••••••"
              />
            </div>
            <div class="form-group">
              <label class="form-label">Confirm Password</label>
              <input
                class="form-input"
                type="password"
                bind:value={confirmPassword}
                placeholder="••••••••"
              />
            </div>
            <button
              type="button"
              class="btn btn-primary"
              style="width:100%;justify-content:center;padding:10px"
              disabled={recoveryLoading || !securityAnswer || !newPassword || !confirmPassword}
              on:click={resetWithQuestion}
            >
              {recoveryLoading ? 'Resetting…' : 'Reset Password'}
            </button>
          {:else if recoveryUsername && !recoveryLoading}
            <p style="color: var(--text-muted); text-align: center; padding: 20px 0;">
              Loading security question…
            </p>
          {/if}
        {/if}

        {#if recoveryTab === 'request'}
          <div class="form-group">
            <label class="form-label">Username</label>
            <input
              class="form-input"
              bind:value={recoveryUsername}
              placeholder="Enter your username"
              on:change={loadMyRequests}
            />
          </div>

          {#if !requestSubmitted}
            <div class="form-group">
              <label class="form-label">Request Type</label>
              <select class="form-input" bind:value={requestType}>
                <option value="password_reset">Password Reset</option>
                <option value="account_unlock">Account Unlock</option>
              </select>
            </div>

            {#if requestType === 'password_reset'}
              <div class="form-group">
                <label class="form-label">New Password</label>
                <input
                  class="form-input"
                  type="password"
                  bind:value={requestNewPassword}
                  placeholder="••••••••"
                />
              </div>
              <div class="form-group">
                <label class="form-label">Confirm Password</label>
                <input
                  class="form-input"
                  type="password"
                  bind:value={requestConfirmPassword}
                  placeholder="••••••••"
                />
              </div>
            {/if}

            <div class="request-note">
              Your request will be reviewed by the super admin.
              You will be notified on your next login once it's resolved.
            </div>

            <button
              type="button"
              class="btn btn-primary"
              style="width:100%;justify-content:center;padding:10px"
              disabled={recoveryLoading || !recoveryUsername}
              on:click={submitAdminRequest}
            >
              {recoveryLoading ? 'Submitting…' : 'Submit Request'}
            </button>
          {:else}
            <div class="request-success">
              ✅ Request submitted successfully.<br>
              Please wait for the super admin to approve it.
            </div>
          {/if}

          {#if myRequests.length > 0}
            <div class="my-requests">
              <p class="my-requests-title">Recent requests</p>
              {#each myRequests as r (r.id)}
                <div class="request-row">
                  <span class="req-type">{r.request_type === 'password_reset' ? 'Password Reset' : 'Account Unlock'}</span>
                  <span class="req-status req-status-{r.status}">{statusLabel(r.status)}</span>
                  {#if r.rejection_reason}
                    <span class="req-reason">"{r.rejection_reason}"</span>
                  {/if}
                </div>
              {/each}
            </div>
          {/if}
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .login-page {
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    background: linear-gradient(-45deg, #0f111a, #1a1d27, #21253a, #0f111a);
    background-size: 400% 400%;
    animation: gradientBG 15s ease infinite;
  }
  
  @keyframes gradientBG {
    0% { background-position: 0% 50%; }
    50% { background-position: 100% 50%; }
    100% { background-position: 0% 50%; }
  }

  .login-card {
    width: 400px;
    background: var(--surface);
    border: 1px solid var(--border);
    backdrop-filter: var(--glass-blur);
    border-radius: 20px;
    padding: 48px 40px 40px;
    box-shadow: var(--shadow-lg), 0 0 24px rgba(108, 99, 255, 0.1);
  }
  .login-brand {
    text-align: center;
    margin-bottom: 32px;
  }
  .login-icon {
    font-size: 40px;
    color: var(--accent);
    display: block;
    margin-bottom: 10px;
  }
  .login-brand h1 { font-size: 1.8rem; margin-bottom: 6px; }
  .login-brand p { color: var(--text-muted); font-size: 13px; }
  .login-form { display: flex; flex-direction: column; gap: 16px; }

  .link-btn {
    background: none;
    border: none;
    color: var(--accent);
    cursor: pointer;
    font-size: 13px;
    text-decoration: none;
    padding: 0;
  }
  .link-btn:hover {
    text-decoration: underline;
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }
  .modal-card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 12px;
    width: 90%;
    max-width: 420px;
    box-shadow: 0 24px 80px rgba(0,0,0,.4);
    max-height: 90vh;
    overflow-y: auto;
  }
  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 20px;
    border-bottom: 1px solid var(--border);
  }
  .modal-header h2 {
    margin: 0;
    font-size: 1.25rem;
  }
  .close-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 20px;
    padding: 0;
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .close-btn:hover {
    color: var(--text);
  }

  .recovery-tabs {
    display: flex;
    border-bottom: 1px solid var(--border);
    padding: 0 20px;
  }
  .tab-btn {
    flex: 1;
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 12px 0;
    border-bottom: 2px solid transparent;
    font-size: 13px;
    font-weight: 500;
    transition: all 0.2s;
  }
  .tab-btn:hover {
    color: var(--text);
  }
  .tab-btn.active {
    color: var(--accent);
    border-bottom-color: var(--accent);
  }

  .recovery-content {
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .security-question {
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 12px;
    font-size: 13px;
    color: var(--text);
  }

  .request-note {
    font-size: 12px;
    color: var(--text-muted);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 10px 12px;
    line-height: 1.5;
  }

  .request-success {
    text-align: center;
    padding: 16px;
    font-size: 13px;
    color: var(--text);
    line-height: 1.6;
    background: rgba(46,204,113,.1);
    border: 1px solid rgba(46,204,113,.3);
    border-radius: 8px;
  }

  .my-requests {
    border-top: 1px solid var(--border);
    padding-top: 12px;
    margin-top: 4px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .my-requests-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    color: var(--text-muted);
    letter-spacing: .05em;
    margin: 0 0 4px;
  }
  .request-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .req-type {
    font-size: 12px;
    color: var(--text-muted);
  }
  .req-status {
    font-size: 11px;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 10px;
  }
  .req-status-pending  { background: rgba(250,200,80,.2);  color: #e8a800; }
  .req-status-approved { background: rgba(60,200,100,.2);  color: #2ecc71; }
  .req-status-rejected { background: rgba(224,82,96,.2);   color: #e05260; }
  .req-reason {
    font-size: 11px;
    color: var(--text-muted);
    font-style: italic;
  }
</style>
