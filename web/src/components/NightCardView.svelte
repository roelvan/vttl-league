<script lang="ts">
  import type { NightCard } from '../lib/night';
  import { urls } from '../lib/nav';
  import { link } from '../lib/router.svelte';
  import { boards, boardFor, deviceAlias } from '../lib/boards.svelte';
  import type { GameSide } from '../lib/live';
  import TimeoutBadge from './match/TimeoutBadge.svelte';
  import LineupDuels from './LineupDuels.svelte';
  import Icon from './Icon.svelte';
  import { whoOf, type Who } from './match/PlayerPopover.svelte';
  import { ownIsAway, tone } from '../lib/viewer.svelte';
  import { shortDate } from '../lib/format';

  let {
    card,
    current = false,
    wide = false,
    inDivision = false,
    oninfo,
  }: {
    card: NightCard;
    current?: boolean;
    /** The overview: also shows the games in progress. */
    wide?: boolean;
    /** The division page: all cards are of one division, so the card shows its day. */
    inDivision?: boolean;
    /** Opens the player widget next to `at`. */
    oninfo?: (at: HTMLElement, who: Who) => void;
  } = $props();

  const r = $derived(card.row);
  const hasScore = $derived(card.homeScore !== null);
  /** The own club plays away: its team comes first and has the home colour. */
  const flip = $derived(ownIsAway(r.homeClub, r.awayClub));
  const teams = $derived.by(() => {
    // The trophy goes to the winner of a finished match, as in the tiles of the games.
    const done = card.state === 'final';
    const h = card.homeScore ?? 0;
    const a = card.awayScore ?? 0;
    const home = { name: r.homeTeam, score: card.homeScore, ours: !card.neutral && card.isHome, won: done && h > a };
    const away = { name: r.awayTeam, score: card.awayScore, ours: !card.neutral && !card.isHome, won: done && a > h };
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
        one: s.people.length === 1 && s.people[0].id ? s.people[0] : null,
        sets: sets[i],
        lead: sets[i] > sets[1 - i],
        pts: pts ? pts[i] : null,
        serves: pts ? b!.server === cls : null,
      });
      return { no: t.no, pos: g.pos, sides: [side(g.home, 'h', 0), side(g.away, 'a', 1)] };
    }),
  );
  const label = $derived(
    `${teams[0].name} tegen ${teams[1].name}${hasScore ? `, ${teams[0].score}–${teams[1].score}` : ''}${card.state === 'live' ? ', bezig' : ''}`,
  );
  const who = (o: number) => (o === 1 ? 'thuis' : o === 2 ? 'bezoekers' : 'scheidsrechter');
</script>

<!-- The card opens the match. It is a block with a link over it, not a link: the player
     buttons of the line-ups are inside, and a link cannot hold a button. -->
<div class="tm" class:current>
  <!-- A match of two other clubs opens on the pages of the home club: that club has the live data. -->
  <a class="tm-open" href={card.neutral ? urls.matchAt(r.homeClub, r.matchId) : urls.match(r.matchId)} onclick={link} aria-current={current ? 'page' : undefined} aria-label={label}></a>
  <div class="tm-top">
    {#if inDivision}
      <span>{r.date ? shortDate(r.date) : r.matchId}</span>
    {:else if card.division && r.divisionId}
      <a class="tm-div" href={urls.division(r.divisionId, r.week)} onclick={link} title="Alle wedstrijden van deze afdeling">{card.division}</a>
    {:else}
      <span>{card.division || r.matchId}</span>
    {/if}
    {#if !card.neutral}<span>·</span><span>{card.isHome ? 'Thuis' : 'Uit'}</span>{/if}
    <span class="spacer"></span>
    {#if card.state === 'live'}
      <span class="chip live">Live</span>
    {:else if card.state === 'final'}
      <span class="chip" class:win={card.outcome === 'won'}>{card.outcome ? outcomeText[card.outcome] : 'Einde'} {card.isHome || card.neutral ? `${card.homeScore}–${card.awayScore}` : `${card.awayScore}–${card.homeScore}`}</span>
    {:else if card.state === 'bye'}
      <span class="chip">Bye</span>
    {:else}
      <span class="chip">{r.time || '–'}</span>
    {/if}
  </div>
  <div class="tm-mid">
    {#each teams as t (t.name)}
      <span class="tn {t.ours ? 'ours' : 'them'}"><span class="tn-t">{t.name}</span>{#if t.won}<span class="trophy"><Icon name="trophy" size={16} /></span>{/if}</span><span class="ts">{hasScore ? t.score : '–'}</span>
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
              {#if s.rank && s.one && oninfo}
                {@const one = s.one}
                <button class="rank" type="button" aria-label="Spelerinfo {one.full}, {s.rank}" aria-haspopup="dialog" onclick={(e) => oninfo(e.currentTarget, whoOf(one, s.cls === 'h' ? 'home' : 'away', s.cls === 'h' ? r.homeTeam : r.awayTeam, flip))}>{s.rank}</button>
              {:else if s.rank}<span class="rank">{s.rank}</span>{/if}
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
  {#if wide && card.lineup}
    <LineupDuels lineup={card.lineup} names={{ home: r.homeTeam, away: r.awayTeam }} {flip}
      oninfo={oninfo ? (at, p, side) => oninfo(at, whoOf(p, side, side === 'home' ? r.homeTeam : r.awayTeam, flip)) : undefined} />
  {/if}
  <div class="tm-top">
    <span>{card.total ? `${card.played} / ${card.total} gespeeld` : r.matchId}</span><span class="spacer"></span><span>{card.tables}</span>
  </div>
</div>
