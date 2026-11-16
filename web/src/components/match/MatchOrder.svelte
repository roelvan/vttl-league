<script lang="ts">
  import MatchTile from './MatchTile.svelte';
  import type { LiveView, GameView } from '../../lib/live';
  import type { LiveGame } from '../../lib/boards.svelte';
  import Icon from '../Icon.svelte';
  import { settings, saveSettings } from '../../lib/settings.svelte';

  let {
    view,
    loading = false,
    live = {},
    timeoutsUsed = {},
    onopen,
    only = null,
    focus = null,
    onclearfocus,
    flip = false,
  }: {
    view: LiveView;
    /** The results still load: tiles of the same size as the real ones, without content. */
    loading?: boolean;
    live?: Record<number, LiveGame>;
    /** Per game index: [home, away] took their timeout (scoreboard games only). */
    timeoutsUsed?: Record<number, [boolean, boolean]>;
    onopen: (g: GameView) => void;
    /** Games of the players picked (clicked) in the Spelers card: only these are shown. Overrides the filter. */
    only?: Set<number> | null;
    /** Games of the player under the pointer in the Spelers card: the others are dimmed. */
    focus?: Set<number> | null;
    onclearfocus?: () => void;
    /** The own club plays away: see MatchTile. */
    flip?: boolean;
  } = $props();

  let filter = $state<'all' | 'todo'>('all');
  const shown = $derived(
    only ? view.games.filter((g) => only.has(g.index)) : filter === 'todo' ? view.games.filter((g) => g.state !== 'final') : view.games,
  );
  function pick(f: 'all' | 'todo') {
    filter = f;
    onclearfocus?.();
  }
  function layout(o: 'grid' | 'list') {
    settings.order = o;
    saveSettings();
  }
  const tableNo = (g: GameView) => (g.table === null ? null : (view.tables[g.table]?.no ?? null));
</script>

<section class="card" aria-label="Wedstrijdvolgorde">
  <div class="card-h">
    <h2>Wedstrijdvolgorde</h2>
    <span class="spacer"></span>
    <div class="filters" role="group" aria-label="Filter wedstrijden">
      <button class="seg-btn" type="button" aria-pressed={!only && filter === 'all'} onclick={() => pick('all')}>Alle {view.games.length}</button>
      <button class="seg-btn" type="button" aria-pressed={!only && filter === 'todo'} onclick={() => pick('todo')}>Te spelen</button>
    </div>
    <div class="filters view-toggle" role="group" aria-label="Weergave">
      <button class="seg-btn icon" type="button" aria-pressed={settings.order === 'grid'} aria-label="Twee kolommen" title="Twee kolommen" onclick={() => layout('grid')}><Icon name="columns" size={18} /></button>
      <button class="seg-btn icon" type="button" aria-pressed={settings.order === 'list'} aria-label="Eén kolom" title="Eén kolom" onclick={() => layout('list')}><Icon name="rows" size={18} /></button>
    </div>
  </div>
  <div class="order" class:cols-2={settings.order === 'grid'}>
    {#if loading}
      <p class="sr-only" role="status">Uitslagen laden…</p>
      {#each view.games as g (g.index)}
        <div class="mt skel" aria-hidden="true">
          <div class="mt-head"><span class="sk head"></span></div>
          <div class="mt-rows"><span class="sk row"></span><span class="sk row"></span></div>
        </div>
      {/each}
    {:else}
    {#each shown as g (g.index)}
      <MatchTile dim={!!focus && !focus.has(g.index)} game={g} boxes={view.sys.boxes} tableNo={tableNo(g)} live={live[g.index] ?? null} used={timeoutsUsed[g.index] ?? null} {onopen} {flip} />
    {:else}
      <p class="muted">Alle wedstrijden zijn gespeeld.</p>
    {/each}
    {/if}
  </div>
</section>
