<script>
  import { forwards, active } from './store.js'
  import { fmtTime, fmtDuration } from './store.js'
  import Badge from './Badge.svelte'

  $: sorted = [...$forwards].reverse()
  $: activeForwards = $active.forwards ?? []
</script>

<div class="view">
  <div class="view-header">
    <div>
      <div class="view-title">Port Forwards</div>
      <div class="view-sub">TCP + UDP sessions — direct Iroh QUIC, no relay bottleneck</div>
    </div>
    <div class="view-count">
      {activeForwards.length} active · {$forwards.length} total
    </div>
  </div>

  {#if activeForwards.length > 0}
    <div class="active-section">
      <div class="active-label">◉ Live Sessions</div>
      <div class="active-cards">
        {#each activeForwards as f}
          <div class="active-card">
            <div class="active-port">{f.port}</div>
            <div class="active-meta">
              <div class="active-row">
                <Badge type="active" label="live" />
                <span class="mono">{f.protocol || 'TCP'}</span>
              </div>
              <div class="active-row dim">
                <span>Started</span>
                <span class="mono">{fmtTime(f.started_at)}</span>
              </div>
              <div class="active-row dim">
                <span>Streams</span>
                <span class="mono">{f.stream_count ?? 0} active</span>
              </div>
              <div class="active-row dim">
                <span>Fingerprint</span>
                <span class="mono">{f.fingerprint || '—'}</span>
              </div>
            </div>
          </div>
        {/each}
      </div>
    </div>
  {/if}

  {#if sorted.length === 0}
    <div class="empty">
      <div class="empty-title">No forward sessions recorded</div>
      <div class="empty-body">Run <code>punch forward expose &lt;port&gt;</code> to start forwarding.</div>
    </div>
  {:else}
    <div class="section-label">History</div>
    <div class="table-wrap">
      <table>
        <thead>
          <tr>
            <th>Time</th>
            <th>Role</th>
            <th>Port</th>
            <th>Protocol</th>
            <th>Token</th>
            <th>Duration</th>
            <th>Fingerprint</th>
          </tr>
        </thead>
        <tbody>
          {#each sorted as f, i}
            <tr style="animation-delay:{i*20}ms">
              <td class="mono dim">{fmtTime(f.timestamp)}</td>
              <td class="mono">{f.role || '—'}</td>
              <td class="mono port">{f.port || '—'}</td>
              <td>{f.protocol || 'TCP'}</td>
              <td><Badge type={f.token_type?.toLowerCase().replace('-','') || 'tno'} label={f.token_type || 'T-No'} /></td>
              <td class="dim">{fmtDuration(f.timestamp, f.ended_at)}</td>
              <td class="mono dim fp">{f.fingerprint || '—'}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
  @import './table.css';

  .view-sub { font-size:10px; color:rgba(240,236,228,0.35); letter-spacing:0.12em; text-transform:uppercase; margin-top:4px; }

  .active-section { margin-top: 24px; margin-bottom: 32px; }

  .active-label {
    font-family: 'Cinzel', serif;
    font-size: 9px;
    letter-spacing: 0.3em;
    text-transform: uppercase;
    color: #f0ece4;
    margin-bottom: 12px;
    animation: pulse 2s infinite;
  }
  @keyframes pulse { 0%,100%{opacity:1} 50%{opacity:0.4} }

  .active-cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 1px;
    background: rgba(240,236,228,0.14);
    border: 1px solid rgba(240,236,228,0.14);
  }

  .active-card {
    background: #080808;
    padding: 20px;
    display: flex;
    gap: 16px;
    align-items: flex-start;
  }

  .active-port {
    font-family: 'Cinzel', serif;
    font-size: 32px;
    font-weight: 900;
    color: #f0ece4;
    letter-spacing: 0.1em;
    flex-shrink: 0;
    padding-top: 2px;
  }

  .active-meta { flex: 1; display: flex; flex-direction: column; gap: 7px; }

  .active-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    gap: 8px;
  }
  .active-row.dim { color: rgba(240,236,228,0.35); }
  .active-row.dim span:last-child { color: #f0ece4; }

  .section-label {
    font-family: 'Cinzel', serif;
    font-size: 9px;
    letter-spacing: 0.3em;
    text-transform: uppercase;
    color: rgba(240,236,228,0.35);
    margin-top: 8px;
    padding-bottom: 10px;
    border-bottom: 1px solid rgba(240,236,228,0.14);
  }

  .port { font-size: 14px; font-weight: 700; }
  .fp   { max-width:120px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; font-size:11px; }
</style>
