<script lang="ts">
  // The captain enters final set scores only (11–6), never point by point: that is the
  // scoreboard's job. Shows the VTTL upload notation as you type.
  import { untrack } from 'svelte';
  import Dialog from '../../dialogs/Dialog.svelte';
  import PlayerPopover, { whoOf } from './PlayerPopover.svelte';
  import Icon from '../Icon.svelte';
  import type { Sheet, SetScore } from '../../lib/sheet/types';
  import type { GameView } from '../../lib/live';
  import { notation } from '../../lib/live';
  import { getSystem } from '../../lib/sheet/systems';
  import { gameResult, setWinner, isEmptySet } from '../../lib/sheet/score';
  import { gameNotation } from '../../lib/sheet/export';
  import { change } from '../../lib/workspace.svelte';
  import { tone } from '../../lib/viewer.svelte';

  let {
    id,
    sheet,
    game,
    editable,
    tableNo = null,
    boardBusy = false,
    onclose,
    onstats,
    flip = false,
  }: {
    id: string;
    sheet: Sheet;
    game: GameView;
    editable: boolean;
    tableNo?: string | null;
    /** The game is live on a scoreboard: read-only until it ends. */
    boardBusy?: boolean;
    onclose: () => void;
    /** Set when a scoreboard counted this game: opens its statistics (after a save). */
    onstats?: () => void;
    /** The own club plays away: the away player comes first, in the home colour. */
    flip?: boolean;
  } = $props();

  const sys = $derived(getSystem(sheet.matchSystemId));
  const readonly = $derived(!editable || boardBusy);
  // Every set goes to the server while you type (after a short pause), so all devices see it.
  // A change from another device is taken over when this device has nothing waiting to send.
  const boxCount = untrack(() => getSystem(sheet.matchSystemId).boxes);
  const fill = (src: SetScore[] | undefined) => Array.from({ length: boxCount }, (_, i) => ({ l: src?.[i]?.l ?? '', r: src?.[i]?.r ?? '' }));
  const initial = untrack(() => fill($state.snapshot(sheet.results[game.index]) as SetScore[]));
  let boxes = $state<SetScore[]>(structuredClone(initial));
  let sent = JSON.stringify(initial);
  let timer: ReturnType<typeof setTimeout> | undefined;

  function push() {
    clearTimeout(timer);
    timer = undefined;
    const now = JSON.stringify(boxes);
    if (readonly || now === sent) return;
    sent = now;
    change(id, [{ p: ['results', game.index], v: $state.snapshot(boxes) }]);
  }
  function pushSoon() {
    clearTimeout(timer);
    timer = setTimeout(push, 700);
  }

  $effect(() => {
    const remote = JSON.stringify(fill($state.snapshot(sheet.results[game.index]) as SetScore[]));
    untrack(() => {
      if (timer || remote === sent) return;
      sent = remote;
      boxes = JSON.parse(remote);
    });
  });

  const result = $derived(gameResult(boxes, sys, sheet.home.forfeit, sheet.away.forfeit));
  const perSet = $derived(boxes.map((s, i) => (isEmptySet(s) ? 'empty' : setWinner(i, s, sys))));
  // A half-typed set is wrong, so errors show only for rows you left, or after "Klaar".
  // Typing in a row hides its error again until you leave it.
  let left = $state<number[]>([]);
  let tried = $state(false);
  const shown = (i: number) => tried || left.includes(i);
  const bad = $derived(perSet.map((w, i) => w === 'error' && shown(i)));
  const showError = $derived(tried ? result.won === 'error' : bad.some(Boolean));
  function leaveRow(i: number, e: FocusEvent) {
    const row = e.currentTarget as HTMLElement;
    if (!row.contains(e.relatedTarget as Node | null) && !left.includes(i)) left.push(i);
  }
  const homeSets = $derived(perSet.filter((w) => w === 'home' || w === 'homeFF').length);
  const awaySets = $derived(perSet.filter((w) => w === 'away' || w === 'awayFF').length);
  /** Sets after the decided one are switched off. */
  const decidedAt = $derived.by(() => {
    let h = 0, a = 0;
    for (let i = 0; i < perSet.length; i++) {
      if (perSet[i] === 'home') h++;
      if (perSet[i] === 'away') a++;
      if (h === sys.setCount || a === sys.setCount) return i;
    }
    return Infinity;
  });
  const code = $derived(
    result.won === 'empty' || result.won === 'partial' || result.won === 'error'
      ? notation(boxes.map((s) => ({ h: Number(s.l), a: Number(s.r) })).filter((s) => Number.isFinite(s.h) && Number.isFinite(s.a)))
      : gameNotation(result.won, boxes, sys),
  );

  /** The sides in the order on screen. */
  const order = $derived<['home' | 'away', 'home' | 'away']>(flip ? ['away', 'home'] : ['home', 'away']);
  const sets = $derived(flip ? [awaySets, homeSets] : [homeSets, awaySets]);
  const names = (side: 'home' | 'away') => game[side].people.map((p) => p.full).join(' / ') || 'Nog niet opgesteld';

  function oninput(i: number, side: 'l' | 'r', e: Event) {
    const inp = e.currentTarget as HTMLInputElement;
    const v = inp.value.toUpperCase().replace(/[^0-9WOF]/g, '');
    inp.value = v;
    boxes[i][side] = v;
    left = left.filter((x) => x !== i);
    tried = false;
    pushSoon();
    // No automatic jump after two characters: people type from the paper without a look at the
    // screen, so each score needs the same key (Tab, an arrow, the space bar) to go on.
  }

  // The same keys as in the set boxes of a table card: a field always has its full score
  // selected, and the arrows, the space bar and the hyphen go from field to field.
  let lines = $state<HTMLElement>();
  function go(n: number) {
    const f = lines?.querySelectorAll('input');
    // A set after the one that decides the game is closed: the cursor stays where it is.
    if (f && n >= 0 && n < f.length && !f[n].disabled) f[n].focus();
  }
  function onfieldkey(i: number, col: number, e: KeyboardEvent) {
    const inp = e.currentTarget as HTMLInputElement;
    const n = i * 2 + col;
    const to =
      e.key === ' ' || e.key === '-' || e.key === '/' || e.key === 'ArrowRight' ? n + 1
      : e.key === 'ArrowLeft' || (e.key === 'Backspace' && !inp.value) ? n - 1
      : e.key === 'ArrowDown' ? n + 2
      : e.key === 'ArrowUp' ? n - 2
      : null;
    if (to === null) return;
    e.preventDefault();
    go(to);
  }

  function clearSet(i: number) {
    boxes[i].l = '';
    boxes[i].r = '';
    left = left.filter((x) => x !== i);
    tried = false;
    pushSoon();
    go(i * 2);
  }

  function quick(q: 'clearAll' | 'homeWO' | 'awayWO') {
    tried = false;
    if (q === 'clearAll') {
      boxes = boxes.map(() => ({ l: '', r: '' }));
      left = [];
      document.getElementById(`g0${flip ? 'a' : 'h'}`)?.focus();
    } else {
      // WO: only in set 1, the other sets stay empty.
      boxes = boxes.map((_, i) => (i === 0 ? (q === 'homeWO' ? { l: 'WO', r: '' } : { l: '', r: 'WO' }) : { l: '', r: '' }));
    }
    pushSoon();
  }

  /** "Klaar": a wrong score keeps the dialog open and shows why (it is saved anyway). */
  function save() {
    push();
    if (result.won === 'error') {
      tried = true;
      return;
    }
    onclose();
  }

  function close() {
    push();
    onclose();
  }

  function onkey(e: KeyboardEvent) {
    if (e.key === 'Enter' && !readonly) {
      e.preventDefault();
      save();
    }
  }
  /** The player widget of this dialog, opened from a ranking. */
  let info: ReturnType<typeof PlayerPopover> | undefined = $state();
