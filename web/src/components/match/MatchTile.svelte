<script lang="ts">
  import type { GameView, GameSide, Person, Side } from '../../lib/live';
  import type { LiveGame } from '../../lib/boards.svelte';
  import Icon from '../Icon.svelte';
  import TimeoutBadge from './TimeoutBadge.svelte';
  import TimeoutUsed from './TimeoutUsed.svelte';
  import { tone } from '../../lib/viewer.svelte';
  import { MediaQuery } from 'svelte/reactivity';

  let {
    game,
    boxes,
    tableNo = null,
    live = null,
    used = null,
    onopen,
    oninfo,
    dim = false,
    flip = false,
  }: {
    /** Not a game of the players picked in the Spelers card. */
    dim?: boolean;
    /** The own club plays away: the away row comes first, in the home colour. */
    flip?: boolean;
    /** [home, away] took their timeout; null = no scoreboard data. */
    used?: [boolean, boolean] | null;
    game: GameView;
    boxes: number;
    tableNo?: string | null;
    /** The game in progress on a scoreboard. */
    live?: LiveGame | null;
    /** Without it, the tile is a plain block (e.g. at the top of the stats dialog). */
    onopen?: (g: GameView) => void;
    /** Opens the player widget next to `at`. Without it, the names have no info button. */
    oninfo?: (at: HTMLElement, p: Person, side: Side) => void;
  } = $props();

  /** A phone: the tile has no room for an info button, so the rank tag opens the player widget. */
  const narrow = new MediaQuery('(max-width: 599px)');

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
        ? `gespeeld, ${flip ? `${game.away.sets}–${game.home.sets}` : `${game.home.sets}–${game.away.sets}`} in sets`
        : game.state === 'live'
          ? `bezig${tableNo ? ` op tafel ${tableNo}` : ''}, ${flip ? `${won[1]}–${won[0]}` : `${won[0]}–${won[1]}`} in sets`
          : status.toLowerCase(),
  );
  const label = $derived(
    `Wedstrijd ${game.pos}: ${flip ? `${who(game.away)} tegen ${who(game.home)}` : `${who(game.home)} tegen ${who(game.away)}`}, ${stateText}`,
  );
</script>

{#snippet name(people: Person[])}
  {#if !people.length || !people.some((p) => p.full)}
    <span class="muted">Nog niet opgesteld</span>
  {:else if people.length === 1}
    {people[0].full}
  {:else}
    {people.map((p) => p.last).join(' / ')}
  {/if}
{/snippet}

{#snippet row(side: GameSide, cls: 'h' | 'a', lead: boolean, idx: 0 | 1)}
  {@const one = oninfo && side.people.length === 1 && side.people[0].id ? side.people[0] : null}
  {@const info = (e: MouseEvent & { currentTarget: HTMLElement }) => {
    // Not a part of the tile button: the click stops here.
    e.stopPropagation();
    if (one) oninfo?.(e.currentTarget, one, cls === 'h' ? 'home' : 'away');
  }}
  <div class="mt-row" class:lose={game.winner && game.winner !== cls} class:bare={!started}>
    {#if one && narrow.current}
      <button class="rank-tag {tone(cls, flip)}" type="button" aria-label="Spelerinfo {one.full}, {one.ranking || 'geen klassement'}" aria-haspopup="dialog" onclick={info}>
        <span>{one.ranking || '–'}</span>
      </button>
    {:else}
      <span class="rank-tag {tone(cls, flip)}" class:pair={side.people.length > 1}>
        {#each side.people.length ? side.people : [null] as p, i (i)}<span>{p?.ranking || '–'}</span>{/each}
      </span>
    {/if}
    <span class="mt-name">
      <span class="n" title={side.people.map((p) => p.full).join(' / ')}>{@render name(side.people)}</span>
      {#if one && !narrow.current}
        <button class="who-info sm" type="button" aria-label="Spelerinfo {one.full}" aria-haspopup="dialog" onclick={info}>
          <Icon name="info" size={16} />
        </button>
      {/if}
      {#if game.winner === cls}<span class="trophy"><Icon name="trophy" size={16} /></span>{/if}
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

<!-- The tile opens the game. It is a block with a button over it, not a button: the info
     buttons of the players are inside, and a button cannot hold a button. A click anywhere
     on the tile comes here; the keyboard uses the button. -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="mt is-{game.state === 'final' ? 'final' : game.state}"
  class:static={!onopen}
  class:dim
  role={onopen ? undefined : 'group'}
  style="--boxes: {boxes}"
  aria-label={onopen ? undefined : label}
  onclick={onopen ? () => onopen(game) : undefined}
>
  {#if onopen}<button class="mt-open" type="button" aria-label={label}></button>{/if}
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
    {#if flip}
      {@render row(game.away, 'a', awayLeads, 1)}
      {@render row(game.home, 'h', homeLeads, 0)}
    {:else}
      {@render row(game.home, 'h', homeLeads, 0)}
      {@render row(game.away, 'a', awayLeads, 1)}
    {/if}
  </div>
</div>
