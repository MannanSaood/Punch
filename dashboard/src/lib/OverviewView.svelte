<script>
  import { stats, sessions, transfers, forwards, shells, active, liveEvents, fmtBytes, fmtTime } from './store.js'
  import Badge from './Badge.svelte'

  $: recentActivity = buildActivity($sessions, $transfers, $forwards, $shells)

  function buildActivity(s, t, f, sh) {
    const items = [
      ...s.map(x => ({ time: x.started_at, type: 'session', label: `Connection · ${x.connection_type || 'relay'}`, sub: x.token_code || '' })),
      ...t.map(x => ({ time: x.timestamp,  type: 'transfer', label: `Transfer · ${x.filename || '?'}`, sub: x.decision || '' })),
      ...f.map(x => ({ time: x.timestamp,  type: 'forward',  label: `Forward · port ${x.port}`, sub: x.protocol || 'TCP' })),
      ...sh.map(x => ({ time: x.started_at, type: 'shell',  label: `Shell · ${x.peer_node_id?.slice(0,10) || '?'}...`, sub: `${x.commands?.length ?? 0} commands` })),
    ]
    return items
      .filter(x => x.time)
      .sort((a, b) => new Date(b.time) - new Date(a.time))
      .slice(0, 20)
  }

  function activityIcon(type) {
    return { session: '⇄', transfer: '↓', forward: '⇒', shell: '$' }[type] || '·'
  }
</script>

