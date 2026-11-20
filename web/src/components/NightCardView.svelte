<script lang="ts">
  import type { NightCard } from '../lib/night';
  import { urls } from '../lib/nav';
  import { link } from '../lib/router.svelte';
  import { boards, boardFor, deviceAlias } from '../lib/boards.svelte';
  import type { GameSide } from '../lib/live';
  import TimeoutBadge from './match/TimeoutBadge.svelte';
  import { ownIsAway, tone } from '../lib/viewer.svelte';

  let {
    card,
    current = false,
    wide = false,
  }: { card: NightCard; current?: boolean; /** The overview: also shows the games in progress. */ wide?: boolean } = $props();

  const r = $derived(card.row);
  const hasScore = $derived(card.homeScore !== null);
  /** The own club plays away: its team comes first and has the home colour. */
  const flip = $derived(ownIsAway(r.homeClub, r.awayClub));
  const teams = $derived.by(() => {
    const home = { name: r.homeTeam, score: card.homeScore, ours: card.isHome };
    const away = { name: r.awayTeam, score: card.awayScore, ours: !card.isHome };
    return flip ? [away, home] : [home, away];
  });
  const outcomeText = { won: 'Gewonnen', lost: 'Verloren', draw: 'Gelijk' } as const;
  /** Running timeouts on the scoreboards of this match. */
  const timeouts = $derived(
    Object.values(boards.live).filter((s) => s.sheet === r.matchId && !s.finished && s.timeout?.active),
  );
  /** Per table with a game in progress: the two sides with their sets and, from a scoreboard, their points. */
  const courts = $derived(
    (wide ? card.live : []).map((t) => {
      const g = t.current!;
      const b = t.device ? boardFor(r.matchId, t.device, card.boxes) : null;
      const pts = b && b.linkedGame === g.index ? b.points : null;
      // While the board counts, its finished sets are not in the match record yet.
      const sets = pts
        ? [b!.sets.filter(([x, y]) => x > y).length, b!.sets.filter(([x, y]) => y > x).length]
        : [g.home.sets, g.away.sets];
      const side = (s: GameSide, cls: 'h' | 'a', i: 0 | 1) => ({
        cls,
        name: (s.people.length === 1 ? s.people[0].full : s.people.map((p) => p.last).join(' / ')) || '–',
        rank: s.people.length === 1 ? s.people[0].ranking : '',
        sets: sets[i],
        lead: sets[i] > sets[1 - i],
        pts: pts ? pts[i] : null,
        serves: pts ? b!.server === cls : null,
      });
      return { no: t.no, pos: g.pos, sides: [side(g.home, 'h', 0), side(g.away, 'a', 1)] };
    }),
  );
  const who = (o: number) => (o === 1 ? 'thuis' : o === 2 ? 'bezoekers' : 'scheidsrechter');
</script>

<a class="tm" class:current href={urls.match(r.matchId)} onclick={link} aria-current={current ? 'page' : undefined}>
  <div class="tm-top">
    <span>{card.division || r.matchId}</span><span>·</span><span>{card.isHome ? 'Thuis' : 'Uit'}</span>
    <span class="spacer"></span>
    {#if card.state === 'live'}
      <span class="chip live">Live</span>
    {:else if card.state === 'final'}
      <span class="chip" class:win={card.outcome === 'won'}>{card.outcome ? outcomeText[card.outcome] : 'Einde'} {card.isHome ? `${card.homeScore}–${card.awayScore}` : `${card.awayScore}–${card.homeScore}`}</span>
    {:else if card.state === 'bye'}
      <span class="chip">Bye</span>
    {:else}
      <span class="chip">{r.time || '–'}</span>
    {/if}
  </div>
  <div class="tm-mid">
    {#each teams as t (t.name)}
      <span class="tn {t.ours ? 'ours' : 'them'}">{t.name}</span><span class="ts">{hasScore ? t.score : '–'}</span>
    {/each}
  </div>
  {#each timeouts as s (s.device)}
    <div class="tm-tmo"><span>{deviceAlias(s.device)}</span><TimeoutBadge timeout={s.timeout!} who="{who(s.timeout!.owner)}, {deviceAlias(s.device)}" /></div>
  {/each}
  {#if courts.length}
    <div class="tm-live">
      {#each courts as c (c.no)}
        <div class="tl" role="group" aria-label="Tafel {c.no}, wedstrijd {c.pos}">
          <span class="tl-t" aria-hidden="true">T{c.no}</span>
          {#each c.sides as s (s.cls)}
            <span class="tl-n">
              <i class="letter {s.cls}"></i><span class="n">{s.name}</span>
              {#if s.rank}<span class="rank">{s.rank}</span>{/if}
              {#if s.serves !== null}<span class="serve" class:off={!s.serves}></span>{/if}
            </span>
            <span class="tl-p">{s.pts ?? ''}</span>
            <span class="tl-s" class:lead={s.lead}>{s.sets}</span>
          {/each}
        </div>
      {/each}
    </div>
  {/if}
  {#if card.segments.length}
    <div class="mini" style="grid-template-columns: repeat({card.segments.length}, 1fr)">
      {#each card.segments as s, i (i)}<i class={tone(s, flip)}></i>{/each}
    </div>
  {/if}
  <div class="tm-top">
    <span>{card.total ? `${card.played} / ${card.total} gespeeld` : r.matchId}</span><span class="spacer"></span><span>{card.tables}</span>
  </div>
</a>
