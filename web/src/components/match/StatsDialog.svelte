<script lang="ts">
  // Statistics of one game that a scoreboard counted: live, or after the game from the
  // stored point sequence. Same numbers as vttl-api (see lib/boardStats.ts).
  import Dialog from '../../dialogs/Dialog.svelte';
  import MatchTile from './MatchTile.svelte';
  import type { GameView, GameSide } from '../../lib/live';
  import type { LiveGame, Play } from '../../lib/boards.svelte';
  import { clock, playStats, servePct, type SideStats } from '../../lib/boardStats';
  import { now } from '../../lib/clock.svelte';

  let {
    game,
    play,
    bestOf,
    tableNo = null,
    onclose,
  }: { game: GameView; play: Play; bestOf: number; tableNo?: string | null; onclose: () => void } = $props();

  /** The duration runs on between updates, but not longer than a board counts as online. */
  const MAX_RUN_ON = 2 * 60 * 1000;

  const st = $derived(playStats(play.points, play.firstServer, bestOf));
  const live = $derived(play.live && !st.finished);
  const running = $derived(play.live && !st.finished && play.points.length > 0 && play.at !== null);
  const duration = $derived(play.durationMs + (running ? Math.min(MAX_RUN_ON, Math.max(0, now() - play.at!)) : 0));
  const tmo = $derived(play.live && play.timeout?.active ? play.timeout : null);
  /** The tile shows the board's sets and points while the game runs; after it, the record. */
  const tile = $derived<LiveGame | null>(
    live ? { points: st.current, sets: st.sets.filter((x) => x.finished).map((x) => x.score), timeout: play.timeout } : null,
  );

  // Finished sets as the match record has them: a captain may have corrected them later.
  const recordSets = $derived(
    game.home.cells.map((h, k) => `${h}-${game.away.cells[k]}`).filter((x) => x !== '-'),
  );
  const changed = $derived(
    !play.live && recordSets.join(',') !== st.sets.filter((x) => x.finished).map((x) => x.score.join('-')).join(','),
  );

  const playedSets = $derived(st.sets.filter((x) => x.progression.length > 1));
  let picked = $state<number | null>(null);
  const set = $derived(playedSets.find((x) => x.index === picked) ?? playedSets[playedSets.length - 1] ?? null);

  const short = (s: GameSide) => s.people.map((p) => p.last).join(' / ') || s.label || '–';
  const names = $derived([short(game.home), short(game.away)] as const);

  /** v = what is shown, n = the number that decides who is best (bold). */
  type Row = { label: string; v: (s: SideStats) => string | number; n: (s: SideStats) => number };
  const rows: Row[] = [
    { label: 'Punten gewonnen', v: (s) => s.points, n: (s) => s.points },
    { label: 'Punten op eigen opslag', v: servePct, n: (s) => (s.served ? s.serveWon / s.served : NaN) }, // NaN: no best when a side did not serve
    { label: 'Langste reeks', v: (s) => s.bestStreak, n: (s) => s.bestStreak },
    { label: 'Grootste voorsprong', v: (s) => s.biggestLead, n: (s) => s.biggestLead },
  ];

  // ---- chart: the score after each point of the set
  const W = 340;
  const H = 168;
  const X0 = 28;
  const X1 = W - 10;
  const Y0 = 136;
  const Y1 = 12;
  const chart = $derived.by(() => {
    if (!set) return null;
    const p = set.progression;
    const max = Math.max(11, ...p.map(([a, b]) => Math.max(a, b)));
    const x = (i: number) => X0 + (p.length > 1 ? (i / (p.length - 1)) * (X1 - X0) : 0);
    const y = (v: number) => Y0 - (v / max) * (Y0 - Y1);
    const line = (side: 0 | 1) => p.map((s, i) => `${x(i).toFixed(1)},${y(s[side]).toFixed(1)}`).join(' ');
    const ticks = [0, Math.round(max / 2), max].map((v) => ({ v, y: y(v) }));
    return { home: line(0), away: line(1), ticks, n: p.length - 1 };
  });
