<script lang="ts">
  import type { Sheet } from '../../lib/sheet/types';
  import { tally, type LiveView, type Side } from '../../lib/live';

  let { sheet, view }: { sheet: Sheet; view: LiveView } = $props();
</script>

{#snippet table(side: Side)}
  {@const cls = side === 'home' ? 'h' : 'a'}
  <table class="tally">
    <thead><tr><th>{sheet[side].name}</th><th></th><th style="text-align:right">W–V</th></tr></thead>
    <tbody>
      {#each tally(sheet, view, side) as r (r.label)}
        <tr>
          <td>
            <span class="who">
              <span class="letter {cls}">{r.label}</span>
              <span class="n">{r.person.full || '–'}</span>
              {#if r.person.ranking}<span class="rank">{r.person.ranking}</span>{/if}
            </span>
          </td>
          <td><span class="pips">{#each r.pips as p, i (i)}<i class="pip {p}"></i>{/each}</span></td>
          <td class="wl">{r.wins}–{r.losses}</td>
        </tr>
      {/each}
    </tbody>
  </table>
{/snippet}

<section class="card" aria-label="Spelers vanavond">
  <div class="card-h"><h2>Spelers vanavond</h2></div>
  <div class="card-b tally-split">
    {@render table('home')}
    {@render table('away')}
  </div>
</section>
