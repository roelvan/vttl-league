<script lang="ts">
  import Icon from './Icon.svelte';
  import { link } from '../lib/router.svelte';
  import { urls } from '../lib/nav';

  let { section, matchId, day }: { section: 'match' | 'night' | 'lineup' | 'other'; matchId: string | null; day: string } = $props();
</script>

<nav class="bottomnav" aria-label="Hoofdmenu">
  <a href={matchId ? urls.match(matchId) : undefined} onclick={link} class:disabled={!matchId} aria-disabled={!matchId || undefined} aria-current={section === 'match' ? 'page' : undefined}>
    <Icon name="match" size={22} />Wedstrijd
  </a>
  <a href={urls.day(day)} onclick={link} aria-current={section === 'night' ? 'page' : undefined}><Icon name="night" size={22} />Clubavond</a>
  <a href={matchId ? urls.lineup(matchId) : undefined} onclick={link} class:disabled={!matchId} aria-disabled={!matchId || undefined} aria-current={section === 'lineup' ? 'page' : undefined}>
    <Icon name="lineup" size={22} />Opstelling
  </a>
</nav>
