<script>
  import { login } from '../lib/api.js'
  import { session } from '../lib/stores/session.js'
  import { toast } from '../lib/toast.js'

  let username = ''
  let password = ''
  let loading = false

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
          placeholder="admin"
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

    <div class="login-hint">
      Default credentials: <code>admin</code> / <code>admin123</code>
    </div>
  </div>
</div>

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
  .login-hint {
    text-align: center;
    margin-top: 20px;
    font-size: 12px;
    color: var(--text-muted);
  }
  code { background: var(--surface2); padding: 1px 6px; border-radius: 4px; font-size: 11px; }
</style>
