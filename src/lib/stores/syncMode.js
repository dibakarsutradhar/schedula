import { writable } from 'svelte/store'
import { get } from 'svelte/store'

const MODE_KEY  = 'schedula_sync_mode'   // 'standalone' | 'server'
const URL_KEY   = 'schedula_hub_url'
const TOKEN_KEY = 'schedula_hub_token'

function load() {
  try {
    return {
      mode:      localStorage.getItem(MODE_KEY)  || 'standalone',
      serverUrl: localStorage.getItem(URL_KEY)   || '',
      token:     localStorage.getItem(TOKEN_KEY) || null,
    }
  } catch {
    return { mode: 'standalone', serverUrl: '', token: null }
  }
}

function createSyncModeStore() {
  const { subscribe, set, update } = writable(load())

  return {
    subscribe,

    /** Switch to standalone mode and clear credentials. */
    setStandalone() {
      localStorage.setItem(MODE_KEY, 'standalone')
      localStorage.removeItem(URL_KEY)
      localStorage.removeItem(TOKEN_KEY)
      set({ mode: 'standalone', serverUrl: '', token: null })
    },

    /** Connect to a hub server (URL without trailing slash). */
    setServer(serverUrl) {
      localStorage.setItem(MODE_KEY, 'server')
      localStorage.setItem(URL_KEY, serverUrl)
      localStorage.removeItem(TOKEN_KEY)
      set({ mode: 'server', serverUrl, token: null })
    },

    /** Store the JWT token returned after a server-mode login. */
    setToken(token) {
      if (token) localStorage.setItem(TOKEN_KEY, token)
      else        localStorage.removeItem(TOKEN_KEY)
      update(s => ({ ...s, token: token || null }))
    },

    clearToken() {
      localStorage.removeItem(TOKEN_KEY)
      update(s => ({ ...s, token: null }))
    },
  }
}

export const syncMode = createSyncModeStore()

/** True when connected to a hub server (URL configured + token present). */
export function isServerMode() {
  const s = get(syncMode)
  return s.mode === 'server' && !!s.serverUrl
}

export function getServerBase() {
  return get(syncMode).serverUrl.replace(/\/$/, '')
}

export function getToken() {
  return get(syncMode).token
}
