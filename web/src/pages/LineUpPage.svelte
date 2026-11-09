<script lang="ts">
  import RosterPick from '../components/lineup/RosterPick.svelte';
  import Icon from '../components/Icon.svelte';
  import { ws, change } from '../lib/workspace.svelte';
  import { members as cachedMembers, loadMembers, clubName } from '../lib/data.svelte';
  import { boards, loadDevices } from '../lib/boards.svelte';
  import { canEdit } from '../lib/access';
  import { liveView } from '../lib/live';
  import { getSystem } from '../lib/sheet/systems';
  import { navigate, link } from '../lib/router.svelte';
  import { urls } from '../lib/nav';
  import { titleCase } from '../lib/format';
  import type { MatchRow, Member } from '../lib/types';
  import type { Person, TableSetup } from '../lib/sheet/types';

  let { matchId, row }: { matchId: string; row: MatchRow | null } = $props();

  const sheet = $derived(ws.entries[matchId]?.view);
  const sys = $derived(sheet ? getSystem(sheet.matchSystemId) : null);
  const view = $derived(sheet ? liveView(sheet) : null);
  const editable = $derived(canEdit(sheet) && (row?.isHome ?? true));
  const anyResult = $derived(!!view && view.games.some((g) => g.state === 'final' || g.state === 'error' || g.home.cells.some(Boolean)));

  if (!boards.loaded) loadDevices().catch(() => {});

  // ---------------------------------------------------------------- tables
  const tables = $derived<TableSetup[]>([0, 1].map((i) => sheet?.tables?.[i] ?? { no: '', device: null }));
  /** Tables used by the other matches of the club on this day. */
  const busy = $derived.by(() => {
    if (!sheet) return new Map<string, string>();
    const m = new Map<string, string>();
    for (const e of Object.values(ws.entries)) {
      const o = e.view;
      if (o.id === sheet.id || o.day !== sheet.day) continue;
      for (const t of o.tables ?? []) if (t.no) m.set(t.no.trim(), o.home.name);
    }
    return m;
  });
  const busyDevices = $derived.by(() => {
    if (!sheet) return new Map<string, string>();
    const m = new Map<string, string>();
    for (const e of Object.values(ws.entries)) {
      const o = e.view;
      if (o.id === sheet.id || o.day !== sheet.day) continue;
      for (const t of o.tables ?? []) if (t.device) m.set(t.device, o.home.name);
    }
    return m;
  });

  function setTable(i: number, patch: Partial<TableSetup>) {
    const next = tables.map((t, j) => (j === i ? { ...t, ...patch } : t));
    // Drop empty tables at the end, so "game i on table i % n" uses only real tables.
    while (next.length && !next.at(-1)!.no.trim() && !next.at(-1)!.device) next.pop();
    change(matchId, [{ p: ['tables'], v: next }]);
  }

  // ---------------------------------------------------------------- doubles
  const doubleSlots = $derived(sys ? sys.slots.flatMap((s) => (s.kind === 'double' ? [s] : [])) : []);

  // ---------------------------------------------------------------- officials
  let homeList = $state<Member[] | undefined>(undefined);
  $effect(() => {
    if (!sheet) return;
    const club = sheet.home.clubId;
    homeList = cachedMembers(club, sheet.playerCategory);
    loadMembers(club, sheet.playerCategory).then((m) => (homeList = m), () => {});
  });
  const people = $derived([...(homeList ?? [])].sort((a, b) => a.lastName.localeCompare(b.lastName) || a.firstName.localeCompare(b.firstName)));
  function setPerson(field: 'chiefReferee' | 'roomCommissioner', cn: string) {
    const m = people.find((x) => String(x.uniqueIndex) === cn);
    const v: Person = m
      ? { name: `${m.lastName.toUpperCase()} ${titleCase(m.firstName)}`, computerNumber: cn, clubId: sheet!.home.clubId, clubName: clubName(sheet!.home.clubId) }
      : { name: '', computerNumber: '', clubId: '', clubName: '' };
    change(matchId, [{ p: [field], v }]);
  }
  const setField = (f: 'startHour' | 'endHour' | 'address' | 'comments', v: string) => change(matchId, [{ p: [f], v }]);

  // ---------------------------------------------------------------- start
  const playersReady = $derived(
    !!sheet && !!sys && (['home', 'away'] as const).every((s) => sheet[s].forfeit || sheet[s].players.slice(0, sys.playerCount).every((p, i) => p.computerNumber || (sys.slots.find((x) => x.kind === 'single' && x.player === i) as { optional?: boolean })?.optional)),
  );
  const tablesReady = $derived(tables.some((t) => t.no.trim()));

  function start() {
    if (!sheet?.started) change(matchId, [{ p: ['started'], v: true }]);
    navigate(urls.match(matchId));
  }
  /** Enough rows for all lines of the text, at least 2. */
  const rowsFor = (s: string) => Math.max(2, (s ?? '').split('\n').length);
