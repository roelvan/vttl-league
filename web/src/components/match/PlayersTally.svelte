<script lang="ts">
  import type { Sheet } from '../../lib/sheet/types';
  import { tally, type LiveView, type Person, type Side, type TallyRow } from '../../lib/live';
  import { tone } from '../../lib/viewer.svelte';
  import Icon from '../Icon.svelte';

  let {
    sheet,
    view,
    picked = [],
    ontoggle,
    onhover,
    oninfo,
    flip = false,
  }: {
    sheet: Sheet;
    view: LiveView;
    /** Keys of the selected rows. */
    picked?: string[];
    /** Select or clear one row; its games stand out in the match order. */
    ontoggle?: (r: TallyRow) => void;
    /** Preview one row's games while the pointer or the focus is on it; null when it leaves. */
    onhover?: (r: TallyRow | null) => void;
    /** Opens the player widget next to `at`. Without it, the rows have no info button. */
    oninfo?: (at: HTMLElement, p: Person, side: Side) => void;
    /** The own club plays away: the away team comes first, in the home colour. */
    flip?: boolean;
  } = $props();
</script>

{#snippet table(side: Side)}
  {@const cls = tone(side === 'home' ? 'h' : 'a', flip)}
  <table class="tally">
    <thead><tr><th>{sheet[side].name}</th><th><span class="sr-only">Uitslagen</span></th><th>W–V</th></tr></thead>
    <tbody>
      {#each tally(sheet, view, side) as r (r.label)}
        {@const on = picked.includes(r.key)}
        <tr
          class:picked={on}
          onpointerenter={onhover ? (e) => e.pointerType === 'mouse' && onhover(r) : undefined}
          onpointerleave={onhover ? (e) => e.pointerType === 'mouse' && onhover(null) : undefined}
          onfocusin={onhover ? (e) => (e.target as Element).matches(':focus-visible') && onhover(r) : undefined}
          onfocusout={onhover ? () => onhover(null) : undefined}
        >
          <td>
            <div class="who-row">
              {#if ontoggle}
                <button class="who pick" type="button" aria-pressed={on} onclick={() => ontoggle(r)}>
                  <span class="rank-tag {cls}">{r.person.ranking || '–'}</span>
                  <span class="n">{r.person.full || '–'}</span>
                </button>
              {:else}
                <span class="who">
                  <span class="rank-tag {cls}">{r.person.ranking || '–'}</span>
                  <span class="n">{r.person.full || '–'}</span>
                </span>
              {/if}
              {#if oninfo && r.person.id}
                <button
                  class="who-info"
                  type="button"
                  aria-label="Spelerinfo {r.person.full}"
                  aria-haspopup="dialog"
                  onclick={(e) => oninfo(e.currentTarget, r.person, side)}
                >
                  <Icon name="info" size={18} />
                </button>
              {/if}
            </div>
          </td>
          <td><span class="pips">{#each r.pips as p, i (i)}<i class="pip {p}"></i>{/each}</span></td>
          <td class="wl">{r.wins}<span class="sep">-</span>{r.losses}</td>
        </tr>
      {/each}
    </tbody>
  </table>
{/snippet}

<section class="card" aria-label="Spelers">
  <div class="card-h"><h2>Spelers</h2></div>
  <div class="card-b tally-split">
    {@render table(flip ? 'away' : 'home')}
    {@render table(flip ? 'home' : 'away')}
  </div>
</section>
