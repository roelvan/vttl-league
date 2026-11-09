<script lang="ts">
  import type { GameView, GameSide, Person } from '../../lib/live';

  let {
    game,
    boxes,
    tableNo = null,
    livePoints = null,
    onopen,
  }: {
    game: GameView;
    boxes: number;
    tableNo?: string | null;
    /** Points of the game in progress from a scoreboard: [home, away]. */
    livePoints?: [number, number] | null;
    onopen: (g: GameView) => void;
  } = $props();

  const started = $derived(game.state === 'final' || game.state === 'live' || game.state === 'error' || game.home.cells.some(Boolean));
  const homeLeads = $derived(game.home.sets > game.away.sets);
  const awayLeads = $derived(game.away.sets > game.home.sets);
  const status = $derived(game.state === 'next' ? (tableNo ? `Volgende op tafel ${tableNo}` : 'Volgende') : 'Later');
  const who = (s: GameSide) => s.people.map((p) => p.full).filter(Boolean).join(' en ') || 'nog niet opgesteld';
  const stateText = $derived(
    game.state === 'error'
      ? 'score nakijken'
      : game.state === 'final'
        ? `gespeeld, ${game.home.sets}–${game.away.sets} in sets`
        : game.state === 'live'
          ? `bezig${tableNo ? ` op tafel ${tableNo}` : ''}, ${game.home.sets}–${game.away.sets} in sets`
          : status.toLowerCase(),
  );
  const label = $derived(`Wedstrijd ${game.pos}: ${who(game.home)} tegen ${who(game.away)}, ${stateText}`);
</script>

{#snippet name(people: Person[])}
  {#if !people.length || !people.some((p) => p.full)}
    <span class="muted">Nog niet opgesteld</span>
  {:else if people.length === 1}
    {#if people[0].initial}<span class="fi">{`${people[0].initial}\u00a0`}</span>{/if}{people[0].last}
  {:else}
    {people.map((p) => p.last).join(' / ')}
  {/if}
{/snippet}

{#snippet row(side: GameSide, cls: 'h' | 'a', lead: boolean, idx: 0 | 1)}
  <div class="mt-row" class:lose={game.winner && game.winner !== cls}>
    <span class="bar {cls}"></span>
    <span class="mt-name">
      <span class="n" title={side.people.map((p) => p.full).join(' / ')}>{@render name(side.people)}</span>
      <span class="rk">{side.label}{side.people.length === 1 && side.people[0].ranking ? ` · ${side.people[0].ranking}` : ''}</span>
    </span>
    {#if started}
      {#each Array.from({ length: boxes }, (_, k) => k) as k (k)}
        {#if side.cells[k]}
          <span class="c {side.won[k] === true ? 'w' : side.won[k] === false ? 'l' : ''}">{side.cells[k]}</span>
        {:else if game.currentBox === k}
          <span class="c cur">{livePoints ? livePoints[idx] : ''}</span>
        {:else}
          <span class="c">·</span>
        {/if}
      {/each}
      <span class="tot" class:lead>{side.sets}</span>
    {:else if idx === 0}
      <span class="status">{status}</span>
    {:else}
      <span class="status"></span>
    {/if}
  </div>
{/snippet}

<button
  class="mt is-{game.state === 'final' ? 'final' : game.state}"
  type="button"
  style="--boxes: {boxes}"
  aria-label={label}
  onclick={() => onopen(game)}
>
  <div class="mt-head">
    <span>Wedstrijd {game.pos}</span>
    {#if game.state === 'live'}<span>· Bezig{tableNo ? ` op tafel ${tableNo}` : ''}</span>{/if}
    {#if game.state === 'error'}<span>· Score nakijken</span>{/if}
    {#if game.result === 'homeWO' || game.result === 'awayWO'}<span>· WO</span>{/if}
  </div>
  <div class="mt-rows">
    {@render row(game.home, 'h', homeLeads, 0)}
    {@render row(game.away, 'a', awayLeads, 1)}
  </div>
</button>
