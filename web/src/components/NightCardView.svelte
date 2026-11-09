<script lang="ts">
  import type { NightCard } from '../lib/night';
  import { urls } from '../lib/nav';
  import { link } from '../lib/router.svelte';
  import { boards, deviceAlias } from '../lib/boards.svelte';
  import TimeoutBadge from './match/TimeoutBadge.svelte';

  let { card, current = false }: { card: NightCard; current?: boolean } = $props();

  const r = $derived(card.row);
  const hasScore = $derived(card.homeScore !== null);
  const outcomeText = { won: 'Gewonnen', lost: 'Verloren', draw: 'Gelijk' } as const;
  /** Running timeouts on the scoreboards of this match. */
  const timeouts = $derived(
    Object.values(boards.live).filter((s) => s.sheet === r.matchId && !s.finished && s.timeout?.active),
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
    <span class="tn {card.isHome ? 'ours' : 'them'}">{r.homeTeam}</span><span class="ts">{hasScore ? card.homeScore : '–'}</span>
    <span class="tn {card.isHome ? 'them' : 'ours'}">{r.awayTeam}</span><span class="ts">{hasScore ? card.awayScore : '–'}</span>
  </div>
  {#each timeouts as s (s.device)}
    <div class="tm-tmo"><span>{deviceAlias(s.device)}</span><TimeoutBadge timeout={s.timeout!} who="{who(s.timeout!.owner)}, {deviceAlias(s.device)}" /></div>
  {/each}
  {#if card.segments.length}
    <div class="mini" style="grid-template-columns: repeat({card.segments.length}, 1fr)">
      {#each card.segments as s, i (i)}<i class={s}></i>{/each}
    </div>
  {/if}
  <div class="tm-top">
    <span>{card.total ? `${card.played} / ${card.total} gespeeld` : r.matchId}</span><span class="spacer"></span><span>{card.tables}</span>
  </div>
</a>
