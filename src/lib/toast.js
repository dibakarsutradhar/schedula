import { writable } from 'svelte/store'

export const toasts = writable([])

let id = 0
export function toast(msg, type = 'success', duration = 3200) {
  const tid = ++id
  toasts.update(t => [...t, { id: tid, msg, type }])
  setTimeout(() => toasts.update(t => t.filter(x => x.id !== tid)), duration)
}
