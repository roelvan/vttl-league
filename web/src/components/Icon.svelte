<script lang="ts">
  // Line icons from the design (24x24, stroke = currentColor).
  type Name =
    | 'match' | 'night' | 'lineup' | 'sun' | 'moon' | 'more' | 'plus' | 'close' | 'chevron' | 'left' | 'right'
    | 'print' | 'upload' | 'check' | 'settings' | 'user' | 'logout' | 'info' | 'board' | 'send' | 'warn' | 'share' | 'refresh';
  let { name, size = 20 }: { name: Name; size?: number } = $props();
</script>

<svg viewBox="0 0 24 24" width={size} height={size} fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  {#if name === 'match'}
    <rect x="3" y="6" width="18" height="12" rx="2" /><path d="M12 6v12M3 12h18" opacity=".5" />
  {:else if name === 'night'}
    <path d="M4 6h16M4 12h16M4 18h10" />
  {:else if name === 'lineup'}
    <circle cx="9" cy="8" r="3.2" /><path d="M3 20c0-3.3 2.7-6 6-6s6 2.7 6 6" /><path d="M16 4.5a3.2 3.2 0 0 1 0 6.3M18 14.3c1.8.8 3 2.6 3 4.7" />
  {:else if name === 'sun'}
    <circle cx="12" cy="12" r="4" /><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" />
  {:else if name === 'moon'}
    <path d="M20 14.5A8 8 0 0 1 9.5 4 8 8 0 1 0 20 14.5Z" />
  {:else if name === 'more'}
    <circle cx="5" cy="12" r="1.4" fill="currentColor" /><circle cx="12" cy="12" r="1.4" fill="currentColor" /><circle cx="19" cy="12" r="1.4" fill="currentColor" />
  {:else if name === 'plus'}
    <path d="M12 5v14M5 12h14" stroke-width="2.4" />
  {:else if name === 'close'}
    <path d="M6 6l12 12M18 6 6 18" stroke-width="2.4" />
  {:else if name === 'chevron' || name === 'right'}
    <path d="m9 6 6 6-6 6" />
  {:else if name === 'left'}
    <path d="m15 6-6 6 6 6" />
  {:else if name === 'print'}
    <path d="M7 9V3h10v6" /><rect x="3" y="9" width="18" height="8" rx="2" /><path d="M7 14h10v7H7z" />
  {:else if name === 'upload'}
    <path d="M12 16V4M7 9l5-5 5 5" /><path d="M4 16v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3" />
  {:else if name === 'check'}
    <path d="M4 12.5 9 17.5 20 6.5" />
  {:else if name === 'settings'}
    <circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1Z" />
  {:else if name === 'user'}
    <circle cx="12" cy="8" r="4" /><path d="M4 21c0-4.4 3.6-8 8-8s8 3.6 8 8" />
  {:else if name === 'logout'}
    <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4M16 17l5-5-5-5M21 12H9" />
  {:else if name === 'info'}
    <circle cx="12" cy="12" r="9" /><path d="M12 11v6M12 7.5v.5" />
  {:else if name === 'board'}
    <circle cx="12" cy="12" r="1.6" fill="currentColor" /><path class="wave" d="M8.5 8.5a5 5 0 0 0 0 7M15.5 8.5a5 5 0 0 1 0 7" /><path class="wave outer" d="M5.5 5.5a9.2 9.2 0 0 0 0 13M18.5 5.5a9.2 9.2 0 0 1 0 13" />
  {:else if name === 'send'}
    <path d="M22 2 11 13M22 2l-7 20-4-9-9-4 20-7Z" />
  {:else if name === 'warn'}
    <path d="M12 3 2 20h20L12 3Z" /><path d="M12 10v4M12 17v.5" />
  {:else if name === 'share'}
    <path d="M10 14a5 5 0 0 0 7 0l3-3a5 5 0 0 0-7-7l-1 1" /><path d="M14 10a5 5 0 0 0-7 0l-3 3a5 5 0 0 0 7 7l1-1" />
  {:else if name === 'refresh'}
    <path d="M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7" />
  {/if}
</svg>
