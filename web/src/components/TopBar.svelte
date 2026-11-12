<script lang="ts">
  import Icon from './Icon.svelte';
  import { link, navigate } from '../lib/router.svelte';
  import { urls, type MenuAction } from '../lib/nav';
  import { settings, toggleTheme } from '../lib/settings.svelte';
  import { session, logout } from '../lib/session.svelte';
  import { ws, pendingCount } from '../lib/workspace.svelte';
  import { publicClub } from '../lib/viewer.svelte';
  import VttlLogo from './VttlLogo.svelte';

  let {
    matchId,
    day,
    datechip = '',
    onaction,
    printActions = false,
    matchActions = false,
  }: {
    matchId: string | null;
    /** The club night the brand link opens. */
    day: string;
    datechip?: string;
    onaction: (a: MenuAction) => void;
    /** Show print (a home match of our club is open, also when validated). */
    printActions?: boolean;
    /** Show validate / upload (a home match of our club that can still change). */
    matchActions?: boolean;
  } = $props();

  let open = $state(false);
  let wrap: HTMLElement | undefined = $state();

  const sync = $derived(!session.online ? 'offline' : pendingCount() ? 'pending' : ws.live ? 'live' : '');
  const syncText = $derived(
    !session.online ? 'Geen verbinding: wijzigingen blijven op dit toestel' : pendingCount() ? `${pendingCount()} wijziging(en) nog niet opgeslagen` : ws.live ? 'Live verbonden' : 'Niet live verbonden',
  );

  function pick(a: MenuAction) {
    open = false;
    onaction(a);
  }

  function onwindowclick(e: MouseEvent) {
    if (open && wrap && !wrap.contains(e.target as Node)) open = false;
  }
</script>

<svelte:window onclick={onwindowclick} onkeydown={(e) => e.key === 'Escape' && (open = false)} />

<header class="topbar">
  <div class="topbar-in">
    <a class="brand" href={urls.day(day)} onclick={link}><VttlLogo alt="VTTL: naar de clubavond" width={64} height={28} /></a>
    <span class="spacer"></span>
    {#if datechip}<span class="datechip">{datechip}</span>{/if}
    {#if session.me && !publicClub()}<span class="sync-dot {sync}" title={syncText} role="img" aria-label={syncText}></span>{/if}
    <button class="btn ghost icon-btn" type="button" onclick={toggleTheme} aria-label="Wissel licht of donker">
      <Icon name={settings.theme === 'dark' ? 'sun' : 'moon'} />
    </button>
    <div class="menu-wrap" bind:this={wrap}>
      <button class="btn ghost icon-btn" type="button" aria-label="Menu" aria-controls="top-menu" aria-expanded={open} onclick={() => (open = !open)}>
        <Icon name="more" />
      </button>
      {#if open}
        <div class="menu" id="top-menu">
          {#if session.me?.account.memberName}<div class="who">{session.me.account.memberName} · {session.me.account.club}{session.me.account.canUpload ? ' · kan uploaden' : ''}</div><hr />{/if}
          {#if printActions}
            <button type="button" onclick={() => pick('print')}><Icon name="print" />Wedstrijdblad</button>
            <button type="button" onclick={() => pick('slips')}><Icon name="print" />Scheidsrechterblaadjes</button>
            {#if matchActions}
              <button type="button" onclick={() => pick('validate')}><Icon name="check" />Valideren</button>
              <button type="button" onclick={() => pick('upload')}><Icon name="upload" />Upload naar VTTL</button>
            {/if}
            <hr />
          {/if}
          {#if matchId}<button type="button" onclick={() => pick('share')}><Icon name="share" />Publieke link kopiëren</button>{/if}
          {#if session.me && !publicClub()}
            <button type="button" onclick={() => ((open = false), navigate(urls.settings()))}><Icon name="settings" />Scoreborden</button>
            <button type="button" onclick={() => ((open = false), navigate('/ik'))}><Icon name="user" />Wie ben ik?</button>
          {/if}
          {#if session.me}
            <hr />
            <button type="button" onclick={() => ((open = false), logout())}><Icon name="logout" />Afmelden</button>
          {:else}
            <hr />
            <button type="button" onclick={() => ((open = false), navigate('/login'))}><Icon name="user" />Aanmelden</button>
          {/if}
        </div>
      {/if}
    </div>
  </div>
</header>
