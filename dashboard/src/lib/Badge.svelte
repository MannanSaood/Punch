<script>
  export let type = 'direct' // direct | relay | tno | qno | pno | high | medium | low | accepted | rejected | active | closed
  export let label = ''
</script>

<span class="badge badge-{type}">
  {#if type === 'direct'}⚡{:else if type === 'relay'}↺{:else if type === 'high'}▲{:else if type === 'medium'}◆{:else if type === 'low'}●{:else if type === 'active'}◉{:else if type === 'closed'}○{/if}
  {label || type.toUpperCase()}
</span>

<style>
  .badge {
    display: inline-block;
    font-family: 'Cinzel', serif;
    font-size: 8px;
    letter-spacing: 0.25em;
    text-transform: uppercase;
    padding: 3px 8px 2px;
    border: 1px solid currentColor;
    white-space: nowrap;
  }
  .badge-direct   { color: #f0ece4; border-color: #f0ece4; }
  .badge-relay    { color: rgba(240,236,228,0.4); border-color: rgba(240,236,228,0.4); }
  .badge-tno      { color: rgba(240,236,228,0.7); border-color: rgba(240,236,228,0.3); }
  .badge-qno      { color: #f0ece4; border-color: rgba(240,236,228,0.6); }
  .badge-pno      { color: #f0ece4; border-color: #f0ece4; }
  .badge-high     { color: #f0ece4; border-color: #f0ece4; }
  .badge-medium   { color: rgba(240,236,228,0.5); border-color: rgba(240,236,228,0.4); }
  .badge-low      { color: rgba(240,236,228,0.25); border-color: rgba(240,236,228,0.2); }
  .badge-accepted { color: #f0ece4; border-color: #f0ece4; }
  .badge-rejected { color: rgba(240,236,228,0.4); border-color: rgba(240,236,228,0.3); }
  .badge-active   { color: #f0ece4; border-color: #f0ece4; animation: pulse 2s infinite; }
  .badge-closed   { color: rgba(240,236,228,0.3); border-color: rgba(240,236,228,0.2); }
  @keyframes pulse { 0%,100%{opacity:1} 50%{opacity:0.4} }
</style>
