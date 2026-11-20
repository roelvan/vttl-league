<script lang="ts">
  import type { Sheet } from '../../lib/sheet/types';
  import type { LiveView, Side } from '../../lib/live';
  import { tone } from '../../lib/viewer.svelte';
  import { link } from '../../lib/router.svelte';

  let {
    sheet,
    view,
    week = '',
    division = '',
    divisionHref = '',
    score = null,
    flip = false,
  }: {
    sheet: Sheet;
    view: LiveView;
    week?: string;
    division?: string;
    /** The page with all matches of the division. */
    divisionHref?: string;
    /** The team score of VTTL, shown while the results of the match still load. */
    score?: [number, number] | null;
    /** The own club plays away: the away team comes first, in the home colour. */
    flip?: boolean;
  } = $props();
  const official = $derived(score ?? [view.homeWins, view.awayWins]);
  const wins = $derived(flip ? [official[1], official[0]] : official);
  const sides = $derived<[Side, Side]>(flip ? ['away', 'home'] : ['home', 'away']);

  const n = $derived(view.games.length);
  const segs = $derived(view.games.map((g) => g.winner ?? (g.state === 'live' ? 'live' : '')));
  const status = $derived(
    sheet.vttlLocked ? 'Gevalideerd' : score ? 'Laden…' : view.complete ? 'Einde' : view.started ? 'live' : sheet.startHour ? `Om ${sheet.startHour}` : 'Nog niet begonnen',
  );
</script>

<!-- "place" is the side on screen (left = home colour); the tag keeps the real home or away. -->
{#snippet team(side: Side, place: Side)}
  <div class="team {place}">
    <span class="team-tag">{side === 'home' ? 'Thuis' : 'Uit'}</span>
    <div class="team-name">{sheet[side].name}</div>
    {#if sheet[side].forfeit}<span class="team-sub">Forfait</span>{/if}
  </div>
{/snippet}

<section class="card board" aria-label="Stand van de ploegenwedstrijd">
  <div class="board-meta">
    <!-- shortDivision joins its parts with " · ": show them with the same dots as the rest. -->
    {#snippet parts()}{#each division.split(' · ').filter(Boolean) as part, i (i)}{#if i}<span class="dot"></span>{/if}<span>{part}</span>{/each}{/snippet}
    {#if division && divisionHref}
      <a class="board-div" href={divisionHref} onclick={link} title="Alle wedstrijden van deze afdeling">{@render parts()}</a><span class="dot"></span>
    {:else if division}
      {@render parts()}<span class="dot"></span>
    {/if}
    {#if week}<span class="hide-sm">Week {week}</span><span class="dot hide-sm"></span>{/if}
    <span class="num">{sheet.matchId}</span>
    <span class="spacer"></span>
    {#if status === 'live'}<span class="chip live">Live</span>{:else}<span class="chip" class:win={status === 'Einde'}>{status}</span>{/if}
  </div>
  <div class="board-main">
    {@render team(sides[0], 'home')}
    <div class="big-score" aria-label="{wins[0]} tegen {wins[1]}">
      <span class="h">{wins[0]}</span><span class="sep">-</span><span class="a">{wins[1]}</span>
    </div>
    {@render team(sides[1], 'away')}
  </div>
  <div class="race">
    <div class="race-bar" style="grid-template-columns: repeat({n}, 1fr)">
      {#each segs as s, i (i)}<span class="seg {tone(s, flip)}" title="Wedstrijd {i + 1}"></span>{/each}
    </div>
  </div>
</section>
