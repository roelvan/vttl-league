<script lang="ts">
  // The full VTTL log of one match, newest first. One short row per attempt; a problem row
  // shows its error below it. Tap a row for the account, the cause and the upload line.
  import Dialog from './Dialog.svelte';
  import VttlIcon from '../components/VttlIcon.svelte';
  import type { UploadLogEntry } from '../lib/api';
  import { logResult, SOURCE_TEXT } from '../lib/uploadLog';

  let { matchId, entries, error, onclose }: { matchId: string; entries: UploadLogEntry[] | null; error: string; onclose: () => void } = $props();

  let onlyProblems = $state(false);
  let open = $state<number | null>(null);

  const rows = $derived((entries ?? []).map((e, i) => ({ e, i, r: logResult(e) })));
  const shown = $derived(onlyProblems ? rows.filter((x) => x.r.status === 'error') : rows);
  const problems = $derived(rows.filter((x) => x.r.status === 'error').length);

  const today = new Date().toDateString();
  function when(t: number) {
    const d = new Date(t);
    const time = d.toLocaleTimeString('nl-BE', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
    return d.toDateString() === today ? time : `${d.toLocaleDateString('nl-BE', { weekday: 'short', day: '2-digit', month: '2-digit' })} ${time}`;
  }
</script>

<Dialog title="VTTL-logboek" chip={matchId} {onclose}>
  {#if error}
    <p class="error">{error}</p>
  {:else if !entries}
    <p class="muted">Laden…</p>
  {:else if !entries.length}
    <p class="muted">Leeg</p>
  {:else}
    <label class="check">
      <input type="checkbox" bind:checked={onlyProblems} />Alleen problemen ({problems})
    </label>
    {#if !shown.length}
      <p class="muted">Geen problemen.</p>
    {:else}
      <ol class="log-list">
        {#each shown as { e, i, r } (i)}
          <li>
            <button class="log-row" type="button" aria-expanded={open === i} onclick={() => (open = open === i ? null : i)}>
              <span class="mono muted">{when(e.at)}</span>
              <span class="muted">{e.partial ? 'tussenstand' : 'volledig'}</span>
              <span class="log-res" class:error={r.status === 'error'}><VttlIcon status={r.status} title={r.text} />{r.text}</span>
            </button>
            {#if r.detail}<p class="log-detail" class:error={r.status === 'error'}>{r.detail}</p>{/if}
            {#if open === i}
              <dl class="info log-more">
                <dt>Oorzaak</dt><dd>{SOURCE_TEXT[e.source] ?? e.source}</dd>
                <dt>Account</dt><dd>{e.account || '–'}</dd>
                {#if e.errors.length && r.status !== 'error'}<dt>VTTL</dt><dd>{e.errors.join('; ')}</dd>{/if}
              </dl>
              <pre class="mono log-data">{e.data}</pre>
            {/if}
          </li>
        {/each}
      </ol>
    {/if}
  {/if}
  {#snippet footer()}<button class="btn" type="button" onclick={onclose}>Sluiten</button>{/snippet}
</Dialog>
