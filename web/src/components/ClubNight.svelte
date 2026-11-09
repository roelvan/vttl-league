<script lang="ts">
  import NightCardView from './NightCardView.svelte';
  import Icon from './Icon.svelte';
  import { data } from '../lib/data.svelte';
  import { ws, loadDay } from '../lib/workspace.svelte';
  import { nightCard, matchesOn, matchDays } from '../lib/night';
  import { navigate } from '../lib/router.svelte';
  import { urls } from '../lib/nav';
  import { longDate, todayIso } from '../lib/format';

  let { day, currentId = null, filters = true }: { day: string; currentId?: string | null; filters?: boolean } = $props();

  type Filter = 'all' | 'live' | 'home' | 'away';
  let filter = $state<Filter>('all');

  const rows = $derived(data.matches?.matches ?? []);
  const days = $derived(matchDays(rows));
  const cards = $derived(matchesOn(rows, day).map((r) => nightCard(r, ws.entries[r.matchId]?.view)));
  const liveCount = $derived(cards.filter((c) => c.state === 'live').length);
  const shown = $derived(
    cards.filter((c) => (filter === 'live' ? c.state === 'live' : filter === 'home' ? c.isHome : filter === 'away' ? !c.isHome : true)),
  );
  const prev = $derived(days.filter((d) => d < day).at(-1));
  const next = $derived(days.find((d) => d > day));

  $effect(() => {
    loadDay(day).catch(() => {});
  });

  const go = (d: string | undefined) => d && navigate(urls.day(d));
</script>

<div class="night-head">
  <h2>Clubavond</h2>
  <span class="spacer"></span>
  {#if liveCount}<span class="chip live">{liveCount} live</span>{/if}
</div>

<div class="daynav">
  <button class="btn ghost" type="button" onclick={() => go(prev)} disabled={!prev} aria-label="Vorige speeldag"><Icon name="left" /></button>
  <label class="day">
    {day === todayIso() ? 'Vandaag' : longDate(day)}
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
      {#if !data.matches}
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
