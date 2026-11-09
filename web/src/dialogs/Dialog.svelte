<script lang="ts">
  // Bottom sheet on phones, centred dialog from 720px (design: score entry sheet).
  import type { Snippet } from 'svelte';
  import Icon from '../components/Icon.svelte';

  let {
    title,
    chip = '',
    onclose,
    children,
    footer,
    wide = false,
  }: { title: string; chip?: string; onclose: () => void; children: Snippet; footer?: Snippet; wide?: boolean } = $props();

  let el: HTMLElement | undefined = $state();
  let before: Element | null = null;

  $effect(() => {
    before = document.activeElement;
    document.body.style.overflow = 'hidden';
    // Focus the first field, else the sheet itself.
    queueMicrotask(() => {
      const first = el?.querySelector<HTMLElement>('input:not([readonly]), select, textarea');
      (first ?? el)?.focus({ preventScroll: true });
    });
    return () => {
      document.body.style.overflow = '';
      (before as HTMLElement | null)?.focus?.({ preventScroll: true });
    };
  });

  function onkey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      onclose();
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="scrim" onclick={(e) => e.target === e.currentTarget && onclose()} onkeydown={onkey}>
  <div class="sheet" class:wide role="dialog" aria-modal="true" aria-label={title} tabindex="-1" bind:this={el}>
    <div class="grab"></div>
    <div class="sheet-h">
      <h3>{title}</h3>
      {#if chip}<span class="chip">{chip}</span>{/if}
      <span class="spacer"></span>
      <button class="x" type="button" aria-label="Sluiten" onclick={onclose}><Icon name="close" size={18} /></button>
    </div>
    {@render children()}
    {#if footer}<div class="sheet-f">{@render footer()}</div>{/if}
  </div>
</div>