</script>

<!-- The ranking in the team colour, as on the match cards. A double: the two ranks, one above the other. -->
{#snippet rank(side: 'home' | 'away')}
  {@const people = game[side].people}
  {@const one = people.length === 1 && people[0].id ? people[0] : null}
  {#if one}
    <button
      class="rank-tag {tone(side === 'home' ? 'h' : 'a', flip)}"
      type="button"
      aria-label="Spelerinfo {one.full}, {one.ranking || 'geen klassement'}"
      aria-haspopup="dialog"
      onclick={(e) => info?.show(e.currentTarget, whoOf(one, side, sheet[side].name, flip))}
    ><span>{one.ranking || '–'}</span></button>
  {:else}
    <span class="rank-tag {tone(side === 'home' ? 'h' : 'a', flip)}" class:pair={people.length > 1}>
      {#each people.length ? people : [null] as p, i (i)}<span>{p?.ranking || '–'}</span>{/each}
    </span>
  {/if}
{/snippet}

<!-- One set score of a side. "l" is always the home score in the record. -->
{#snippet box(i: number, b: SetScore, w: (typeof perSet)[number], side: 'home' | 'away')}
  {@const cls = side === 'home' ? 'h' : 'a'}
  <input
    id="g{i}{cls}"
    inputmode="numeric"
    maxlength="2"
    autocomplete="off"
    aria-label="Set {i + 1} {names(side)}"
    value={side === 'home' ? b.l : b.r}
    readonly={readonly}
    disabled={!readonly && i > decidedAt}
    class="{tone(cls, flip)}"
    class:won={w === side || w === `${side}FF`}
    class:bad={bad[i]}
    aria-invalid={bad[i] || undefined}
    aria-describedby={showError ? 'score-error' : undefined}
    oninput={(e) => oninput(i, side === 'home' ? 'l' : 'r', e)}
    onfocus={(e) => e.currentTarget.select()}
    onmousedown={(e) => {
      // A click never puts the cursor inside the score: it selects the full score.
      e.preventDefault();
      e.currentTarget.focus();
      e.currentTarget.select();
    }}
    onkeydown={(e) => onfieldkey(i, (side === 'home') !== flip ? 0 : 1, e)}
  />
{/snippet}

<Dialog title="Wedstrijd {game.pos}" chip={tableNo ? `Tafel ${tableNo}${boardBusy ? ' · scorebord' : ''}` : game.state === 'final' ? 'Gespeeld' : 'Nog niet begonnen'} onclose={close}>
  <div class="vs">
    <div class="p">{@render rank(order[0])}<span class="n">{names(order[0])}</span></div>
    <div class="sum" aria-label="{sets[0]} tegen {sets[1]} in sets"><span class="h">{sets[0]}</span><span class="sep">–</span><span class="a">{sets[1]}</span></div>
    <div class="p r">{@render rank(order[1])}<span class="n">{names(order[1])}</span></div>
  </div>

  {#if boardBusy}
    <div class="readonly-note">
      <Icon name="board" />
      <span>De scores op tafel {tableNo} komen van het scorebord. Na de wedstrijd kan een kapitein ze nog verbeteren.</span>
    </div>
  {:else if !editable}
    <div class="readonly-note"><Icon name="info" /><span>Alleen lezen.</span></div>
  {/if}

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="glines" class:can-clear={!readonly} onkeydown={onkey} bind:this={lines}>
    {#each boxes as b, i (i)}
      {@const w = perSet[i]}
      <div class="gline" class:off={i > decidedAt} onfocusout={(e) => leaveRow(i, e)}>
        <label for="g{i}{flip ? 'a' : 'h'}">Set {i + 1}</label>
        {@render box(i, b, w, flip ? 'away' : 'home')}
        <span class="dash">–</span>
        {@render box(i, b, w, flip ? 'home' : 'away')}
        {#if !readonly}
          <!-- Not a tab stop: Tab goes from score to score. With the keyboard, type over the score. -->
          <button class="clr" type="button" tabindex="-1" aria-label="Set {i + 1} wissen" title="Set {i + 1} wissen" onclick={() => clearSet(i)}>
            <Icon name="close" size={16} />
          </button>
        {/if}
      </div>
    {/each}
  </div>

  {#if !readonly}
    <!-- The WO buttons are in the sequence of the players on screen. A set is cleared with the
         button at the end of its row. -->
    <div class="quick" role="group" aria-label="WO en wissen">
      {#each order as side (side)}
        <button type="button" aria-label="{side === 'home' ? 'Thuisspeler' : 'Uitspeler'} WO: {names(side)}" onclick={() => quick(`${side}WO`)}>{side === 'home' ? 'Thuisspeler' : 'Uitspeler'} WO</button>
      {/each}
      <button type="button" onclick={() => quick('clearAll')} disabled={boxes.every(isEmptySet)}>Alles wissen</button>
    </div>
  {/if}

  <!-- One block: the alert region stays in the page for screen readers, but when it is empty
       it must not take a row (and a second gap) of the dialog grid. -->
  <div class="outcome">
    <div role="alert">
      {#if showError}
        <p class="error" id="score-error">Deze score klopt niet. Een set gaat tot {sys.pointCount}, met 2 punten verschil.</p>
      {/if}
    </div>
    <div class="code"><span>VTTL-notatie</span><code>{code || '–'}</code></div>
  </div>

  {#snippet footer()}
    {#if onstats}
      <button class="btn big ghost" type="button" onclick={() => (push(), onstats())}><Icon name="stats" />Statistieken</button>
    {/if}
    {#if readonly}
      <button class="btn big" type="button" onclick={onclose}>Sluiten</button>
    {:else}
      <button class="btn big" type="button" onclick={save}>Klaar</button>
    {/if}
  {/snippet}
  <!-- Inside the dialog: the page behind a modal dialog is inert, and its widget with it. -->
  <PlayerPopover bind:this={info} />
</Dialog>
