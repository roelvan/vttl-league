<script lang="ts">
  // Referee slips, A4 portrait: RefereeDefaultTemplate (2x4), RefereeHorizontalTemplate (1x8)
  // and RefereeTableTemplate (one page per table) of ScoreSheet.
  import type { Sheet } from '../../lib/sheet/types';
  import { getSystem } from '../../lib/sheet/systems';
  import { slipPages, type SlipLayout } from '../../lib/sheet/print';

  let { sheet, layout }: { sheet: Sheet; layout: SlipLayout } = $props();

  const pages = $derived(slipPages(sheet, getSystem(sheet.matchSystemId), layout));
</script>

{#each pages as page, pi (pi)}
  <div class="page portrait rs">
    {#if layout === '2x4'}
      <div class="rs-grid2x4">
        {#each Array(8) as _, i (i)}
          {@const s = page.slips[i]}
          <div class="rs-cell" class:rs-left={i < 4} style:grid-column={i < 4 ? 1 : 2} style:grid-row={(i % 4) + 1}>
            {#if s}
              <div class="rs-t">{s.title}</div>
              <div class="rs-players"><div>{s.home}</div><div>-</div><div>{s.away}</div></div>
              <div class="rs-boxes">{#each Array(10) as _, b (b)}<span></span>{/each}</div>
              <div class="rs-foot"><span>Match: {s.number}</span><span>Resultaat:</span></div>
              <div class="rs-ref">{s.referee ?? ''}</div>
            {/if}
            {#if i < 3}<div class="rs-scissors">✁</div>{/if}
          </div>
        {/each}
      </div>
    {:else}
      <div class="rs-rows" class:rs-titled={layout === 'table'}>
        {#if layout === 'table'}<div class="rs-ptitle">{page.title}</div>{/if}
        {#each Array(8) as _, i (i)}
          {@const s = page.slips[i]}
          <div class="rs-row">
            {#if s}
              <div class="rs-rt"><b>{s.title ?? ''}</b><span>Match: {s.number}</span></div>
              <div class="rs-rp"><span>{s.home}</span><span>&nbsp;&nbsp;-&nbsp;&nbsp;</span><span>{s.away}</span></div>
              <div class="rs-rsets"><span>Set 1:</span><span>Set 2:</span><span>Set 3:</span><span>Set 4:</span><span>Set 5:</span><span>Resultaat:</span></div>
              <div class="rs-ref">{s.referee ?? ''}</div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>
{/each}

<style>
  .rs { font-family: 'Segoe UI', Arial, sans-serif; font-size: 12px; color: #000; background: #fff; padding: 1.5cm 2cm; }
  .rs-grid2x4 { display: grid; grid-template-columns: 1fr 1fr; grid-template-rows: repeat(4, 1fr); height: 100%; }
  .rs-cell { position: relative; padding: 0.3cm 0.5cm 0; display: grid; grid-template-rows: auto 1fr auto auto auto; border: 0 solid #000; }
  .rs-cell.rs-left { border-right-width: 0.5px; }
  .rs-cell:not(:nth-child(4)):not(:nth-child(8)) { border-bottom-width: 0.5px; }
  .rs-t { font-size: 0.45cm; font-weight: 700; }
  .rs-players { display: grid; place-content: center; text-align: center; font-size: 0.35cm; }
  .rs-boxes { display: grid; grid-template-columns: repeat(5, 1fr); grid-template-rows: 1fr 1fr; height: 1.5cm; }
  .rs-boxes span { border: 0.5px solid #000; }
  .rs-foot { display: grid; grid-template-columns: 2fr 3fr; margin: 0.2cm; font-size: 0.35cm; }
  .rs-ref { font-size: 0.25cm; min-height: 0.35cm; }
  .rs-scissors { position: absolute; left: -0.2cm; bottom: -0.32cm; font-size: 0.5cm; background: #fff; line-height: 1; }
  .rs-rows { display: grid; grid-template-rows: repeat(8, 1fr); height: 100%; }
  .rs-rows.rs-titled { grid-template-rows: auto repeat(8, 1fr); }
  .rs-ptitle { font-size: 0.6cm; font-weight: 700; }
  .rs-row { padding: 0.3cm 0.5cm 0; display: grid; grid-template-rows: auto 1fr auto auto; border-bottom: 1px solid #000; }
  .rs-row:last-child { border-bottom: 0; }
  .rs-rt { display: flex; justify-content: space-between; font-size: 0.45cm; }
  .rs-rt span { font-size: 0.35cm; }
  .rs-rp { display: grid; grid-template-columns: 1fr auto 1fr; align-items: center; text-align: center; font-size: 0.35cm; }
  .rs-rsets { display: grid; grid-template-columns: repeat(6, 1fr); font-size: 0.35cm; }
</style>
