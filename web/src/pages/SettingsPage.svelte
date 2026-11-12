<script lang="ts">
  // The scoreboards of the club: device id (required, as on the board) and an alias.
  import Icon from '../components/Icon.svelte';
  import { boards, loadDevices, saveDevice, removeDevice } from '../lib/boards.svelte';
  import { session } from '../lib/session.svelte';

  let id = $state('');
  let alias = $state('');
  let error = $state('');
  let busy = $state(false);
  let editing = $state<Record<string, string>>({});

  loadDevices().catch((e) => (error = e.message));

  const ago = (t: number | null) => {
    if (!t) return 'Nog nooit gezien';
    const s = Math.round((Date.now() - t) / 1000);
    if (s < 120) return 'Online';
    if (s < 3600) return `${Math.round(s / 60)} min geleden gezien`;
    if (s < 86400) return `${Math.round(s / 3600)} u geleden gezien`;
    return `Gezien op ${new Date(t).toLocaleDateString('nl-BE')}`;
  };

  async function add(e: SubmitEvent) {
    e.preventDefault();
    if (!id.trim()) return;
    busy = true;
    error = '';
    try {
      await saveDevice(id, alias);
      id = '';
      alias = '';
    } catch (err) {
      error = err instanceof Error ? err.message : 'Bewaren mislukt.';
    } finally {
      busy = false;
    }
  }

  async function rename(devId: string) {
    try {
      await saveDevice(devId, editing[devId] ?? '');
      delete editing[devId];
    } catch (err) {
      error = err instanceof Error ? err.message : 'Bewaren mislukt.';
    }
  }

  async function remove(devId: string) {
    if (!confirm(`Scorebord ${devId} verwijderen?`)) return;
    try {
      await removeDevice(devId);
    } catch (err) {
      error = err instanceof Error ? err.message : 'Verwijderen mislukt.';
    }
  }
</script>

<div class="narrow">
  <section class="card">
    <div class="card-h"><h2>Scoreborden van {session.me?.club?.name ?? 'de club'}</h2></div>
    <div class="card-b stack">
      {#if error}<p class="error">{error}</p>{/if}
      <ul class="list">
        {#each boards.devices as d (d.id)}
          {@const online = !!d.lastSeen && Date.now() - d.lastSeen < 120000}
          <li class="row-btn board-row">
            {#if d.id in editing}
              <input class="grow" bind:value={editing[d.id]} placeholder="Naam" aria-label="Naam van {d.id}" />
              <button class="btn icon-btn" type="button" onclick={() => rename(d.id)} aria-label="Bewaren" title="Bewaren"><Icon name="check" /></button>
              <button class="btn ghost icon-btn" type="button" onclick={() => delete editing[d.id]} aria-label="Annuleren" title="Annuleren"><Icon name="close" /></button>
            {:else}
              <span class="grow board-info">
                <b>{d.alias || d.id}</b>
                <span class="muted mono">{d.id}</span>
                <small class:ok={online} class:muted={!online}>{ago(d.lastSeen)}</small>
              </span>
              <button class="btn ghost icon-btn" type="button" onclick={() => (editing[d.id] = d.alias)} aria-label="Naam wijzigen" title="Naam wijzigen"><Icon name="edit" /></button>
              <button class="btn danger icon-btn" type="button" onclick={() => remove(d.id)} aria-label="Verwijderen" title="Verwijderen"><Icon name="trash" /></button>
            {/if}
          </li>
        {:else}
          <li class="muted">{boards.loaded ? 'Nog geen scoreborden.' : 'Laden…'}</li>
        {/each}
      </ul>
    </div>
  </section>

  <form class="card" onsubmit={add}>
    <div class="card-h"><h2>Scorebord toevoegen</h2></div>
    <div class="card-b form-grid">
      <label class="field">ID (verplicht)<input bind:value={id} required autocapitalize="off" spellcheck="false" placeholder="bv. flippr-07" /></label>
      <label class="field">Naam<input bind:value={alias} placeholder="bv. Tafel 7" /></label>
      <button class="btn wide" type="submit" disabled={busy || !id.trim()}><Icon name="plus" />Toevoegen</button>
    </div>
  </form>
</div>

<style>
  .board-row { flex-wrap: nowrap; }
  .board-row input.grow { min-width: 0; }
  .board-info { display: grid; gap: 2px; min-width: 0; }
  .board-info > * { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
