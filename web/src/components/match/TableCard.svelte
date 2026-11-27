<script lang="ts">
  import { tick } from 'svelte';
  import Icon from '../Icon.svelte';
  import TimeoutBadge from './TimeoutBadge.svelte';
  import TimeoutUsed from './TimeoutUsed.svelte';
  import type { TableView, GameView, GameSide, BoardLive, Person, Side } from '../../lib/live';
  import type { SetScore } from '../../lib/sheet/types';
  import type { MatchSystem } from '../../lib/sheet/systems';
  import { setWinner, isEmptySet } from '../../lib/sheet/score';
  import { tone } from '../../lib/viewer.svelte';

  let {
    table,
    boxes,
    editable,
    board = null,
    onenter,
    onsend,
    onstats,
    oninfo,
    onsets,
    sys,
    flip = false,
  }: {
    table: TableView;
    boxes: number;
    editable: boolean;
    board?: BoardLive | null;
    onenter: (g: GameView) => void;
    onsend?: (g: GameView) => void;
    /** Opens the statistics of the game on the board. */
    onstats?: (g: GameView) => void;
    /** Opens the player widget next to `at` (the ranking). */
    oninfo?: (at: HTMLElement, p: Person, side: Side) => void;
    /** Saves the set scores typed in the set boxes. Without it the boxes only show. */
    onsets?: (g: GameView, sets: SetScore[]) => void;
    /** The match system: closes the sets after the one that decides the game. */
    sys?: MatchSystem;
    /** The own club plays away: the away player comes first, in the home colour. */
    flip?: boolean;
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

  // ---- quick entry in the set boxes
  // A click on a box changes the row of boxes into fields. A box is saved when the focus leaves
  // it; the row shows the scores again when the focus leaves the row. Not while a scoreboard counts.
  const canType = $derived(editable && !!onsets && !!g && !pts);
  let editing = $state<number | null>(null); // the game index in edit
  let draft = $state<SetScore[]>([]);
  let sent = '';
  let row = $state<HTMLElement>();
  /** The record keeps home in "l"; on screen the first field is the away side in a flipped match. */
  const key = (s: 0 | 1): 'l' | 'r' => ((s === 0) !== flip ? 'l' : 'r');
  /** The set that decides the game in the typed scores; the sets after it are closed. */
  const decidedAt = $derived.by(() => {
    if (!sys) return Infinity;
    let h = 0, a = 0;
    for (let i = 0; i < draft.length; i++) {
      const w = isEmptySet(draft[i]) ? 'empty' : setWinner(i, draft[i], sys);
      if (w === 'home') h++;
      if (w === 'away') a++;
      if (h === sys.setCount || a === sys.setCount) return i;
    }
    return Infinity;
  });
  const fields = () => [...(row?.querySelectorAll('input') ?? [])];

  async function edit(k: number) {
    if (!g || !canType) return;
    draft = Array.from({ length: boxes }, (_, i) => ({ l: g.home.cells[i] ?? '', r: g.away.cells[i] ?? '' }));
    sent = JSON.stringify(draft);
    editing = g.index;
    // A box after the first empty set: start at that empty set.
    const first = draft.findIndex((b) => !b.l && !b.r);
    await tick();
    const f = fields()[Math.min(k, first < 0 ? k : first) * 2];
    (f && !f.disabled ? f : fields()[0])?.focus();
  }
  function commit() {
    const now = JSON.stringify(draft);
    if (editing === null || !g || g.index !== editing || now === sent) return;
    sent = now;
    onsets?.(g, $state.snapshot(draft));
  }
  // The game ended (or another device changed the table): the fields go with it.
  $effect(() => {
    if (editing !== null && (g?.index !== editing || !canType)) editing = null;
  });

  function go(i: number) {
    const f = fields();
    if (i < 0) return;
    // After the last open set: leave the row, which saves the scores.
    if (i >= f.length || f[i].disabled) (document.activeElement as HTMLElement | null)?.blur();
    else f[i].focus();
  }
  function oninput(k: number, s: 0 | 1, e: Event) {
    const inp = e.currentTarget as HTMLInputElement;
    const v = inp.value.replace(/\D/g, '');
    inp.value = v;
    // No automatic jump after two digits: people type from the paper without a look at the
    // screen, so each score needs the same key (Tab, an arrow, the space bar) to go on.
    draft[k][key(s)] = v;
  }
  function onkey(k: number, s: 0 | 1, e: KeyboardEvent) {
    const inp = e.currentTarget as HTMLInputElement;
    const i = k * 2 + s;
    if (e.key === 'Escape') {
      // Drop what is not saved yet.
      e.preventDefault();
      draft = JSON.parse(sent);
      inp.blur();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      go((k + 1) * 2);
    } else if (e.key === ' ' || e.key === '-' || e.key === '/' || e.key === 'ArrowRight') {
      e.preventDefault();
      go(i + 1);
    } else if (e.key === 'ArrowLeft' || (e.key === 'Backspace' && !inp.value)) {
      e.preventDefault();
      go(i - 1);
    }
  }
  function leaveBox(e: FocusEvent) {
    if (!(e.target instanceof HTMLInputElement)) return;
    const to = e.relatedTarget as Node | null;
    if (!(e.currentTarget as HTMLElement).contains(to)) commit();
    if (!row?.contains(to)) editing = null;
  }
  /** A second send starts the game on the board again, so it asks first. */
  function send() {
    if (!g || !onsend) return;
    if (linked && !confirm('⚠️ Ben je zeker dat je de match opnieuw wil starten?')) return;
    onsend(g);
  }
  /** The sides in the order on screen. */
  const first = $derived((flip ? g?.away : g?.home)!);
  const second = $derived((flip ? g?.home : g?.away)!);
  const names = (s: GameSide) => s.people.map((p) => p.last).join(' / ') || '–';
</script>

{#snippet prow(side: GameSide, cls: 'h' | 'a', i: 0 | 1)}
  {@const lead = setsWon[i] > setsWon[1 - i]}
  {@const used = tmo ? (i === 0 ? tmo.p1Used : tmo.p2Used) : false}
  <div class="prow">
    <span class="letter {tone(cls, flip)}"></span>
    <span class="pname">
      {#if linked}<span class="serve" class:off={board?.server !== cls}></span>{/if}
      <span class="n">{side.people.map((p) => p.full).join(' / ') || 'Nog niet opgesteld'}</span>
      {#if side.people.length === 1 && side.people[0].ranking}
        {@const one = side.people[0]}
        {#if oninfo && one.id}
          <button class="rank" type="button" aria-label="Spelerinfo {one.full}, {one.ranking}" aria-haspopup="dialog" onclick={(e) => oninfo(e.currentTarget, one, cls === 'h' ? 'home' : 'away')}>{one.ranking}</button>
        {:else}
          <span class="rank">{one.ranking}</span>
        {/if}
      {/if}
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

{#snippet shown(k: number)}
  <small>S{k + 1}</small>
  {#if bsets?.[k]}
    {@const [a, b] = flip ? [bsets[k][1], bsets[k][0]] : bsets[k]}
    <span class:w={a > b}>{a}</span>–<span class:w={b > a}>{b}</span>
  {:else if !bsets && (first.cells[k] || second.cells[k])}
    <span class:w={first.won[k]}>{first.cells[k]}</span>–<span class:w={second.won[k]}>{second.cells[k]}</span>
  {:else if curBox === k && pts}
    {pts[flip ? 1 : 0]}<span class="sep">–</span>{pts[flip ? 0 : 1]}
  {:else if curBox === k}
    <span class="playing">Bezig</span>
  {:else}
    –
  {/if}
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
      {#if flip}
        {@render prow(g.away, 'a', 1)}
        {@render prow(g.home, 'h', 0)}
      {:else}
        {@render prow(g.home, 'h', 0)}
        {@render prow(g.away, 'a', 1)}
      {/if}
    </div>
    <div class="court-games" class:typing={editing !== null} style="--boxes: {boxes}" bind:this={row}>
      {#each Array.from({ length: boxes }, (_, k) => k) as k (k)}
        {#if editing !== null && draft[k]}
          <div class="gbox edit" class:off={k > decidedAt} role="group" aria-label="Set {k + 1}" onfocusout={leaveBox}>
            <small>S{k + 1}</small>
            <span class="pair">
              {#each [0, 1] as const as s (s)}
                {#if s}<span class="sep">–</span>{/if}
                <input
                  inputmode="numeric"
                  maxlength="2"
                  autocomplete="off"
                  enterkeyhint="next"
                  disabled={k > decidedAt}
                  aria-label="Set {k + 1}, {names(s === 0 ? first : second)}"
                  value={draft[k][key(s)]}
                  oninput={(e) => oninput(k, s, e)}
                  onkeydown={(e) => onkey(k, s, e)}
                  onfocus={(e) => e.currentTarget.select()}
                  onmousedown={(e) => {
                    // A click never puts the cursor inside the score: it selects the full score.
                    e.preventDefault();
                    e.currentTarget.focus();
                    e.currentTarget.select();
                  }}
                />
              {/each}
            </span>
          </div>
        {:else}
          {@const cur = curBox === k}
          {@const bad = !bsets && !!g.home.cells[k] && !!g.away.cells[k] && g.home.won[k] === null}
          {#if canType}
            <button type="button" class="gbox" class:cur class:bad title="Score invoeren" onclick={() => edit(k)}>{@render shown(k)}</button>
          {:else}
            <div class="gbox" class:cur class:bad>{@render shown(k)}</div>
          {/if}
        {/if}
      {/each}
    </div>
  {:else}
    <!-- No current game but a next one: the evening has not started yet. -->
    <p class="court-idle">{table.next ? 'Nog niet begonnen.' : 'Alle wedstrijden op deze tafel zijn gespeeld.'}</p>
  {/if}

  <div class="court-f">
    {#if table.next}<span>Volgende: <b>{names(flip ? table.next.away : table.next.home)} – {names(flip ? table.next.home : table.next.away)}</b></span>{/if}
    {#if g && editable}
      {#if table.device && onsend}
        <div class="btns">
          <button class="btn" class:ghost={linked} type="button" onclick={send}>
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
