<script lang="ts">
  import { api, ApiError, OfflineError } from '../lib/api';
  import { route, link } from '../lib/router.svelte';
  import { longDate, playerName } from '../lib/format';
  import type { TeamMatch, Player } from '../lib/types';

  let { matchId }: { matchId: string } = $props();

  let m = $state<TeamMatch | null>(null);
  let error = $state('');

  $effect(() => {
    const id = matchId;
    const season = route.query.get('seizoen') ? Number(route.query.get('seizoen')) : undefined;
    const key = `vl.match.${id}`;
    try {
      const c = localStorage.getItem(key);
      if (c) m = JSON.parse(c);
    } catch {}
    api.match(id, season).then(
      (r) => {
        m = r;
        try {
          localStorage.setItem(key, JSON.stringify(r));
        } catch {}
      },
      (e) => {
        if (!m) error = e instanceof ApiError || e instanceof OfflineError ? e.message : 'Laden mislukt.';
      },
    );
  });

  const d = $derived(m?.details ?? null);
  const teams = $derived(
    d
      ? [
          { label: 'Thuis', ps: d.homePlayers, cap: d.homeCaptain },
          { label: 'Bezoekers', ps: d.awayPlayers, cap: d.awayCaptain },
        ]
      : [],
  );
  const byPos = (ps: Player[], pos: number) => ps.find((p) => p.position === pos);

  /** "1|-3,2|-4,3|-7" -> "-3 -4 -7" (TabT: + = home won the set, the number is the loser's points). */
  const sets = (s: string) =>
    s
      .split(',')
      .map((x) => x.split('|')[1])
      .filter(Boolean)
      .join('  ');

  function side(ps: Player[], idx: number[]) {
    return idx.map((i) => byPos(ps, i)).map((p) => (p ? playerName(p) : '?')).join(' / ');
  }
</script>

<main>
  <p><a href="/" onclick={link}>← Speeldagen</a></p>
  {#if error}<p class="error">{error}</p>{/if}
  {#if m}
    <h1 class="m-title">{m.homeTeam} <span class="score">{m.score || '–'}</span> {m.awayTeam}</h1>
    <p class="muted">
      <span class="mono">{m.matchId}</span>{' · '}{longDate(m.date)}{#if m.time}{' · '}{m.time}{/if}
      {#if m.venue}<br />{m.venue.name}, {m.venue.street}, {m.venue.town}{/if}
    </p>

    {#if d && (d.homePlayers.length || d.awayPlayers.length)}
      <div class="two">
        {#each teams as { label, ps, cap } (label)}
          <section class="card">
            <h3>{label}</h3>
            <ol class="players">
              {#each ps as p (p.position)}
                <li>
                  <span class="grow">{playerName(p)}{#if p.uniqueIndex === cap} <span class="tag">K</span>{/if}</span>
                  <span class="tag">{p.ranking}</span>
                  <span class="muted">{p.victoryCount} gew.</span>
                </li>
              {/each}
            </ol>
          </section>
        {/each}
      </div>
    {/if}

    {#if d?.results.length}
      <table class="results">
        <thead><tr><th>#</th><th>Thuis</th><th>Bezoekers</th><th>Sets</th><th>Uitslag</th></tr></thead>
        <tbody>
          {#each d.results as r (r.position)}
            <tr>
              <td>{r.position}</td>
              <td>{side(d.homePlayers, r.homeIndexes)}</td>
              <td>{side(d.awayPlayers, r.awayIndexes)}</td>
              <td class="mono">{sets(r.scores)}</td>
              <td class="mono">{r.isHomeForfeited ? 'WO' : r.isAwayForfeited ? 'WO' : `${r.homeSets}-${r.awaySets}`}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <p class="muted">Nog geen opstelling of uitslag bij VTTL. Het wedstrijdblad komt in de volgende fase.</p>
    {/if}
  {:else if !error}
    <p class="muted">Laden…</p>
  {/if}
</main>
