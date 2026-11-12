<script lang="ts">
  import TopBar from '../components/TopBar.svelte';
  import BottomNav from '../components/BottomNav.svelte';
  import MatchTabs from '../components/MatchTabs.svelte';
  import ClubNight from '../components/ClubNight.svelte';
  import Toast from '../components/Toast.svelte';
  import MatchPage from './MatchPage.svelte';
  import LineUpPage from './LineUpPage.svelte';
  import SettingsPage from './SettingsPage.svelte';
  import Dialog from '../dialogs/Dialog.svelte';
  import PrintDialog from '../dialogs/PrintDialog.svelte';
  import UploadDialog from '../dialogs/UploadDialog.svelte';
  import PrintSheet from '../components/print/PrintSheet.svelte';
  import RefereeSlips from '../components/print/RefereeSlips.svelte';
  import { route, match } from '../lib/router.svelte';
  import { matchIdFromSlug, urls, type MenuAction } from '../lib/nav';
  import { data, loadCategories, loadClubs, loadMatches, loadPublicMatches } from '../lib/data.svelte';
  import { viewer, viewClub, publicClub } from '../lib/viewer.svelte';
  import { ws, connect, scheduleFlush, ensureMatch, setCurrent, onServerEvent } from '../lib/workspace.svelte';
  import { onBoardMessage, loadDevices } from '../lib/boards.svelte';
  import { session } from '../lib/session.svelte';
  import { canEdit, isHomeMember } from '../lib/access';
  import { validateSheet } from '../lib/validation';
  import { shortDate, playDayIso } from '../lib/format';
  import type { PrintJob } from '../lib/sheet/print';
  import { tick } from 'svelte';

  // Reference data of the own club in the background; the cached copies are used meanwhile.
  if (session.me) {
    loadCategories().catch(() => {});
    loadClubs().catch(() => {});
    loadMatches().catch(() => {});
    scheduleFlush(0);
  }
  onServerEvent((msg) => {
    if (msg.type === 'board') onBoardMessage(msg as never);
  });

  // ---------------------------------------------------------------- route
  // "/c/{club}/…" = the public pages of a club (no login needed); without it: the own club.
  const m = $derived.by(() => {
    const pm = /^\/c\/([^/]+)(\/.*)?$/.exec(route.path);
    const club = pm ? decodeURIComponent(pm[1]) : null;
    const p = pm ? (pm[2] ?? '/') : route.path;
    let r: Record<string, string> | null;
    if ((r = match('/wedstrijd/:id/opstelling', p))) return { club, section: 'lineup' as const, id: matchIdFromSlug(r.id), day: null };
    if ((r = match('/wedstrijd/:id', p))) return { club, section: 'match' as const, id: matchIdFromSlug(r.id), day: null };
    if ((r = match('/dag/:day', p)) && /^\d{4}-\d{2}-\d{2}$/.test(r.day)) return { club, section: 'night' as const, id: null, day: r.day };
    if (p === '/instellingen' && !club && session.me) return { club, section: 'other' as const, id: null, day: null };
    return { club, section: 'night' as const, id: null, day: playDayIso() };
  });

  $effect.pre(() => {
    viewer.club = m.club;
  });
  const pub = $derived(publicClub());

  // Public: the club's match list without login. The live connection and boards follow the club.
  $effect(() => {
    if (pub) loadPublicMatches(pub).catch((e) => (ws.message = `Laden mislukt: ${e.message}`));
  });
  $effect(() => {
    void pub;
    connect();
    loadDevices().catch(() => {});
  });

  const list = $derived(pub ? data.publicMatches[pub] : data.matches);
  const rows = $derived(list?.matches ?? []);
  const row = $derived(m.id ? (rows.find((x) => x.matchId === m.id) ?? null) : null);
  const sheet = $derived(m.id ? ws.entries[m.id]?.view : undefined);
  const day = $derived(m.day ?? row?.date ?? sheet?.day ?? (ws.current ? ws.entries[ws.current]?.view.day : null) ?? playDayIso());

  $effect(() => {
    const id = m.id;
    if (!id) return;
    setCurrent(id);
    if (row && list) ensureMatch(row, list.season).catch((e) => (ws.message = `Laden mislukt: ${e.message}`));
  });

  $effect(() => {
    m.section;
    m.id;
    window.scrollTo(0, 0);
  });

  // ---------------------------------------------------------------- menu actions
  type DialogState = { kind: 'print'; what: 'sheet' | 'slips' } | { kind: 'upload' } | { kind: 'validate'; errors: string[] } | null;
  let dialog = $state<DialogState>(null);
  let printJob = $state<PrintJob | null>(null);

  const homeMatch = $derived(!publicClub() && isHomeMember(sheet) && (row?.isHome ?? true));
  const ourMatch = $derived(homeMatch && canEdit(sheet));
  /** The page heading for screen readers; the visible headings start at h2. */
  const pageTitle = $derived(
    m.section === 'other'
      ? 'Instellingen'
      : m.section === 'night'
        ? 'Clubavond'
        : `${m.section === 'lineup' ? 'Opstelling' : 'Wedstrijd'}${sheet ? `: ${sheet.home.name} – ${sheet.away.name}` : ''}`,
  );
  $effect(() => {
    document.title = `${pageTitle} · VTTL Competitie`;
  });

  async function onaction(a: MenuAction) {
    if (a === 'share') {
      const club = viewClub();
      if (!club) return;
      const url = `${location.origin}${urls.share(club, m.id, day)}`;
      try {
        await navigator.clipboard.writeText(url);
        ws.message = 'Publieke link gekopieerd. Iedereen met de link kan de wedstrijd live volgen.';
      } catch {
        prompt('Kopieer de link:', url);
      }
    } else if (!sheet) return;
    else if (a === 'print') dialog = { kind: 'print', what: 'sheet' };
    else if (a === 'slips') dialog = { kind: 'print', what: 'slips' };
    else if (a === 'upload') dialog = { kind: 'upload' };
    else if (a === 'validate') dialog = { kind: 'validate', errors: validateSheet(sheet) };
  }

  async function doPrint(job: PrintJob) {
    dialog = null;
    printJob = job;
    await tick();
    // Wait for the images (the VTTL logo), else the print can open without them.
    const imgs = document.querySelectorAll<HTMLImageElement>('#print-root img');
    await Promise.all([...imgs].map((img) => img.decode().catch(() => {})));
    const page =document.getElementById('page-size') ?? document.head.appendChild(Object.assign(document.createElement('style'), { id: 'page-size' }));
    page.textContent = `@page { size: A4 ${job.what === 'sheet' ? 'landscape' : 'portrait'}; margin: 0; }`;
    addEventListener(
      'afterprint',
      () => {
        printJob = null;
      },
      { once: true },
    );
    window.print();
  }

