<script lang="ts">
  import MenuBar from '../components/MenuBar.svelte';
  import NavBar from '../components/NavBar.svelte';
  import StatusBar from '../components/StatusBar.svelte';
  import SheetView from '../components/SheetView.svelte';
  import Dialog from '../dialogs/Dialog.svelte';
  import NewMatchday from '../dialogs/NewMatchday.svelte';
  import SelectPlayers from '../dialogs/SelectPlayers.svelte';
  import { ws, openSheets, activate, connect, scheduleFlush, importFromVttl } from '../lib/workspace.svelte';
  import { data, loadCategories, loadClubs, loadMatches, defaultCategory } from '../lib/data.svelte';
  import { getSystem, SYSTEMS } from '../lib/sheet/systems';
  import { computeScore } from '../lib/sheet/score';
  import { validateSheet } from '../lib/validation';
  import UploadDialog from '../dialogs/UploadDialog.svelte';
  import { custom } from '../lib/sheet/create';
  import { todayIso } from '../lib/format';
  import Credits from '../components/Credits.svelte';

  type DialogState =
    | { kind: 'matchday' }
    | { kind: 'custom' }
    | { kind: 'about' }
    | { kind: 'upload' }
    | { kind: 'players'; id: string; side: 'home' | 'away' }
    | { kind: 'validate'; id: string; errors: string[] }
    | null;
  let dialog = $state<DialogState>(null);

  // Reference data in the background; the cached copies are used meanwhile.
  loadCategories().catch(() => {});
  loadClubs().catch(() => {});
  loadMatches().catch(() => {});
  connect();
  scheduleFlush(0);

  const active = $derived(ws.active && ws.active !== 'overview' ? ws.entries[ws.active] : null);

  function runValidation(id: string) {
    const s = ws.entries[id]?.view;
    if (s) dialog = { kind: 'validate', id, errors: validateSheet(s) };
  }

  function onaction(a: 'matchday' | 'custom' | 'validate' | 'about' | 'upload' | 'import') {
    if (a === 'import') {
      for (const id of ws.open) importFromVttl(id).catch((e) => (ws.message = `VTTL ophalen mislukt: ${e.message}`));
      return;
    }
    if (a === 'validate') {
      if (ws.active && ws.active !== 'overview') runValidation(ws.active);
    } else dialog = { kind: a };
  }

  let customSystem = $state(2);
  function makeCustom() {
    openSheets([{ local: custom(customSystem, defaultCategory(), todayIso()) }]);
    dialog = null;
  }

  function onkey(e: KeyboardEvent) {
    if (e.key === 'F11') {
      e.preventDefault();
      if (document.fullscreenElement) document.exitFullscreen();
      else document.documentElement.requestFullscreen?.();
    }
  }

  const overview = $derived(
    ws.open
      .map((id) => ws.entries[id]?.view)
      .filter(Boolean)
      .map((s) => {
        const sys = getSystem(s.matchSystemId);
        const sc = computeScore(s, sys);
        const played = sc.games.filter((g) => g.result.won !== 'empty' && g.result.won !== 'partial' && g.result.won !== 'error').length;
        return { s, sc, played, total: sys.games.length };
      }),
  );
</script>

<svelte:window onkeydown={onkey} />

<div class="app">
  <MenuBar {onaction} />
  <NavBar onvalidate={runValidation} />

  <div class="screen">
    {#if active}
      {#key ws.active}
        <SheetView id={ws.active!} sheet={active.view} onbrowse={(side) => (dialog = { kind: 'players', id: ws.active!, side })} />
      {/key}
    {:else if ws.open.length && ws.active === 'overview'}
      <div class="overview">
        {#each overview as o (o.s.id)}
          <button class="ov-card" onclick={() => activate(o.s.id)}>
            <span class="ov-teams"><span>{o.s.home.name}</span><span>{o.s.away.name}</span></span>
            <span class="ov-score" class:win={o.sc.homeWins > o.sc.awayWins} class:loss={o.sc.homeWins < o.sc.awayWins}>
              <b>{o.sc.homeWins}</b><b>{o.sc.awayWins}</b>
            </span>
            <span class="ov-meta">{o.s.matchId} · {o.s.startHour} · {o.played}/{o.total} wedstrijden gespeeld</span>
          </button>
        {/each}
      </div>
    {:else}
      <div class="welcome">
        <h1>Welkom! Wat wil je doen?</h1>
        <p class="muted">Klik op de actie die je wil uitvoeren.</p>
        <button class="big" onclick={() => (dialog = { kind: 'matchday' })}>
          <strong>Nieuwe speeldag beginnen</strong>
          <span>Selecteer een datum, en krijg automatisch alle wedstrijden van die speeldag.</span>
        </button>
        <button class="big" onclick={() => (dialog = { kind: 'custom' })}>
          <strong>Blanco wedstrijdblad aanmaken</strong>
          <span>Alle data moet manueel ingevuld worden; kies deze optie voor niet-competitie wedstrijden (bijv. vriendenmatchen).</span>
        </button>
        <button
          class="big"
          onclick={() => {
            loadMatches().catch(() => {});
            loadClubs().catch(() => {});
            loadCategories().catch(() => {});
          }}
        >
          <strong>Database bijwerken</strong>
          <span>Haal de kalender, de clubs en de spelerscategorieën opnieuw op bij VTTL.{data.matches ? ` Nu: ${data.matches.matches.length} wedstrijden.` : ''}</span>
        </button>
        <Credits />
      </div>
    {/if}
  </div>

  <StatusBar />
</div>

{#if dialog?.kind === 'matchday'}
  <NewMatchday onclose={() => (dialog = null)} />
{:else if dialog?.kind === 'players'}
  <SelectPlayers id={dialog.id} side={dialog.side} onclose={() => (dialog = null)} />
{:else if dialog?.kind === 'validate'}
  <Dialog title="Valideer" onclose={() => (dialog = null)}>
    {#if dialog.errors.length}
      <p>De volgende validatiefouten werden gevonden:</p>
      <ul class="errors">{#each dialog.errors as e (e)}<li>{e}</li>{/each}</ul>
    {:else}
      <p>Geen validatiefouten gevonden.</p>
    {/if}
    {#snippet footer()}<button onclick={() => (dialog = null)}>OK</button>{/snippet}
  </Dialog>
{:else if dialog?.kind === 'custom'}
  <Dialog title="Onofficieel wedstrijdblad" onclose={() => (dialog = null)}>
    <label>Matchsysteem
      <select bind:value={customSystem}>
        {#each Object.values(SYSTEMS) as s (s.id)}<option value={s.id}>{s.name}</option>{/each}
      </select>
    </label>
    {#snippet footer()}
      <button class="ghost" onclick={() => (dialog = null)}>Annuleren</button>
      <button onclick={makeCustom}>OK</button>
    {/snippet}
  </Dialog>
{:else if dialog?.kind === 'upload'}
  <UploadDialog onclose={() => (dialog = null)} />
{:else if dialog?.kind === 'about'}
  <Dialog title="Over VTTL League" onclose={() => (dialog = null)}>
    <Credits long />
    {#snippet footer()}<button onclick={() => (dialog = null)}>OK</button>{/snippet}
  </Dialog>
{/if}
