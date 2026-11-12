<script lang="ts">
  import ScoreHeader from '../components/match/ScoreHeader.svelte';
  import TableCard from '../components/match/TableCard.svelte';
  import MatchOrder from '../components/match/MatchOrder.svelte';
  import PlayersTally from '../components/match/PlayersTally.svelte';
  import MatchDetails from '../components/match/MatchDetails.svelte';
  import ScoreEntry from '../components/match/ScoreEntry.svelte';
  import StatsDialog from '../components/match/StatsDialog.svelte';
  import Icon from '../components/Icon.svelte';
  import { ws } from '../lib/workspace.svelte';
  import { liveView, tally, type GameView, type TallyRow } from '../lib/live';
  import { shortDivision } from '../lib/night';
  import { canEdit } from '../lib/access';
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
  const editable = $derived(canEdit(sheet) && (row?.isHome ?? true));

  let entry = $state<number | null>(null);
  const entryGame = $derived(entry !== null && view ? view.games[entry] : null);
  const tableOf = (g: GameView) => (g.table !== null && view ? view.tables[g.table] : null);
  const boards = $derived(view ? view.tables.map((t) => (t.device ? boardFor(matchId, t.device, view.sys.boxes) : null)) : []);
  const liveGames = $derived(view ? liveGamesFor(matchId, view) : {});
  const timeoutsUsed = $derived(sheet && view ? timeoutsUsedFor(matchId, sheet, view) : {});

  /** Keys of the rows picked in the Spelers card. */
  let picked = $state<string[]>([]);
  /** Key of the row under the pointer; it previews only that player's games. */
  let hovered = $state<string | null>(null);
  const focus = $derived.by(() => {
    const keys = hovered ? [hovered] : picked;
    if (!keys.length || !sheet || !view) return null;
    const rows = [...tally(sheet, view, 'home'), ...tally(sheet, view, 'away')].filter((r) => keys.includes(r.key));
    return new Set(rows.flatMap((r) => r.games));
  });
  $effect(() => {
    void matchId;
    picked = [];
    hovered = null;
  });
  const togglePlayer = (r: TallyRow) => (picked = picked.includes(r.key) ? picked.filter((k) => k !== r.key) : [...picked, r.key]);

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

{#if !sheet || !view}
  <div class="card empty">
    {#if !sheet}<p>Wedstrijd laden…</p>{:else}<p>Dit matchsysteem kent de app niet ({sheet.matchSystemId}).</p>{/if}
  </div>
{:else}
  {#if sheet.vttlLocked}
    <div class="banner"><Icon name="lock" />VTTL heeft deze uitslag gevalideerd. Niemand kan nog iets wijzigen.</div>
  {/if}

  <ScoreHeader {sheet} {view} week={row?.week ?? ''} division={row?.divisionName ? shortDivision(row.divisionName) : sheet.series} />

  <div class="match-grid">
    <div class="match-main">
      {#if editable && !view.started}
        <a class="card prep" href={urls.lineup(matchId)} onclick={link}>
          <span class="ico"><Icon name="lineup" size={22} /></span>
          <span><span class="t">Opstelling en scoreborden</span><p>Kies de spelers en de scoreborden. Daarna start de avond.</p></span>
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
              board={boards[t.index]}
              onenter={(g) => (entry = g.index)}
              onsend={(g) => sendToBoard(matchId, sheet, t, g)}
              onstats={(g) => (stats = g.index)}
            />
          {/each}
        </section>
      {/if}

      <MatchOrder {view} live={liveGames} {timeoutsUsed} {focus} onclearfocus={() => (picked = [])} onopen={(g) => (entry = g.index)} />
    </div>

    <div class="match-aside">
      <PlayersTally {sheet} {view} {picked} ontoggle={togglePlayer} onhover={(r) => (hovered = r?.key ?? null)} />
      <MatchDetails {sheet} {view} onaction={canPrint ? onaction : undefined} canUpload={editable} />
      {#if editable}
        <a class="card prep" href={urls.lineup(matchId)} onclick={link}>
          <span class="ico"><Icon name="lineup" size={22} /></span>
          <span><span class="t">Opstelling &amp; scoreborden</span><p>{view.started ? 'Wijzigen kan nog, met een waarschuwing.' : 'Vul in voor wedstrijd 1.'}</p></span>
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
