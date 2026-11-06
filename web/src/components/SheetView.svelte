<script lang="ts">
  import type { Sheet, Level } from '../lib/sheet/types';
  import { getSystem } from '../lib/sheet/systems';
  import { computeScore } from '../lib/sheet/score';
  import { change } from '../lib/workspace.svelte';
  import { settings } from '../lib/settings.svelte';
  import TeamBox from './TeamBox.svelte';
  import ScoreGrid from './ScoreGrid.svelte';
  import PersonBox from './PersonBox.svelte';
  import VttlIcon from './VttlIcon.svelte';
  import { vttlStatus, STATUS_TEXT } from '../lib/sheet/vttl';

  let { id, sheet, onbrowse }: { id: string; sheet: Sheet; onbrowse: (side: 'home' | 'away') => void } = $props();

  const sys = $derived(getSystem(sheet.matchSystemId));
  const score = $derived(computeScore(sheet, sys));
  /** "Officiële velden onveranderbaar maken" */
  const locked = $derived(settings.protectMatchInfo && sheet.isOfficial);
  const watermark = $derived.by(() => {
    const last = sheet.home.name.trim().split(' ').at(-1) ?? '';
    return last.length === 1 ? last.toUpperCase() : '';
  });

  const status = $derived(sheet.isOfficial ? vttlStatus(sheet, sys) : null);
  const time = (ms: number) => new Date(ms).toLocaleString('nl-BE', { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' });

  const val = (e: Event) => (e.currentTarget as HTMLInputElement).value;
  const text = (field: string) => (e: Event) => change(id, [{ p: [field], v: val(e) }]);

  /** Men/Women and the match types are exclusive groups, like in ScoreSheet. */
  function exclusive(group: string[], field: string, on: boolean) {
    change(
      id,
      on ? group.map((g) => ({ p: [g], v: g === field })) : [{ p: [field], v: false }],
    );
  }
  const genders = ['men', 'women'];
  const types = ['interclub', 'super', 'cup', 'youth', 'veterans'];
  const levels: [Level, string][] = [
    ['super', 'Super'],
    ['national', 'Nationaal'],
    ['regional', 'Landelijk'],
    ['provincial', 'Provinciaal'],
  ];
</script>

{#if sheet.vttlLocked}
  <div class="locked-banner"><VttlIcon status="locked" /> Gevalideerd door VTTL: dit wedstrijdblad toont de officiële uitslag en kan niet meer gewijzigd worden.</div>
{/if}
<div class="sheet" style:zoom={settings.zoom}>
  {#if watermark}<div class="watermark" aria-hidden="true">{watermark}</div>{/if}
  <fieldset disabled={!!sheet.vttlLocked}>

  <div class="row">
    <section class="box address">
      <div class="box-title">LOKAAL</div>
      <textarea class="ed" rows="3" value={sheet.address} disabled={locked} oninput={text('address')}></textarea>
    </section>

    {#if sys.competitive}
      <section class="box mtype">
        {#each [['men', 'Heren'], ['women', 'Dames']] as [k, label] (k)}
          <label class="inline"><input type="checkbox" disabled={locked} checked={sheet[k as 'men']} onchange={(e) => exclusive(genders, k, (e.currentTarget as HTMLInputElement).checked)} /> {label}</label>
        {/each}
        <span class="sep"></span>
        {#each [['interclub', 'Interclub'], ['super', 'Super'], ['cup', 'Beker'], ['youth', 'Jeugd'], ['veterans', 'Veteranen']] as [k, label] (k)}
          <label class="inline"><input type="checkbox" disabled={locked} checked={sheet[k as 'cup']} onchange={(e) => exclusive(types, k, (e.currentTarget as HTMLInputElement).checked)} /> {label}</label>
        {/each}
      </section>
    {/if}

    <section class="box minfo">
      <label class="inline"><span>Datum</span><input class="ed" value={sheet.date} disabled={locked} oninput={text('date')} /></label>
      <label class="inline"><span>Match N°</span><input class="ed" value={sheet.matchId} disabled={locked} oninput={text('matchId')} /></label>
      {#if sys.competitive}
        <label class="inline"><span>Afdeling</span>
          <select class="ed" value={sheet.level} disabled={locked} onchange={(e) => change(id, [{ p: ['level'], v: (e.currentTarget as HTMLSelectElement).value }])}>
            {#each levels as [v, l] (v)}<option value={v}>{l}</option>{/each}
          </select>
        </label>
      {/if}
      <label class="inline"><span>Reeks</span><input class="ed" value={sheet.series} disabled={locked} oninput={text('series')} /></label>
      <label class="inline"><span>Aanvangsuur</span><input class="ed hour" value={sheet.startHour} disabled={locked} oninput={text('startHour')} /></label>
      <label class="inline"><span>Einde match</span><input class="ed hour" value={sheet.endHour} oninput={text('endHour')} /></label>
    </section>
  </div>

  <div class="row">
    <div class="teams">
      <section class="box">
        <div class="box-title">THUISPLOEG</div>
        <TeamBox {id} side="home" {sheet} {sys} {score} {locked} onbrowse={() => onbrowse('home')} />
      </section>
      <section class="box">
        <div class="box-title">BEZOEKERS</div>
        <TeamBox {id} side="away" {sheet} {sys} {score} {locked} onbrowse={() => onbrowse('away')} />
      </section>
    </div>
    <div class="scores">
      <section class="box"><ScoreGrid {id} {sheet} {sys} {score} /></section>
      <label class="inline a632">
        Akkoord om de wedstrijden twee aan twee te spelen
        <input type="checkbox" checked={sheet.article632} onchange={(e) => change(id, [{ p: ['article632'], v: (e.currentTarget as HTMLInputElement).checked }])} />
        <b>ja</b> (artikel 6.32)
      </label>
      {#if status}
        <p class="upload-info">
          <VttlIcon {status} />
          {STATUS_TEXT[status]}{#if sheet.upload}. Laatste upload: {time(sheet.upload.at)}{sheet.upload.partial ? ' (tussenstand)' : ''}{sheet.upload.dryRun ? ' – proef, niet naar VTTL gestuurd' : ''}{!sheet.upload.ok ? ` – mislukt: ${sheet.upload.errors.join('; ')}` : ''}{/if}
        </p>
      {/if}
    </div>
  </div>

  <div class="row">
    <PersonBox {id} field="chiefReferee" title="HOOFDSCHEIDSRECHTER" person={sheet.chiefReferee} onbrowse={() => onbrowse('home')} />
    <PersonBox {id} field="homeCaptain" title="KAPITEIN THUISPLOEG" person={sheet.homeCaptain} onbrowse={() => onbrowse('home')} />
    <PersonBox {id} field="awayCaptain" title="KAPITEIN BEZOEKERS" person={sheet.awayCaptain} onbrowse={() => onbrowse('away')} />
  </div>
  <div class="row">
    <section class="box comments">
      <div class="box-title">EVENTUEEL VOORBEHOUD</div>
      <textarea class="ed" rows="3" value={sheet.comments} oninput={text('comments')}></textarea>
    </section>
    {#if sys.competitive}
      <PersonBox {id} field="roomCommissioner" title="ZAALCOMMISSARIS" person={sheet.roomCommissioner} onbrowse={() => onbrowse('home')} />
    {/if}
  </div>
  </fieldset>
</div>