<div class="view">

  <!-- STAT GRID -->
  <div class="stat-grid">
    <div class="stat-card large">
      <div class="stat-label">Sessions</div>
      <div class="stat-value">{$stats.totalSessions}</div>
      <div class="stat-breakdown">
        <span>{$stats.directSessions} direct</span>
        <span class="sep">·</span>
        <span class="dim">{$stats.relaySessions} relay</span>
      </div>
    </div>

    <div class="stat-card large">
      <div class="stat-label">Transfers</div>
      <div class="stat-value">{$stats.totalTransfers}</div>
      <div class="stat-breakdown">
        <span>{$stats.acceptedTransfers} accepted</span>
        <span class="sep">·</span>
        <span class="dim">{$stats.totalTransfers - $stats.acceptedTransfers} rejected</span>
      </div>
    </div>

    <div class="stat-card large">
      <div class="stat-label">Data Moved</div>
      <div class="stat-value">{fmtBytes($stats.bytesTransferred)}</div>
      <div class="stat-breakdown dim">sent + received</div>
    </div>

    <div class="stat-card large">
      <div class="stat-label">Tokens</div>
      <div class="stat-value">{$stats.totalTokens}</div>
      <div class="stat-breakdown dim">active</div>
    </div>

    <!-- Live indicators -->
    <div class="stat-card live" class:is-active={$stats.activeForwards > 0}>
      <div class="stat-label">
        {#if $stats.activeForwards > 0}<span class="pulse-dot"></span>{/if}
        Port Forwards
      </div>
      <div class="stat-value">{$stats.activeForwards}</div>
      <div class="stat-breakdown dim">currently active</div>
    </div>

    <div class="stat-card live" class:is-active={$stats.activeShells > 0}>
      <div class="stat-label">
        {#if $stats.activeShells > 0}<span class="pulse-dot"></span>{/if}
        Shell Sessions
      </div>
      <div class="stat-value">{$stats.activeShells}</div>
      <div class="stat-breakdown dim">currently active</div>
    </div>
  </div>

  <!-- ACTIVE NOW -->
  {#if ($active.forwards?.length > 0) || ($active.shells?.length > 0)}
    <div class="section-head">
      <div class="section-title">◉ Active Now</div>
    </div>
    <div class="active-now">
      {#each ($active.forwards ?? []) as f}
        <div class="active-pill">
          <span class="active-icon">⇒</span>
          <span class="mono">:{f.port}</span>
          <span class="dim">{f.protocol || 'TCP'}</span>
          <Badge type="active" label="forward" />
        </div>
      {/each}
      {#each ($active.shells ?? []) as s}
        <div class="active-pill">
          <span class="active-icon">$</span>
          <span class="mono">{s.peer_node_id?.slice(0,10) || '?'}...</span>
          <Badge type="active" label="shell" />
        </div>
      {/each}
    </div>
  {/if}

  <!-- RECENT ACTIVITY -->
  <div class="section-head" style="margin-top:40px">
    <div class="section-title">Recent Activity</div>
    <div class="section-count">{recentActivity.length} events</div>
  </div>

  {#if recentActivity.length === 0}
    <div class="empty">
      <div class="empty-title">No activity recorded</div>
      <div class="empty-body">Use Punch and activity will appear here.</div>
    </div>
  {:else}
    <div class="activity-feed">
      {#each recentActivity as item, i}
        <div class="activity-row" style="animation-delay:{i*15}ms">
          <span class="activity-icon mono">{activityIcon(item.type)}</span>
          <span class="activity-time mono dim">{fmtTime(item.time)}</span>
          <span class="activity-label">{item.label}</span>
          <span class="activity-sub mono dim">{item.sub}</span>
        </div>
      {/each}
    </div>
  {/if}

  <!-- LIVE EVENTS -->
  {#if $liveEvents.length > 0}
    <div class="section-head" style="margin-top:40px">
      <div class="section-title">◉ Live Event Stream</div>
      <div class="section-count">{$liveEvents.length} events</div>
    </div>
    <div class="live-feed">
      {#each $liveEvents.slice(0, 15) as ev, i}
        <div class="live-row mono" style="animation-delay:{i*10}ms">
          <span class="dim">{fmtTime(ev.timestamp || new Date().toISOString())}</span>
          <span class="event-type">{ev.type?.replace(/_/g,' ') || '—'}</span>
          <span class="dim">{JSON.stringify(ev.data || {})}</span>
        </div>
      {/each}
    </div>
  {/if}

</div>

<style>
  @import './table.css';

  .stat-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    border-top: 1px solid rgba(240,236,228,0.14);
  }

  @media (max-width:700px) {
    .stat-grid { grid-template-columns: repeat(2,1fr); }
  }

  .stat-card {
    padding: 28px 0;
    border-right: 1px solid rgba(240,236,228,0.14);
    border-bottom: 1px solid rgba(240,236,228,0.14);
    position: relative;
    padding-left: 24px;
    transition: background 0.2s;
  }
  .stat-card:nth-child(3n) { border-right: none; padding-left:24px; }
  .stat-card:first-child   { padding-left: 0; }
  .stat-card.is-active { background: rgba(240,236,228,0.03); }

  .stat-label {
    font-family:'Cinzel',serif; font-size:9px;
    letter-spacing:0.3em; text-transform:uppercase;
    color:rgba(240,236,228,0.35); margin-bottom:10px;
    display:flex; align-items:center; gap:8px;
  }

  .stat-value {
    font-family:'Cinzel',serif; font-weight:900;
    font-size:clamp(28px,4vw,48px); line-height:1;
    color:#f0ece4; margin-bottom:6px;
  }

  .stat-breakdown {
    font-size:10px; color:#f0ece4;
    letter-spacing:0.08em; text-transform:uppercase;
    display:flex; gap:6px; align-items:center;
  }
  .stat-breakdown.dim { color:rgba(240,236,228,0.35); }

  .sep { color:rgba(240,236,228,0.2); }

  .pulse-dot {
    width:6px; height:6px; border-radius:50%;
    background:#f0ece4; flex-shrink:0;
    animation:pulse 2s infinite;
  }
  @keyframes pulse { 0%,100%{opacity:1} 50%{opacity:0.2} }

  .section-head {
    display:flex; align-items:baseline; justify-content:space-between;
    padding-bottom:10px; border-bottom:1px solid #f0ece4; margin-top:40px;
  }
  .section-title {
    font-family:'Cinzel',serif; font-weight:600; font-size:11px;
    letter-spacing:0.35em; text-transform:uppercase;
  }
  .section-count { font-size:10px; color:rgba(240,236,228,0.35); letter-spacing:0.15em; text-transform:uppercase; }

  .active-now {
    display:flex; flex-wrap:wrap; gap:8px; padding-top:12px;
  }

  .active-pill {
    display:flex; align-items:center; gap:10px;
    border:1px solid rgba(240,236,228,0.2); padding:8px 14px;
    font-size:12px;
  }
  .active-icon { font-size:14px; }

  .activity-feed { display:flex; flex-direction:column; }

  .activity-row {
    display:grid; grid-template-columns:24px 130px 1fr auto;
    gap:16px; align-items:center;
    padding:11px 0; font-size:12px;
    border-bottom:1px solid rgba(240,236,228,0.06);
    animation:fadeUp 0.25s ease both;
  }
  @keyframes fadeUp { from{opacity:0;transform:translateY(3px)} to{opacity:1;transform:translateY(0)} }

  .activity-icon  { text-align:center; color:rgba(240,236,228,0.5); font-size:13px; }
  .activity-time  { font-size:11px; }
  .activity-label { }
  .activity-sub   { font-size:11px; text-align:right; }

  .live-feed { display:flex; flex-direction:column; }

  .live-row {
    display:grid; grid-template-columns:130px 120px 1fr;
    gap:16px; padding:8px 0; font-size:11px;
    border-bottom:1px solid rgba(240,236,228,0.05);
    animation:fadeUp 0.2s ease both;
    overflow:hidden;
  }
  .event-type { text-transform:uppercase; letter-spacing:0.08em; font-size:10px; }
</style>
