<script lang="ts">
  import logoLight from '../assets/vttl-logo.svg';
  import logoDark from '../assets/vttl-logo-dark.svg';

  /** The VTTL logo. The dark theme uses a version with light lines and a dark oval. */
  let { alt = '', width, height, class: cls = '' }: { alt?: string; width: number; height: number; class?: string } = $props();
</script>

<img class="vttl-logo on-dark {cls}" src={logoDark} {alt} {width} {height} />
<img class="vttl-logo on-light {cls}" src={logoLight} alt={alt} {width} {height} />
