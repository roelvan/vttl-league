<script lang="ts">
  import { api, type ClubLine } from '../lib/api';
  import { link } from '../lib/router.svelte';
  import { clubPath } from '../lib/nav';

  /** The page asked for without a club (an old link): it opens for the club picked. */
  let { rest = '/' }: { rest?: string } = $props();

  const KEY = 'vl.clublist';
  function kept(): ClubLine[] {
    try {
      return JSON.parse(localStorage.getItem(KEY) ?? '[]');
    } catch {
      return [];
    }
  }

  let clubs = $state<ClubLine[]>(kept());
  let query = $state('');
  let error = $state('');
  let loading = $state(true);
  let last = '';
  try {
    last = localStorage.getItem('vl.lastclub') ?? '';
  } catch {}

  api
    .publicClubs()
    .then((c) => {
      clubs = c;
      try {
        localStorage.setItem(KEY, JSON.stringify(c));
      } catch {}
    })
    .catch((e) => (error = clubs.length ? '' : `Clubs laden mislukt: ${e.message}`))
    .finally(() => (loading = false));

  const norm = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/[^a-z0-9]/gi, '').toLowerCase();
  const MAX = 30;
  const found = $derived.by(() => {
    const words = query.trim().split(/\s+/).map(norm).filter(Boolean);
    if (!words.length) return [];
    return clubs.filter((c) => {
      const hay = norm(`${c.id} ${c.name} ${c.longName}`);
      return words.every((w) => hay.includes(w));
    });
  });
  const lastClub = $derived(last ? (clubs.find((c) => norm(c.id) === norm(last)) ?? { id: last, name: last, longName: '', categoryName: '' }) : null);
</script>

{#snippet line(c: ClubLine)}
  <li>
    <a class="row-btn" href={clubPath(c.id, rest)} onclick={link}>
      <strong class="grow">{c.longName || c.name}</strong>
      <span class="muted">{c.categoryName}</span>
      <span class="rank">{c.id}</span>
    </a>
  </li>
{/snippet}

<div class="narrow">
  <section class="card">
    <div class="card-h"><h2>Kies een club</h2></div>
    <div class="card-b stack">
      <p class="muted">Volg de wedstrijden van een club live. Aanmelden is niet nodig.</p>
      <!-- svelte-ignore a11y_autofocus -->
      <input type="search" placeholder="Clubnaam of clubnummer" bind:value={query} autofocus aria-label="Zoek een club" />
      {#if error}<p class="error">{error}</p>{/if}
      {#if query.trim()}
        <ul class="list">{#each found.slice(0, MAX) as c (c.id)}{@render line(c)}{/each}</ul>
        {#if found.length > MAX}<p class="muted">Nog {found.length - MAX} clubs. Typ meer om de lijst korter te maken.</p>{/if}
        {#if !found.length && !loading && !error}<p class="muted">Geen club gevonden.</p>{/if}
        {#if !found.length && loading}<p class="muted">Clubs laden…</p>{/if}
      {:else if lastClub}
        <p class="muted">Laatst bekeken:</p>
        <ul class="list">{@render line(lastClub)}</ul>
      {/if}
    </div>
  </section>
</div>
