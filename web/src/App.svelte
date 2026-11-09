<script lang="ts">
  import { session, loadMe } from './lib/session.svelte';
  import { route, navigate } from './lib/router.svelte';
  import { applyTheme } from './lib/settings.svelte';
  import Login from './pages/Login.svelte';
  import WhoAmI from './pages/WhoAmI.svelte';
  import Shell from './pages/Shell.svelte';

  applyTheme();
  loadMe();

  const needsMember = $derived(!!session.me && !session.me.account.member);

  $effect(() => {
    if (session.loading) return;
    if (!session.me && route.path !== '/login') navigate('/login', true);
    else if (session.me && route.path === '/login') navigate('/', true);
  });
</script>

{#if !session.online}
  <div class="offline" role="status">
    {session.offlineKind === 'portal'
      ? 'De wifi vraagt om opnieuw aan te melden. Je wijzigingen blijven bewaard op dit toestel.'
      : 'Geen verbinding. Je wijzigingen blijven bewaard op dit toestel.'}
  </div>
{/if}

{#if session.loading && !session.me}
  <main class="center"><p class="muted">Laden…</p></main>
{:else if !session.me}
  <Login />
{:else if needsMember || route.path === '/ik'}
  <WhoAmI />
{:else}
  <Shell />
{/if}
