<script lang="ts">
  // ScoreSheet's Print wizard: the match sheet or the referee slips, with a preview.
  import Dialog from './Dialog.svelte';
  import PrintSheet from '../components/print/PrintSheet.svelte';
  import RefereeSlips from '../components/print/RefereeSlips.svelte';
  import { ws } from '../lib/workspace.svelte';
  import { SLIP_LAYOUTS, type PrintJob } from '../lib/sheet/print';
  import { validateSheet } from '../lib/validation';
  import { settings, saveSettings } from '../lib/settings.svelte';

  let { id, onclose, onprint }: { id: string; onclose: () => void; onprint: (job: PrintJob) => void } = $props();

  const sheet = $derived(ws.entries[id]?.view);
  let what = $state<'sheet' | 'slips'>('sheet');
  const errors = $derived(sheet && what === 'sheet' ? validateSheet(sheet) : []);

  function go() {
    // ScoreSheet validates before it prints the match sheet.
    if (errors.length && !confirm(`De wedstrijd die u wil afdrukken bevat validatiefouten:\n\n${errors.join('\n')}\n\nWeet u zeker dat u de wedstrijd wil afdrukken?`)) return;
    onprint({ id, what, layout: settings.slipLayout });
  }
</script>

<Dialog title="Afdrukken – {sheet?.home.name} – {sheet?.away.name}" {onclose} wide>
  <p class="muted">Selecteer wat u van de geselecteerde wedstrijd wil afdrukken:</p>
  <div class="pr-opts">
    <label class="check"><input type="radio" bind:group={what} value="sheet" /> Wedstrijdblad</label>
    <label class="check"><input type="radio" bind:group={what} value="slips" /> Scheidsrechterblaadjes</label>
    {#if what === 'slips'}
      <select bind:value={settings.slipLayout} onchange={saveSettings}>
        {#each SLIP_LAYOUTS as [v, l] (v)}<option value={v}>{l}</option>{/each}
      </select>
    {/if}
  </div>
  {#if errors.length}
    <details class="pr-errors"><summary>{errors.length} validatiefout(en)</summary><ul class="errors">{#each errors as e (e)}<li>{e}</li>{/each}</ul></details>
  {/if}
  <div class="preview" class:portrait={what === 'slips'}>
    {#if sheet}
      {#if what === 'sheet'}<PrintSheet {sheet} />{:else}<RefereeSlips {sheet} layout={settings.slipLayout} />{/if}
    {/if}
  </div>
  <p class="muted small">Kies "Opslaan als PDF" in het afdrukvenster van de browser voor een PDF.</p>
  {#snippet footer()}
    <button class="ghost" onclick={onclose}>Annuleren</button>
    <button onclick={go}>Afdrukken</button>
  {/snippet}
</Dialog>
