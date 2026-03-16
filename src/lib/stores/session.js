import { writable } from 'svelte/store'
import { invoke }   from '@tauri-apps/api/core'
import { get }      from 'svelte/store'
import { syncMode } from './syncMode.js'

const STORAGE_KEY = 'schedula_session'

function createSessionStore() {
  const stored = (() => {
    try { return JSON.parse(localStorage.getItem(STORAGE_KEY) || 'null') } catch { return null }
  })()

  const { subscribe, set } = writable(stored)

  return {
    subscribe,

    set(val) {
      if (val) localStorage.setItem(STORAGE_KEY, JSON.stringify(val))
      else     localStorage.removeItem(STORAGE_KEY)
      set(val)
    },

    async restore() {
      const { mode, serverUrl, token } = get(syncMode)

      if (mode === 'server' && serverUrl && token) {
        try {
          const base = serverUrl.replace(/\/$/, '')
          const res = await fetch(`${base}/api/auth/session`, {
            headers: { 'Authorization': `Bearer ${token}` },
          })
          if (res.ok) {
            const s = await res.json()
            this.set(s)
            return s
          } else {
            // Token expired — clear it so the login screen appears
            syncMode.clearToken()
            this.set(null)
            return null
          }
        } catch {
          this.set(null)
          return null
        }
      }

      // Standalone Tauri mode
      try {
        const s = await invoke('get_session')
        this.set(s)
        return s
      } catch {
        this.set(null)
        return null
      }
    },
  }
}

export const session      = createSessionStore()
export const isSuperAdmin = (s) => s?.role === 'super_admin'
export const isAdmin      = (s) => s?.role === 'admin' || s?.role === 'super_admin'
