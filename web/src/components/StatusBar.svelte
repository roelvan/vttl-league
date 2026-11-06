<script lang="ts">
  import { ws, pendingCount } from '../lib/workspace.svelte';
  import { session } from '../lib/session.svelte';
  import { settings, saveSettings } from '../lib/settings.svelte';

  const pending = $derived(pendingCount());
  const state = $derived(
    !session.online ? 'offline' : pending > 0 ? 'pending' : ws.live ? 'live' : 'online',
  );
  const label = { offline: 'Offline', pending: 'Wijzigingen versturen…', live: 'Live verbonden', online: 'Verbonden' };
</script>

<footer class="statusbar">
  <span class="sb-dot {state}" aria-hidden="true"></span>
  <span class="sb-state">{label[state]}{pending ? ` (${pending})` : ''}</span>
  <span class="sb-msg">{ws.message}</span>
  <span class="sb-who">{session.me?.account.memberName ?? ''} · {session.me?.club?.name ?? ''}</span>
  <label class="sb-zoom">Zoom:
    <select bind:value={settings.zoom} onchange={saveSettings}>
      {#each [0.75, 1, 1.25, 1.5, 2] as z (z)}<option value={z}>{z * 100}%</option>{/each}
    </select>
  </label>
</footer>
