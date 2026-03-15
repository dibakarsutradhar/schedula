import { writable } from 'svelte/store'
import { invoke } from '@tauri-apps/api/core'

// Persists in localStorage for UI state (not a security boundary)
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
      else localStorage.removeItem(STORAGE_KEY)
      set(val)
    },
    async restore() {
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

export const session = createSessionStore()

export const isSuperAdmin = (s) => s?.role === 'super_admin'
export const isAdmin = (s) => s?.role === 'admin' || s?.role === 'super_admin'
