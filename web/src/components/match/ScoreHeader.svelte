<script lang="ts">
  import type { Sheet } from '../../lib/sheet/types';
  import type { LiveView } from '../../lib/live';

  let { sheet, view, week = '', division = '' }: { sheet: Sheet; view: LiveView; week?: string; division?: string } = $props();

  const n = $derived(view.games.length);
  const segs = $derived(view.games.map((g) => g.winner ?? (g.state === 'live' ? 'live' : '')));
  /** The win mark sits after segment `needed` (3px gaps between segments). */
  const markLeft = $derived(`calc((100% - ${(n - 1) * 3}px) * ${view.needed / n} + ${(view.needed - 1) * 3 + 1.5}px)`);
  const status = $derived(
    sheet.vttlLocked ? 'Gevalideerd' : view.complete ? 'Einde' : view.started ? 'live' : sheet.startHour ? `Om ${sheet.startHour}` : 'Nog niet begonnen',
  );
  const played = $derived(view.games.filter((g) => g.state === 'final').length);
  const drawText = $derived(n % 2 === 0 ? `${n / 2}–${n / 2} is gelijk` : '');
</script>

<section class="card board" aria-label="Stand van de ploegenwedstrijd">
  <div class="board-meta">
    {#if division}<span>{division}</span><span class="dot"></span>{/if}
    {#if week}<span class="hide-sm">Week {week}</span><span class="dot hide-sm"></span>{/if}
    <span class="num">{sheet.matchId}</span>
    <span class="spacer"></span>
    {#if status === 'live'}<span class="chip live">Live</span>{:else}<span class="chip" class:win={status === 'Einde'}>{status}</span>{/if}
  </div>
  <div class="board-main">
    <div class="team home">
      <span class="team-tag">Thuis</span>
      <div class="team-name">{sheet.home.name}</div>
      {#if sheet.home.forfeit}<span class="team-sub">Forfait</span>{/if}
    </div>
    <div class="big-score" aria-label="{view.homeWins} tegen {view.awayWins}">
      <span class="h">{view.homeWins}</span><span class="sep">:</span><span class="a">{view.awayWins}</span>
    </div>
    <div class="team away">
      <span class="team-tag">Uit</span>
      <div class="team-name">{sheet.away.name}</div>
      {#if sheet.away.forfeit}<span class="team-sub">Forfait</span>{/if}
    </div>
  </div>
  <div class="race">
    <div class="race-bar" style="grid-template-columns: repeat({n}, 1fr)">
      {#each segs as s, i (i)}<span class="seg {s}" title="Wedstrijd {i + 1}"></span>{/each}
      <span class="mark" style="left: {markLeft}" title="{view.needed} overwinningen winnen de avond"></span>
    </div>
    <div class="race-legend">
      <span><b>{played}</b> / {n} gespeeld</span>
      <span class="mid">{view.needed} wint de avond{drawText ? ` · ${drawText}` : ''}</span>
    </div>
  </div>
</section>
