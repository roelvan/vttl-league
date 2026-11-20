<script lang="ts" module>
  import type { DivisionView } from '../lib/types';

  /** The answers of this session, so a page seen before shows at once. */
  const kept = new Map<string, DivisionView>();
</script>

<script lang="ts">
  // All matches of a division in one week, with the standings. The data is of VTTL; a match
  // of the club on screen also shows the live record of that club, as in the club night.
  import NightCardView from '../components/NightCardView.svelte';
  import PlayerPopover from '../components/match/PlayerPopover.svelte';
  import TeamPopover from '../components/TeamPopover.svelte';
  import Icon from '../components/Icon.svelte';
  import Loading from '../components/Loading.svelte';
  import ViewPicker from '../components/ViewPicker.svelte';
  import { api } from '../lib/api';
  import { data, clubName } from '../lib/data.svelte';
  import { ws, loadDay } from '../lib/workspace.svelte';
  import { nightCard, placeClass } from '../lib/night';
  import { navigate, link } from '../lib/router.svelte';
  import { urls, clubPath } from '../lib/nav';
  import { shortDate } from '../lib/format';
  import { isPublic, viewClub } from '../lib/viewer.svelte';
  import { fromVttl } from '../lib/sheet/create';
  import { importOps } from '../lib/sheet/vttl';
  import { getSystem } from '../lib/sheet/systems';
  import { applyOps } from '../lib/sheet/ops';
  import type { Sheet } from '../lib/sheet/types';
  import type { MatchRow, Team, TeamMatch } from '../lib/types';

  let { id, week, teams = [] }: { id: number; week: string | null; /** The teams of the club on screen. */ teams?: Team[] } = $props();

  /** VTTL has new data after a minute at the earliest (the cache of the server). */
  const REFRESH_MS = 60_000;

  const club = $derived(viewClub());
  let view = $state<DivisionView | null>(null);
  let error = $state('');

  $effect(() => {
    const c = club;
    const w = week;
    const div = id;
    if (!c) return;
    const key = (wk: string | null) => `${c}|${div}|${wk ?? ''}`;
    view = kept.get(key(w)) ?? null;
    error = '';
    let stop = false;
    const load = async () => {
      if (document.hidden) return;
      try {
        const v = await api.publicDivision(c, div, w);
        if (stop) return;
        kept.set(key(w), v);
        kept.set(key(v.week), v);
        view = v;
        error = '';
      } catch (e) {
        if (!stop && !kept.has(key(w))) error = e instanceof Error ? e.message : 'Laden mislukt';
      }
    };
    load();
    const timer = setInterval(load, REFRESH_MS);
    document.addEventListener('visibilitychange', load);
    return () => {
      stop = true;
      clearInterval(timer);
      document.removeEventListener('visibilitychange', load);
    };
  });

  const norm = (c: string | null | undefined) => (c ?? '').replace('-', '').toUpperCase();
  const title = $derived(view ? view.division.name.split(' - ').map((x) => x.trim()).filter(Boolean).join(' · ') : 'Afdeling');
  const at = $derived(view ? view.weeks.findIndex((w) => w.week === view!.week) : -1);
  const now = $derived(view?.weeks[at]);
  const prev = $derived(at > 0 ? view!.weeks[at - 1] : undefined);
  const next = $derived(at >= 0 ? view!.weeks[at + 1] : undefined);
  const go = (w: string | undefined) => w && navigate(urls.division(id, w));
  const span = (w: { from: string; to: string }) => (w.from === w.to ? shortDate(w.from) : `${shortDate(w.from)} – ${shortDate(w.to)}`);

  /** A record of the match from the VTTL details: it gives the line-ups. Not stored. */
  function build(row: MatchRow, m: TeamMatch | undefined, season: number): Sheet | undefined {
    if (!m?.details || row.isBye) return undefined;
    try {
      const doc = fromVttl(row, season, data.categories);
      return applyOps(doc, importOps(doc, getSystem(doc.matchSystemId), m, { member: () => undefined, clubName }));
    } catch {
      return undefined; // unknown match system
    }
  }

  const cards = $derived.by(() => {
    if (!view) return [];
    const details = new Map(view.matches.map((m) => [m.matchId, m]));
    return [...view.rows]
      .sort((a, b) => a.date.localeCompare(b.date) || a.time.localeCompare(b.time) || a.matchId.localeCompare(b.matchId))
      .map((r) => {
        const fromVttlData = build(r, details.get(r.matchId), view!.season);
        if (r.neutral) return nightCard(r, undefined, fromVttlData);
        // A match of the club: the stored record is the live truth. A draft is a copy of VTTL data.
        const e = ws.entries[r.matchId];
        const stored = e && !e.draft ? e.view : undefined;
        return nightCard(r, stored, stored ?? fromVttlData ?? e?.view);
      });
  });

  // The live records of the club for its own matches of the week.
  const ownDays = $derived([...new Set((view?.rows ?? []).filter((r) => !r.neutral && r.date).map((r) => r.date))].join(','));
  $effect(() => {
    for (const d of ownDays.split(',').filter(Boolean)) loadDay(d).catch(() => {});
  });

  /** The club night of the day on which the club plays in this week. */
  const clubHref = $derived(ownDays ? urls.day(ownDays.split(',')[0]) : urls.home());

  let info: ReturnType<typeof PlayerPopover> | undefined = $state();
  let teamInfo: ReturnType<typeof TeamPopover> | undefined = $state();
  /** A visitor without a login only gets the players of the club on screen. */
  const canInfo = (neutral: boolean) => !neutral || !isPublic();
