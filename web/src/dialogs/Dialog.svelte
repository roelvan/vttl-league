<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    title,
    onclose,
    children,
    footer,
    wide = false,
  }: { title: string; onclose: () => void; children: Snippet; footer?: Snippet; wide?: boolean } = $props();

  let el: HTMLDialogElement;
  $effect(() => {
    el.showModal();
    return () => el.close();
  });
</script>

<dialog bind:this={el} class="dlg" class:wide onclose={onclose} oncancel={(e) => { e.preventDefault(); onclose(); }}>
  <header class="dlg-head">
    <h2>{title}</h2>
    <button class="x" aria-label="Sluiten" onclick={onclose}>×</button>
  </header>
  <div class="dlg-body">{@render children()}</div>
  {#if footer}<footer class="dlg-foot">{@render footer()}</footer>{/if}
</dialog>
