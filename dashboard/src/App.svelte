<script>
  import { onMount, onDestroy } from 'svelte'
  import { loadAll, connectWS, wsStatus, lastUpdated, stats, active } from './lib/store.js'
  import OverviewView   from './lib/OverviewView.svelte'
  import SessionsView   from './lib/SessionsView.svelte'
  import TokensView     from './lib/TokensView.svelte'
  import TransfersView  from './lib/TransfersView.svelte'
  import ForwardsView   from './lib/ForwardsView.svelte'
  import ShellView      from './lib/ShellView.svelte'
  import './app.css'

  let activeTab = 'overview'
  let ws
  let interval

  const tabs = [
    { id: 'overview',   label: 'Overview'  },
    { id: 'sessions',   label: 'Sessions'  },
    { id: 'tokens',     label: 'Tokens'    },
    { id: 'transfers',  label: 'Transfers' },
    { id: 'forwards',   label: 'Forwards'  },
    { id: 'shell',      label: 'Shell'     },
  ]

  onMount(() => {
    loadAll()
    ws = connectWS()
    interval = setInterval(loadAll, 10000)
  })

  onDestroy(() => {
    ws?.close()
    clearInterval(interval)
  })

  function fmt(d) {
    if (!d) return '—'
    return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })
  }

  $: hasLive = $stats.activeForwards > 0 || $stats.activeShells > 0
</script>

<!-- ── HEADER ── -->
<header>
  <div class="logo-block">
    <div class="logotype">PUNCH<span class="dot">●</span></div>
    <div class="logo-sub">Local Intelligence · Zero External Requests</div>
  </div>

  <nav>
    {#each tabs as tab}
      <button
        class="nav-btn"
        class:active={activeTab === tab.id}
        on:click={() => activeTab = tab.id}
      >
        {tab.label}
        {#if tab.id === 'forwards' && $stats.activeForwards > 0}
          <span class="nav-dot"></span>
        {/if}
        {#if tab.id === 'shell' && $stats.activeShells > 0}
          <span class="nav-dot"></span>
        {/if}
      </button>
    {/each}
  </nav>

  <div class="header-right">
    <div class="ws-status" class:live={$wsStatus === 'live'} class:offline={$wsStatus === 'offline'}>
      <span class="ws-dot"></span>
      {$wsStatus}
    </div>
    <div class="updated">Updated {fmt($lastUpdated)}</div>
  </div>
</header>

<div class="header-rule"></div>

<!-- ── MAIN ── -->
<main>
  {#if activeTab === 'overview'}
    <OverviewView />
  {:else if activeTab === 'sessions'}
    <SessionsView />
  {:else if activeTab === 'tokens'}
    <TokensView />
  {:else if activeTab === 'transfers'}
    <TransfersView />
  {:else if activeTab === 'forwards'}
    <ForwardsView />
  {:else if activeTab === 'shell'}
    <ShellView />
  {/if}
</main>

<!-- ── FOOTER ── -->
<footer>
  <div class="footer-left">PUNCH · Local Dashboard · localhost:7777</div>
  <div class="footer-right">
    {$stats.totalSessions} sessions ·
    {$stats.totalTransfers} transfers ·
    {$stats.totalTokens} tokens
  </div>
</footer>

<style>
  /* ── HEADER ── */
  header {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: end;
    gap: 40px;
    padding: 40px 60px 28px;
  }

  @media (max-width: 900px) {
    header {
      grid-template-columns: 1fr;
      padding: 24px;
      gap: 20px;
    }
  }

  .logo-block { flex-shrink: 0; }

  .logotype {
    font-family: 'Cinzel', serif;
    font-weight: 900;
    font-size: clamp(40px, 6vw, 72px);
    letter-spacing: 0.28em;
    line-height: 1;
    color: #f0ece4;
    position: relative;
    display: inline-block;
  }

  .dot {
    font-size: 11px;
    vertical-align: super;
    margin-left: 4px;
    color: #f0ece4;
    letter-spacing: 0;
  }

  .logo-sub {
    font-size: 9px;
    letter-spacing: 0.25em;
    text-transform: uppercase;
    color: rgba(240,236,228,0.3);
    margin-top: 6px;
  }

  /* ── NAV ── */
  nav {
    display: flex;
    align-items: flex-end;
    gap: 0;
    border-bottom: 1px solid rgba(240,236,228,0.14);
    padding-bottom: 0;
    overflow-x: auto;
  }

  .nav-btn {
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    color: rgba(240,236,228,0.35);
    font-family: 'Cinzel', serif;
    font-size: 9px;
    letter-spacing: 0.3em;
    text-transform: uppercase;
    padding: 10px 20px 10px;
    cursor: crosshair;
    transition: all 0.15s;
    position: relative;
    display: flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
    margin-bottom: -1px;
  }

  .nav-btn:hover {
    color: rgba(240,236,228,0.7);
  }

  .nav-btn.active {
    color: #f0ece4;
    border-bottom-color: #f0ece4;
  }

  .nav-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: #f0ece4;
    animation: pulse 2s infinite;
  }

  @keyframes pulse { 0%,100%{opacity:1} 50%{opacity:0.2} }

  /* ── HEADER RIGHT ── */
  .header-right {
    text-align: right;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding-bottom: 12px;
  }

  .ws-status {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 6px;
    font-size: 9px;
    letter-spacing: 0.25em;
    text-transform: uppercase;
    color: rgba(240,236,228,0.3);
  }

  .ws-status.live   { color: #f0ece4; }
  .ws-status.offline{ color: rgba(240,236,228,0.2); }

  .ws-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
  }

  .ws-status.live .ws-dot {
    animation: pulse 2s infinite;
  }

  .updated {
    font-size: 9px;
    color: rgba(240,236,228,0.25);
    letter-spacing: 0.15em;
    text-transform: uppercase;
  }

  /* ── RULE ── */
  .header-rule {
    height: 3px;
    background: #f0ece4;
    margin: 0 60px;
  }

  @media (max-width: 900px) {
    .header-rule { margin: 0 24px; }
  }

  /* ── MAIN ── */
  main {
    padding: 40px 60px 100px;
    max-width: 1400px;
    margin: 0 auto;
  }

  @media (max-width: 900px) {
    main { padding: 24px 24px 100px; }
  }

  /* ── FOOTER ── */
  footer {
    position: fixed;
    bottom: 0; left: 0; right: 0;
    padding: 10px 60px;
    border-top: 1px solid rgba(240,236,228,0.14);
    background: #080808;
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 10px;
    color: rgba(240,236,228,0.25);
    letter-spacing: 0.12em;
    text-transform: uppercase;
    z-index: 100;
  }

  @media (max-width: 600px) {
    footer { padding: 10px 24px; }
    .footer-right { display: none; }
  }
</style>
