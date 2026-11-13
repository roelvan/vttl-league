<script lang="ts">
  import { api, ApiError, OfflineError } from '../lib/api';
  import { setMe } from '../lib/session.svelte';
  import VttlLogo from '../components/VttlLogo.svelte';
  import Credit from '../components/Credit.svelte';

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
    <VttlLogo class="logo" alt="VTTL – Vlaamse Tafeltennisliga" width={190} height={83} />
    <p class="muted">Meld je aan met je VTTL-account.</p>
    <label class="field">Login <input bind:value={account} autocomplete="username" autocapitalize="off" required /></label>
    <label class="field">Wachtwoord <input type="password" bind:value={password} autocomplete="current-password" required /></label>
    {#if error}<p class="error">{error}</p>{/if}
    <button class="btn big" type="submit" disabled={busy}>{busy ? 'Bezig…' : 'Aanmelden'}</button>
  </form>
  <Credit />
</main>
