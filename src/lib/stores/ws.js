import { writable } from 'svelte/store'
import { get } from 'svelte/store'
import { syncMode } from './syncMode.js'

export const wsConnected  = writable(false)
export const lastWsEvent  = writable(null)   // { entity, action, _ts }

let socket          = null
let reconnectTimer  = null
let shouldReconnect = false

export function connectWs() {
  const { mode, serverUrl } = get(syncMode)
  if (mode !== 'server' || !serverUrl) return

  shouldReconnect = true
  const wsUrl = serverUrl.replace(/^http/, 'ws').replace(/\/$/, '') + '/ws'

  try {
    socket = new WebSocket(wsUrl)

    socket.onopen = () => {
      wsConnected.set(true)
      if (reconnectTimer) { clearTimeout(reconnectTimer); reconnectTimer = null }
    }

    socket.onclose = () => {
      wsConnected.set(false)
      socket = null
      if (shouldReconnect) {
        reconnectTimer = setTimeout(connectWs, 5000)
      }
    }

    socket.onerror = () => {
      wsConnected.set(false)
    }

    socket.onmessage = (e) => {
      try {
        const event = JSON.parse(e.data)
        lastWsEvent.set({ ...event, _ts: Date.now() })
      } catch (_) {}
    }
  } catch (_) {
    wsConnected.set(false)
  }
}

export function disconnectWs() {
  shouldReconnect = false
  if (reconnectTimer) { clearTimeout(reconnectTimer); reconnectTimer = null }
  if (socket) { socket.close(); socket = null }
  wsConnected.set(false)
}

/** Ping the hub /health endpoint to test reachability. */
export async function pingHub(serverUrl) {
  try {
    const res = await fetch(serverUrl.replace(/\/$/, '') + '/health', { signal: AbortSignal.timeout(5000) })
    return res.ok
  } catch {
    return false
  }
}
