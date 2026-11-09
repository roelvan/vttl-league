<script lang="ts">
  // Print the VTTL match sheet or the referee slips, with a preview.
  import Dialog from './Dialog.svelte';
  import PrintSheet from '../components/print/PrintSheet.svelte';
  import RefereeSlips from '../components/print/RefereeSlips.svelte';
  import { ws } from '../lib/workspace.svelte';
  import { SLIP_LAYOUTS, type PrintJob } from '../lib/sheet/print';
  import { validateSheet } from '../lib/validation';
  import { settings, saveSettings } from '../lib/settings.svelte';

  let { id, what, onclose, onprint }: { id: string; what: 'sheet' | 'slips'; onclose: () => void; onprint: (job: PrintJob) => void } = $props();

  const sheet = $derived(ws.entries[id]?.view);
  const errors = $derived(sheet && what === 'sheet' ? validateSheet(sheet) : []);
  let confirmErrors = $state(false);

  function go() {
    // ScoreSheet validates before it prints the match sheet.
    if (errors.length && !confirmErrors) {
      confirmErrors = true;
      return;
    }
    onprint({ id, what, layout: settings.slipLayout });
  }
</script>

<Dialog title={what === 'sheet' ? 'Wedstrijdblad afdrukken' : 'Scheidsrechterblaadjes'} chip={sheet?.matchId} {onclose} wide>
  {#if what === 'slips'}
    <label class="field">Schikking
      <select bind:value={settings.slipLayout} onchange={saveSettings}>
        {#each SLIP_LAYOUTS as [v, l] (v)}<option value={v}>{l}</option>{/each}
      </select>
    </label>
  {:else}
    <label class="check"><input type="checkbox" bind:checked={settings.autoUpload} onchange={saveSettings} /> Na het afdrukken de uitslag naar VTTL sturen (als ze geldig is)</label>
  {/if}
  {#if errors.length}
    <details class="banner warn" open={confirmErrors}>
      <summary>{errors.length} validatiefout(en){confirmErrors ? ': klik opnieuw op Afdrukken om toch af te drukken' : ''}</summary>
      <ul class="errors">{#each errors as e (e)}<li>{e}</li>{/each}</ul>
    </details>
  {/if}
  <div class="preview" class:portrait={what === 'slips'}>
    {#if sheet}
      {#if what === 'sheet'}<PrintSheet {sheet} />{:else}<RefereeSlips {sheet} layout={settings.slipLayout} />{/if}
    {/if}
  </div>
  <p class="hint" style="margin:0">Kies "Opslaan als PDF" in het afdrukvenster voor een PDF.</p>
  {#snippet footer()}
    <button class="btn ghost" type="button" onclick={onclose}>Annuleren</button>
    <button class="btn" type="button" onclick={go}>{confirmErrors ? 'Toch afdrukken' : 'Afdrukken'}</button>
  {/snippet}
</Dialog>
