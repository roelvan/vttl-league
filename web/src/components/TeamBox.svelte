<script lang="ts">
  import type { Sheet } from '../lib/sheet/types';
  import type { MatchSystem } from '../lib/sheet/systems';
  import { slotLabel } from '../lib/sheet/systems';
  import type { Score } from '../lib/sheet/score';
  import type { Op } from '../lib/sheet/ops';
  import { change } from '../lib/workspace.svelte';
  import { clubName } from '../lib/data.svelte';

  let {
    id,
    side,
    sheet,
    sys,
    score,
    locked,
    onbrowse,
  }: { id: string; side: 'home' | 'away'; sheet: Sheet; sys: MatchSystem; score: Score; locked: boolean; onbrowse: () => void } = $props();

  const team = $derived(sheet[side]);
  const individual = $derived(side === 'home' ? score.homeIndividual : score.awayIndividual);
  const captainKey = $derived(side === 'home' ? 'homeCaptain' : 'awayCaptain');

  const val = (e: Event) => (e.currentTarget as HTMLInputElement).value;

  function setPlayer(i: number, field: string, v: unknown) {
    change(id, [{ p: [side, 'players', i, field], v }]);
  }

  /** One captain per team; the captain also fills the captain box below (ScoreSheet SetCaptain). */
  function setCaptain(i: number, on: boolean) {
    const ops: Op[] = team.players.map((_, j) => ({ p: [side, 'players', j, 'captain'], v: on && j === i }));
    if (on) {
      const p = team.players[i];
      ops.push({
        p: [captainKey],
        v: { name: p.name, computerNumber: p.computerNumber, clubId: team.clubId, clubName: clubName(team.clubId) },
      });
    }
    change(id, ops);
  }

  /** A forfeit team gets "?" as computer number on every empty player. */
  function setForfeit(on: boolean) {
    const ops: Op[] = [{ p: [side, 'forfeit'], v: on }];
    if (on) team.players.forEach((p, j) => !p.computerNumber && ops.push({ p: [side, 'players', j, 'computerNumber'], v: '?' }));
    change(id, ops);
  }

  const subGames = $derived(sys.games.filter((g) => g.allowSub));
</script>

<!-- One grid for the header and the rows, so every header stays above its column. -->
<div class="team tgrid">
  <div class="team-id">
    <label class="inline"><span>Ploeg</span>
      <input class="ed name" value={team.name} disabled={locked} oninput={(e) => change(id, [{ p: [side, 'name'], v: val(e) }])} />
    </label>
    <label class="inline ff"><input type="checkbox" checked={team.forfeit} onchange={(e) => setForfeit((e.currentTarget as HTMLInputElement).checked)} /> FF</label>
    <label class="inline"><span>Aanwijzingsnummer</span>
      <input class="ed club" value={team.clubId} disabled={locked} oninput={(e) => change(id, [{ p: [side, 'clubId'], v: val(e) }])} />
    </label>
  </div>
  <span class="th">Computer<br />nummer</span><span class="th">Volg<br />num</span><span class="th">Index</span><span class="th">Klasse<br />ment</span><span class="th">Ind.<br />overw</span><span class="th">Kapi-<br />tein</span>

  {#each sys.slots as slot, si (si)}
    {#if slot.kind === 'single'}
      {@const p = team.players[slot.player]}
      {#if p}
        <span class="pl">{slotLabel(sys, si, side === 'away')}</span>
        <input class="ed" value={p.name} aria-label="Naam speler {slot.label}" oninput={(e) => setPlayer(slot.player, 'name', val(e))} />
        <button class="dots" title="Spelers kiezen" onclick={onbrowse}>…</button>
        <input class="ed num" value={p.computerNumber} aria-label="Computernummer" oninput={(e) => setPlayer(slot.player, 'computerNumber', val(e))} />
        <input class="ed num" value={p.position} aria-label="Volgnummer" oninput={(e) => setPlayer(slot.player, 'position', val(e))} />
        <input class="ed num" value={p.index} aria-label="Index" oninput={(e) => setPlayer(slot.player, 'index', val(e))} />
        <input class="ed num" value={p.ranking} aria-label="Klassement" oninput={(e) => setPlayer(slot.player, 'ranking', val(e))} />
        <span class="win">{individual[si]}</span>
        <input class="cap" type="checkbox" checked={p.captain} aria-label="Kapitein" onchange={(e) => setCaptain(slot.player, (e.currentTarget as HTMLInputElement).checked)} />
      {/if}
    {:else if slot.kind === 'double'}
      <span class="dl">Dubbel:</span>
      <select class="ed span-sel" value={team.doubles[slot.double] ?? slot.options[0]} onchange={(e) => change(id, [{ p: [side, 'doubles', slot.double], v: (e.currentTarget as HTMLSelectElement).value }])}>
        {#each slot.options as o (o)}<option>{o}</option>{/each}
      </select>
      <span class="win">{individual[si]}</span>
      <span></span>
    {:else}
      <span class="dl">Wissel:</span>
      <select
        class="ed span-sel"
        value={team.substitutes[slot.sub] ?? ''}
        onchange={(e) => {
          const v = (e.currentTarget as HTMLSelectElement).value;
          change(id, [{ p: [side, 'substitutes', slot.sub], v: v === '' ? null : Number(v) }]);
        }}
      >
        <option value="">Geen wissel</option>
        {#each subGames as g (g.pos)}<option value={g.pos}>Wissel in match #{g.pos}</option>{/each}
      </select>
      <span></span><span></span>
    {/if}
  {/each}
</div>
