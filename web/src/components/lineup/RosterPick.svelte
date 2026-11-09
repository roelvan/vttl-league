<script lang="ts">
  // Pick the players of one team from the strength list. Picked players get their slot in
  // ranking order (strength-list index), as on the VTTL sheet. Players with the same index may
  // stand in any order: they keep the order of the clicks.
  import { members as cachedMembers, loadMembers, clubName } from '../../lib/data.svelte';
  import { change } from '../../lib/workspace.svelte';
  import Icon from '../Icon.svelte';
  import { titleCase } from '../../lib/format';
  import type { Sheet } from '../../lib/sheet/types';
  import type { MatchSystem } from '../../lib/sheet/systems';
  import type { Member } from '../../lib/types';
  import type { Op } from '../../lib/sheet/ops';

  let { id, sheet, sys, side, step, editable }: { id: string; sheet: Sheet; sys: MatchSystem; side: 'home' | 'away'; step: number; editable: boolean } = $props();

  const team = $derived(sheet[side]);
  let list = $state<Member[] | undefined>(undefined);
  let error = $state('');
  let query = $state('');
  let showAll = $state(false);

  $effect(() => {
    const club = team.clubId;
    const cat = sheet.playerCategory;
    list = cachedMembers(club, cat);
    loadMembers(club, cat).then(
      (m) => (list = m),
      (e) => !list && (error = e.message),
    );
  });

  const memberName = (m: Member) => `${m.lastName.toUpperCase()} ${titleCase(m.firstName)}`;
  const singles = $derived(sys.slots.flatMap((s, i) => (s.kind === 'single' ? [{ slot: i, player: s.player }] : [])));
  const count = $derived(singles.length);
  const picked = $derived(team.players.slice(0, count).map((p) => p.computerNumber).filter(Boolean));
  const captain = $derived(team.players.find((p) => p.captain)?.computerNumber ?? '');

  /** Where this team's players usually are: team A = index 1, B = 5, … */
  const around = $derived.by(() => {
    const l = team.name.trim().split(' ').at(-1) ?? '';
    return l.length === 1 && /[A-Z]/i.test(l) ? (l.toUpperCase().charCodeAt(0) - 65) * 4 + 1 : 1;
  });

  const shown = $derived.by(() => {
    const src = [...(list ?? [])].sort((a, b) => a.position - b.position);
    const words = query.toLowerCase().split(/\s+/).filter(Boolean);
    if (words.length)
      return src.filter((m) => words.every((w) => `${m.firstName} ${m.lastName} ${m.ranking}`.toLowerCase().includes(w) || String(m.uniqueIndex).includes(w)));
    if (showAll) return src;
    const near = src.filter((m) => m.rankingIndex >= around - 4 && m.rankingIndex < around + 12);
    const extra = src.filter((m) => picked.includes(String(m.uniqueIndex)) && !near.includes(m));
    return [...extra, ...near];
  });

  const slotOf = (m: Member) => picked.indexOf(String(m.uniqueIndex));

  const memberOf = (n: string) => (list ?? []).find((m) => String(m.uniqueIndex) === n);

  /** `nums` is the wanted order; only the index decides beyond that (stable sort). */
  function write(nums: string[], cap = captain) {
    const ms = nums.map(memberOf).filter((m): m is Member => !!m);
    // Players that are not in the list (typed before, or from VTTL) drop out.
    ms.sort((a, b) => a.rankingIndex - b.rankingIndex);
    const ops: Op[] = [];
    singles.forEach(({ player }, i) => {
      const m = ms[i];
      ops.push({
        p: [side, 'players', player],
        v: m
          ? { name: memberName(m), computerNumber: String(m.uniqueIndex), index: String(m.rankingIndex), ranking: m.ranking, position: String(m.position), captain: String(m.uniqueIndex) === cap }
          : { name: '', computerNumber: '', index: '', ranking: '', position: '', captain: false },
      });
    });
    const cm = ms.find((m) => String(m.uniqueIndex) === cap);
    ops.push({
      p: [side === 'home' ? 'homeCaptain' : 'awayCaptain'],
      v: cm
        ? { name: memberName(cm), computerNumber: cap, clubId: team.clubId, clubName: clubName(team.clubId) }
        : { name: '', computerNumber: '', clubId: '', clubName: '' },
    });
    change(id, ops);
  }

  function toggle(m: Member) {
    const n = String(m.uniqueIndex);
    if (picked.includes(n)) write(picked.filter((x) => x !== n), captain === n ? '' : captain);
    else if (picked.length < count) write([...picked, n]);
  }

  function setForfeit(on: boolean) {
    change(id, [{ p: [side, 'forfeit'], v: on }]);
  }
</script>

<section class="card">
  <div class="card-h">
    <span class="step-n">{step}</span>
    <h2>{team.name}</h2>
    <span class="spacer"></span>
    <span class="eyebrow">{picked.length} van {count}</span>
  </div>
  <div class="card-b stack">
    {#if team.forfeit}
      <div class="banner warn">Deze ploeg is forfait (W-O). Alle wedstrijden gaan naar de tegenstander.</div>
    {/if}
    <input type="search" placeholder="Zoek een speler (naam, klassement, lidnummer)" bind:value={query} aria-label="Zoek een speler" />
    {#if error}<p class="error">{error}</p>{/if}
    <div class="roster">
      {#each shown as m (m.uniqueIndex)}
        {@const slot = slotOf(m)}
        {@const n = String(m.uniqueIndex)}
        <div class="rpick-wrap">
          <button
            type="button"
            class="rpick {side === 'away' ? 'away' : ''}"
            class:has-cap={slot >= 0}
            aria-pressed={slot >= 0}
            disabled={!editable || (slot < 0 && picked.length >= count)}
            onclick={() => toggle(m)}
          >
            <span class="slot">{slot >= 0 ? slot + 1 : '+'}</span>
            <span class="who">
              <span class="n">{titleCase(m.firstName)} {titleCase(m.lastName)}</span>
              <span class="meta">Index {m.rankingIndex} · nr. {m.position}{m.status === 'V' ? ' · reserve' : ''}</span>
            </span>
            <span class="rank">{m.ranking}</span>
          </button>
          {#if slot >= 0}
            <button
              type="button"
              class="cap-btn"
              aria-pressed={captain === n}
              aria-label={captain === n ? `${titleCase(m.firstName)} is kapitein` : `Maak ${titleCase(m.firstName)} kapitein`}
              title={captain === n ? 'Kapitein' : 'Maak kapitein'}
              disabled={!editable}
              onclick={() => write(picked, captain === n ? '' : n)}
            ><Icon name="captain" size={22} /></button>
          {/if}
        </div>
      {:else}
        <p class="muted">{list ? 'Niemand gevonden.' : 'Sterktelijst laden…'}</p>
      {/each}
    </div>
    {#if !query}
      <button class="link-btn" type="button" onclick={() => (showAll = !showAll)}>{showAll ? 'Toon alleen de spelers rond deze ploeg' : 'Toon de hele sterktelijst'}</button>
    {/if}
    <label class="check"><input type="checkbox" checked={team.forfeit} disabled={!editable} onchange={(e) => setForfeit(e.currentTarget.checked)} /> Ploeg forfait (W-O)</label>
    {#if side === 'away'}<p class="hint">Uit de sterktelijst van VTTL. Spreek de opstelling af met de kapitein van de bezoekers.</p>{/if}
  </div>
</section>