</script>

<TopBar
  matchId={m.id}
  {day}
  datechip={!row && pub ? (data.publicMatches[pub]?.club?.name ?? pub) : ''}
  {onaction}
  printActions={homeMatch}
  matchActions={ourMatch}
/>

<div class="shell" class:single={m.section === 'other'}>
  {#if m.section !== 'other'}
    <aside class="side-night" aria-label="Clubavond">
      <ClubNight {day} {rows} loaded={!!list} currentId={m.id} filters={false} />
    </aside>
  {/if}

  <main class="view">
    <h1 class="sr-only">{pageTitle} · VTTL Competitie</h1>
    {#if m.section === 'night'}
      <div class="view-night-only view"><ClubNight {day} {rows} loaded={!!list} currentId={ws.current} /></div>
      <div class="card empty hide-mobile">
        <h2>Kies een wedstrijd</h2>
        <p>Kies links een wedstrijd van de clubavond om ze live te volgen.</p>
      </div>
    {:else if m.section === 'match' && m.id}
      <MatchTabs section="match" matchId={m.id} />
      {#key m.id}<MatchPage matchId={m.id} {row} {onaction} canPrint={homeMatch} />{/key}
    {:else if m.section === 'lineup' && m.id}
      <MatchTabs section="lineup" matchId={m.id} />
      {#key m.id}<LineUpPage matchId={m.id} {row} />{/key}
    {:else}
      <SettingsPage />
    {/if}
  </main>
</div>

{#if (m.section === 'match' || m.section === 'lineup') && m.id}<BottomNav section={m.section} matchId={m.id} {day} />{/if}
<Toast />

{#if dialog?.kind === 'print' && m.id}
  <PrintDialog id={m.id} what={dialog.what} onclose={() => (dialog = null)} onprint={doPrint} />
{:else if dialog?.kind === 'upload' && m.id}
  <UploadDialog id={m.id} onclose={() => (dialog = null)} />
{:else if dialog?.kind === 'validate'}
  <Dialog title="Valideren" onclose={() => (dialog = null)}>
    {#if dialog.errors.length}
      <p>De app vond deze validatiefouten:</p>
      <ul class="errors">{#each dialog.errors as e (e)}<li>{e}</li>{/each}</ul>
    {:else}
      <p class="ok">Geen validatiefouten gevonden.</p>
    {/if}
    {#snippet footer()}<button class="btn big" type="button" onclick={() => (dialog = null)}>OK</button>{/snippet}
  </Dialog>
{/if}

<div id="print-root">
  {#if printJob && ws.entries[printJob.id]}
    {#if printJob.what === 'sheet'}<PrintSheet sheet={ws.entries[printJob.id].view} />
    {:else}<RefereeSlips sheet={ws.entries[printJob.id].view} layout={printJob.layout} />{/if}
  {/if}
</div>
