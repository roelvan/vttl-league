<script lang="ts">
  import Icon from './Icon.svelte';
  import { link } from '../lib/router.svelte';
  import { urls } from '../lib/nav';

  /** Only shown while a match is open: back to the club night, and the two views of the match. */
  let {
    section,
    matchId,
    day,
    lineup = true,
  }: { section: 'match' | 'lineup'; matchId: string; day: string; /** The line-up page: only for members of the home club. */ lineup?: boolean } = $props();
</script>

<nav class="bottomnav" class:two={!lineup} aria-label="Wedstrijd">
  <a href={urls.day(day)} onclick={link}><Icon name="left" size={22} />Clubavond</a>
  <a href={urls.match(matchId)} onclick={link} aria-current={section === 'match' ? 'page' : undefined}><Icon name="match" size={22} />Wedstrijd</a>
  {#if lineup}<a href={urls.lineup(matchId)} onclick={link} aria-current={section === 'lineup' ? 'page' : undefined}><Icon name="lineup" size={22} />Opstelling</a>{/if}
</nav>