</script>

<div class="night-head">
  <ViewPicker {title} {teams} current={id} week={view?.week ?? week} {clubHref} />
  {#if now}
    <div class="daynav">
      <button class="btn ghost" type="button" onclick={() => go(prev?.week)} disabled={!prev} aria-label="Vorige week"><Icon name="left" /></button>
      <label class="day">
        Week {now.week}{' · '}{span(now)}
        <select value={now.week} onchange={(e) => go(e.currentTarget.value)} aria-label="Kies een week">
          {#each view!.weeks as w (w.week)}<option value={w.week}>Week {w.week} · {span(w)}</option>{/each}
        </select>
      </label>
      <button class="btn ghost" type="button" onclick={() => go(next?.week)} disabled={!next} aria-label="Volgende week"><Icon name="right" /></button>
    </div>
  {/if}
</div>

<PlayerPopover bind:this={info} />
<TeamPopover bind:this={teamInfo} />

<!-- The cards and the standings: side by side on a wide screen, else the standings below. -->
<div class="div-body">
<div class="night-list" style="--n: {Math.max(cards.length, 1)}">
  {#each cards as c (c.row.matchId)}
    <NightCardView card={c} wide inDivision oninfo={canInfo(c.neutral) ? (el, who) => info?.show(el, who) : undefined}
      ranking={view?.ranking} onteam={(el, t) => teamInfo?.show(el, t, c.row, view?.ranking.length ?? 0)} />
  {:else}
    <div class="card empty">
      {#if error}
        <p>Laden mislukt: {error}</p>
      {:else if !view}
        <Loading label="Wedstrijden laden" />
      {:else}
        <p>Geen wedstrijden in deze week.</p>
      {/if}
    </div>
  {/each}
</div>

{#if view?.ranking.length}
  <section class="card standings" aria-label="Rangschikking">
    <table>
      <thead>
        <tr>
          <th class="n" scope="col"><abbr title="Plaats">#</abbr></th>
          <th scope="col">Ploeg</th>
          <th class="n" scope="col"><abbr title="Gespeeld">Gesp</abbr></th>
          <th class="n" scope="col"><abbr title="Gewonnen">W</abbr></th>
          <th class="n" scope="col"><abbr title="Gelijk">G</abbr></th>
          <th class="n" scope="col"><abbr title="Verloren">V</abbr></th>
          <th class="n hide-sm" scope="col"><abbr title="Individuele wedstrijden: gewonnen en verloren">Wedstr.</abbr></th>
          <th class="n" scope="col"><abbr title="Punten">Ptn</abbr></th>
        </tr>
      </thead>
      <tbody>
        {#each view.ranking as e (`${e.position}|${e.team}`)}
          <tr class:ours={norm(e.teamClub) === norm(club)}>
            <td class="n"><span class="rank-tag place round {placeClass(e.position)}">{e.position}</span></td>
            <th scope="row"><a href={clubPath(e.teamClub)} onclick={link} title="Naar de clubavond van {e.team}">{e.team}</a></th>
            <td class="n">{e.played}</td>
            <td class="n">{e.won}</td>
            <td class="n">{e.draw}</td>
            <td class="n">{e.lost}</td>
            <td class="n hide-sm">{e.gamesWon}–{e.gamesLost}</td>
            <td class="n total">{e.points}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </section>
{/if}
</div>
