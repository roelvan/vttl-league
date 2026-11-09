<script lang="ts">
  import VttlIcon from '../VttlIcon.svelte';
  import type { Sheet } from '../../lib/sheet/types';
  import type { LiveView } from '../../lib/live';
  import { vttlStatus, STATUS_TEXT } from '../../lib/sheet/vttl';

  let { sheet, view }: { sheet: Sheet; view: LiveView } = $props();

  const status = $derived(sheet.isOfficial ? vttlStatus(sheet, view.sys) : null);
  const upload = $derived(sheet.upload);
  const when = (t: number) => new Date(t).toLocaleTimeString('nl-BE', { hour: '2-digit', minute: '2-digit' });
  const person = (p: { name: string }) => p.name || '–';
</script>

<section class="card" aria-label="Wedstrijdgegevens">
  <div class="card-h"><h2>Wedstrijdgegevens</h2></div>
  <div class="card-b">
    <dl class="info">
      <dt>Zaal</dt><dd>{sheet.address.split('\n').join(', ') || '–'}</dd>
      <dt>Aanvang</dt><dd>{sheet.startHour || '–'}{sheet.endHour ? ` · einde ${sheet.endHour}` : ''}</dd>
      <dt>Tafels</dt><dd>{view.tables.map((t) => `${t.no}${t.device ? ' (scorebord)' : ''}`).join(' · ') || '–'}</dd>
      <dt>Formaat</dt><dd>{view.sys.name} · best of {view.sys.boxes}</dd>
      <dt>Scheidsrechter</dt><dd>{person(sheet.chiefReferee)}</dd>
      {#if status}
        <dt>VTTL</dt>
        <dd style="display:flex;gap:6px;justify-content:flex-end;align-items:center">
          <VttlIcon {status} />
          <span class="muted">{STATUS_TEXT[status]}</span>
        </dd>
      {/if}
      {#if upload}
        <dt>Laatste upload</dt>
        <dd class:error={!upload.ok}>
          {when(upload.at)} · {upload.partial ? 'tussenstand' : 'volledig'}{upload.dryRun ? ' (proef)' : ''}{upload.ok ? '' : ' · mislukt'}
        </dd>
      {/if}
    </dl>
    {#if sheet.comments}<p class="hint">{sheet.comments}</p>{/if}
  </div>
</section>
