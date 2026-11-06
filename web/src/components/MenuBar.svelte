<script lang="ts">
  import { settings, saveSettings } from '../lib/settings.svelte';
  import { ws, close, closeAll } from '../lib/workspace.svelte';
  import { logout } from '../lib/session.svelte';
  import { navigate } from '../lib/router.svelte';

  let { onaction }: { onaction: (a: 'matchday' | 'custom' | 'validate' | 'about' | 'upload' | 'import' | 'print' | 'export') => void } = $props();

  let open = $state<string | null>(null);
  const toggle = (m: string) => (e: MouseEvent) => {
    e.stopPropagation();
    open = open === m ? null : m;
  };
  const hover = (m: string) => () => {
    if (open) open = m;
  };
  const sheetOpen = $derived(!!ws.active && ws.active !== 'overview');

  function fullscreen() {
    if (document.fullscreenElement) document.exitFullscreen();
    else document.documentElement.requestFullscreen?.();
  }
</script>

<svelte:window onclick={() => (open = null)} onkeydown={(e) => e.key === 'Escape' && (open = null)} />

<div class="menubar" role="menubar">
  <div class="mb-item">
    <button class="mb-top" class:open={open === 'file'} onclick={toggle('file')} onmouseenter={hover('file')}>Bestand</button>
    {#if open === 'file'}
      <div class="mb-menu" role="menu">
        <button role="menuitem" onclick={() => onaction('matchday')}>Nieuwe wedstrijddag (voor deze club)</button>
        <button role="menuitem" onclick={() => onaction('custom')}>Nieuw onofficieel wedstrijdblad</button>
        <hr />
        <button role="menuitem" disabled={!sheetOpen} onclick={() => ws.active && close(ws.active)}>Sluiten</button>
        <button role="menuitem" disabled={!ws.open.length} onclick={closeAll}>Alles sluiten</button>
        <hr />
        <button role="menuitem" disabled={!sheetOpen} onclick={() => onaction('validate')}>Valideer</button>
        <button role="menuitem" disabled={!sheetOpen} onclick={() => onaction('print')}>Printen <kbd>Ctrl+P</kbd></button>
        <button role="menuitem" disabled={!ws.open.length} onclick={() => onaction('export')}>Exporteer wedstrijden</button>
        <button role="menuitem" disabled={!ws.open.length} onclick={() => onaction('upload')}>Upload naar competitiewebsite</button>
        <button role="menuitem" disabled={!ws.open.length} onclick={() => onaction('import')}>Gegevens ophalen bij VTTL</button>
        <hr />
        <button role="menuitem" onclick={logout}>Afmelden</button>
      </div>
    {/if}
  </div>
  <div class="mb-item">
    <button class="mb-top" class:open={open === 'opt'} onclick={toggle('opt')} onmouseenter={hover('opt')}>Opties</button>
    {#if open === 'opt'}
      <div class="mb-menu" role="menu">
        <button role="menuitem" onclick={fullscreen}>Volledig scherm <kbd>F11</kbd></button>
        <button role="menuitemcheckbox" aria-checked={settings.protectMatchInfo} onclick={() => { settings.protectMatchInfo = !settings.protectMatchInfo; saveSettings(); }}>
          <span class="chk">{settings.protectMatchInfo ? '✓' : ''}</span> Officiële velden onveranderbaar maken
        </button>
        <button role="menuitemcheckbox" aria-checked={settings.showResultsInNav} onclick={() => { settings.showResultsInNav = !settings.showResultsInNav; saveSettings(); }}>
          <span class="chk">{settings.showResultsInNav ? '✓' : ''}</span> Uitslag tonen in de navigatie
        </button>
        <button role="menuitemcheckbox" aria-checked={settings.autoUpload} onclick={() => { settings.autoUpload = !settings.autoUpload; saveSettings(); }}>
          <span class="chk">{settings.autoUpload ? '✓' : ''}</span> Automatisch uploaden na afdrukken
        </button>
        <hr />
        <span class="mb-label">Live-updates naar VTTL tijdens de match</span>
        {#each [['on', 'Aan'], ['super', 'Enkel voor Super'], ['off', 'Uit']] as [v, l] (v)}
          <button role="menuitemradio" aria-checked={settings.liveUpdates === v} onclick={() => { settings.liveUpdates = v as 'on'; saveSettings(); }}>
            <span class="chk">{settings.liveUpdates === v ? '●' : ''}</span> {l}
          </button>
        {/each}
        <hr />
        <button role="menuitem" onclick={() => navigate('/ik')}>Wie ben ik? (club wijzigen)</button>
      </div>
    {/if}
  </div>
  <div class="mb-item">
    <button class="mb-top" class:open={open === 'help'} onclick={toggle('help')} onmouseenter={hover('help')}>Help</button>
    {#if open === 'help'}
      <div class="mb-menu" role="menu">
        <button role="menuitem" onclick={() => onaction('about')}>Over VTTL League</button>
      </div>
    {/if}
  </div>
</div>
