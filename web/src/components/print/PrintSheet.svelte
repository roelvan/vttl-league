<script lang="ts">
  // The VTTL "scheidsrechtersblad", A4 landscape. Positions in cm come from ScoreSheet's
  // Views/Templates/VttlScoreTemplate.xaml and its Partial/*.xaml.
  import type { Sheet } from '../../lib/sheet/types';
  import { getSystem, slotLabel } from '../../lib/sheet/systems';
  import { computeScore } from '../../lib/sheet/score';
  import { matchTypeMarks, LEVEL_NAME, printedComments, playersLabel } from '../../lib/sheet/print';

  let { sheet }: { sheet: Sheet } = $props();

  const sys = $derived(getSystem(sheet.matchSystemId));
  const score = $derived(computeScore(sheet, sys));
  const marks = $derived(matchTypeMarks(sheet));
  const x = (b: boolean) => (b ? '✗' : '');

  const teams = $derived(
    (['home', 'away'] as const).map((side) => ({
      side,
      title: side === 'home' ? 'THUISPLOEG' : 'BEZOEKERS',
      team: sheet[side],
      wins: side === 'home' ? score.homeIndividual : score.awayIndividual,
    })),
  );
  const shown = (won: string) => won !== 'empty' && won !== 'error';
</script>

<div class="page landscape ps">
  <!-- ScoreSheet keeps the top band for the VTTL sponsor image, which we do not print. The content
       (4.19-28.45 cm wide, 5.19-20.25 cm high) is moved to the middle of the A4 page instead. -->
  <div class="ps-content">
  {#if !sheet.isOfficial}<div class="ps-ps-unofficial">ONOFFICIEEL WEDSTRIJDBLAD</div>{/if}

  <div class="ps-box" style="left:4.19cm; top:5.19cm; width:6.15cm; height:2.32cm; display:flex">
    <div class="ps-f35" style="padding:0 0.05cm">Lokaal:<br />Str.Nr.:<br />Plaats:</div>
    <div class="ps-f35 ps-b ps-pre">{sheet.address}</div>
  </div>
  {#if sheet.isOfficial}
    <div class="ps-abs ps-f40 ps-b" style="left:11.9cm; top:5.3cm">VLAAMSE TAFELTENNISLIGA v.z.w.</div>
    <div class="ps-abs ps-f35" style="left:13.26cm; top:5.8cm">SCHEIDSRECHTERSBLAD</div>
    <div class="ps-abs ps-f25" style="left:17.9cm; top:5.9cm">© VTTL</div>
  {/if}

  <!-- MatchType.xaml -->
  {#if sys.competitive}
    <table class="ps-abs ps-grid ps-mtype" style="left:10.8cm; top:6.41cm; width:8.5cm; height:1.1cm">
      <tbody>
        <tr><td class="ps-xbox">{x(marks.interclubMen)}</td><td>Interclub Heren</td><td class="ps-xbox">{x(marks.interclubYouth)}</td><td>Interclub Jeugd</td><td class="ps-xbox">{x(marks.cupMen)}</td><td>Beker Heren</td></tr>
        <tr><td class="ps-xbox">{x(marks.interclubWomen)}</td><td>Interclub Dames</td><td class="ps-xbox">{x(marks.super)}</td><td>Super Afdeling</td><td class="ps-xbox">{x(marks.cupWomen)}</td><td>Beker Dames</td></tr>
      </tbody>
    </table>
  {/if}

  <!-- MatchInfo.xaml -->
  <div class="ps-box ps-minfo ps-f35" style="left:19.75cm; top:5.19cm; width:8.7cm; height:2.32cm">
    <span><i style="width:1.38cm">Datum</i><b>{sheet.date}</b></span>
    <span><i style="width:1.6cm">Match n°</i><b>{sheet.matchId}</b></span>
    <span>{#if sys.competitive}<i style="width:1.38cm">Afdeling</i><b>{LEVEL_NAME[sheet.level]}</b>{/if}</span>
    <span><i style="width:1.6cm">Reeks</i><b>{sheet.series}</b></span>
    <span><i style="width:2.15cm">Aanvangsuur</i><b>{sheet.startHour}</b></span>
    <span><i style="width:2cm">Einde match</i><b>{sheet.endHour}</b></span>
  </div>

  <!-- OfficialTeamInfo.xaml x2 -->
  <div class="ps-abs" style="left:4.19cm; top:7.7cm; width:10.35cm">
    {#each teams as t, ti (t.side)}
      <table class="ps-grid ps-team" style:margin-top={ti ? '0.2cm' : '0'}>
        <colgroup><col /><col style="width:1.4cm" /><col style="width:0.7cm" /><col style="width:0.8cm" /><col style="width:0.9cm" /><col style="width:0.7cm" /><col style="width:0.8cm" /></colgroup>
        <tbody>
          <tr><td colspan="7" class="ps-c ps-b ps-f40">{t.title}</td></tr>
          <tr class="ps-head">
            <td class="ps-f35 ps-left"><div>Ploeg: <b>{t.team.name}</b></div><div>Aanwijzingsnummer: <b>{t.team.clubId}</b></div></td>
            <td>Computer<br />nummer</td><td>Volg<br />num</td><td>Index</td><td>Klasse<br />ment</td><td>Ind<br />over</td><td>Kapi-<br />tein</td>
          </tr>
          {#each sys.slots as slot, si (si)}
            {#if slot.kind === 'single'}
              {@const p = t.team.players[slot.player]}
              <tr class="ps-prow">
                <td class="ps-left"><span class="ps-pos">{slotLabel(sys, si, t.side === 'away')}</span>{p?.name ?? ''}</td>
                <td>{p?.computerNumber ?? ''}</td><td>{p?.position ?? ''}</td><td>{p?.index ?? ''}</td><td>{p?.ranking ?? ''}</td>
                <td class="ps-fit">{t.wins[si]}</td><td class="ps-b">{x(!!p?.captain)}</td>
              </tr>
            {:else if slot.kind === 'double'}
              <tr class="ps-prow">
                <td class="ps-left"><span class="ps-pos">Dubbel</span>{t.team.doubles[slot.double] ?? ''}</td>
                <td></td><td></td><td></td><td></td><td class="ps-fit">{t.wins[si]}</td><td></td>
              </tr>
            {/if}
          {/each}
        </tbody>
      </table>
    {/each}
  </div>

  <!-- Matches.xaml + MatchResult.xaml -->
  <table class="ps-abs ps-grid ps-games" style="left:14.95cm; top:7.7cm; width:12.2cm; --sets:{sys.boxes}">
    <colgroup>
      <col style="width:0.45cm" /><col style="width:0.45cm" /><col style="width:0.45cm" />
      {#each Array(sys.boxes) as _, b (b)}<col />{/each}
      <col style="width:0.75cm" /><col style="width:0.75cm" /><col style="width:0.75cm" /><col style="width:0.75cm" />
    </colgroup>
    <thead>
      <tr><th colspan={7 + sys.boxes}>VOLGORDE DER WEDSTRIJDEN</th></tr>
      <tr><th colspan="3"></th><th colspan={sys.boxes}>BEHAALDE PUNTEN</th><th colspan="2">SETS</th><th colspan="2">WEDSTR</th></tr>
    </thead>
    <tbody>
      {#each sys.games as g, gi (g.pos)}
        {@const row = score.games[gi]}
        <tr>
          <td>{g.pos}</td><td>{playersLabel(sheet, sys, gi, 'home')}</td><td>{playersLabel(sheet, sys, gi, 'away')}</td>
          {#each Array(sys.boxes) as _, b (b)}
            {@const s = sheet.results[gi]?.[b]}
            <td class="ps-set">{#if s && (s.l || s.r)}<span>{s.l}</span>-<span>{s.r}</span>{/if}</td>
          {/each}
          <td>{shown(row.result.won) ? row.result.homeSets : ''}</td><td>{shown(row.result.won) ? row.result.awaySets : ''}</td>
          <td>{row.homeTotal ?? ''}</td><td>{row.awayTotal ?? ''}</td>
        </tr>
      {/each}
    </tbody>
  </table>

  <!-- ChiefReferee.xaml -->
  <table class="ps-abs ps-grid ps-people" style="left:4.19cm; top:15.7cm; width:10.35cm">
    <colgroup><col /><col style="width:2.8cm" /><col style="width:2.8cm" /></colgroup>
    <tbody>
      <tr><td colspan="3" class="ps-c ps-b ps-f40">HOOFDSCHEIDSRECHTER</td></tr>
      <tr><td class="ps-c">Naam + Voornaam</td><td class="ps-c">Club</td><td class="ps-c">Handtekening</td></tr>
      <tr class="ps-tall">
        <td class="ps-left">{sheet.chiefReferee.name}<br />Aansluitingsnummer: {sheet.chiefReferee.computerNumber}</td>
        <td class="ps-left">{sheet.chiefReferee.clubName}<br />Clubnr: {sheet.chiefReferee.clubId}</td>
        <td></td>
      </tr>
    </tbody>
  </table>

  <!-- Captains.xaml -->
  <table class="ps-abs ps-grid ps-people ps-captains" style="left:14.95cm; top:15.7cm; width:13.5cm; height:2.6cm">
    <tbody>
      <tr><td colspan="2" class="ps-c">Kapitein Thuisploeg</td><td colspan="2" class="ps-c">Kapitein Bezoekers</td></tr>
      <tr><td class="ps-c">Naam + Voornaam</td><td class="ps-c">Handtekening</td><td class="ps-c">Naam + Voornaam</td><td class="ps-c">Handtekening</td></tr>
      <tr class="ps-sign"><td class="ps-left">{sheet.homeCaptain.name}</td><td></td><td class="ps-left">{sheet.awayCaptain.name}</td><td></td></tr>
      <tr><td colspan="4" class="ps-left">Akkoord om de wedstrijden twee aan twee te spelen: <b style="margin:0 0.2cm">{sheet.article632 ? 'JA' : 'NEE'}</b>(Art. 6.32)</td></tr>
    </tbody>
  </table>

  <div class="ps-box" style="left:4.19cm; top:18.5cm; width:14.15cm; height:1.75cm; display:flex; flex-direction:column">
    <div style="margin:0.05cm">Eventueel voorbehoud:</div>
    <div class="ps-comments">{printedComments(sheet.comments)}</div>
  </div>
  <div class="ps-box" style="left:18.59cm; top:18.5cm; width:9.86cm; height:1.75cm">
    <div style="margin:0.05cm">Naam+Voornaam zaalcommissaris: {sheet.roomCommissioner.name}</div>
    <div style="margin:0.05cm">Aansluitingsnummer: {sheet.roomCommissioner.computerNumber}</div>
  </div>
</div>
</div>

<style>
  .ps { position: relative; font-family: 'Segoe UI', Arial, sans-serif; font-size: 12px; color: #000; background: #fff; }
  .ps :global(*) { box-sizing: border-box; }
  .ps-content { position: absolute; inset: 0; transform: translate(-1.47cm, -2.22cm); }
  .ps-abs, .ps-box { position: absolute; }
  .ps-box { border: 1px solid #000; }
  .ps-f25 { font-size: 0.25cm; }
  .ps-f35 { font-size: 0.35cm; }
  .ps-f40 { font-size: 0.4cm; }
  .ps-b { font-weight: 700; }
  .ps-c { text-align: center; }
  .ps-pre { white-space: pre-line; }
  .ps-ps-unofficial { position: absolute; left: 11.9cm; top: 5.4cm; font-size: 0.45cm; font-weight: 700; }
  table.ps-grid { border-collapse: collapse; table-layout: fixed; }
  table.ps-grid td, table.ps-grid th { border: 0.5px solid #000; padding: 0; text-align: center; vertical-align: middle; overflow: hidden; white-space: nowrap; }
  table.ps-grid { border: 1px solid #000; }
  table.ps-team { width: 100%; }
  .ps-mtype td { font-size: 0.3cm; text-align: left; padding-left: 0.05cm; }
  .ps-mtype .ps-xbox { width: 0.5cm; text-align: center; font-weight: 700; font-size: 0.35cm; padding: 0; }
  .ps-minfo { display: grid; grid-template-columns: 1fr 1fr; grid-template-rows: repeat(3, 1fr); align-items: center; }
  .ps-minfo span { display: flex; white-space: nowrap; }
  .ps-minfo i { font-style: normal; margin: 0 0.1cm; display: inline-block; flex: none; }
  .ps-team .ps-head td { font-size: 0.28cm; line-height: 1.1; }
  .ps-team .ps-head td.ps-left { font-size: 0.35cm; line-height: 1.3; padding: 0.05cm; }
  .ps-team .ps-prow td { border-top: 0; border-bottom: 0; height: 0.45cm; }
  .ps-team tr:last-child td { border-bottom: 1px solid #000; }
  td.ps-left { text-align: left !important; padding-left: 0.05cm !important; }
  .ps-pos { display: inline-block; margin-right: 0.2cm; }
  .ps-fit { font-size: 11px; }
  .ps-games th { font-weight: 400; font-size: 12px; }
  .ps-games td { font-size: 0.28cm; height: 0.39cm; line-height: 0.37cm; }
  .ps-games td.ps-set { max-width: 1.57cm; display: table-cell; }
  .ps-games td.ps-set span { display: inline-block; width: 42%; }
  .ps-people td { padding: 0.05cm; white-space: normal; }
  .ps-people .ps-tall td { height: 1.1cm; vertical-align: top; line-height: 1.5; }
  .ps-captains .ps-sign td { height: 1.2cm; vertical-align: top; white-space: normal; }
  .ps-comments { flex: 1; display: grid; place-items: center; text-align: center; margin: 0 0.05cm 0.05cm; overflow: hidden; white-space: pre-wrap; word-break: break-word; }
</style>