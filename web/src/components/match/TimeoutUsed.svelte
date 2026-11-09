<script lang="ts">
  // The team took its timeout: a solid rounded square with the "T" cut out (the card shows through).
  let { size = 18 }: { size?: number } = $props();
</script>

<svg class="tmo-used" width={size} height={size} viewBox="0 0 20 20" role="img" aria-label="Time-out genomen">
  <title>Time-out genomen</title>
  <path fill-rule="evenodd" d="M5 0h10a5 5 0 0 1 5 5v10a5 5 0 0 1-5 5H5a5 5 0 0 1-5-5V5a5 5 0 0 1 5-5ZM4.5 4.5v3.2h3.9V16h3.2V7.7h3.9V4.5Z" />
</svg>
