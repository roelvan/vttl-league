<script lang="ts">
  import Icon from '../Icon.svelte';
  import TimeoutBadge from './TimeoutBadge.svelte';
  import TimeoutUsed from './TimeoutUsed.svelte';
  import type { TableView, GameView, GameSide, BoardLive } from '../../lib/live';

  let {
    table,
    boxes,
    editable,
    board = null,
    onenter,
    onsend,
    onstats,
  }: {
    table: TableView;
    boxes: number;
    editable: boolean;
    board?: BoardLive | null;
    onenter: (g: GameView) => void;
    onsend?: (g: GameView) => void;
    /** Opens the statistics of the game on the board. */
    onstats?: (g: GameView) => void;
  } = $props();

  const g = $derived(table.current);
  const linked = $derived(!!board && !!g && board.linkedGame === g.index);
  const pts = $derived(linked ? board!.points : null);
  // While the board counts, its finished sets are not in the match record yet.
  const bsets = $derived(linked && pts ? board!.sets : null);
  const setsWon = $derived(
    bsets ? [bsets.filter(([a, b]) => a > b).length, bsets.filter(([a, b]) => b > a).length] : g ? [g.home.sets, g.away.sets] : [0, 0],
  );
  const curBox = $derived(bsets ? bsets.length : (g?.currentBox ?? null));
  const tmo = $derived(linked ? board!.timeout : null);
  const names = (s: GameSide) => s.people.map((p) => p.last).join(' / ') || '–';
</script>

{#snippet prow(side: GameSide, cls: 'h' | 'a', i: 0 | 1)}
  {@const lead = setsWon[i] > setsWon[1 - i]}
  {@const used = tmo ? (i === 0 ? tmo.p1Used : tmo.p2Used) : false}
  <div class="prow">
    <span class="letter {cls}"></span>
    <span class="pname">
      {#if linked}<span class="serve" class:off={board?.server !== cls}></span>{/if}
      <span class="n">{side.people.map((p) => p.full).join(' / ') || 'Nog niet opgesteld'}</span>
      {#if side.people.length === 1 && side.people[0].ranking}<span class="rank">{side.people[0].ranking}</span>{/if}
      {#if tmo?.active && tmo.owner === i + 1}
        <TimeoutBadge timeout={tmo} compact who={i === 0 ? 'thuis' : 'bezoekers'} />
      {:else if used}
        <TimeoutUsed size={20} />
      {/if}
    </span>
    <span class="pts">{pts ? pts[i] : ''}</span>
    <span class="games" class:lead>{setsWon[i]}</span>
  </div>
{/snippet}

<article class="card court">
  <div class="court-h">
    <span class="tno">Tafel {table.no}</span>
    {#if g}<span class="mno">Wedstrijd {g.pos}</span>{/if}
    <span class="spacer"></span>
    {#if tmo?.active && tmo.owner === 0}
      <TimeoutBadge timeout={tmo} who="scheidsrechter" />
    {:else if linked && board?.moment}
      <span class="chip moment">{board.moment}</span>
    {/if}
    {#if table.device}
      <span class="chip board-link" title={board?.online === false ? 'Scorebord is offline' : 'Punten komen van het scorebord'}>
        <Icon name="board" size={14} />{board?.alias || 'Scorebord'}
      </span>
    {:else}
      <span class="chip">Manueel</span>
    {/if}
  </div>

  {#if g}
    <div class="rows">
      {@render prow(g.home, 'h', 0)}
      {@render prow(g.away, 'a', 1)}
    </div>
    <div class="court-games" style="--boxes: {boxes}">
      {#each Array.from({ length: boxes }, (_, k) => k) as k (k)}
        <div class="gbox" class:cur={curBox === k}>
          <small>S{k + 1}</small>
          {#if bsets?.[k]}
            {@const [a, b] = bsets[k]}
            <span class:w={a > b}>{a}</span>–<span class:w={b > a}>{b}</span>
          {:else if !bsets && (g.home.cells[k] || g.away.cells[k])}
            <span class:w={g.home.won[k]}>{g.home.cells[k]}</span>–<span class:w={g.away.won[k]}>{g.away.cells[k]}</span>
          {:else if curBox === k && pts}
            {pts[0]}–{pts[1]}
          {:else if curBox === k}
            <span class="playing">Bezig</span>
          {:else}
            –
          {/if}
        </div>
      {/each}
    </div>
  {:else}
    <p class="court-idle">Alle wedstrijden op deze tafel zijn gespeeld.</p>
  {/if}

  <div class="court-f">
    {#if table.next}<span>Volgende: <b>{names(table.next.home)} – {names(table.next.away)}</b></span>{/if}
    {#if g && editable}
      {#if table.device && onsend}
        <div class="btns">
          <button class="btn" class:ghost={linked} type="button" onclick={() => onsend(g)}>
            <Icon name="send" />{linked ? 'Opnieuw sturen' : 'Naar scorebord'}
          </button>
          {#if linked && pts && onstats}
            <button class="btn ghost" type="button" onclick={() => onstats(g)}><Icon name="stats" />Statistieken</button>
          {:else}
            <button class="btn ghost" type="button" onclick={() => onenter(g)}>Score</button>
          {/if}
        </div>
      {:else}
        <button class="btn" type="button" onclick={() => onenter(g)}><Icon name="plus" />Score invoeren</button>
      {/if}
    {:else if g && linked && pts && onstats}
      <button class="btn ghost" type="button" onclick={() => onstats(g)}><Icon name="stats" />Statistieken</button>
    {:else if g}
      <button class="btn ghost" type="button" onclick={() => onenter(g)}>Details</button>
    {/if}
  </div>
</article>
