<script lang="ts">
  // A running board timeout: clock icon + countdown from the board's deadline (m:ss).
  import Icon from '../Icon.svelte';
  import { clock, timeoutLeft, type BoardTimeout } from '../../lib/boardStats';
  import { now } from '../../lib/clock.svelte';

  let { timeout, who = '', compact = false }: { timeout: BoardTimeout; who?: string; compact?: boolean } = $props();

  const left = $derived(timeoutLeft(timeout, now()));
  const text = $derived(left === null ? '' : clock(left * 1000));
  const spoken = $derived(`Time-out${who ? ` ${who}` : ''}${left === null ? '' : `, nog ${left} seconden`}`);
</script>

<span class="tmo" class:compact class:zero={left === 0} class:h={timeout.owner === 1} class:a={timeout.owner === 2} role="timer" aria-label={spoken}>
  <Icon name="timer" size={compact ? 13 : 15} />{#if text}<b>{text}</b>{/if}
</span>
