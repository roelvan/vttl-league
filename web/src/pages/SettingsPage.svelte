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
      <p class="hint">
        Voeg elk scorebord één keer toe met zijn ID (zoals op het scorebord) en een naam. In de opstelling kies je daarna per tafel een scorebord.
      </p>
      {#if error}<p class="error">{error}</p>{/if}
      <ul class="list">
        {#each boards.devices as d (d.id)}
          {@const online = !!d.lastSeen && Date.now() - d.lastSeen < 120000}
          <li class="row-btn">
            <Icon name="board" />
            {#if d.id in editing}
              <input class="grow" bind:value={editing[d.id]} placeholder="Naam" aria-label="Naam van {d.id}" />
              <button class="btn" type="button" onclick={() => rename(d.id)}>Bewaren</button>
              <button class="btn ghost" type="button" onclick={() => delete editing[d.id]}>Annuleren</button>
            {:else}
              <span class="grow">
                <b>{d.alias || d.id}</b><br />
                <span class="muted mono">{d.id}</span> · <span class:ok={online} class:muted={!online}>{ago(d.lastSeen)}</span>
              </span>
              <button class="btn ghost" type="button" onclick={() => (editing[d.id] = d.alias)}>Naam wijzigen</button>
              <button class="btn danger" type="button" onclick={() => remove(d.id)}>Verwijderen</button>
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
