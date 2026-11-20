<script lang="ts">
  // A spinner for data that still loads. The text is for a screen reader only.
  let { label = 'Laden' }: { label?: string } = $props();
</script>

<span class="loading" role="status"><span class="spinner"></span><span class="sr-only">{label}…</span></span>
