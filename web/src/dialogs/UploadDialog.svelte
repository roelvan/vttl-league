<script lang="ts">
  // Manual upload of one match to VTTL. The server also uploads by itself after every change
  // (partial after each game, full when complete); this is for a retry or a correction.
  import Dialog from './Dialog.svelte';
  import VttlIcon from '../components/VttlIcon.svelte';
  import { ws, uploadSheet, type UploadResult } from '../lib/workspace.svelte';
  import { session } from '../lib/session.svelte';
  import { getSystem } from '../lib/sheet/systems';
  import { computeScore } from '../lib/sheet/score';
  import { vttlStatus, STATUS_TEXT } from '../lib/sheet/vttl';
  import { validateSheet } from '../lib/validation';

  let { id, onclose }: { id: string; onclose: () => void } = $props();

  const s = $derived(ws.entries[id]?.view);
  const sys = $derived(s ? getSystem(s.matchSystemId) : null);
  const sc = $derived(s && sys ? computeScore(s, sys) : null);
  const status = $derived(s && sys ? vttlStatus(s, sys) : null);
  const errors = $derived(s ? validateSheet(s) : []);
  const complete = $derived(sc && sc.winner !== 'incomplete' && sc.winner !== 'error');
  const canUpload = $derived(!!s && !!sys && s.isOfficial && sys.competitive && !s.vttlLocked);

  let result = $state<UploadResult | { ok: false; error: string } | null>(null);
  let busy = $state(false);
  let confirmErrors = $state(false);

  async function go() {
    if (errors.length && !confirmErrors) {
      confirmErrors = true;
      return;
    }
    busy = true;
    try {
      result = await uploadSheet(id);
    } catch (e) {
      result = { ok: false, error: e instanceof Error ? e.message : 'Upload mislukt' };
    } finally {
      busy = false;
    }
  }
</script>

<Dialog title="Upload naar VTTL" chip={s?.matchId} {onclose}>
  {#if !session.me?.uploadEnabled}
    <div class="banner warn">Proefmodus: de server stuurt niets naar VTTL. De uploadlijn wordt getoond en bewaard.</div>
  {/if}
  {#if s && sc && status}
    <dl class="info">
      <dt>Wedstrijd</dt><dd>{s.home.name} – {s.away.name}</dd>
      <dt>Uitslag</dt><dd>{sc.homeWins}–{sc.awayWins}{complete ? '' : ' (tussenstand)'}</dd>
      <dt>VTTL</dt><dd style="display:flex;gap:6px;justify-content:flex-end;align-items:center"><VttlIcon {status} /><span class="muted">{STATUS_TEXT[status]}</span></dd>
    </dl>
    {#if !canUpload}
      <p class="muted" style="margin:0">Deze wedstrijd kan niet naar VTTL (geen officiële competitie, of al gevalideerd).</p>
    {/if}
    {#if errors.length}
      <details class="banner warn" open={confirmErrors}>
        <summary>{errors.length} validatiefout(en){confirmErrors ? ': klik opnieuw om toch te uploaden' : ''}</summary>
        <ul class="errors">{#each errors as e (e)}<li>{e}</li>{/each}</ul>
      </details>
    {/if}
    {#if result}
      {#if 'error' in result}<p class="error" style="margin:0">{result.error}</p>
      {:else if !result.ok}<p class="error" style="margin:0">{result.errors.join('; ')}</p>
      {:else}
        <p class="ok" style="margin:0">{result.dryRun ? 'Proefupload OK' : 'Geüpload'}</p>
        <details><summary class="muted">Uploadlijn</summary><pre class="mono" style="white-space:pre-wrap;word-break:break-all">{result.data}</pre></details>
      {/if}
    {/if}
  {/if}
  {#snippet footer()}
    <button class="btn ghost" type="button" onclick={onclose}>{result ? 'Sluiten' : 'Annuleren'}</button>
    <button class="btn" type="button" onclick={go} disabled={busy || !canUpload}>
      {busy ? 'Bezig…' : confirmErrors ? 'Toch uploaden' : complete ? 'Volledige uitslag uploaden' : 'Tussenstand uploaden'}
    </button>
  {/snippet}
</Dialog>
