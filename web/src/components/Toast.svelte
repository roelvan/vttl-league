<script lang="ts">
  import Icon from './Icon.svelte';
  import { ws } from '../lib/workspace.svelte';

  let shown = $state('');
  let timer: ReturnType<typeof setTimeout>;
  $effect(() => {
    const m = ws.message;
    if (!m || m.startsWith('Alle wijzigingen opgeslagen') || m.includes('wachten op verbinding')) return;
    shown = m;
    clearTimeout(timer);
    timer = setTimeout(() => (shown = ''), 6000);
  });
</script>

{#if shown}
  <div class="toast" role="status">
    <span>{shown}</span>
    <button class="x" type="button" aria-label="Sluiten" onclick={() => (shown = '')}><Icon name="close" size={18} /></button>
  </div>
{/if}

<style>
  .toast {
    position: fixed; left: 50%; transform: translateX(-50%); z-index: 45;
    bottom: calc(84px + env(safe-area-inset-bottom, 0px)); width: min(560px, calc(100vw - 32px));
    display: flex; gap: 10px; align-items: center; padding: 10px 10px 10px 14px;
    background: var(--surface); border: 1px solid var(--line); border-radius: 12px; box-shadow: var(--shadow); font-size: 15px;
  }
  .toast span { flex: 1; }
  @media (min-width: 1100px) { .toast { bottom: 24px; } }
</style>
