<script lang="ts">
  // The Spelers card of the match page: the same line-up widget as in the club night cards,
  // with full names and a dot per game.
  import type { Sheet } from '../../lib/sheet/types';
  import { lineup, type LineRow, type LiveView, type Person, type Side } from '../../lib/live';
  import LineupDuels from '../LineupDuels.svelte';

  let {
    sheet,
    view,
    pinned = $bindable(null),
    onhover,
    oninfo,
    flip = false,
  }: {
    sheet: Sheet;
    view: LiveView;
    /** Preview one row's games while the pointer or the focus is on it; null when it leaves. */
    onhover?: (r: LineRow | null) => void;
    /** Key of the row that a click keeps on: only its games show in the match order. */
    pinned?: string | null;
    /** Opens the player widget next to `at` (the ranking). */
    oninfo?: (at: HTMLElement, p: Person, side: Side) => void;
    /** The own club plays away: the away team comes first, in the home colour. */
    flip?: boolean;
  } = $props();

  const rows = $derived({ home: lineup(sheet, view, 'home'), away: lineup(sheet, view, 'away') });
</script>

<section class="card" aria-label="Spelers">
  <div class="card-h"><h2>Spelers</h2></div>
  <div class="card-b nl-box">
    <LineupDuels lineup={rows} names={{ home: sheet.home.name, away: sheet.away.name }} {flip} full bind:pinned onfocusrow={onhover} {oninfo} />
  </div>
</section>
