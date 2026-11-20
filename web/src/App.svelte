<script lang="ts">
  import { session, loadMe } from './lib/session.svelte';
  import { route, navigate } from './lib/router.svelte';
  import { applyTheme } from './lib/settings.svelte';
  import { loginUrl, safeReturn } from './lib/nav';
  import Login from './pages/Login.svelte';
  import WhoAmI from './pages/WhoAmI.svelte';
  import Shell from './pages/Shell.svelte';

  applyTheme();
  loadMe();

  const needsMember = $derived(!!session.me && !session.me.account.member);
  /** "/c/{club}/…": the pages of a club, open for everyone without a login. */
  const isClubPath = $derived(route.path.startsWith('/c/'));
  /** Pages of the account itself: these need a login. */
  const isPrivatePath = $derived(route.path === '/ik' || route.path === '/instellingen');
  const here = $derived(route.path + (route.query.size ? `?${route.query}` : ''));

  $effect(() => {
    if (session.loading) return;
    if (!session.me && isPrivatePath) navigate(loginUrl(here), true);
    else if (session.me && route.path === '/login') {
      // Back to the page the login came from. A new account first says who it is.
      const back = safeReturn(route.query.get('terug'));
      navigate(session.me.account.member ? back : `/ik?terug=${encodeURIComponent(back)}`, true);
    }
  });
</script>

{#if !session.online}
  <div class="offline-banner" role="status">
    {session.offlineKind === 'portal'
      ? 'De wifi vraagt om opnieuw aan te melden. Je wijzigingen blijven bewaard op dit toestel.'
      : 'Geen verbinding. Je wijzigingen blijven bewaard op dit toestel.'}
  </div>
{/if}

{#if route.path === '/login'}
  {#if session.me}<main class="center"><p class="muted">Laden…</p></main>{:else}<Login />{/if}
{:else if isClubPath}
  <Shell />
{:else if session.loading && !session.me}
  <main class="center"><p class="muted">Laden…</p></main>
{:else if isPrivatePath && !session.me}
  <main class="center"><p class="muted">Laden…</p></main>
{:else if needsMember || route.path === '/ik'}
  <WhoAmI />
{:else}
  <Shell />
{/if}
