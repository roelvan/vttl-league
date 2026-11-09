<script lang="ts">
  import MatchTile from './MatchTile.svelte';
  import type { LiveView, GameView } from '../../lib/live';
  import type { LiveGame } from '../../lib/boards.svelte';

  let {
    view,
    live = {},
    timeoutsUsed = {},
    onopen,
  }: {
    view: LiveView;
    live?: Record<number, LiveGame>;
    /** Per game index: [home, away] took their timeout (scoreboard games only). */
    timeoutsUsed?: Record<number, [boolean, boolean]>;
    onopen: (g: GameView) => void;
  } = $props();

  let filter = $state<'all' | 'todo'>('all');
  const shown = $derived(filter === 'todo' ? view.games.filter((g) => g.state !== 'final') : view.games);
  const tableNo = (g: GameView) => (g.table === null ? null : (view.tables[g.table]?.no ?? null));
</script>

<section class="card" aria-label="Wedstrijdvolgorde">
  <div class="card-h">
    <h2>Wedstrijdvolgorde</h2>
    <span class="spacer"></span>
    <div class="filters" role="group" aria-label="Filter wedstrijden">
      <button class="seg-btn" type="button" aria-pressed={filter === 'all'} onclick={() => (filter = 'all')}>Alle {view.games.length}</button>
      <button class="seg-btn" type="button" aria-pressed={filter === 'todo'} onclick={() => (filter = 'todo')}>Te spelen</button>
    </div>
  </div>
  <div class="order">
    {#each shown as g (g.index)}
      <MatchTile game={g} boxes={view.sys.boxes} tableNo={tableNo(g)} live={live[g.index] ?? null} used={timeoutsUsed[g.index] ?? null} {onopen} />
    {:else}
      <p class="muted">Alle wedstrijden zijn gespeeld.</p>
    {/each}
  </div>
</section>
