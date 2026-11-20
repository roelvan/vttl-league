<script lang="ts">
  // The heading of the club night and of the division page. It is also the switch between
  // them: a list with the club night and the divisions of the teams of the club.
  import Icon from './Icon.svelte';
  import { navigate } from '../lib/router.svelte';
  import { urls } from '../lib/nav';
  import { shortDivision } from '../lib/night';
  import type { Team } from '../lib/types';

  let {
    title,
    teams,
    current = null,
    week = null,
    clubHref,
  }: {
    title: string;
    /** The teams of the club on screen: they give the divisions of the list. */
    teams: Team[];
    /** The division on screen; null on the club night. */
    current?: number | null;
    /** The week on screen: the page that opens shows the same week. */
    week?: string | null;
    /** The club night that goes with this page. */
    clubHref: string;
  } = $props();

  /** One line per division, with the letters of the teams of the club in it. */
  const divisions = $derived.by(() => {
    const out: { id: number; name: string; letters: string[] }[] = [];
    for (const t of teams) {
      const d = out.find((x) => x.id === t.divisionId);
      if (d) d.letters.push(t.letter);
      else out.push({ id: t.divisionId, name: shortDivision(t.divisionName), letters: [t.letter] });
    }
    return out;
  });
  /** A division without a team of the club (a link from another club). */
  const other = $derived(current !== null && !divisions.some((d) => d.id === current));

  function pick(v: string) {
    navigate(v ? urls.division(Number(v), week) : clubHref);
  }
</script>

<h2 class="div-title">
  <label class="view-pick">
    <span class="vp-t">{title}</span><Icon name="chevron" size={18} />
    <select value={current === null ? '' : String(current)} onchange={(e) => pick(e.currentTarget.value)} aria-label="Kies de clubavond of een afdeling">
      <option value="">Clubavond</option>
      <optgroup label="Afdelingen">
        {#if other}<option value={String(current)}>{title}</option>{/if}
        {#each divisions as d (d.id)}<option value={String(d.id)}>{d.name} ({d.letters.join(', ')})</option>{/each}
      </optgroup>
    </select>
  </label>
</h2>