</script>

{#if !sheet || !sys || !view}
  <div class="card empty"><p>Wedstrijd laden…</p></div>
{:else}
  <section class="card board">
    <div class="board-meta">
      <span>{view.started ? 'Avond bezig' : 'Voor wedstrijd 1'}</span><span class="dot"></span><span>{sheet.home.name} – {sheet.away.name}</span>
    </div>
    <div class="card-h"><h2>Opstelling en tafels</h2></div>
    <p class="hint">
      Kies de tafels en de spelers van beide ploegen. De spelers staan in volgorde van klassement, zoals op het VTTL-blad. De {view.games.length} wedstrijden volgen daaruit.
    </p>
  </section>

  {#if !editable}
    <div class="banner"><Icon name="info" />Alleen lezen. Alleen leden van de thuisclub kunnen de opstelling wijzigen.</div>
  {:else if anyResult}
    <div class="banner warn"><Icon name="warn" />De avond is begonnen. Een wijziging verandert de spelers van de wedstrijden die al gespeeld zijn niet mee: hun uitslag blijft op dezelfde plaats.</div>
  {/if}

  <div class="steps">
    <section class="card">
      <div class="card-h"><span class="step-n">1</span><h2>Tafels</h2><span class="spacer"></span><span class="eyebrow">{tables.filter((t) => t.no.trim()).length} van 2</span></div>
      <div class="card-b">
        <div class="tables-pick">
          {#each tables as t, i (i)}
            {@const usedBy = t.no.trim() ? busy.get(t.no.trim()) : undefined}
            <div class="tpick">
              <span class="tt">{i === 0 ? 'Eerste tafel' : 'Tweede tafel'}</span>
              <div class="row">
                <label for="tno{i}" class="muted">Nummer</label>
                <input id="tno{i}" inputmode="numeric" value={t.no} placeholder="bv. {i === 0 ? 7 : 8}" disabled={!editable} onchange={(e) => setTable(i, { no: e.currentTarget.value.trim() })} />
              </div>
              <div class="row">
                <label for="tdev{i}" class="muted">Scorebord</label>
                <select id="tdev{i}" value={t.device ?? ''} disabled={!editable} onchange={(e) => setTable(i, { device: e.currentTarget.value || null })}>
                  <option value="">Geen (manueel)</option>
                  {#each boards.devices as d (d.id)}
                    {@const other = tables.findIndex((x, j) => j !== i && x.device === d.id)}
                    <option value={d.id} disabled={other >= 0}>{d.alias || d.id}{other >= 0 ? ` (al op ${other === 0 ? 'eerste' : 'tweede'} tafel)` : ''}{busyDevices.has(d.id) ? ` (ook bij ${busyDevices.get(d.id)})` : ''}</option>
                  {/each}
                </select>
              </div>
              {#if usedBy}<span class="chip warn">Ook in gebruik door {usedBy}</span>{/if}
            </div>
          {/each}
        </div>
        <p class="hint">
          Oneven wedstrijden gaan naar de eerste tafel, even wedstrijden naar de tweede. Met één tafel gaan alle wedstrijden naar die tafel.
          {#if !boards.devices.length}Scoreborden voeg je toe via <a href={urls.settings()} onclick={link}>Instellingen</a>.{/if}
        </p>
      </div>
    </section>

    <div class="lineup-cols">
      <RosterPick id={matchId} {sheet} {sys} side="home" step={2} {editable} />
      <RosterPick id={matchId} {sheet} {sys} side="away" step={3} {editable} />
    </div>

    {#if doubleSlots.length}
      <section class="card">
        <div class="card-h"><span class="step-n">4</span><h2>Dubbel</h2></div>
        <div class="card-b form-grid">
          {#each doubleSlots as d (d.double)}
            {#each ['home', 'away'] as const as side (side)}
              <label class="field">{sheet[side].name}
                <select value={sheet[side].doubles[d.double] ?? ''} disabled={!editable} onchange={(e) => change(matchId, [{ p: [side, 'doubles', d.double], v: e.currentTarget.value }])}>
                  <option value="">Nog niet gekozen</option>
                  {#each d.options as o (o)}
                    {@const [a, b] = o.split('/').map((x) => Number(x) - 1)}
                    <option value={o}>{o}: {sheet[side].players[a]?.name || '?'} + {sheet[side].players[b]?.name || '?'}</option>
                  {/each}
                </select>
              </label>
            {/each}
          {/each}
          <p class="hint wide">De dubbel mag je ook later kiezen, voor de dubbelwedstrijd begint.</p>
        </div>
      </section>
    {/if}

    <section class="card">
      <div class="card-h"><span class="step-n">{doubleSlots.length ? 5 : 4}</span><h2>Officials en gegevens</h2></div>
      <div class="card-b form-grid">
        <label class="field">Scheidsrechter
          <select value={sheet.chiefReferee.computerNumber} disabled={!editable} onchange={(e) => setPerson('chiefReferee', e.currentTarget.value)}>
            <option value="">Geen</option>
            {#if sheet.chiefReferee.computerNumber && !people.some((p) => String(p.uniqueIndex) === sheet.chiefReferee.computerNumber)}
              <option value={sheet.chiefReferee.computerNumber}>{sheet.chiefReferee.name}</option>
            {/if}
            {#each people as p (p.uniqueIndex)}<option value={String(p.uniqueIndex)}>{p.lastName.toUpperCase()} {titleCase(p.firstName)}</option>{/each}
          </select>
        </label>
        <label class="field">Zaalcommissaris
          <select value={sheet.roomCommissioner.computerNumber} disabled={!editable} onchange={(e) => setPerson('roomCommissioner', e.currentTarget.value)}>
            <option value="">Geen</option>
            {#if sheet.roomCommissioner.computerNumber && !people.some((p) => String(p.uniqueIndex) === sheet.roomCommissioner.computerNumber)}
              <option value={sheet.roomCommissioner.computerNumber}>{sheet.roomCommissioner.name}</option>
            {/if}
            {#each people as p (p.uniqueIndex)}<option value={String(p.uniqueIndex)}>{p.lastName.toUpperCase()} {titleCase(p.firstName)}</option>{/each}
          </select>
        </label>
        <label class="field">Aanvang
          <input type="time" value={sheet.startHour} disabled={!editable} onchange={(e) => setField('startHour', e.currentTarget.value)} />
        </label>
        <label class="field">Einde
          <input type="time" value={sheet.endHour} disabled={!editable} onchange={(e) => setField('endHour', e.currentTarget.value)} />
        </label>
        <label class="field wide">Zaal
          <textarea rows={rowsFor(sheet.address)} value={sheet.address} disabled={!editable} onchange={(e) => setField('address', e.currentTarget.value)}></textarea>
        </label>
        <label class="field wide">Opmerkingen (komen op het wedstrijdblad)
          <textarea rows={rowsFor(sheet.comments)} value={sheet.comments} disabled={!editable} onchange={(e) => setField('comments', e.currentTarget.value)}></textarea>
        </label>
      </div>
    </section>

    <section class="card">
      <div class="card-b stack">
        {#if editable}
          {#if !tablesReady || !playersReady}
            <p class="hint">
              {!tablesReady ? 'Vul minstens één tafel in. ' : ''}{!playersReady ? 'Nog niet alle spelers zijn gekozen. Je kunt toch al starten.' : ''}
            </p>
          {/if}
          <button class="btn big" type="button" onclick={start} disabled={!tablesReady}>{view.started ? 'Naar de wedstrijd' : 'Start de avond'}</button>
        {:else}
          <a class="btn big ghost" href={urls.match(matchId)} onclick={link}>Naar de wedstrijd</a>
        {/if}
      </div>
    </section>
  </div>
{/if}
