<script lang="ts">
  import ScoreHeader from '../components/match/ScoreHeader.svelte';
  import TableCard from '../components/match/TableCard.svelte';
  import MatchOrder from '../components/match/MatchOrder.svelte';
  import PlayersTally from '../components/match/PlayersTally.svelte';
  import PlayerPopover from '../components/match/PlayerPopover.svelte';
  import MatchDetails from '../components/match/MatchDetails.svelte';
  import ScoreEntry from '../components/match/ScoreEntry.svelte';
  import StatsDialog from '../components/match/StatsDialog.svelte';
  import Icon from '../components/Icon.svelte';
  import { ws } from '../lib/workspace.svelte';
  import { liveView, tally, type GameView, type Person, type Side, type TallyRow } from '../lib/live';
  import { shortDivision } from '../lib/night';
  import { canEdit } from '../lib/access';
  import { ownIsAway, tone } from '../lib/viewer.svelte';
  import { warmPlayers } from '../lib/players.svelte';
  import { boardFor, liveGamesFor, playFor, sendToBoard, timeoutsUsedFor } from '../lib/boards.svelte';
  import { link } from '../lib/router.svelte';
  import { urls, type MenuAction } from '../lib/nav';
  import type { MatchRow } from '../lib/types';

  let {
    matchId,
    row,
    onaction,
    canPrint,
  }: { matchId: string; row: MatchRow | null; onaction: (a: MenuAction) => void; canPrint: boolean } = $props();

  const sheet = $derived(ws.entries[matchId]?.view);
  const view = $derived.by(() => {
    if (!sheet) return null;
    try {
      return liveView(sheet);
    } catch {
      return null;
    }
  });
  /** The own club plays away: its side comes first and has the home colour. */
  const flip = $derived(!!sheet && ownIsAway(sheet.home.clubId, sheet.away.clubId));
  const editable = $derived(canEdit(sheet) && (row?.isHome ?? true));

  /** The VTTL numbers of the line-up, as text: the server gets these players ready for the player widget. */
  const lineup = $derived(
    sheet
      ? [...sheet.home.players, ...sheet.away.players]
          .map((p) => Number(p?.computerNumber))
          .filter((n) => Number.isInteger(n) && n > 0)
          .join(',')
      : '',
  );
  $effect(() => {
    if (lineup) warmPlayers(lineup.split(',').map(Number));
  });

  /** The team score that VTTL has, from the match list: known before the results come in. */
  const vttlScore = $derived.by((): [number, number] | null => {
    const s = /^\s*(\d+)\s*-\s*(\d+)/.exec(row?.score ?? '');
    return s && (s[1] !== '0' || s[2] !== '0') ? [Number(s[1]), Number(s[2])] : null;
  });
  /**
   * VTTL has results, but they are not in the match yet (first visit on this device): show the
   * known score and a skeleton, not an empty match that changes a moment later.
   */
  const pending = $derived(!!ws.loading[matchId] && !!view && !view.started && !!vttlScore);

  let entry = $state<number | null>(null);
  const entryGame = $derived(entry !== null && view ? view.games[entry] : null);
  const tableOf = (g: GameView) => (g.table !== null && view ? view.tables[g.table] : null);
  const boards = $derived(view ? view.tables.map((t) => (t.device ? boardFor(matchId, t.device, view.sys.boxes) : null)) : []);
  const liveGames = $derived(view ? liveGamesFor(matchId, view) : {});
  const timeoutsUsed = $derived(sheet && view ? timeoutsUsedFor(matchId, sheet, view) : {});

  /** Keys of the rows picked (clicked) in the Spelers card: only their games are shown. */
  let picked = $state<string[]>([]);
  /** Key of the row under the pointer: its games stand out, the others are dimmed. */
  let hovered = $state<string | null>(null);
  function gamesOf(keys: string[]): Set<number> | null {
    if (!keys.length || !sheet || !view) return null;
    const rows = [...tally(sheet, view, 'home'), ...tally(sheet, view, 'away')].filter((r) => keys.includes(r.key));
    return new Set(rows.flatMap((r) => r.games));
  }
  const only = $derived(gamesOf(picked));
  const focus = $derived(hovered ? gamesOf([hovered]) : null);
  $effect(() => {
    void matchId;
    picked = [];
    hovered = null;
  });
  const togglePlayer = (r: TallyRow) => (picked = picked.includes(r.key) ? picked.filter((k) => k !== r.key) : [...picked, r.key]);

  /** The player widget: one for the page, opened from the Spelers card and from the game tiles. */
  let info: ReturnType<typeof PlayerPopover> | undefined = $state();
  function showInfo(at: HTMLElement, p: Person, side: Side) {
    if (!sheet || !p.id) return;
    info?.show(at, { id: p.id, name: p.full, ranking: p.ranking, team: sheet[side].name, cls: tone(side === 'home' ? 'h' : 'a', flip) });
  }

  /** The game whose scoreboard statistics are open. */
  let stats = $state<number | null>(null);
  const statsGame = $derived(stats !== null && view ? view.games[stats] : null);
  const statsPlay = $derived(stats !== null && sheet && view ? playFor(matchId, sheet, view, stats) : null);
  const hasPlay = (g: GameView) => !!(sheet && view && playFor(matchId, sheet, view, g.index));

  function boardBusy(g: GameView) {
    const t = tableOf(g);
    const b = t ? boards[t.index] : null;
    return !!b && b.linkedGame === g.index && g.state !== 'final';
  }
