<script lang="ts">
  import type { GameView, GameSide, Person } from '../../lib/live';
  import type { LiveGame } from '../../lib/boards.svelte';
  import TimeoutBadge from './TimeoutBadge.svelte';
  import TimeoutUsed from './TimeoutUsed.svelte';

  let {
    game,
    boxes,
    tableNo = null,
    live = null,
    used = null,
    onopen,
    dim = false,
  }: {
    /** Not a game of the players picked in the Spelers card. */
    dim?: boolean;
    /** [home, away] took their timeout; null = no scoreboard data. */
    used?: [boolean, boolean] | null;
    game: GameView;
    boxes: number;
    tableNo?: string | null;
    /** The game in progress on a scoreboard. */
    live?: LiveGame | null;
    /** Without it, the tile is a plain block (e.g. at the top of the stats dialog). */
    onopen?: (g: GameView) => void;
  } = $props();

  const started = $derived(game.state === 'final' || game.state === 'live' || game.state === 'error' || game.home.cells.some(Boolean));
  // While the board counts, its finished sets are not in the match record yet.
  const bsets = $derived(live?.sets ?? null);
  const won = $derived(
    bsets ? [bsets.filter(([a, b]) => a > b).length, bsets.filter(([a, b]) => b > a).length] : [game.home.sets, game.away.sets],
  );
  const curBox = $derived(bsets ? bsets.length : game.currentBox);
  const homeLeads = $derived(won[0] > won[1]);
  const awayLeads = $derived(won[1] > won[0]);
  const tmo = $derived(live?.timeout?.active ? live.timeout : null);
  const status = $derived(game.state === 'next' ? (tableNo ? `Volgende op tafel ${tableNo}` : 'Volgende') : 'Later');
  const who = (s: GameSide) => s.people.map((p) => p.full).filter(Boolean).join(' en ') || 'nog niet opgesteld';
  const stateText = $derived(
    game.state === 'error'
      ? 'score nakijken'
      : game.state === 'final'
        ? `gespeeld, ${game.home.sets}–${game.away.sets} in sets`
        : game.state === 'live'
          ? `bezig${tableNo ? ` op tafel ${tableNo}` : ''}, ${won[0]}–${won[1]} in sets`
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
  <div class="mt-row" class:lose={game.winner && game.winner !== cls} class:bare={!started}>
    <span class="bar {cls}"></span>
    <span class="mt-name">
      <span class="n" title={side.people.map((p) => p.full).join(' / ')}>{@render name(side.people)}</span>
      {#if used?.[idx] && !(tmo && tmo.owner === idx + 1)}
        <TimeoutUsed />
      {/if}
    </span>
    {#if started}
      {#each Array.from({ length: boxes }, (_, k) => k) as k (k)}
        {#if bsets?.[k]}
          {@const own = bsets[k][idx]}
          {@const other = bsets[k][1 - idx]}
          <span class="c {own > other ? 'w' : 'l'}">{own}</span>
        {:else if !bsets && side.cells[k]}
          <span class="c {side.won[k] === true ? 'w' : side.won[k] === false ? 'l' : ''}">{side.cells[k]}</span>
        {:else if curBox === k}
          <span class="c cur">{live ? live.points[idx] : ''}</span>
        {:else}
          <span class="c">·</span>
        {/if}
      {/each}
      <span class="tot" class:lead>{won[idx]}</span>
    {/if}
  </div>
{/snippet}

<svelte:element
  this={onopen ? 'button' : 'div'}
  class="mt is-{game.state === 'final' ? 'final' : game.state}"
  class:static={!onopen}
  class:dim
  type={onopen ? 'button' : undefined}
  role={onopen ? undefined : 'group'}
  style="--boxes: {boxes}"
  aria-label={label}
  onclick={onopen ? () => onopen(game) : undefined}
>
  <div class="mt-head">
    <span>Wedstrijd {game.pos}</span>
    {#if game.state === 'live' && tableNo}<span>· Tafel {tableNo}</span>{/if}
    {#if game.state === 'error'}<span>· Score nakijken</span>{/if}
    {#if game.result === 'homeWO' || game.result === 'awayWO'}<span>· WO</span>{/if}
    <span class="spacer"></span>
    {#if tmo}<TimeoutBadge timeout={tmo} compact who={tmo.owner === 1 ? 'thuis' : tmo.owner === 2 ? 'bezoekers' : 'scheidsrechter'} />{/if}
    {#if game.state === 'live'}<span class="chip live">Live</span>{:else if !started}<span class="status">{status}</span>{/if}
  </div>
  <div class="mt-rows">
    {@render row(game.home, 'h', homeLeads, 0)}
    {@render row(game.away, 'a', awayLeads, 1)}
  </div>
</svelte:element>
