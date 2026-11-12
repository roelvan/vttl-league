<script lang="ts">
  // The VTTL log of one match: every upload attempt the server made (src/uploader.rs), newest
  // first, with the read-back result. Loaded when opened, and again after each new upload.
  import { onDestroy } from 'svelte';
  import VttlIcon from '../VttlIcon.svelte';
  import { api, type UploadLogEntry } from '../../lib/api';
  import { onServerEvent } from '../../lib/workspace.svelte';
  import { keyLabel } from '../../lib/sheet/vttl';

  let { id }: { id: string } = $props();

  let open = $state(false);
  let entries = $state<UploadLogEntry[] | null>(null);
  let error = $state('');

  async function load() {
    try {
      entries = await api.uploadLog(id);
      error = '';
    } catch (e) {
      error = e instanceof Error ? e.message : 'Laden mislukt';
    }
  }

  const off = onServerEvent((m) => {
    if (open && m.type === 'upload' && m.id === id) load();
  });
  onDestroy(off);

  const SOURCE: Record<string, string> = { auto: 'na wijziging', manual: 'uploadknop', retry: 'nieuwe poging' };
  const when = (t: number) =>
    new Date(t).toLocaleString('nl-BE', { weekday: 'short', day: '2-digit', month: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit' });

  function result(e: UploadLogEntry): { status: 'synced' | 'pending' | 'error'; text: string } {
    if (e.dryRun) return { status: 'pending', text: 'Proef: niets naar VTTL gestuurd' };
    if (e.confirmed === true) return { status: 'synced', text: 'Bevestigd: VTTL heeft deze gegevens' };
    if (e.confirmed === false) return { status: 'error', text: `VTTL toont andere gegevens: ${e.diff.map(keyLabel).join(', ')}` };
    if (!e.ok) return { status: 'error', text: `Mislukt: ${e.errors.join('; ') || 'onbekende fout'}` };
    return { status: 'pending', text: `Aanvaard door VTTL, niet gecontroleerd${e.checkError ? ` (${e.checkError})` : ''}` };
  }
</script>

<details class="vttl-log" bind:open ontoggle={() => open && load()}>
  <summary>VTTL-logboek</summary>
  {#if error}
    <p class="error">{error}</p>
  {:else if !entries}
    <p class="muted">Laden…</p>
  {:else if !entries.length}
    <p class="muted">Nog geen upload naar VTTL.</p>
  {:else}
    <ol class="log-list">
      {#each entries as e, i (i)}
        {@const r = result(e)}
        <li class="log-item">
          <VttlIcon status={r.status} title={r.text} />
          <div>
            <div class="muted">
              {when(e.at)} · {e.partial ? 'tussenstand' : 'volledig'} · {SOURCE[e.source] ?? e.source}{e.account ? ` · ${e.account}` : ''}
            </div>
            <div class:error={r.status === 'error'}>{r.text}</div>
            {#if e.confirmed !== null && !e.ok && e.errors.length}<div class="muted">{e.errors.join('; ')}</div>{/if}
            <details><summary class="muted">Uploadlijn</summary><pre class="mono">{e.data}</pre></details>
          </div>
        </li>
      {/each}
    </ol>
  {/if}
</details>
