<script lang="ts">
  import Icon from './Icon.svelte';
  import { link } from '../lib/router.svelte';
  import { urls } from '../lib/nav';

  /** Only shown while a match is open: back to the club night, and the two views of the match. */
  let { section, matchId, day }: { section: 'match' | 'lineup'; matchId: string; day: string } = $props();
</script>

<nav class="bottomnav" aria-label="Wedstrijd">
  <a href={urls.day(day)} onclick={link}><Icon name="left" size={22} />Clubavond</a>
  <a href={urls.match(matchId)} onclick={link} aria-current={section === 'match' ? 'page' : undefined}><Icon name="match" size={22} />Wedstrijd</a>
  <a href={urls.lineup(matchId)} onclick={link} aria-current={section === 'lineup' ? 'page' : undefined}><Icon name="lineup" size={22} />Opstelling</a>
</nav>
