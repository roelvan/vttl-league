<script lang="ts">
  import type { Sheet } from '../../lib/sheet/types';
  import type { LiveView } from '../../lib/live';

  let { sheet, view, week = '', division = '' }: { sheet: Sheet; view: LiveView; week?: string; division?: string } = $props();

  const n = $derived(view.games.length);
  const segs = $derived(view.games.map((g) => g.winner ?? (g.state === 'live' ? 'live' : '')));
  const status = $derived(
    sheet.vttlLocked ? 'Gevalideerd' : view.complete ? 'Einde' : view.started ? 'live' : sheet.startHour ? `Om ${sheet.startHour}` : 'Nog niet begonnen',
  );
</script>

<section class="card board" aria-label="Stand van de ploegenwedstrijd">
  <div class="board-meta">
    <!-- shortDivision joins its parts with " · ": show them with the same dots as the rest. -->
    {#each division.split(' · ').filter(Boolean) as part, i (i)}<span>{part}</span><span class="dot"></span>{/each}
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
    </div>
  </div>
</section>
