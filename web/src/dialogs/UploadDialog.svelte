<script lang="ts">
  // The ScoreSheet "Upload" wizard (SelectMatches + MatchUploader): pick the open official
  // sheets, see their validation errors, upload them one by one.
  import Dialog from './Dialog.svelte';
  import VttlIcon from '../components/VttlIcon.svelte';
  import { ws, uploadSheet, type UploadResult } from '../lib/workspace.svelte';
  import { session } from '../lib/session.svelte';
  import { getSystem } from '../lib/sheet/systems';
  import { computeScore } from '../lib/sheet/score';
  import { vttlStatus } from '../lib/sheet/vttl';
  import { validateSheet } from '../lib/validation';

  let { onclose }: { onclose: () => void } = $props();

  const rows = ws.open
    .map((id) => ws.entries[id]?.view)
    .filter((s) => s && s.isOfficial && getSystem(s.matchSystemId).competitive)
    .map((s) => {
      const sys = getSystem(s.matchSystemId);
      const sc = computeScore(s, sys);
      return { id: s.id, s, sc, errors: validateSheet(s), status: vttlStatus(s, sys) };
    });

  let picked = $state<Record<string, boolean>>(Object.fromEntries(rows.map((r) => [r.id, r.status === 'pending' || r.status === 'error'])));
  let results = $state<Record<string, UploadResult | { ok: false; error: string }>>({});
  let busy = $state(false);
  let open = $state<string | null>(null);

  const chosen = $derived(rows.filter((r) => picked[r.id] && r.status !== 'locked'));

  async function go() {
    if (chosen.some((r) => r.errors.length) && !confirm('U heeft wedstrijden geselecteerd met validatiefouten in. Weet u zeker dat u wil verdergaan?')) return;
    busy = true;
    for (const r of chosen) {
      try {
        results[r.id] = await uploadSheet(r.id, false);
      } catch (e) {
        results[r.id] = { ok: false, error: e instanceof Error ? e.message : 'Upload mislukt' };
      }
    }
    busy = false;
  }

  const score = (r: (typeof rows)[number]) => `${r.sc.homeWins}-${r.sc.awayWins}`;
</script>

<Dialog title="Upload naar competitiewebsite" {onclose} wide>
  {#if !session.me?.uploadEnabled}
    <p class="locked-banner">Proefmodus: de server stuurt niets naar VTTL. De uploadlijn wordt getoond en bewaard. (Zet UPLOAD_ENABLED=true op de server voor echte uploads.)</p>
  {/if}
  {#if !rows.length}
    <p class="muted">Er zijn geen officiële competitiewedstrijden open.</p>
  {:else}
    <table class="up">
      <thead><tr><th></th><th></th><th>Wedstrijd</th><th>Uitslag</th><th>Validatie</th><th>Resultaat</th></tr></thead>
      <tbody>
        {#each rows as r (r.id)}
          {@const res = results[r.id]}
          <tr>
            <td><input type="checkbox" bind:checked={picked[r.id]} disabled={busy || r.status === 'locked'} aria-label="Selecteer {r.s.home.name}" /></td>
            <td><VttlIcon status={r.status} /></td>
            <td><b>{r.s.home.name}</b> – {r.s.away.name}<br /><span class="mono muted">{r.s.matchId}</span></td>
            <td>{r.sc.winner === 'incomplete' ? `${score(r)} (onvolledig)` : r.sc.winner === 'error' ? 'fout' : score(r)}</td>
            <td>
              {#if r.errors.length}
                <button class="link" onclick={() => (open = open === r.id ? null : r.id)}>{r.errors.length} fout(en)</button>
              {:else}<span class="ok">OK</span>{/if}
            </td>
            <td>
              {#if res}
                {#if 'error' in res}<span class="error">{res.error}</span>
                {:else if !res.ok}<span class="error">{res.errors.join('; ')}</span>
                {:else}<span class="ok">{res.dryRun ? 'Proef OK' : 'Geüpload'}</span>
                  <details><summary>Uploadlijn</summary><pre>{res.data}</pre></details>{/if}
              {/if}
            </td>
          </tr>
          {#if open === r.id}
            <tr><td colspan="6"><ul class="errors">{#each r.errors as e (e)}<li>{e}</li>{/each}</ul></td></tr>
          {/if}
        {/each}
      </tbody>
    </table>
  {/if}
  {#snippet footer()}
    <button class="ghost" onclick={onclose}>{Object.keys(results).length ? 'Sluiten' : 'Annuleren'}</button>
    <button onclick={go} disabled={busy || !chosen.length}>{busy ? 'Bezig…' : `Uploaden (${chosen.length})`}</button>
  {/snippet}
</Dialog>
