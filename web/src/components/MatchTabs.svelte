<script lang="ts">
  import { link } from '../lib/router.svelte';
  import { urls } from '../lib/nav';

  /** The two views of one match. Shown on wide screens; on phones the bottom bar has the same links. */
  let { section, matchId }: { section: 'match' | 'lineup'; matchId: string } = $props();
</script>

<nav class="match-tabs hide-mobile" aria-label="Wedstrijd">
  <a href={urls.match(matchId)} onclick={link} aria-current={section === 'match' ? 'page' : undefined}>Live</a>
  <a href={urls.lineup(matchId)} onclick={link} aria-current={section === 'lineup' ? 'page' : undefined}>Opstelling</a>
</nav>
