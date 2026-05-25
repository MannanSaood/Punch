<script>
  import { tokens } from './store.js'
  import { fmtTime, tokenLabel } from './store.js'
  import Badge from './Badge.svelte'
</script>

<div class="view">
  <div class="view-header">
    <div>
      <div class="view-title">Active Tokens</div>
      <div class="view-sub">T-No · Q-No · P-No — stored on this device only</div>
    </div>
    <div class="view-count">{$tokens.length} active</div>
  </div>

  {#if $tokens.length === 0}
    <div class="empty">
      <div class="empty-title">No active tokens</div>
      <div class="empty-body">Generate with <code>punch generate --uses N</code> or <code>punch generate --permanent</code></div>
    </div>
  {:else}
    <div class="token-grid">
      {#each $tokens as t, i}
        {@const type = tokenLabel(t.token_type)}
        {@const remaining = t.token_type?.QNo?.remaining}
        {@const verified = t.token_type?.PNo?.verified}
        <div class="token-card" style="animation-delay:{i*40}ms">
          <div class="token-code">{t.code}</div>
          <div class="token-type">
            <Badge type={type.toLowerCase().replace('-','')} label={type} />
          </div>
          <div class="token-rows">
            <div class="token-row">
              <span>Created</span>
              <span class="mono">{fmtTime(t.created_at)}</span>
            </div>
            <div class="token-row">
              <span>Last used</span>
              <span class="mono">{t.last_used ? fmtTime(t.last_used) : '—'}</span>
            </div>
            {#if remaining !== undefined}
              <div class="token-row">
                <span>Remaining</span>
                <span class="mono">{remaining} uses</span>
              </div>
              <div class="usage-bar">
                <div class="usage-fill" style="width:{Math.min(100, remaining * 10)}%"></div>
              </div>
            {/if}
            {#if verified !== undefined}
              <div class="token-row">
                <span>Verified</span>
                <span class="mono">{verified ? '✓ Yes' : '✗ No — run: punch verify ' + t.code}</span>
              </div>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  @import './table.css';

  .view-sub { font-size:10px; color:rgba(240,236,228,0.35); letter-spacing:0.12em; text-transform:uppercase; margin-top:4px; }

  .token-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 1px;
    background: rgba(240,236,228,0.14);
    border: 1px solid rgba(240,236,228,0.14);
    margin-top: 1px;
  }

  .token-card {
    background: #080808;
    padding: 28px 24px;
    animation: fadeUp 0.4s ease both;
    transition: background 0.2s;
  }
  .token-card:hover { background: rgba(240,236,228,0.03); }

  .token-code {
    font-family: 'Cinzel', serif;
    font-size: 40px;
    font-weight: 900;
    letter-spacing: 0.2em;
    color: #f0ece4;
    display: block;
    margin-bottom: 12px;
  }

  .token-type { margin-bottom: 16px; }

  .token-rows { display: flex; flex-direction: column; gap: 7px; }

  .token-row {
    display: flex;
    justify-content: space-between;
    font-size: 10px;
    color: rgba(240,236,228,0.35);
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }
  .token-row span:last-child { color: #f0ece4; }

  .usage-bar {
    height: 1px;
    background: rgba(240,236,228,0.14);
    margin-top: 4px;
    position: relative;
  }
  .usage-fill {
    position: absolute;
    top: 0; left: 0; height: 1px;
    background: #f0ece4;
    transition: width 0.6s ease;
  }

  @keyframes fadeUp {
    from { opacity:0; transform:translateY(6px); }
    to   { opacity:1; transform:translateY(0); }
  }
</style>
