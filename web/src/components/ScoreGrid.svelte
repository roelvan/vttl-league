<script lang="ts">
  import type { Sheet } from '../lib/sheet/types';
  import type { MatchSystem } from '../lib/sheet/systems';
  import { slotLabel } from '../lib/sheet/systems';
  import { gameSlots, type Score } from '../lib/sheet/score';
  import { change } from '../lib/workspace.svelte';

  let { id, sheet, sys, score }: { id: string; sheet: Sheet; sys: MatchSystem; score: Score } = $props();

  let grid: HTMLElement;

  /** Label of the players in a game: "4", "Y", "D" (double), "1+2" (free-time double). */
  function who(gi: number, side: 'home' | 'away') {
    const slots = gameSlots(sheet, sys, gi, side);
    return slots
      .map((s) => {
        const slot = sys.slots[s];
        return slot.kind === 'double' ? 'D' : slotLabel(sys, s, side === 'away');
      })
      .join('+');
  }

  function input(gi: number, b: number, side: 'l' | 'r', e: Event) {
    const el = e.currentTarget as HTMLInputElement;
    // Only digits, WO and FF make sense; keep what the user typed but in capitals.
    const v = el.value.toUpperCase().replace(/[^0-9WOF]/g, '');
    if (v !== el.value) el.value = v;
    change(id, [{ p: ['results', gi, b, side], v }]);
  }

  function box(g: number, b: number, side: 'l' | 'r') {
    return grid.querySelector<HTMLInputElement>(`input[data-g="${g}"][data-b="${b}"][data-s="${side}"]`);
  }

  function focus(el: HTMLInputElement | null, atEnd: boolean) {
    if (!el) return false;
    el.focus();
    const n = atEnd ? el.value.length : 0;
    el.setSelectionRange(n, n);
    return true;
  }

  /** Keyboard moves of ScoreSheet's FastMatches grid. */
  function keydown(e: KeyboardEvent) {
    const el = e.currentTarget as HTMLInputElement;
    const g = Number(el.dataset.g);
    const b = Number(el.dataset.b);
    const left = el.dataset.s === 'l';
    const last = sys.boxes - 1;
    const next = () => focus(left ? box(g, b, 'r') : b === last ? box(g + 1, 0, 'l') : box(g, b + 1, 'l'), false);
    const prev = () => focus(left ? (b === 0 ? box(g - 1, last, 'r') : box(g, b - 1, 'r')) : box(g, b, 'l'), true);
    const caretStart = el.selectionStart === 0 && el.selectionEnd === 0;
    const caretEnd = el.selectionStart === el.value.length;
    let moved = false;
    switch (e.key) {
      case 'Tab':
        moved = e.shiftKey ? prev() : next();
        break;
      case 'ArrowLeft':
        if (caretStart) moved = prev();
        break;
      case 'ArrowRight':
        if (caretEnd) moved = next();
        break;
      case '-':
      case 'Subtract':
        moved = next() || true; // "-" is never part of a score
        break;
      case 'Enter':
        moved = focus(box(g + 1, 0, 'l'), false);
        break;
      case 'ArrowUp':
        moved = focus(box(g - 1, b, left ? 'l' : 'r'), false);
        break;
      case 'ArrowDown':
        moved = focus(box(g + 1, b, left ? 'l' : 'r'), false);
        break;
      case 'Backspace':
        if (el.value.length === 0) moved = prev();
        break;
    }
    if (moved) e.preventDefault();
  }

  const sets = $derived(Array.from({ length: sys.boxes }, (_, i) => i));
</script>

<div class="grid" bind:this={grid} style:--boxes={sys.boxes}>
  <div class="g-title">VOLGORDE DER WEDSTRIJDEN</div>
  <div class="g-head">
    <span class="g-n"></span><span class="g-p"></span><span class="g-p"></span>
    <span class="g-pts">BEHAALDE PUNTEN</span>
    <span class="g-tot">SETS</span>
    <span class="g-tot">WEDSTR</span>
  </div>
  {#each sys.games as game, gi (game.pos)}
    {@const row = score.games[gi]}
    {@const won = row?.result.won}
    <div class="g-row" class:g-done={won && won !== 'empty' && won !== 'partial' && won !== 'error'}>
      <span class="g-n">{game.pos}</span>
      <span class="g-p">{who(gi, 'home')}</span>
      <span class="g-p">{who(gi, 'away')}</span>
      <span class="g-sets">
        {#each sets as b (b)}
          {@const s = sheet.results[gi]?.[b]}
          {@const ok = row?.result.valid[b] ?? true}
          <span class="g-set" class:bad={!ok}>
            <input
              value={s?.l ?? ''}
              data-g={gi}
              data-b={b}
              data-s="l"
              inputmode="text"
              autocomplete="off"
              aria-label="Wedstrijd {game.pos} set {b + 1} thuis"
              oninput={(e) => input(gi, b, 'l', e)}
              onkeydown={keydown}
            /><input
              value={s?.r ?? ''}
              data-g={gi}
              data-b={b}
              data-s="r"
              inputmode="text"
              autocomplete="off"
              aria-label="Wedstrijd {game.pos} set {b + 1} uit"
              oninput={(e) => input(gi, b, 'r', e)}
              onkeydown={keydown}
            />
          </span>
        {/each}
      </span>
      <span class="g-tot">
        <b>{won && won !== 'empty' && won !== 'error' ? row.result.homeSets : ''}</b><b>{won && won !== 'empty' && won !== 'error' ? row.result.awaySets : ''}</b>
      </span>
      <span class="g-tot">
        <b>{row?.homeTotal ?? ''}</b><b>{row?.awayTotal ?? ''}</b>
      </span>
    </div>
  {/each}
</div>
