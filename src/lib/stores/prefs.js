import { writable } from 'svelte/store'

const STORAGE_KEY = 'schedula_prefs'

const ACCENT_PRESETS = [
  '#6c63ff', '#3b82f6', '#06b6d4', '#10b981',
  '#f59e0b', '#ef4444', '#ec4899', '#8b5cf6',
]

function loadPrefs() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw) return JSON.parse(raw)
  } catch (_) {}
  return { theme: 'dark', accentColor: '#6c63ff' }
}

function applyTheme(theme, accentColor) {
  const root = document.documentElement
  if (theme === 'light') {
    root.setAttribute('data-theme', 'light')
  } else if (theme === 'system') {
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches
    if (prefersDark) root.removeAttribute('data-theme')
    else root.setAttribute('data-theme', 'light')
  } else {
    root.removeAttribute('data-theme')
  }
  root.style.setProperty('--accent', accentColor)
  // derive accent2 as lighter version (just use the accent color slightly brighter)
  root.style.setProperty('--accent2', accentColor)
}

function createPrefsStore() {
  const initial = loadPrefs()
  const { subscribe, set, update } = writable(initial)

  // Apply on init
  if (typeof document !== 'undefined') {
    applyTheme(initial.theme, initial.accentColor)
  }

  return {
    subscribe,
    setTheme(theme) {
      update(p => {
        const next = { ...p, theme }
        localStorage.setItem(STORAGE_KEY, JSON.stringify(next))
        applyTheme(next.theme, next.accentColor)
        return next
      })
    },
    setAccent(accentColor) {
      update(p => {
        const next = { ...p, accentColor }
        localStorage.setItem(STORAGE_KEY, JSON.stringify(next))
        applyTheme(next.theme, next.accentColor)
        return next
      })
    },
  }
}

export const prefs = createPrefsStore()
export { ACCENT_PRESETS }
