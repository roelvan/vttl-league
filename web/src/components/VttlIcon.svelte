<script lang="ts">
  import { STATUS_TEXT, type VttlStatus } from '../lib/sheet/vttl';

  let { status, title }: { status: VttlStatus | 'played' | null; title?: string } = $props();

  const glyph: Record<string, string> = { synced: '✓', pending: '↑', error: '!', locked: '🔒', played: '✓' };
  const text = $derived(title ?? (status === 'played' ? 'Uitslag staat op VTTL' : status ? STATUS_TEXT[status as VttlStatus] : ''));
</script>

{#if status && status !== 'none'}
  <span class="vttl-icon {status}" title={text} aria-label={text} role="img">{glyph[status]}</span>
{/if}
