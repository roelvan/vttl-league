<script lang="ts">
  import type { Person } from '../lib/sheet/types';
  import { change } from '../lib/workspace.svelte';

  let { id, field, title, person, onbrowse }: { id: string; field: string; title: string; person: Person; onbrowse: () => void } = $props();

  const set = (k: keyof Person) => (e: Event) => change(id, [{ p: [field, k], v: (e.currentTarget as HTMLInputElement).value }]);
</script>

<section class="box person">
  <div class="box-title"><span>{title}</span><button class="dots" title="Kiezen uit de sterktelijst" onclick={onbrowse}>…</button></div>
  <div class="person-grid">
    <label><span>Naam:</span><input class="ed" value={person.name} oninput={set('name')} /></label>
    <label><span>Club:</span><input class="ed" value={person.clubName} oninput={set('clubName')} /></label>
    <label><span>Aansluitingsnummer:</span><input class="ed" value={person.computerNumber} oninput={set('computerNumber')} /></label>
    <label><span>Clubnr:</span><input class="ed" value={person.clubId} oninput={set('clubId')} /></label>
  </div>
</section>
