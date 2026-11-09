<script lang="ts">
  import NightCardView from './NightCardView.svelte';
  import Icon from './Icon.svelte';
  import { ws, loadDay } from '../lib/workspace.svelte';
  import { nightCard, matchesOn, matchDays } from '../lib/night';
  import { navigate } from '../lib/router.svelte';
  import { urls } from '../lib/nav';
  import { longDate, shortDate } from '../lib/format';
  import type { MatchRow } from '../lib/types';

  let {
    day,
    rows,
    loaded,
    currentId = null,
    filters = true,
  }: { day: string; rows: MatchRow[]; loaded: boolean; currentId?: string | null; filters?: boolean } = $props();

  type Filter = 'all' | 'live' | 'home' | 'away';
  let filter = $state<Filter>('all');

  const days = $derived(matchDays(rows));
  const cards = $derived(matchesOn(rows, day).map((r) => nightCard(r, ws.entries[r.matchId]?.view)));
  const shown = $derived(
    cards.filter((c) => (filter === 'live' ? c.state === 'live' : filter === 'home' ? c.isHome : filter === 'away' ? !c.isHome : true)),
  );
  const week = $derived(matchesOn(rows, day).find((r) => r.week)?.week ?? '');
  const prev = $derived(days.filter((d) => d < day).at(-1));
  const next = $derived(days.find((d) => d > day));

  $effect(() => {
    loadDay(day).catch(() => {});
  });

  const go = (d: string | undefined) => d && navigate(urls.day(d));
</script>

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
    {#each [['all', 'Alle'], ['live', 'Live'], ['home', 'Thuis'], ['away', 'Uit']] as [k, l] (k)}
      <button class="seg-btn" type="button" aria-pressed={filter === k} onclick={() => (filter = k as Filter)}>{l}</button>
    {/each}
  </div>
{/if}

<div class="night-list">
  {#each shown as c (c.row.matchId)}
    <NightCardView card={c} current={c.row.matchId === currentId} />
  {:else}
    <div class="card empty">
      {#if !loaded}
        <p>Wedstrijden laden…</p>
      {:else if cards.length}
        <p>Geen wedstrijden voor deze filter.</p>
      {:else}
        <p>Geen wedstrijden op deze dag.</p>
        {#if next}<button class="btn ghost" type="button" onclick={() => go(next)}>Naar {longDate(next)}</button>{/if}
      {/if}
    </div>
  {/each}
</div>
