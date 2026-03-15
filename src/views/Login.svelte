<script>
  import { login, getSecurityQuestion, resetPasswordWithRecoveryCode, resetPasswordWithSecurityAnswer } from '../lib/api.js'
  import { session } from '../lib/stores/session.js'
  import { toast } from '../lib/toast.js'

  let username = ''
  let password = ''
  let loading = false

  // Recovery modal state
  let showRecoveryModal = false
  let recoveryTab = 'code' // 'code' or 'question'
  let recoveryUsername = ''
  let recoveryCode = ''
  let newPassword = ''
  let confirmPassword = ''
  let securityQuestion = ''
  let securityAnswer = ''
  let recoveryLoading = false

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
  <div class="login-card">
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
    background: var(--bg);
  }
  .login-card {
    width: 380px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 20px;
    padding: 40px 36px 32px;
    box-shadow: 0 24px 80px rgba(0,0,0,.4);
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
</style>
