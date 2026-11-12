<script lang="ts">
  import VttlIcon from '../VttlIcon.svelte';
  import type { Sheet } from '../../lib/sheet/types';
  import type { LiveView } from '../../lib/live';
  import { vttlStatus, STATUS_TEXT } from '../../lib/sheet/vttl';
  import Icon from '../Icon.svelte';
  import type { MenuAction } from '../../lib/nav';
  import { deviceAlias } from '../../lib/boards.svelte';
  import { publicClub } from '../../lib/viewer.svelte';
  import { onDestroy } from 'svelte';
  import { api, type UploadLogEntry } from '../../lib/api';
  import { onServerEvent } from '../../lib/workspace.svelte';
  import { logResult } from '../../lib/uploadLog';
  import UploadLogDialog from '../../dialogs/UploadLogDialog.svelte';

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
  // The VTTL log: loaded with the match for the count on the button, again after each upload.
  const logId = $derived(sheet.isOfficial && !publicClub() ? sheet.id : null);
  let log = $state<UploadLogEntry[] | null>(null);
  let logError = $state('');
  let showLog = $state(false);
  async function loadLog(id: string) {
    try {
      const l = await api.uploadLog(id);
      if (id === logId) [log, logError] = [l, ''];
    } catch (e) {
      logError = e instanceof Error ? e.message : 'Laden mislukt';
    }
  }
  $effect(() => {
    log = null;
    if (logId) loadLog(logId);
  });
  onDestroy(onServerEvent((m) => {
    if (logId && m.type === 'upload' && m.id === logId) loadLog(logId);
  }));
  const lastFailed = $derived(!!log?.length && logResult(log[0]).status === 'error');

  const check = $derived(
    !upload || upload.dryRun ? '' : upload.confirmed === true ? ' · bevestigd door VTTL' : upload.confirmed === false ? ' · VTTL toont andere gegevens' : upload.ok ? ' · niet gecontroleerd' : '',
  );
</script>

<section class="card" aria-label="Wedstrijdgegevens">
  <div class="card-h"><h2>Wedstrijdgegevens</h2></div>
  <div class="card-b">
    <dl class="info">
      <dt>Zaal</dt><dd>{sheet.address.split('\n').join(', ') || '–'}</dd>
      <dt>Aanvang</dt><dd>{sheet.startHour || '–'}{sheet.endHour ? ` · einde ${sheet.endHour}` : ''}</dd>
      <dt>Tafels</dt><dd>{view.tables.map((t) => `${t.no}: ${t.device ? deviceAlias(t.device) : 'manueel'}`).join(' · ')}</dd>
      <dt>Formaat</dt><dd>{view.sys.name} · best of {view.sys.boxes}</dd>
      <dt>Scheidsrechter</dt><dd>{person(sheet.chiefReferee)}</dd>
      {#if status}
        <dt>VTTL</dt>
        <dd class="with-icon">
          <VttlIcon {status} />
          <span class="muted">{STATUS_TEXT[status]}</span>
        </dd>
      {/if}
      {#if upload}
        <dt>Laatste upload</dt>
        <dd class:error={!upload.ok || upload.confirmed === false}>
          {when(upload.at)} · {upload.partial ? 'tussenstand' : 'volledig'}{upload.dryRun ? ' (proef)' : ''}{upload.ok ? '' : ' · mislukt'}{check}
        </dd>
      {/if}
    </dl>
    {#if sheet.comments}<p class="hint">{sheet.comments}</p>{/if}
    {#if onaction || logId}
      <div class="detail-actions">
        {#if onaction}
          <button class="btn ghost" type="button" onclick={() => onaction('print')}><Icon name="print" size={16} />Wedstrijdblad</button>
          <button class="btn ghost" type="button" onclick={() => onaction('slips')}><Icon name="print" size={16} />Blaadjes</button>
          {#if canUpload}<button class="btn ghost" type="button" onclick={() => onaction('upload')}><Icon name="upload" size={16} />Upload</button>{/if}
        {/if}
        {#if logId}
          <button class="btn ghost" class:danger={lastFailed} type="button" onclick={() => (showLog = true)}>
            <Icon name={lastFailed ? 'warn' : 'info'} size={16} />VTTL-logboek{log ? ` (${log.length})` : ''}
          </button>
        {/if}
      </div>
    {/if}
  </div>
</section>

{#if showLog && logId}
  <UploadLogDialog matchId={sheet.matchId} entries={log} error={logError} onclose={() => (showLog = false)} />
{/if}
