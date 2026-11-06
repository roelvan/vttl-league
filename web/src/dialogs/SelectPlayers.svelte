<script lang="ts">
  // Port of SelectPlayersViewModel + CompetitiveMatchViewModel.FillPlayerInfo/FillPerson.
  import { untrack } from 'svelte';
  import Dialog from './Dialog.svelte';
  import { ws, change } from '../lib/workspace.svelte';
  import { members as cachedMembers, loadMembers, defaultCategory, clubName } from '../lib/data.svelte';
  import { getSystem } from '../lib/sheet/systems';
  import { computeScore, gameSlots } from '../lib/sheet/score';
  import type { Op } from '../lib/sheet/ops';
  import type { Member } from '../lib/types';
  import { titleCase } from '../lib/format';

  let { id, side, onclose }: { id: string; side: 'home' | 'away'; onclose: () => void } = $props();

  // The dialog is made new each time it opens: read the props once.
  const sheet = untrack(() => ws.entries[id].view);
  const SIDE = untrack(() => side);
  const sys = getSystem(sheet.matchSystemId);
  const team = sheet[SIDE];
  const category = sheet.playerCategory;
  const defCat = defaultCategory();
  const canShowAll = category !== defCat;

  let error = $state('');
  let filter = $state('');
  let relevant = $state(true);
  let catList = $state<Member[] | undefined>(cachedMembers(team.clubId, category));
  let allList = $state<Member[] | undefined>(canShowAll ? cachedMembers(team.clubId, defCat) : undefined);

  loadMembers(team.clubId, category).then((m) => (catList = m), (e) => !catList && (error = e.message));
  if (canShowAll) loadMembers(team.clubId, defCat).then((m) => (allList = m), () => {});

  const memberName = (m: Member) => `${m.lastName.toUpperCase()} ${titleCase(m.firstName)}`;
  const find = (cn: string) => {
    const n = Number(cn);
    if (!cn || !Number.isFinite(n)) return null;
    return catList?.find((m) => m.uniqueIndex === n) ?? allList?.find((m) => m.uniqueIndex === n) ?? null;
  };

  interface Pick {
    member: Member | null;
    /** Text that was on the sheet but is not in the list (kept unless cleared). */
    placeholder: string;
    wo: boolean;
    captain: boolean;
  }
  const score = computeScore(sheet, sys);
  const isWO = SIDE === 'home' ? score.homeIsWO : score.awayIsWO;
  const singleSlots = sys.slots.flatMap((s, i) => (s.kind === 'single' ? [{ slot: i, player: s.player, label: s.label }] : []));

  let picks = $state<Pick[]>(
    singleSlots.map(({ slot, player }) => {
      const p = team.players[player];
      return { member: find(p?.computerNumber ?? ''), placeholder: p?.name ?? '', wo: isWO[slot], captain: !!p?.captain };
    }),
  );
  let referee = $state<Pick>({ member: find(sheet.chiefReferee.computerNumber), placeholder: sheet.chiefReferee.name, wo: false, captain: false });
  let commissioner = $state<Pick>({ member: find(sheet.roomCommissioner.computerNumber), placeholder: sheet.roomCommissioner.name, wo: false, captain: false });

  // Once the lists arrive, match the existing names again. Only the lists are tracked: reading
  // the picks here would undo a "Leegmaken" (×) right away. A slot cleared by the user has no
  // placeholder any more, so it is not filled again when the second list arrives.
  $effect(() => {
    if (!catList) return;
    void allList;
    untrack(() => {
      for (const [i, { player }] of singleSlots.entries()) {
        const p = picks[i];
        if (!p.member && p.placeholder) p.member = find(team.players[player]?.computerNumber ?? '');
      }
      if (!referee.member && referee.placeholder) referee.member = find(sheet.chiefReferee.computerNumber);
      if (!commissioner.member && commissioner.placeholder) commissioner.member = find(sheet.roomCommissioner.computerNumber);
    });
  });

  /** The slot that a click in the list fills: 0..n-1 = players, 'ref', 'com'. */
  let target = $state<number | 'ref' | 'com'>(Math.max(0, picks.findIndex((p) => !p.member)));

  const rankGroup = (r: string) => (!r ? '?' : r.startsWith('A') ? 'A' : r);
  const statusTag = (s: string) => (s === 'V' ? ' (R)' : s === 'P' ? ' (V)' : '');

  const shown = $derived.by(() => {
    const src = (relevant ? catList : allList) ?? [];
    const words = filter.toLowerCase().split(/\s+/).filter(Boolean);
    const list = words.length
      ? src.filter((m) => words.every((w) => m.firstName.toLowerCase().includes(w) || m.lastName.toLowerCase().includes(w) || m.ranking.toLowerCase() === w || String(m.uniqueIndex).includes(w)))
      : src;
    return [...list].sort((a, b) => a.position - b.position);
  });

  function choose(m: Member) {
    if (target === 'ref') referee.member = m;
    else if (target === 'com') commissioner.member = m;
    else {
      // A player can only be in one slot.
      picks.forEach((p, i) => i !== target && p.member?.uniqueIndex === m.uniqueIndex && (p.member = null));
      picks[target].member = m;
      const next = picks.findIndex((p, i) => i > (target as number) && !p.member);
      if (next >= 0) target = next;
    }
  }

  function clear(p: Pick) {
    p.member = null;
    p.placeholder = '';
    p.wo = false;
    p.captain = false;
  }

  function setWO(i: number, on: boolean) {
    picks.forEach((p, j) => (p.wo = on && j === i));
    if (on) picks[i].captain = false;
  }
  function setCaptain(i: number, on: boolean) {
    picks.forEach((p, j) => (p.captain = on && j === i));
  }

  // Scroll to the ranking where this team's players usually are (A = index 1, B = 5, …).
  let listEl: HTMLElement | undefined = $state();
  let scrolled = false;
  $effect(() => {
    if (scrolled || !catList || !listEl) return;
    const letter = team.name.trim().split(' ').at(-1) ?? '';
    if (letter.length !== 1 || !/[A-Z]/i.test(letter)) return;
    const idx = (letter.toUpperCase().charCodeAt(0) - 65) * 4 + 1;
    const rank = [...catList].sort((a, b) => a.position - b.position).find((m) => m.rankingIndex >= idx)?.ranking;
    if (rank) listEl.querySelector(`[data-rank="${CSS.escape(rankGroup(rank))}"]`)?.scrollIntoView({ block: 'start' });
    scrolled = true;
  });

  function ok() {
    const level = sheet.level;
    if (level !== 'super' && !sheet.cup) {
      let prev = -1;
      let bad = false;
      for (const p of picks) {
        if (!p.member) continue;
        if (p.member.rankingIndex < prev) bad = true;
        prev = p.member.rankingIndex;
      }
      if (
        bad &&
        !confirm('De spelersvolgorde die u geselecteerd heeft, is ongeldig. Spelers moeten volgens klassement/index gerangschikt worden. Weet u zeker dat u wil doorgaan met de gekozen opstelling?')
      )
        return;
    }
    const ops: Op[] = [];
    const person = (field: string, p: Pick) => {
      if (!p.member && p.placeholder) return; // unknown name typed by hand: keep it
      ops.push({
        p: [field],
        v: p.member
          ? { name: memberName(p.member), computerNumber: String(p.member.uniqueIndex), clubId: team.clubId, clubName: clubName(team.clubId) }
          : { name: '', computerNumber: '', clubId: '', clubName: '' },
      });
    };
    person('chiefReferee', referee);
    person('roomCommissioner', commissioner);

    singleSlots.forEach(({ slot, player }, i) => {
      const p = picks[i];
      if (!p.member && p.placeholder) return;
      const m = p.member;
      ops.push({
        p: [side, 'players', player],
        v: m
          ? { name: memberName(m), computerNumber: String(m.uniqueIndex), index: String(m.rankingIndex), ranking: m.ranking, position: String(m.position), captain: p.captain }
          : { name: '', computerNumber: '', index: '', ranking: '', position: '', captain: false },
      });
      // WO: put "WO" in the first box of every game of this player (if empty); else remove it.
      const box = side === 'home' ? 'l' : 'r';
      sys.games.forEach((_, gi) => {
        if (!gameSlots(sheet, sys, gi, side).includes(slot) || gameSlots(sheet, sys, gi, side).length !== 1) return;
        const cur = sheet.results[gi]?.[0]?.[box] ?? '';
        if (p.wo && m && cur === '') ops.push({ p: ['results', gi, 0, box], v: 'WO' });
        else if ((!p.wo || !m) && cur.toUpperCase() === 'WO') ops.push({ p: ['results', gi, 0, box], v: '' });
      });
    });

    // The captain also fills the captain box (UpdateTeamCaptain).
    const capIdx = picks.findIndex((p) => p.captain && p.member);
    if (capIdx >= 0) {
      const m = picks[capIdx].member!;
      ops.push({ p: [side === 'home' ? 'homeCaptain' : 'awayCaptain'], v: { name: memberName(m), computerNumber: String(m.uniqueIndex), clubId: team.clubId, clubName: clubName(team.clubId) } });
    }
    change(id, ops);
    onclose();
  }

  const label = (p: Pick) => (p.member ? `${memberName(p.member)} (${p.member.ranking})` : p.placeholder);
