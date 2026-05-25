import { writable, derived } from 'svelte/store'

// ── RAW DATA ─────────────────────────────────────────────────────────────────
export const sessions  = writable([])
export const tokens    = writable([])
export const transfers = writable([])
export const forwards  = writable([])
export const shells    = writable([])
export const active    = writable({ forwards: [], shells: [] })
export const lastUpdated = writable(null)
export const wsStatus  = writable('connecting') // 'connecting' | 'live' | 'offline'
export const liveEvents = writable([])

// ── DERIVED STATS ────────────────────────────────────────────────────────────
export const stats = derived(
  [sessions, transfers, forwards, tokens, active, shells],
  ([$sessions, $transfers, $forwards, $tokens, $active, $shells]) => ({
    totalSessions:   $sessions.length,
    directSessions:  $sessions.filter(s => s.connection_type === 'Direct').length,
    relaySessions:   $sessions.filter(s => s.connection_type === 'Relay').length,
    totalTransfers:  $transfers.length,
    acceptedTransfers: $transfers.filter(t => t.decision === 'accepted').length,
    totalForwards:   $forwards.length,
    activeForwards:  $active.forwards?.length ?? 0,
    activeShells:    $active.shells?.length ?? 0,
    totalTokens:     $tokens.length,
    totalShells:     $shells.length,
    bytesTransferred: $sessions.reduce((a, s) => a + (s.bytes_sent ?? 0) + (s.bytes_received ?? 0), 0),
  })
)

// ── API ──────────────────────────────────────────────────────────────────────
async function safeFetch(url) {
  try {
    const r = await fetch(url)
    return r.ok ? await r.json() : []
  } catch { return [] }
}

export async function loadAll() {
  const [s, t, tr, fw, sh, ac] = await Promise.all([
    safeFetch('/api/sessions'),
    safeFetch('/api/tokens'),
    safeFetch('/api/transfers'),
    safeFetch('/api/forwards'),
    safeFetch('/api/shells'),
    safeFetch('/api/active'),
  ])
  sessions.set(s)
  tokens.set(t)
  transfers.set(tr)
  forwards.set(fw)
  shells.set(sh)
  active.set(ac)
  lastUpdated.set(new Date())
}

// ── WEBSOCKET ─────────────────────────────────────────────────────────────────
export function connectWS() {
  const protocol = location.protocol === 'https:' ? 'wss' : 'ws'
  const ws = new WebSocket(`${protocol}://${location.host}/ws`)

  ws.onopen = () => {
    wsStatus.set('live')
  }

  ws.onmessage = (e) => {
    try {
      const event = JSON.parse(e.data)
      liveEvents.update(evts => [event, ...evts].slice(0, 100))

      // Patch specific stores on live events
      switch (event.type) {
        case 'session_start':
        case 'session_end':
          loadAll()
          break
        case 'transfer_decision':
          loadAll()
          break
        case 'forward_start':
          active.update(a => ({ ...a, forwards: [...(a.forwards ?? []), event.data] }))
          break
        case 'forward_end':
          active.update(a => ({
            ...a,
            forwards: (a.forwards ?? []).filter(f => f.id !== event.data.id)
          }))
          loadAll()
          break
        case 'shell_start':
          active.update(a => ({ ...a, shells: [...(a.shells ?? []), event.data] }))
          break
        case 'shell_end':
          active.update(a => ({
            ...a,
            shells: (a.shells ?? []).filter(s => s.id !== event.data.id)
          }))
          loadAll()
          break
        case 'shell_command':
          shells.update(sh => sh.map(s =>
            s.session_id === event.data.session_id
              ? { ...s, commands: [...(s.commands ?? []), event.data.command] }
              : s
          ))
          break
      }
    } catch {}
  }

  ws.onclose = () => {
    wsStatus.set('offline')
    // Reconnect after 3s
    setTimeout(connectWS, 3000)
  }

  ws.onerror = () => wsStatus.set('offline')

  return ws
}

// ── FORMATTERS ───────────────────────────────────────────────────────────────
export function fmtBytes(n) {
  if (!n) return '0 B'
  if (n < 1024) return n + ' B'
  if (n < 1048576) return (n / 1024).toFixed(1) + ' KB'
  if (n < 1073741824) return (n / 1048576).toFixed(2) + ' MB'
  return (n / 1073741824).toFixed(2) + ' GB'
}

export function fmtTime(iso) {
  if (!iso) return '—'
  const d = new Date(iso)
  return d.toLocaleDateString('en-GB', { day: '2-digit', month: 'short' }) +
    '  ' + d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}

export function fmtDuration(start, end) {
  if (!start) return '—'
  const secs = Math.abs(Math.round((new Date(end || Date.now()) - new Date(start)) / 1000))
  if (secs < 60) return secs + 's'
  if (secs < 3600) return Math.floor(secs / 60) + 'm ' + (secs % 60) + 's'
  return Math.floor(secs / 3600) + 'h ' + Math.floor((secs % 3600) / 60) + 'm'
}

export function tokenLabel(type) {
  if (!type) return 'T-No'
  if (type.QNo !== undefined) return 'Q-No'
  if (type.PNo !== undefined) return 'P-No'
  return 'T-No'
}
