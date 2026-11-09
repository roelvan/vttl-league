<script lang="ts">
  // The captain enters final set scores only (11–6), never point by point: that is the
  // scoreboard's job. Shows the VTTL upload notation as you type.
  import { untrack } from 'svelte';
  import Dialog from '../../dialogs/Dialog.svelte';
  import Icon from '../Icon.svelte';
  import type { Sheet, SetScore } from '../../lib/sheet/types';
  import type { GameView } from '../../lib/live';
  import { notation } from '../../lib/live';
  import { getSystem } from '../../lib/sheet/systems';
  import { gameResult, setWinner, isEmptySet } from '../../lib/sheet/score';
  import { gameNotation } from '../../lib/sheet/export';
  import { change } from '../../lib/workspace.svelte';

  let {
    id,
    sheet,
    game,
    editable,
    tableNo = null,
    boardBusy = false,
    onclose,
    onstats,
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
  let focus = $state(Math.min(initial.filter((s) => !isEmptySet(s)).length, boxCount - 1));

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

  const names = (side: 'home' | 'away') => game[side].people.map((p) => (p.initial ? `${p.initial} ${p.last}` : p.last)).join(' / ') || 'Nog niet opgesteld';

  function oninput(i: number, side: 'l' | 'r', e: Event) {
    const inp = e.currentTarget as HTMLInputElement;
    const v = inp.value.toUpperCase().replace(/[^0-9WOF]/g, '');
    inp.value = v;
    boxes[i][side] = v;
    left = left.filter((x) => x !== i);
    tried = false;
    pushSoon();
  }

  function quick(q: 'home11' | 'away11' | 'clear' | 'homeWO' | 'awayWO') {
    const b = boxes[focus];
    // A quick button takes the focus out of the row: the row is still being filled in.
    left = left.filter((x) => x !== focus);
    tried = false;
    if (q === 'clear') {
      b.l = '';
      b.r = '';
    } else if (q === 'home11') {
      b.l = String(sys.pointCount);
      document.getElementById(`g${focus}a`)?.focus();
    } else if (q === 'away11') {
      b.r = String(sys.pointCount);
      document.getElementById(`g${focus}h`)?.focus();
    } else {
      // WO: only in set 1, the other sets stay empty.
      boxes = boxes.map((_, i) => (i === 0 ? (q === 'homeWO' ? { l: 'WO', r: '' } : { l: '', r: 'WO' }) : { l: '', r: '' }));
      focus = 0;
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
</script>

<Dialog title="Wedstrijd {game.pos}" chip={tableNo ? `Tafel ${tableNo}${boardBusy ? ' · scorebord' : ''}` : game.state === 'final' ? 'Gespeeld' : 'Nog niet begonnen'} onclose={close}>
  <div class="vs">
    <div class="p"><span class="letter h"></span><span class="n">{names('home')}</span></div>
    <div class="sum" aria-label="{homeSets} tegen {awaySets} in sets"><span class="h">{homeSets}</span><span class="sep">–</span><span class="a">{awaySets}</span></div>
    <div class="p r"><span class="letter a"></span><span class="n">{names('away')}</span></div>
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
  <div class="glines" onkeydown={onkey}>
    {#each boxes as b, i (i)}
      {@const w = perSet[i]}
      <div class="gline" class:off={i > decidedAt} onfocusout={(e) => leaveRow(i, e)}>
        <label for="g{i}h">Set {i + 1}</label>
        <input
          id="g{i}h"
          inputmode="numeric"
          maxlength="2"
          autocomplete="off"
          aria-label="Set {i + 1} {names('home')}"
          value={b.l}
          readonly={readonly}
          class:won={w === 'home' || w === 'homeFF'}
          class:h={true}
          class:bad={bad[i]}
          aria-invalid={bad[i] || undefined}
          aria-describedby={showError ? 'score-error' : undefined}
          oninput={(e) => oninput(i, 'l', e)}
          onfocus={() => (focus = i)}
        />
        <span class="dash">–</span>
        <input
          id="g{i}a"
          inputmode="numeric"
          maxlength="2"
          autocomplete="off"
          aria-label="Set {i + 1} {names('away')}"
          value={b.r}
          readonly={readonly}
          class:won={w === 'away' || w === 'awayFF'}
          class:a={true}
          class:bad={bad[i]}
          aria-invalid={bad[i] || undefined}
          aria-describedby={showError ? 'score-error' : undefined}
          oninput={(e) => oninput(i, 'r', e)}
          onfocus={() => (focus = i)}
        />
      </div>
    {/each}
  </div>

  {#if !readonly}
    <div class="quick" role="group" aria-label="Snel invullen">
      <button type="button" onclick={() => quick('home11')}>{sys.pointCount} –</button>
      <button type="button" onclick={() => quick('away11')}>– {sys.pointCount}</button>
      <button type="button" onclick={() => quick('clear')}>Set wissen</button>
      <button type="button" onclick={() => quick('homeWO')}>Thuisspeler WO</button>
      <button type="button" onclick={() => quick('awayWO')}>Uitspeler WO</button>
    </div>
  {/if}

  <div role="alert">
    {#if showError}
      <p class="error" id="score-error">Deze score klopt niet. Een set gaat tot {sys.pointCount}, met 2 punten verschil.</p>
    {/if}
  </div>
  <div class="code"><span>VTTL-notatie</span><code>{code || '–'}</code></div>

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
</Dialog>
