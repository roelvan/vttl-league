<script lang="ts">
  import { session, loadMe, logout } from './lib/session.svelte';
  import { route, match, navigate, link } from './lib/router.svelte';
  import Login from './pages/Login.svelte';
  import WhoAmI from './pages/WhoAmI.svelte';
  import Matches from './pages/Matches.svelte';
  import MatchPage from './pages/MatchPage.svelte';

  loadMe();

  const needsMember = $derived(!!session.me && !session.me.account.member);
  const matchParams = $derived(match('/wedstrijd/:id'));

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
{:else}
  <header class="top">
    <a href="/" onclick={link} class="brand">VTTL League</a>
    {#if session.me.club}<span class="club">{session.me.club.name}</span>{/if}
    <span class="spacer"></span>
    <span class="who">{session.me.account.memberName ?? session.me.account.account}</span>
    <button class="ghost" onclick={logout}>Afmelden</button>
  </header>
  {#if needsMember || route.path === '/ik'}
    <WhoAmI />
  {:else if matchParams}
    <MatchPage matchId={matchParams.id} />
  {:else}
    <Matches />
  {/if}
{/if}
