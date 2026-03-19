<script>
  import { fly } from 'svelte/transition'
  import { backOut } from 'svelte/easing'
  import { createEventDispatcher } from 'svelte'
  import { setupAccount, registerUser, login } from '../lib/api.js'
  import { session } from '../lib/stores/session.js'
  import { toast } from '../lib/toast.js'

  const dispatch = createEventDispatcher()

  let mode = 'signup' // 'signup' | 'login'

  // Signup state
  let step = 1 // 1 = welcome, 2 = account details
  let name = ''
  let email = ''
  let username = ''
  let password = ''
  let confirmPassword = ''
  let loading = false
  let usernameEdited = false

  // Login state
  let loginUsername = ''
  let loginPassword = ''
  let loginLoading = false

  $: if (name && !usernameEdited) {
    username = name.toLowerCase().replace(/\s+/g, '.').replace(/[^a-z0-9.]/g, '')
  }

  function handleUsernameInput() {
    usernameEdited = true
  }

  $: canContinue = name.trim().length >= 2 && email.trim().includes('@')

  $: canSubmit = username.trim().length >= 2
    && password.length >= 8
    && password === confirmPassword
    && !loading

  async function handleLogin() {
    if (!loginUsername || !loginPassword) return
    loginLoading = true
    try {
      const payload = await login({ username: loginUsername, password: loginPassword })
      session.set(payload)
    } catch (e) {
      toast(e?.message ?? 'Login failed', 'error')
    } finally {
      loginLoading = false
    }
  }

  async function submit() {
    if (!canSubmit) return
    loading = true
    try {
      const payload = await setupAccount({
        name: name.trim(),
        email: email.trim(),
        username: username.trim(),
        password,
      })
      session.set(payload)
      registerUser({ name: name.trim(), email: email.trim() }) // fire-and-forget
      dispatch('complete')
    } catch (e) {
      toast(e?.message ?? 'Setup failed', 'error')
    } finally {
      loading = false
    }
  }
</script>

