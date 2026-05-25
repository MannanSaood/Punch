<script>
  import { transfers } from './store.js'
  import { fmtTime, fmtBytes } from './store.js'
  import Badge from './Badge.svelte'

  $: sorted = [...$transfers].reverse()
  $: accepted = $transfers.filter(t => t.decision === 'accepted').length
  $: rejected = $transfers.filter(t => t.decision === 'rejected').length

  function riskType(level) {
    if (!level) return 'low'
    if (level.includes('HIGH'))   return 'high'
    if (level.includes('MEDIUM')) return 'medium'
    return 'low'
  }
</script>

<div class="view">
  <div class="view-header">
    <div>
      <div class="view-title">Transfer Log</div>
      <div class="view-sub">All receive decisions — always recorded regardless of --log flag</div>
    </div>
    <div class="counts">
      <span class="count-accept">{accepted} accepted</span>
      <span class="sep">·</span>
      <span class="count-reject">{rejected} rejected</span>
    </div>
  </div>

  {#if sorted.length === 0}
    <div class="empty">
      <div class="empty-title">No transfers recorded</div>
      <div class="empty-body">Every <code>punch receive</code> decision appears here automatically.</div>
    </div>
  {:else}
    <div class="table-wrap">
      <table>
        <thead>
          <tr>
            <th>Time</th>
            <th>File</th>
            <th>Size</th>
            <th>Risk</th>
            <th>Decision</th>
            <th>Fingerprint</th>
            <th>Destination</th>
          </tr>
        </thead>
        <tbody>
          {#each sorted as t, i}
            <tr style="animation-delay:{i*20}ms">
              <td class="mono dim">{fmtTime(t.timestamp)}</td>
              <td class="filename">{t.filename || '—'}</td>
              <td class="mono dim">{fmtBytes(t.size_bytes)}</td>
              <td><Badge type={riskType(t.risk_level)} label={t.risk_level || 'LOW'} /></td>
              <td><Badge type={t.decision || 'rejected'} label={t.decision?.toUpperCase() || '—'} /></td>
              <td class="mono dim fp">{t.fingerprint || '—'}</td>
              <td class="dim dest">{t.dest_path || '—'}</td>
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

  .counts {
    display: flex;
    gap: 8px;
    align-items: center;
    font-size: 10px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }
  .count-accept { color: #f0ece4; }
  .count-reject { color: rgba(240,236,228,0.35); }
  .sep          { color: rgba(240,236,228,0.2); }

  .filename {
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: 'Courier Prime', monospace;
  }

  .fp, .dest {
    max-width: 140px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 11px;
  }
</style>
