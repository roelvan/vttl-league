<script lang="ts">
  let { long = false }: { long?: boolean } = $props();
</script>

<p class="credits">
  Interclub Live volgt en scoort VTTL-ploegenwedstrijden live. De regels komen uit
  <a href="https://score.pieterp.be/" target="_blank" rel="noopener">ScoreSheet</a> van PieterP.be.
  {#if long}
    De regels voor de scores, de matchsystemen en de validaties komen uit de broncode van ScoreSheet
    (© 2024 PieterP.be, BSD-licentie). Dank aan het ScoreSheet-team voor hun werk voor het Belgische tafeltennis.
    <br /><span class="muted">Deze app is geen officieel product van PieterP.be of de VTTL. Gegevens komen van de VTTL TabT API.</span>
  {/if}
</p>
