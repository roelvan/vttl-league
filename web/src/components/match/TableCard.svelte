<script lang="ts">
  import Icon from '../Icon.svelte';
  import type { TableView, GameView, GameSide, BoardLive } from '../../lib/live';

  let {
    table,
    boxes,
    editable,
    board = null,
    onenter,
    onsend,
  }: {
    table: TableView;
    boxes: number;
    editable: boolean;
    board?: BoardLive | null;
    onenter: (g: GameView) => void;
    onsend?: (g: GameView) => void;
  } = $props();

  const g = $derived(table.current);
  const linked = $derived(!!board && !!g && board.linkedGame === g.index);
  const pts = $derived(linked ? board!.points : null);
  const names = (s: GameSide) => s.people.map((p) => p.last).join(' / ') || s.label;
</script>

{#snippet prow(side: GameSide, cls: 'h' | 'a', i: 0 | 1)}
  {@const lead = i === 0 ? g!.home.sets > g!.away.sets : g!.away.sets > g!.home.sets}
  <div class="prow">
    <span class="letter {cls}">{side.label.length > 2 ? 'D' : side.label}</span>
    <span class="pname">
      {#if linked}<span class="serve" class:off={board?.server !== cls}></span>{/if}
      <span class="n">{side.people.map((p) => p.full).join(' / ') || 'Nog niet opgesteld'}</span>
      {#if side.people.length === 1 && side.people[0].ranking}<span class="rank">{side.people[0].ranking}</span>{/if}
    </span>
    <span class="pts">{pts ? pts[i] : ''}</span>
    <span class="games" class:lead>{side.sets}</span>
  </div>
{/snippet}

<article class="card court">
  <div class="court-h">
    <span class="tno">Tafel {table.no}</span>
    {#if g}<span class="mno">Wedstrijd {g.pos}</span>{/if}
    <span class="spacer"></span>
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
        <div class="gbox" class:cur={g.currentBox === k}>
          <small>S{k + 1}</small>
          {#if g.home.cells[k] || g.away.cells[k]}
            <span class:w={g.home.won[k]}>{g.home.cells[k]}</span>–<span class:w={g.away.won[k]}>{g.away.cells[k]}</span>
          {:else if g.currentBox === k && pts}
            {pts[0]}–{pts[1]}
          {:else if g.currentBox === k}
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
          <button class="btn ghost" type="button" onclick={() => onenter(g)}>Score</button>
        </div>
      {:else}
        <button class="btn" type="button" onclick={() => onenter(g)}><Icon name="plus" />Score invoeren</button>
      {/if}
    {:else if g}
      <button class="btn ghost" type="button" onclick={() => onenter(g)}>Details</button>
    {/if}
  </div>
</article>