</script>

{#snippet stat(label: string, a: string | number, b: string | number, na = 0, nb = 0)}
  <div class="st-row">
    <span class="st-v h" class:best={na > nb}>{a}</span>
    <span class="st-l">{label}</span>
    <span class="st-v a" class:best={nb > na}>{b}</span>
  </div>
{/snippet}

<Dialog title="Statistieken wedstrijd {game.pos}" {onclose}>
  <!-- The same tile as in the match order. -->
  <MatchTile {game} boxes={bestOf} {tableNo} live={tile} used={play.timeoutsUsed} />

  <div class="st-meta">
    <span class="st-time">
      {#if !live}<span class="chip">Gespeeld</span>{/if}
      <span title="Speeltijd volgens het scorebord">Speeltijd <b>{clock(duration)}</b></span>
    </span>
    {#if !tmo && live && st.moment}
      <span class="chip moment">{st.moment}</span>
    {/if}
  </div>

  {#if changed}
    <p class="hint">Een kapitein heeft de uitslag na het scorebord nog gewijzigd. Deze cijfers komen van het scorebord.</p>
  {/if}

  <section class="st-card" aria-label="Hele wedstrijd">
    <h3 class="eyebrow">Hele wedstrijd</h3>
    {#each rows as r (r.label)}{@render stat(r.label, r.v(st.match[0]), r.v(st.match[1]), r.n(st.match[0]), r.n(st.match[1]))}{/each}
    {#if play.timeoutsUsed}
      {@render stat('Time-out genomen', play.timeoutsUsed[0] ? 'Ja' : 'Nee', play.timeoutsUsed[1] ? 'Ja' : 'Nee')}
    {/if}
    {#if st.last.length}
      <div class="st-last">
        <span class="st-l">Laatste {st.last.length} punten</span>
        <span class="dots" role="img" aria-label="Laatste punten: {st.last.map((w) => names[w]).join(', ')}">
          {#each st.last as w, i (i)}<i class={w === 0 ? 'h' : 'a'} title={names[w]}></i>{/each}
        </span>
      </div>
    {/if}
  </section>

  {#if set && chart}
    <section class="st-card" aria-label="Per set">
      <div class="st-sets" role="group" aria-label="Kies een set">
        {#each playedSets as x (x.index)}
          <button class="seg-btn" type="button" aria-pressed={x.index === set.index} onclick={() => (picked = x.index)}>
            Set {x.index + 1}
          </button>
        {/each}
      </div>
      <h3 class="eyebrow">Set {set.index + 1} · {set.score[0]}–{set.score[1]}{set.finished ? '' : ' · bezig'}</h3>
      {#each rows.slice(1) as r (r.label)}{@render stat(r.label, r.v(set.sides[0]), r.v(set.sides[1]), r.n(set.sides[0]), r.n(set.sides[1]))}{/each}

      <svg class="st-chart" viewBox="0 0 {W} {H}" role="img" aria-label="Verloop van set {set.index + 1}: {set.score[0]}–{set.score[1]} na {chart.n} punten">
        {#each chart.ticks as t (t.v)}
          <line x1={X0} x2={X1} y1={t.y} y2={t.y} class="grid" />
          <text x={X0 - 6} y={t.y + 4} text-anchor="end">{t.v}</text>
        {/each}
        <polyline points={chart.away} class="ln a" />
        <polyline points={chart.home} class="ln h" />
        <text x={X0} y={H - 8}>Start</text>
        <text x={X1} y={H - 8} text-anchor="end">Punt {chart.n}</text>
      </svg>
      <div class="st-legend">
        <span><i class="h"></i>{names[0]}</span>
        <span><i class="a"></i>{names[1]}</span>
      </div>
    </section>
  {/if}

  {#snippet footer()}
    <button class="btn big" type="button" onclick={onclose}>Sluiten</button>
  {/snippet}
</Dialog>
