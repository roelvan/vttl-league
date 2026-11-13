<script lang="ts">
  import { api, ApiError, OfflineError } from '../lib/api';
  import { session, setMe } from '../lib/session.svelte';
  import { navigate } from '../lib/router.svelte';
  import { playerName } from '../lib/format';
  import type { Member } from '../lib/types';
  import Credit from '../components/Credit.svelte';

  let query = $state('');
  let results = $state<Member[]>([]);
  let busy = $state(false);
  let error = $state('');
  let timer: ReturnType<typeof setTimeout>;
  let seq = 0;

  function onInput() {
    clearTimeout(timer);
    error = '';
    if (query.trim().length < 3) {
      results = [];
      return;
    }
    timer = setTimeout(search, 250);
  }

  async function search() {
    const mine = ++seq;
    busy = true;
    try {
      const r = await api.searchMembers(query.trim());
      if (mine === seq) results = r;
    } catch (e) {
      if (mine === seq) error = e instanceof ApiError || e instanceof OfflineError ? e.message : 'Zoeken mislukt.';
    } finally {
      if (mine === seq) busy = false;
    }
  }

  async function pick(m: Member) {
    busy = true;
    try {
      setMe(await api.setMember(m.uniqueIndex));
      navigate('/', true);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Opslaan mislukt.';
      busy = false;
    }
  }
</script>

<main class="shell single">
  <div class="narrow">
    <section class="card">
      <div class="card-h"><h2>Wie ben jij?</h2></div>
      <div class="card-b stack">
        <p class="muted">
          VTTL geeft de club van een account niet door. Zoek jezelf één keer op naam. De app toont daarna de wedstrijden van jouw club.
        </p>
        {#if session.me?.account.member}
          <p>Nu gekozen: <strong>{session.me.account.memberName}</strong> ({session.me.account.club})</p>
          <button class="btn ghost" type="button" onclick={() => navigate('/', true)}>← Terug naar de clubavond</button>
        {/if}
        <!-- svelte-ignore a11y_autofocus -->
        <input type="search" placeholder="Familienaam of voornaam" bind:value={query} oninput={onInput} autofocus aria-label="Zoek op naam" />
        {#if error}<p class="error">{error}</p>{/if}
        {#if busy && !results.length}<p class="muted">Zoeken…</p>{/if}
        <ul class="list">
          {#each results as m (m.uniqueIndex)}
            <li>
              <button class="row-btn" type="button" onclick={() => pick(m)} disabled={busy}>
                <strong class="grow">{playerName(m)}</strong>
                <span class="muted">{m.club}</span>
                <span class="rank">{m.ranking}</span>
              </button>
            </li>
          {/each}
        </ul>
        {#if !busy && query.trim().length >= 3 && !results.length && !error}<p class="muted">Niemand gevonden.</p>{/if}
      </div>
    </section>
    <Credit />
  </div>
</main>
