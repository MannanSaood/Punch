<script>
  import { sessions } from './store.js'
  import { fmtTime, fmtDuration, fmtBytes } from './store.js'
  import Badge from './Badge.svelte'

  $: sorted = [...$sessions].reverse()
</script>

<div class="view">
  <div class="view-header">
    <div>
      <div class="view-title">Session History</div>
      <div class="view-sub">All peer connections — direct and relay</div>
    </div>
    <div class="view-count">{$sessions.length} total</div>
  </div>

  {#if sorted.length === 0}
    <div class="empty">
      <div class="empty-title">No sessions recorded</div>
      <div class="empty-body">Run <code>punch generate --log</code> or <code>punch connect --log</code> to begin logging.</div>
    </div>
  {:else}
    <div class="table-wrap">
      <table>
        <thead>
          <tr>
            <th>Time</th>
            <th>Token</th>
            <th>Connection</th>
            <th>Duration</th>
            <th>Sent</th>
            <th>Received</th>
          </tr>
        </thead>
        <tbody>
          {#each sorted as s, i}
            <tr style="animation-delay: {i * 20}ms">
              <td class="mono dim">{fmtTime(s.started_at)}</td>
              <td class="mono">{s.token_code || '—'}</td>
              <td>
                <Badge
                  type={s.connection_type?.toLowerCase() === 'direct' ? 'direct' : 'relay'}
                  label={s.connection_type || 'relay'}
                />
              </td>
              <td class="dim">{fmtDuration(s.started_at, s.ended_at)}</td>
              <td class="mono">{fmtBytes(s.bytes_sent)}</td>
              <td class="mono">{fmtBytes(s.bytes_received)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
  @import './table.css';
  .view-sub { font-size: 10px; color: rgba(240,236,228,0.35); letter-spacing: 0.12em; text-transform: uppercase; margin-top: 4px; }
</style>
