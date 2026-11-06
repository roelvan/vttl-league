<script lang="ts">
  import { ws, activate, hasPending, close } from '../lib/workspace.svelte';
  import { settings } from '../lib/settings.svelte';
  import { getSystem } from '../lib/sheet/systems';
  import { computeScore } from '../lib/sheet/score';

  let { onvalidate }: { onvalidate: (id: string) => void } = $props();

  function score(id: string) {
    const s = ws.entries[id]?.view;
    if (!s) return null;
    try {
      return computeScore(s, getSystem(s.matchSystemId));
    } catch {
      return null;
    }
  }

  let menu = $state<{ id: string; x: number; y: number } | null>(null);
  function context(e: MouseEvent, id: string) {
    e.preventDefault();
    menu = { id, x: e.clientX, y: e.clientY };
  }
</script>

<svelte:window onclick={() => (menu = null)} />

<nav class="nav" aria-label="Open wedstrijdbladen">
  <button class="tile" class:active={ws.active === 'overview'} onclick={() => activate('overview')}>
    <span class="tile-text">Overzicht</span>
  </button>
  {#each ws.open as id (id)}
    {@const s = ws.entries[id]?.view}
    {@const sc = score(id)}
    {#if s}
      <button
        class="tile"
        class:active={ws.active === id}
        class:uploaded={s.disablePartialUpload}
        title={hasPending(id) ? 'Het wedstrijdblad bevat wijzigingen die nog niet op de server staan' : 'Alle wijzigingen staan op de server'}
        onclick={() => activate(id)}
        oncontextmenu={(e) => context(e, id)}
      >
        {#if hasPending(id)}<span class="star" aria-label="Niet opgeslagen">★</span>{/if}
        <span class="tile-text">{s.home.name || '…'}<br />{s.away.name || '…'}</span>
        {#if settings.showResultsInNav && sc}
          <span class="tile-score" class:win={sc.homeWins > sc.awayWins} class:loss={sc.homeWins < sc.awayWins}>
            <b>{sc.homeWins}</b><b>{sc.awayWins}</b>
          </span>
        {/if}
      </button>
    {/if}
  {/each}
</nav>

{#if menu}
  <div class="ctx" style:left="{menu.x}px" style:top="{menu.y}px" role="menu">
    <button role="menuitem" onclick={() => close(menu!.id)}>Sluiten</button>
    <button role="menuitem" disabled title="Komt in fase 3">Printen</button>
    <button role="menuitem" onclick={() => onvalidate(menu!.id)}>Valideer</button>
  </div>
{/if}