<div class="setup-page">
  {#if step === 1}
    <div
      class="setup-card"
      in:fly={{ y: 24, duration: 600, easing: backOut }}
    >
      {#if mode === 'signup'}
        <div class="setup-brand">
          <span class="setup-icon">◈</span>
          <h1>Welcome to Schedula</h1>
          <p>Let's set up your account. This takes less than a minute.</p>
        </div>

        <div class="setup-form">
          <div class="form-group">
            <label class="form-label">Your name</label>
            <input
              class="form-input"
              bind:value={name}
              placeholder="e.g. Dr. Sarah Chen"
              autocomplete="name"
              autofocus
            />
          </div>
          <div class="form-group">
            <label class="form-label">Work email</label>
            <input
              class="form-input"
              type="email"
              bind:value={email}
              placeholder="you@university.edu"
              autocomplete="email"
            />
            <span class="field-hint">Used only to restore access if you're locked out.</span>
          </div>
          <button
            class="btn btn-primary setup-btn"
            disabled={!canContinue}
            on:click={() => (step = 2)}
          >
            Continue
          </button>
        </div>

        <div class="mode-toggle">
          Already have an account?
          <button type="button" class="link-btn" on:click={() => { mode = 'login'; loginUsername = ''; loginPassword = '' }}>
            Sign in
          </button>
        </div>

      {:else}
        <div class="setup-brand">
          <span class="setup-icon">◈</span>
          <h1>Schedula</h1>
          <p>Sign in to your account.</p>
        </div>

        <form class="setup-form" on:submit|preventDefault={handleLogin}>
          <div class="form-group">
            <label class="form-label">Username</label>
            <input
              class="form-input"
              bind:value={loginUsername}
              placeholder="username"
              autocomplete="username"
              autofocus
              spellcheck="false"
            />
          </div>
          <div class="form-group">
            <label class="form-label">Password</label>
            <input
              class="form-input"
              type="password"
              bind:value={loginPassword}
              placeholder="••••••••"
              autocomplete="current-password"
            />
          </div>
          <button
            type="submit"
            class="btn btn-primary setup-btn"
            disabled={loginLoading || !loginUsername || !loginPassword}
          >
            {loginLoading ? 'Signing in…' : 'Sign In'}
          </button>
        </form>

        <div class="mode-toggle">
          No account yet?
          <button type="button" class="link-btn" on:click={() => { mode = 'signup' }}>
            Create one
          </button>
        </div>
      {/if}
    </div>

  {:else}
    <div
      class="setup-card"
      in:fly={{ x: 24, duration: 400, easing: backOut }}
    >
      <button type="button" class="back-btn" on:click={() => (step = 1)}>
        ← Back
      </button>

      <div class="setup-brand">
        <h2>Create your password</h2>
        <p>You'll use these credentials every time you sign in.</p>
      </div>

      <form class="setup-form" on:submit|preventDefault={submit}>
        <div class="form-group">
          <label class="form-label">Username</label>
          <input
            class="form-input"
            bind:value={username}
            on:input={handleUsernameInput}
            placeholder="username"
            autocomplete="username"
            spellcheck="false"
          />
          <span class="field-hint">Auto-generated from your name — you can change it.</span>
        </div>
        <div class="form-group">
          <label class="form-label">Password</label>
          <input
            class="form-input"
            type="password"
            bind:value={password}
            placeholder="Min. 8 characters"
            autocomplete="new-password"
          />
        </div>
        <div class="form-group">
          <label class="form-label">Confirm password</label>
          <input
            class="form-input"
            type="password"
            bind:value={confirmPassword}
            placeholder="••••••••"
            autocomplete="new-password"
          />
          {#if confirmPassword && password !== confirmPassword}
            <span class="field-error">Passwords do not match</span>
          {/if}
        </div>

        <button
          type="submit"
          class="btn btn-primary setup-btn"
          disabled={!canSubmit}
        >
          {loading ? 'Creating account…' : 'Create account'}
        </button>
      </form>
    </div>
  {/if}
</div>

<style>
  .setup-page {
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    background: linear-gradient(-45deg, #0f111a, #1a1d27, #21253a, #0f111a);
    background-size: 400% 400%;
    animation: gradientBG 15s ease infinite;
  }

  @keyframes gradientBG {
    0%   { background-position: 0%   50%; }
    50%  { background-position: 100% 50%; }
    100% { background-position: 0%   50%; }
  }

  .setup-card {
    width: 420px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 20px;
    padding: 48px 40px 40px;
    box-shadow: var(--shadow-lg), 0 0 24px rgba(108, 99, 255, 0.1);
    position: relative;
  }

  .setup-brand {
    text-align: center;
    margin-bottom: 32px;
  }
  .setup-icon {
    font-size: 40px;
    color: var(--accent);
    display: block;
    margin-bottom: 10px;
  }
  .setup-brand h1 { font-size: 1.7rem; margin-bottom: 8px; }
  .setup-brand h2 { font-size: 1.4rem; margin-bottom: 8px; }
  .setup-brand p  { color: var(--text-muted); font-size: 13px; line-height: 1.5; }

  .setup-form {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .setup-btn {
    width: 100%;
    justify-content: center;
    padding: 11px;
    margin-top: 4px;
    font-size: 14px;
  }

  .field-hint {
    font-size: 11px;
    color: var(--text-muted);
    margin-top: 4px;
    display: block;
  }
  .field-error {
    font-size: 11px;
    color: #e05260;
    margin-top: 4px;
    display: block;
  }

  .back-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 12px;
    padding: 0;
    margin-bottom: 20px;
    display: block;
  }
  .back-btn:hover { color: var(--text); }

  .mode-toggle {
    text-align: center;
    margin-top: 20px;
    font-size: 13px;
    color: var(--text-muted);
  }
  .link-btn {
    background: none;
    border: none;
    color: var(--accent);
    cursor: pointer;
    font-size: 13px;
    padding: 0;
  }
  .link-btn:hover { text-decoration: underline; }
</style>
