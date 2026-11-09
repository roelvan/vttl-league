<script lang="ts">
  import type { Sheet } from '../../lib/sheet/types';
  import { tally, type LiveView, type Side, type TallyRow } from '../../lib/live';

  let {
    sheet,
    view,
    picked = [],
    ontoggle,
    onhover,
  }: {
    sheet: Sheet;
    view: LiveView;
    /** Keys of the selected rows. */
    picked?: string[];
    /** Select or clear one row; its games stand out in the match order. */
    ontoggle?: (r: TallyRow) => void;
    /** Preview one row's games while the pointer or the focus is on it; null when it leaves. */
    onhover?: (r: TallyRow | null) => void;
  } = $props();
</script>

{#snippet table(side: Side)}
  {@const cls = side === 'home' ? 'h' : 'a'}
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
          </td>
          <td><span class="pips">{#each r.pips as p, i (i)}<i class="pip {p}"></i>{/each}</span></td>
          <td class="wl">{r.wins}–{r.losses}</td>
        </tr>
      {/each}
    </tbody>
  </table>
{/snippet}

<section class="card" aria-label="Spelers">
  <div class="card-h"><h2>Spelers</h2></div>
  <div class="card-b tally-split">
    {@render table('home')}
    {@render table('away')}
  </div>
</section>
