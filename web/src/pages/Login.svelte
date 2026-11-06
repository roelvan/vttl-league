<script lang="ts">
  import { api, ApiError, OfflineError } from '../lib/api';
  import { setMe } from '../lib/session.svelte';

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
    <h1>VTTL League</h1>
    <p class="muted">Meld je aan met je VTTL-account (competitie.vttl.be).</p>
    <label>Login <input bind:value={account} autocomplete="username" autocapitalize="off" required /></label>
    <label>Wachtwoord <input type="password" bind:value={password} autocomplete="current-password" required /></label>
    {#if error}<p class="error">{error}</p>{/if}
    <button type="submit" disabled={busy}>{busy ? 'Bezig…' : 'Aanmelden'}</button>
  </form>
</main>
