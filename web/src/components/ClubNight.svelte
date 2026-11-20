<script lang="ts">
  import Loading from './Loading.svelte';
  import NightCardView from './NightCardView.svelte';
  import PlayerPopover from './match/PlayerPopover.svelte';
  import { warmPlayers } from '../lib/players.svelte';
  import Icon from './Icon.svelte';
  import ViewPicker from './ViewPicker.svelte';
  import { ws, loadDay } from '../lib/workspace.svelte';
  import { nightCard, matchesOn, matchDays } from '../lib/night';
  import { navigate } from '../lib/router.svelte';
  import { urls } from '../lib/nav';
  import { longDate, shortDate } from '../lib/format';
  import type { MatchRow, Team } from '../lib/types';

  let {
    day,
    rows,
    loaded,
    currentId = null,
    filters = true,
    wide = false,
    teams = [],
  }: {
    day: string;
    rows: MatchRow[];
    loaded: boolean;
    currentId?: string | null;
    filters?: boolean;
    /** The overview of the day: the cards also show the games in progress. */
    wide?: boolean;
    /** The teams of the club: the overview gets the switch to their divisions. */
    teams?: Team[];
  } = $props();

  type Filter = 'all' | 'live' | 'final';
  let filter = $state<Filter>('all');

  const days = $derived(matchDays(rows));
  // A draft is only a local copy of VTTL data: the card then shows the VTTL row itself.
  const stored = (id: string) => {
    const e = ws.entries[id];
    return e && !e.draft ? e.view : undefined;
  };
  const cards = $derived(matchesOn(rows, day).map((r) => nightCard(r, stored(r.matchId), wide ? ws.entries[r.matchId]?.view : undefined)));

  /** The player widget: one for all cards. */
  let info: ReturnType<typeof PlayerPopover> | undefined = $state();
  /** The players of the line-ups shown, as text: the server gets them ready for the player widget. */
  const playerIds = $derived(
    cards
      .flatMap((c) => (c.lineup ? [...c.lineup.home, ...c.lineup.away] : []))
      .map((r) => r.person.id ?? 0)
      .filter((n) => n > 0)
      .join(','),
  );
  $effect(() => {
    if (playerIds) warmPlayers(playerIds.split(',').map(Number));
  });
  const shown = $derived(
    cards.filter((c) => (filter === 'all' ? true : c.state === filter)),
  );
  const week = $derived(matchesOn(rows, day).find((r) => r.week)?.week ?? '');
  const prev = $derived(days.filter((d) => d < day).at(-1));
  const next = $derived(days.find((d) => d > day));

  $effect(() => {
    loadDay(day).catch(() => {});
  });

  const go = (d: string | undefined) => d && navigate(urls.day(d));
</script>

<div class="night-head">
  {#if wide && teams.length}<ViewPicker title="Clubavond" {teams} week={week || null} clubHref={urls.day(day)} />{/if}
  <div class="daynav">
    <button class="btn ghost" type="button" onclick={() => go(prev)} disabled={!prev} aria-label="Vorige speeldag"><Icon name="left" /></button>
    <label class="day">
      {shortDate(day)}{week ? ` · week ${week}` : ''}
      <input type="date" value={day} onchange={(e) => go(e.currentTarget.value)} aria-label="Kies een datum" />
    </label>
    <button class="btn ghost" type="button" onclick={() => go(next)} disabled={!next} aria-label="Volgende speeldag"><Icon name="right" /></button>
  </div>

  {#if filters && cards.length > 1}
    <div class="filters" role="group" aria-label="Filter ploegen">
      {#each [['all', 'Alle'], ['live', 'Bezig'], ['final', 'Klaar']] as [k, l] (k)}
        <button class="seg-btn" type="button" aria-pressed={filter === k} onclick={() => (filter = k as Filter)}>{l}</button>
      {/each}
    </div>
  {/if}
</div>

<PlayerPopover bind:this={info} />

<div class="night-list" style="--n: {Math.max(shown.length, 1)}">
  {#each shown as c (c.row.matchId)}
    <NightCardView card={c} current={c.row.matchId === currentId} {wide} oninfo={(at, who) => info?.show(at, who)} />
  {:else}
    <div class="card empty">
      {#if !loaded}
        <Loading label="Wedstrijden laden" />
      {:else if cards.length}
        <p>Geen wedstrijden voor deze filter.</p>
      {:else}
        <p>Geen wedstrijden op deze dag.</p>
        {#if next}<button class="btn ghost" type="button" onclick={() => go(next)}>Naar {longDate(next)}</button>{/if}
      {/if}
    </div>
  {/each}
</div>