</script>

<PlayerPopover bind:this={info} />

{#if !sheet || !view}
  <div class="card empty">
    {#if !sheet}<p>Wedstrijd laden…</p>{:else}<p>Dit matchsysteem kent de app niet ({sheet.matchSystemId}).</p>{/if}
  </div>
{:else}
  {#if sheet.vttlLocked}
    <div class="banner"><Icon name="lock" />VTTL heeft deze uitslag gevalideerd. Niemand kan nog iets wijzigen.</div>
  {/if}

  <ScoreHeader {sheet} {view} {flip} score={pending ? vttlScore : null} week={row?.week ?? ''} division={row?.divisionName ? shortDivision(row.divisionName) : sheet.series} />

  <div class="match-grid">
    <div class="match-main">
      {#if pending}
        <!-- The results are on their way: no tables or line-up card that go away again. -->
      {:else if editable && !view.started}
        <a class="card prep" href={urls.lineup(matchId)} onclick={link}>
          <span class="t">Opstelling en scoreborden</span>
          <span class="spacer"></span>
          <Icon name="chevron" />
        </a>
      {:else if !view.complete}
        <section class="tables" aria-label="Tafels in gebruik">
          {#each view.tables as t (t.index)}
            <TableCard
              table={t}
              boxes={view.sys.boxes}
              {editable}
              {flip}
              board={boards[t.index]}
              onenter={(g) => (entry = g.index)}
              onsend={(g) => sendToBoard(matchId, sheet, t, g)}
              onstats={(g) => (stats = g.index)}
            />
          {/each}
        </section>
      {/if}

      <MatchOrder {view} {flip} loading={pending} live={liveGames} {timeoutsUsed} {only} {focus} onclearfocus={() => (picked = [])} onopen={(g) => (entry = g.index)} oninfo={showInfo} />
    </div>

    <div class="match-aside">
      <PlayersTally {sheet} {view} {flip} {picked} ontoggle={togglePlayer} onhover={(r) => (hovered = r?.key ?? null)} oninfo={showInfo} />
      <MatchDetails {sheet} {view} onaction={canPrint ? onaction : undefined} canUpload={editable} division={row?.divisionName ?? ''} />
      {#if editable}
        <a class="card prep" href={urls.lineup(matchId)} onclick={link}>
          <span><span class="t">Opstelling en scoreborden</span><p>{view.started ? 'Wijzigen kan nog, met een waarschuwing.' : 'Vul in voor wedstrijd 1.'}</p></span>
          <span class="spacer"></span>
          <Icon name="chevron" />
        </a>
      {/if}
    </div>
  </div>

  {#if entryGame}
    <ScoreEntry
      id={matchId}
      {sheet}
      game={entryGame}
      {editable}
      {flip}
      tableNo={tableOf(entryGame)?.no ?? null}
      boardBusy={boardBusy(entryGame)}
      onclose={() => (entry = null)}
      onstats={hasPlay(entryGame) ? () => ((stats = entryGame.index), (entry = null)) : undefined}
    />
  {/if}

  {#if statsGame && statsPlay}
    <StatsDialog game={statsGame} play={statsPlay} bestOf={view.sys.boxes} tableNo={tableOf(statsGame)?.no ?? null} onclose={() => (stats = null)} />
  {/if}
{/if}
