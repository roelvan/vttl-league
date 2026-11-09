<script lang="ts">
  // Bottom sheet on phones, centred dialog from 720px (design: score entry sheet).
  // A native modal <dialog>: the focus stays inside and the page behind it is inert.
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

  let el: HTMLDialogElement | undefined = $state();
  const titleId = `dlg-${Math.random().toString(36).slice(2, 8)}`;

  $effect(() => {
    if (!el) return;
    const before = document.activeElement as HTMLElement | null;
    document.body.style.overflow = 'hidden';
    el.showModal();
    // Focus the first field, else the sheet itself (not the close button).
    const first = el.querySelector<HTMLElement>('input:not([readonly]), select, textarea');
    (first ?? el).focus({ preventScroll: true });
    return () => {
      document.body.style.overflow = '';
      before?.focus?.({ preventScroll: true });
    };
  });

  /** Escape: the parent decides (it removes the dialog), so the native close is cancelled. */
  function oncancel(e: Event) {
    e.preventDefault();
    onclose();
  }

  /** A click on the backdrop lands on the <dialog> itself, outside its box. */
  function onclick(e: MouseEvent) {
    if (e.target !== el) return;
    const r = el.getBoundingClientRect();
    if (e.clientX < r.left || e.clientX > r.right || e.clientY < r.top || e.clientY > r.bottom) onclose();
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<dialog class="sheet" class:wide aria-labelledby={titleId} tabindex="-1" bind:this={el} {oncancel} {onclick}>
  <div class="grab"></div>
  <div class="sheet-h">
    <h2 id={titleId}>{title}</h2>
    {#if chip}<span class="chip">{chip}</span>{/if}
    <span class="spacer"></span>
    <button class="x" type="button" aria-label="Sluiten" onclick={onclose}><Icon name="close" size={18} /></button>
  </div>
  {@render children()}
  {#if footer}<div class="sheet-f">{@render footer()}</div>{/if}
</dialog>
