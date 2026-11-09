<script lang="ts">
  import { api, ApiError, OfflineError } from '../lib/api';
  import { setMe } from '../lib/session.svelte';
  import Credits from '../components/Credits.svelte';

  let account = $state('');
  let password = $state('');
  let busy = $state(false);
  let error = $state('');

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    error = '';
    try {
      setMe(await api.login(account, password));
    } catch (err) {
      if (err instanceof ApiError && err.status === 401) error = 'Login of wachtwoord is fout.';
      else if (err instanceof ApiError || err instanceof OfflineError) error = err.message;
      else error = 'Aanmelden mislukt.';
    } finally {
      busy = false;
    }
  }
</script>

<main class="center">
  <form class="card login" onsubmit={submit}>
    <div class="brand">
      <svg viewBox="0 0 32 32" aria-hidden="true"><circle cx="20" cy="12" r="7" fill="var(--home)" /><path d="M4 26 14 16" stroke="var(--fg)" stroke-width="3.2" stroke-linecap="round" /><circle cx="9" cy="22" r="5.5" fill="none" stroke="var(--fg)" stroke-width="2.6" /></svg>
      <h1>Interclub Live</h1>
    </div>
    <p class="muted" style="margin:0">Meld je aan met je VTTL-account (competitie.vttl.be).</p>
    <label class="field">Login <input bind:value={account} autocomplete="username" autocapitalize="off" required /></label>
    <label class="field">Wachtwoord <input type="password" bind:value={password} autocomplete="current-password" required /></label>
    {#if error}<p class="error" style="margin:0">{error}</p>{/if}
    <button class="btn big" type="submit" disabled={busy}>{busy ? 'Bezig…' : 'Aanmelden'}</button>
    <Credits />
  </form>
</main>
