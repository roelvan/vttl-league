<script lang="ts">
  import Icon from './Icon.svelte';
  import { STATUS_TEXT, type VttlStatus } from '../lib/sheet/vttl';

  let { status, title }: { status: VttlStatus | 'played' | null; title?: string } = $props();

  const glyph = { synced: 'check', pending: 'upload', error: 'warn', locked: 'lock', played: 'check' } as const;
  const text = $derived(title ?? (status === 'played' ? 'Uitslag staat op VTTL' : status ? STATUS_TEXT[status as VttlStatus] : ''));
</script>

{#if status && status !== 'none'}
  <span class="vttl-icon {status}" title={text} aria-label={text} role="img"><Icon name={glyph[status as keyof typeof glyph]} size={12} /></span>
{/if}
