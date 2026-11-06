<script lang="ts">
  import Dialog from './Dialog.svelte';
  import { api, ApiError, OfflineError, type StoredSheet } from '../lib/api';
  import { data, loadCategories, loadMatches } from '../lib/data.svelte';
  import { settings, saveSettings } from '../lib/settings.svelte';
  import { openSheets } from '../lib/workspace.svelte';
  import { fromVttl } from '../lib/sheet/create';
  import { longDate, shortDate, todayIso, byTeam } from '../lib/format';
  import type { MatchRow } from '../lib/types';

  let { onclose }: { onclose: () => void } = $props();

  let error = $state('');
  let busy = $state(false);
  let picked = $state<string | null>(null);

  loadMatches().catch((e) => {
    if (!data.matches) error = e instanceof ApiError || e instanceof OfflineError ? e.message : 'Laden mislukt.';
  });
  loadCategories().catch(() => {});

  const all = $derived(data.matches?.matches ?? []);
  const dated = $derived(all.filter((m) => m.date && (settings.openAway || m.isHome) && !m.isBye));
  const days = $derived.by(() => {
    const map = new Map<string, MatchRow[]>();
    for (const m of dated) {
      if (!map.has(m.date)) map.set(m.date, []);
      map.get(m.date)!.push(m);
    }
    for (const v of map.values()) v.sort(byTeam);
    return [...map.entries()].sort(([a], [b]) => a.localeCompare(b));
  });
  const day = $derived.by(() => {
    if (picked && days.some(([d]) => d === picked)) return picked;
    const today = todayIso();
    return days.find(([d]) => d >= today)?.[0] ?? days.at(-1)?.[0] ?? null;
  });
  /** The matches that open: those of the day, plus the byes of the same week if asked. */
  const chosen = $derived.by(() => {
    const ms = days.find(([d]) => d === day)?.[1] ?? [];
    if (!settings.openByes || !ms.length) return ms;
    const weeks = new Set(ms.map((m) => m.week));
    return [...ms, ...all.filter((m) => m.isBye && weeks.has(m.week))];
  });

  let list: HTMLElement | undefined = $state();
  $effect(() => {
    day;
    list?.querySelector('[aria-current="true"]')?.scrollIntoView({ block: 'center' });
  });

  async function ok() {
    if (!day || !data.matches) return;
    busy = true;
    error = '';
    try {
      if (!data.categories.length) await loadCategories();
      let stored: StoredSheet[] = [];
      try {
        stored = await api.sheetsOn(day);
      } catch (e) {
        if (!(e instanceof OfflineError)) throw e; // offline: make them here, they sync later
      }
      const season = data.matches.season;
      openSheets(
        chosen.map((m) => {
          const s = stored.find((x) => x.id === m.matchId);
          return s ? { stored: s } : { local: fromVttl(m, season, data.categories) };
        }),
      );
      onclose();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Openen mislukt.';
    } finally {
      busy = false;
    }
  }
</script>

<Dialog title="Nieuwe speeldag" {onclose} wide>
  <p class="muted">De lijst hieronder toont alle beschikbare speeldagen. Kies de speeldag waarvan je de wedstrijdbladen wil genereren en klik op OK.</p>
  <div class="md">
    <ul class="md-days" bind:this={list}>
      {#each days as [d, ms] (d)}
        <li>
          <button class="md-day" aria-current={d === day} onclick={() => (picked = d)} ondblclick={ok}>
            <span>{shortDate(d)}</span><small>{ms.length}</small>
          </button>
        </li>
      {/each}
    </ul>
    <div class="md-matches">
      {#if day}
        <h3 class="day">{longDate(day)}</h3>
        <ul>
          {#each chosen as m (m.matchId)}
            <li>
              <span class="mono muted">{m.matchId}</span>
              <span class="muted">{m.time}</span>
              <span class:mine={m.isHome}>{m.homeTeam}</span> – <span class:mine={!m.isHome}>{m.awayTeam}</span>
            </li>
          {/each}
        </ul>
      {:else if !error}
        <p class="muted">Laden…</p>
      {/if}
    </div>
  </div>
  <label class="check"><input type="checkbox" bind:checked={settings.openAway} onchange={saveSettings} /> Genereer ook wedstrijdbladen voor uitwedstrijden</label>
  <label class="check"><input type="checkbox" bind:checked={settings.openByes} onchange={saveSettings} /> Open bye-wedstrijden die deze week gespeeld worden</label>
  {#if error}<p class="error">{error}</p>{/if}
  {#snippet footer()}
    <button class="ghost" onclick={onclose}>Annuleren</button>
    <button onclick={ok} disabled={busy || !chosen.length}>{busy ? 'Bezig…' : `OK (${chosen.length})`}</button>
  {/snippet}
</Dialog>
