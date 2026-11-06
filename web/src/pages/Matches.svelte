<script lang="ts">
  import { api, ApiError, OfflineError } from '../lib/api';
  import { route, navigate, link } from '../lib/router.svelte';
  import { longDate, shortDate, todayIso } from '../lib/format';
  import type { MatchList, MatchRow, Season } from '../lib/types';

  const CACHE = 'vl.matches.';

  let seasons = $state<Season[]>([]);
  let season = $state<number | undefined>(route.query.get('seizoen') ? Number(route.query.get('seizoen')) : undefined);
  let list = $state<MatchList | null>(null);
  let error = $state('');
  let loading = $state(true);
  let showAway = $state(readFlag('vl.showAway', false));
  let showByes = $state(readFlag('vl.showByes', false));

  function readFlag(k: string, d: boolean) {
    try {
      const v = localStorage.getItem(k);
      return v === null ? d : v === '1';
    } catch {
      return d;
    }
  }
  function writeFlag(k: string, v: boolean) {
    try {
      localStorage.setItem(k, v ? '1' : '0');
    } catch {}
  }

  async function load(s: number | undefined) {
    error = '';
    loading = true;
    const key = CACHE + (s ?? 'current');
    try {
      const cached = localStorage.getItem(key);
      if (cached) list = JSON.parse(cached);
    } catch {}
    try {
      const fresh = await api.clubMatches(s);
      list = fresh;
      try {
        localStorage.setItem(key, JSON.stringify(fresh));
      } catch {}
    } catch (e) {
      if (!list) error = e instanceof ApiError || e instanceof OfflineError ? e.message : 'Laden mislukt.';
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    load(season);
  });
  api.seasons().then((s) => (seasons = s.slice().reverse()), () => {});

  const visible = $derived(
    (list?.matches ?? []).filter((m) => (showAway || m.isHome) && (showByes || !m.isBye)),
  );

  /** Matches grouped per date, in date order; matches without a date at the end. */
  const days = $derived.by(() => {
    const map = new Map<string, MatchRow[]>();
    for (const m of visible) {
      const k = m.date || '';
      if (!map.has(k)) map.set(k, []);
      map.get(k)!.push(m);
    }
    for (const v of map.values()) v.sort((a, b) => (a.time + a.teamLetter).localeCompare(b.time + b.teamLetter));
    return [...map.entries()].sort(([a], [b]) => (a === '' ? 1 : b === '' ? -1 : a.localeCompare(b)));
  });

  let picked = $state<string | null>(route.query.get('dag'));
  /** The selected day: the chosen one, else today or the next match day, else the last one. */
  const day = $derived.by(() => {
    if (picked !== null && days.some(([d]) => d === picked)) return picked;
    const today = todayIso();
    return days.find(([d]) => d && d >= today)?.[0] ?? days.at(-1)?.[0] ?? null;
  });
  const dayMatches = $derived(days.find(([d]) => d === day)?.[1] ?? []);

  function pick(d: string) {
    picked = d;
    const p = new URLSearchParams(route.query);
    p.set('dag', d);
    navigate(`/?${p}`, true);
  }

  let chips: HTMLElement | undefined = $state();
  $effect(() => {
    // Keep the selected day in view in the horizontal list.
    day;
    chips?.querySelector('[aria-current="true"]')?.scrollIntoView({ block: 'nearest', inline: 'center' });
  });

  function status(m: MatchRow) {
    if (m.isValidated) return { t: 'Gevalideerd', c: 'ok' };
    if (m.isLocked) return { t: 'Vergrendeld', c: 'ok' };
    if (m.score) return { t: 'Uitslag ingegeven', c: 'info' };
    return null;
  }
</script>

<main>
  <div class="toolbar">
    <h1>Speeldagen</h1>
    <span class="spacer"></span>
    <label class="check"><input type="checkbox" bind:checked={showAway} onchange={() => writeFlag('vl.showAway', showAway)} /> Uitwedstrijden</label>
    <label class="check"><input type="checkbox" bind:checked={showByes} onchange={() => writeFlag('vl.showByes', showByes)} /> Vrij</label>
    {#if seasons.length}
      <select bind:value={season} aria-label="Seizoen">
        <option value={undefined}>Huidig seizoen</option>
        {#each seasons as s (s.id)}<option value={s.id}>{s.name}</option>{/each}
      </select>
    {/if}
  </div>

  {#if error}<p class="error">{error}</p>{/if}
  {#if loading && !list}<p class="muted">Wedstrijden laden…</p>{/if}

  {#if days.length}
    <nav class="chips" bind:this={chips} aria-label="Speeldag">
      {#each days as [d, ms] (d)}
        <button class="chip" aria-current={d === day} onclick={() => pick(d)}>
          <span>{shortDate(d)}</span><small>{ms.length}</small>
        </button>
      {/each}
    </nav>

    <h2 class="day">{longDate(day ?? '')}</h2>
    <ul class="matches">
      {#each dayMatches as m (m.matchId)}
        {@const st = status(m)}
        <li>
          <a class="match" href={`/wedstrijd/${encodeURIComponent(m.matchId)}${season ? `?seizoen=${season}` : ''}`} onclick={link}>
            <div class="m-top">
              <span class="mono muted">{m.matchId}</span>
              {#if m.time}<span class="muted">{m.time}</span>{/if}
              <span class="tag" class:home={m.isHome}>{m.isHome ? 'Thuis' : 'Uit'}</span>
              {#if st}<span class="tag {st.c}">{st.t}</span>{/if}
            </div>
            <div class="m-teams">
              <span class:mine={m.isHome}>{m.homeTeam}</span>
              <span class="score">{m.score || '–'}</span>
              <span class:mine={!m.isHome}>{m.awayTeam}</span>
            </div>
            <div class="muted small">
              {[m.divisionName, m.venue && `${m.venue.name}, ${m.venue.town}`].filter(Boolean).join(' · ')}
            </div>
          </a>
        </li>
      {/each}
    </ul>
  {:else if list && !loading}
    <p class="muted">Geen wedstrijden gevonden.</p>
  {/if}
</main>
