<script lang="ts">
  // ScoreSheet "Exporteer wedstrijden": the TabT CSV file of the chosen sheets.
  // (PDF: use Printen and "Opslaan als PDF".)
  import Dialog from './Dialog.svelte';
  import { ws } from '../lib/workspace.svelte';
  import { getSystem } from '../lib/sheet/systems';
  import { exportCsv } from '../lib/sheet/export';

  let { onclose }: { onclose: () => void } = $props();

  const sheets = ws.open.map((id) => ws.entries[id]?.view).filter((s) => s && s.isOfficial);
  let picked = $state<Record<string, boolean>>(Object.fromEntries(sheets.map((s) => [s.id, true])));

  function download() {
    const chosen = sheets.filter((s) => picked[s.id]).map((sheet) => ({ sheet: $state.snapshot(sheet), sys: getSystem(sheet.matchSystemId) }));
    const blob = new Blob([exportCsv(chosen)], { type: 'text/csv;charset=utf-8' });
    const a = Object.assign(document.createElement('a'), { href: URL.createObjectURL(blob), download: `wedstrijden-${sheets[0]?.day ?? 'export'}.csv` });
    a.click();
    URL.revokeObjectURL(a.href);
    onclose();
  }
</script>

<Dialog title="Exporteer wedstrijden" {onclose}>
  <p class="muted small">Een CSV-bestand in het TabT-uploadformaat. Voor een PDF: kies Printen en "Opslaan als PDF".</p>
  {#each sheets as s (s.id)}
    <label class="check"><input type="checkbox" bind:checked={picked[s.id]} /> {s.home.name} – {s.away.name} <span class="mono muted">{s.matchId}</span></label>
  {:else}
    <p class="muted">Er zijn geen officiële wedstrijden open.</p>
  {/each}
  {#snippet footer()}
    <button class="ghost" onclick={onclose}>Annuleren</button>
    <button onclick={download} disabled={!sheets.some((s) => picked[s.id])}>CSV downloaden</button>
  {/snippet}
</Dialog>
