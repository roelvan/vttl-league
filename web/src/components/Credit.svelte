<script lang="ts">
  const year = new Date().getFullYear();
</script>

<footer class="credit">
  © {year} · Built by <a href="https://vaneyghen.be" target="_blank" rel="noopener">vaneyghen.be</a>
</footer>

<style>
  .credit { text-align: center; font-size: 13px; color: var(--muted); padding: 8px 0; }
  .credit a { color: inherit; text-underline-offset: 2px; }
  .credit a:hover { color: var(--fg); }
</style>