</script>

<Dialog title="Selecteer spelers – {team.name}" {onclose} wide>
  <p class="muted small">Klik op een vakje links en daarna op een speler in de lijst. Een speler aanklikken vult het gekozen vakje.</p>
  <div class="sp">
    <div class="sp-slots">
      {#each singleSlots as s, i (s.slot)}
        <div class="sp-slot" class:target={target === i}>
          <button class="sp-pick" onclick={() => (target = i)}>
            <span class="sp-n">{s.label}</span>
            <span class="grow">{label(picks[i]) || '— leeg —'}</span>
          </button>
          <label class="check" title="Walkover"><input type="checkbox" checked={picks[i].wo} onchange={(e) => setWO(i, (e.currentTarget as HTMLInputElement).checked)} /> WO</label>
          <label class="check" title="Kapitein"><input type="checkbox" checked={picks[i].captain} disabled={picks[i].wo} onchange={(e) => setCaptain(i, (e.currentTarget as HTMLInputElement).checked)} /> K</label>
          <button class="x" aria-label="Leegmaken" onclick={() => clear(picks[i])}>×</button>
        </div>
      {/each}
      <div class="sp-slot" class:target={target === 'ref'}>
        <button class="sp-pick" onclick={() => (target = 'ref')}><span class="sp-n wide">Hoofdscheidsrechter</span><span class="grow">{label(referee) || '— leeg —'}</span></button>
        <button class="x" aria-label="Leegmaken" onclick={() => clear(referee)}>×</button>
      </div>
      {#if sys.competitive}
        <div class="sp-slot" class:target={target === 'com'}>
          <button class="sp-pick" onclick={() => (target = 'com')}><span class="sp-n wide">Zaalcommissaris</span><span class="grow">{label(commissioner) || '— leeg —'}</span></button>
          <button class="x" aria-label="Leegmaken" onclick={() => clear(commissioner)}>×</button>
        </div>
      {/if}
    </div>

    <div class="sp-list">
      <div class="sp-tools">
        <input placeholder="Zoeken (naam, klassement, nummer)" bind:value={filter} />
        {#if canShowAll}
          <div class="seg">
            <button class:on={relevant} onclick={() => (relevant = true)}>Relevante sterktelijst</button>
            <button class:on={!relevant} onclick={() => (relevant = false)}>Alle spelers</button>
          </div>
        {/if}
      </div>
      {#if error}<p class="error">{error}</p>{/if}
      {#if !catList && !error}<p class="muted">Sterktelijst laden…</p>{/if}
      <ul bind:this={listEl}>
        {#each shown as m, i (m.uniqueIndex)}
          {#if relevant && (i === 0 || rankGroup(shown[i - 1].ranking) !== rankGroup(m.ranking))}
            <li class="sp-rank" data-rank={rankGroup(m.ranking)}>{rankGroup(m.ranking)}</li>
          {/if}
          <li>
            <button class="sp-member" onclick={() => choose(m)}>
              {m.position}. {memberName(m)}{statusTag(m.status)}
              {#if !relevant}<span class="muted">({m.ranking})</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
    </div>
  </div>
  {#snippet footer()}
    <button class="ghost" onclick={onclose}>Annuleren</button>
    <button onclick={ok}>OK</button>
  {/snippet}
</Dialog>
