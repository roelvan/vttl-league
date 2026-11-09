<script lang="ts">
  import VttlIcon from '../VttlIcon.svelte';
  import type { Sheet } from '../../lib/sheet/types';
  import type { LiveView } from '../../lib/live';
  import { vttlStatus, STATUS_TEXT } from '../../lib/sheet/vttl';
  import Icon from '../Icon.svelte';
  import type { MenuAction } from '../../lib/nav';

  let {
    sheet,
    view,
    onaction,
    canUpload = false,
  }: {
    sheet: Sheet;
    view: LiveView;
    /** Set for home-club members: shows the print buttons. */
    onaction?: (a: MenuAction) => void;
    canUpload?: boolean;
  } = $props();

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
    {#if onaction}
      <div class="detail-actions" class:two={!canUpload}>
        <button class="btn ghost" type="button" onclick={() => onaction('print')}><Icon name="print" size={16} />Wedstrijdblad</button>
        <button class="btn ghost" type="button" onclick={() => onaction('slips')}><Icon name="print" size={16} />Blaadjes</button>
        {#if canUpload}<button class="btn ghost" type="button" onclick={() => onaction('upload')}><Icon name="upload" size={16} />Upload</button>{/if}
      </div>
    {/if}
  </div>
</section>
