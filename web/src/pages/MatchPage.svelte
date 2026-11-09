<script lang="ts">
  import ScoreHeader from '../components/match/ScoreHeader.svelte';
  import TableCard from '../components/match/TableCard.svelte';
  import MatchOrder from '../components/match/MatchOrder.svelte';
  import PlayersTally from '../components/match/PlayersTally.svelte';
  import MatchDetails from '../components/match/MatchDetails.svelte';
  import ScoreEntry from '../components/match/ScoreEntry.svelte';
  import Icon from '../components/Icon.svelte';
  import { ws } from '../lib/workspace.svelte';
  import { liveView, type GameView } from '../lib/live';
  import { shortDivision } from '../lib/night';
  import { canEdit } from '../lib/access';
  import { boardFor, livePointsFor, sendToBoard } from '../lib/boards.svelte';
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
  const boards = $derived(view ? view.tables.map((t) => (t.device ? boardFor(matchId, t.device) : null)) : []);
  const livePoints = $derived(view ? livePointsFor(matchId, view) : {});

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
  <ScoreHeader {sheet} {view} week={row?.week ?? ''} division={row?.divisionName ? shortDivision(row.divisionName) : sheet.series} />

  {#if sheet.vttlLocked}
    <div class="banner"><Icon name="check" />VTTL heeft deze uitslag gevalideerd. Niemand kan nog iets wijzigen.</div>
  {/if}

  <div class="match-grid">
    <div class="match-main">
      {#if view.tables.length && !view.complete}
        <section class="tables" aria-label="Tafels in gebruik">
          {#each view.tables as t (t.index)}
            <TableCard
              table={t}
              boxes={view.sys.boxes}
              {editable}
              board={boards[t.index]}
              onenter={(g) => (entry = g.index)}
              onsend={(g) => sendToBoard(matchId, sheet, t, g)}
            />
          {/each}
        </section>
      {:else if editable && !view.started}
        <a class="card prep" href={urls.lineup(matchId)} onclick={link}>
          <span class="ico"><Icon name="lineup" size={22} /></span>
          <span><span class="t">Opstelling en tafels</span><p>Kies de tafels en de spelers. Daarna start de avond.</p></span>
          <span class="spacer"></span>
          <Icon name="chevron" />
        </a>
      {/if}

      <MatchOrder {view} {livePoints} onopen={(g) => (entry = g.index)} />
    </div>

    <div class="match-aside">
      <PlayersTally {sheet} {view} />
      <MatchDetails {sheet} {view} onaction={canPrint ? onaction : undefined} canUpload={editable} />
      {#if editable}
        <a class="card prep" href={urls.lineup(matchId)} onclick={link}>
          <span class="ico"><Icon name="lineup" size={22} /></span>
          <span><span class="t">Opstelling &amp; tafels</span><p>{view.started ? 'Wijzigen kan nog, met een waarschuwing.' : 'Vul in voor wedstrijd 1.'}</p></span>
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
    />
  {/if}
{/if}
