<script>
  import { shells, active, liveEvents } from './store.js'
  import { fmtTime, fmtDuration } from './store.js'
  import Badge from './Badge.svelte'

  $: sorted = [...$shells].reverse()
  $: activeShells = $active.shells ?? []

  // Pull live shell commands from event stream
  $: liveCommands = $liveEvents
    .filter(e => e.type === 'shell_command')
    .slice(0, 50)

  let selectedSession = null

  function selectSession(id) {
    selectedSession = selectedSession === id ? null : id
  }

  function dispositionClass(d) {
    if (!d) return 'dim'
    if (d === 'Blocked' || d === 'SuspiciousBlocked') return 'blocked'
    if (d === 'SuspiciousAllowed') return 'suspicious'
    return ''
  }
</script>

<div class="view">
  <div class="view-header">
    <div>
      <div class="view-title">Shell Sessions</div>
      <div class="view-sub">Remote terminal access — command log + security alerts</div>
    </div>
    <div class="view-count">
      {activeShells.length} active · {$shells.length} total
    </div>
  </div>

  <!-- ACTIVE SESSIONS -->
  {#if activeShells.length > 0}
    <div class="active-section">
      <div class="active-label">◉ Active Shell Sessions</div>

      {#each activeShells as s}
        <div class="active-shell">
          <div class="shell-header">
            <div class="shell-peer">
              <Badge type="active" label="live" />
              <span class="mono peer-id">{s.peer_node_id?.slice(0,16) || '?'}...</span>
            </div>
            <div class="shell-meta-right">
              <span class="dim">{s.token_type || 'T-No'}</span>
              <span class="dim">·</span>
              <span class="dim">{fmtDuration(s.started_at, null)}</span>
            </div>
          </div>

          <!-- Live command feed -->
          <div class="command-feed">
            <div class="feed-label">Live Commands</div>
            {#if liveCommands.filter(e => e.data?.session_id === s.session_id).length === 0}
              <div class="feed-empty mono dim">Waiting for input...</div>
            {:else}
              {#each liveCommands.filter(e => e.data?.session_id === s.session_id) as ev}
                <div class="feed-line mono {dispositionClass(ev.data?.disposition)}">
                  <span class="feed-time dim">{fmtTime(ev.data?.timestamp)}</span>
                  <span class="feed-cmd">{ev.data?.command || ''}</span>
                  {#if ev.data?.disposition === 'Blocked' || ev.data?.disposition === 'SuspiciousBlocked'}
                    <span class="feed-tag blocked">⛔ BLOCKED</span>
                  {:else if ev.data?.disposition === 'SuspiciousAllowed'}
                    <span class="feed-tag suspicious">⚠ ALLOWED</span>
                  {/if}
                </div>
              {/each}
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}

  <!-- SESSION HISTORY -->
  {#if sorted.length === 0 && activeShells.length === 0}
    <div class="empty">
      <div class="empty-title">No shell sessions recorded</div>
      <div class="empty-body">Run <code>punch shell host</code> on this device to allow remote terminal access.</div>
    </div>
  {:else if sorted.length > 0}
    <div class="section-label">History — click to expand commands</div>
    <div class="session-list">
      {#each sorted as s, i}
        {@const isOpen = selectedSession === s.session_id}
        <div class="session-item" style="animation-delay:{i*20}ms">
          <button class="session-row" on:click={() => selectSession(s.session_id)}>
            <span class="mono dim">{fmtTime(s.started_at)}</span>
            <span class="mono peer">{s.peer_node_id?.slice(0,12) || '?'}...</span>
            <Badge type={s.token_type?.toLowerCase().replace('-','') || 'tno'} label={s.token_type || 'T-No'} />
            <span class="dim">{fmtDuration(s.started_at, s.ended_at)}</span>
            <span class="cmd-count mono">{s.commands?.length ?? 0} cmds</span>
            <span class="terminated dim">{s.terminated_by?.replace(/_/g,' ') || '—'}</span>
            <span class="expand-icon dim">{isOpen ? '▲' : '▼'}</span>
          </button>

          {#if isOpen && s.commands?.length > 0}
            <div class="commands-expanded">
              <div class="cmd-table-head">
                <span>Time</span>
                <span>Command</span>
                <span>Disposition</span>
              </div>
              {#each s.commands as cmd}
                <div class="cmd-row mono {dispositionClass(cmd.disposition)}">
                  <span class="dim">{fmtTime(cmd.timestamp)}</span>
                  <span class="cmd-text">{cmd.command}</span>
                  <span class="disposition {dispositionClass(cmd.disposition)}">
                    {#if cmd.disposition === 'Blocked' || cmd.disposition === 'SuspiciousBlocked'}
                      ⛔ {cmd.disposition}
                    {:else if cmd.disposition === 'SuspiciousAllowed'}
                      ⚠ {cmd.disposition}
                    {:else}
                      ✓ {cmd.disposition || 'Allowed'}
                    {/if}
                  </span>
                </div>
              {/each}
            </div>
          {:else if isOpen}
            <div class="no-commands mono dim">No commands recorded for this session.</div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  @import './table.css';

  .view-sub { font-size:10px; color:rgba(240,236,228,0.35); letter-spacing:0.12em; text-transform:uppercase; margin-top:4px; }

  .active-section { margin-top:24px; margin-bottom:32px; }

  .active-label {
    font-family:'Cinzel',serif;
    font-size:9px; letter-spacing:0.3em; text-transform:uppercase;
    color:#f0ece4; margin-bottom:12px;
    animation:pulse 2s infinite;
  }
  @keyframes pulse { 0%,100%{opacity:1} 50%{opacity:0.4} }

  .active-shell {
    border:1px solid rgba(240,236,228,0.14);
    margin-bottom:12px;
  }

  .shell-header {
    display:flex; justify-content:space-between; align-items:center;
    padding:14px 16px;
    border-bottom:1px solid rgba(240,236,228,0.08);
  }

  .shell-peer { display:flex; align-items:center; gap:12px; }
  .peer-id    { font-size:12px; }

  .shell-meta-right {
    display:flex; gap:8px; align-items:center;
    font-size:10px; letter-spacing:0.1em; text-transform:uppercase;
  }

  .command-feed { padding:12px 16px; min-height:64px; }

  .feed-label {
    font-family:'Cinzel',serif; font-size:8px;
    letter-spacing:0.3em; text-transform:uppercase;
    color:rgba(240,236,228,0.3); margin-bottom:10px;
  }

  .feed-empty { font-size:11px; }

  .feed-line {
    display:flex; align-items:center; gap:12px;
    font-size:11px; padding:4px 0;
    border-bottom:1px solid rgba(240,236,228,0.04);
  }

  .feed-time { min-width:80px; font-size:10px; }
  .feed-cmd  { flex:1; }
  .feed-tag  { font-size:9px; letter-spacing:0.2em; font-family:'Cinzel',serif; }

  .blocked   { color:rgba(240,236,228,0.5); }
  .suspicious{ color:rgba(240,236,228,0.7); }

  .section-label {
    font-family:'Cinzel',serif; font-size:9px;
    letter-spacing:0.3em; text-transform:uppercase;
    color:rgba(240,236,228,0.35);
    padding-bottom:10px; border-bottom:1px solid rgba(240,236,228,0.14);
    margin-top:8px;
  }

  .session-list { display:flex; flex-direction:column; }

  .session-item {
    border-bottom:1px solid rgba(240,236,228,0.08);
    animation:fadeUp 0.3s ease both;
  }
  @keyframes fadeUp { from{opacity:0;transform:translateY(4px)} to{opacity:1;transform:translateY(0)} }

  .session-row {
    display:grid;
    grid-template-columns: 130px 120px auto 80px 70px 1fr 20px;
    gap:16px; align-items:center;
    width:100%; background:none; border:none;
    color:#f0ece4; text-align:left;
    padding:13px 0; font-size:12px;
    cursor:crosshair;
    transition:background 0.15s;
  }
  .session-row:hover { background:rgba(240,236,228,0.03); }

  .peer      { font-size:11px; }
  .cmd-count { font-size:11px; }
  .terminated{ font-size:10px; letter-spacing:0.05em; text-transform:uppercase; }
  .expand-icon{ font-size:9px; text-align:right; }

  .commands-expanded {
    background:rgba(240,236,228,0.02);
    border-top:1px solid rgba(240,236,228,0.08);
    padding:0 0 8px;
  }

  .cmd-table-head {
    display:grid; grid-template-columns:120px 1fr 160px;
    gap:16px; padding:10px 16px 6px;
    font-family:'Cinzel',serif; font-size:8px;
    letter-spacing:0.25em; text-transform:uppercase;
    color:rgba(240,236,228,0.3);
    border-bottom:1px solid rgba(240,236,228,0.08);
  }

  .cmd-row {
    display:grid; grid-template-columns:120px 1fr 160px;
    gap:16px; padding:8px 16px;
    font-size:11px;
    border-bottom:1px solid rgba(240,236,228,0.04);
  }

  .cmd-text { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }

  .disposition { font-size:10px; letter-spacing:0.08em; text-transform:uppercase; }
  .disposition.blocked    { color:rgba(240,236,228,0.5); }
  .disposition.suspicious { color:rgba(240,236,228,0.7); }

  .no-commands {
    padding:16px; font-size:11px;
    border-top:1px solid rgba(240,236,228,0.08);
  }
</style>
