<script lang="ts">
  // The team widget: the line of a team in the standings of its division, in a popover next
  // to the place of the team. Same behaviour as the player widget (PlayerPopover).
  import { link } from '../lib/router.svelte';
  import { urls, clubPath } from '../lib/nav';
  import { shortDivision, placeClass } from '../lib/night';
  import type { RankingEntry } from '../lib/types';

  /** The division and the week of the team: a match row has them. */
  interface Where {
    divisionId: number;
    divisionName: string | null;
    week: string;
  }

  let el: HTMLDivElement | undefined = $state();
  let team = $state<RankingEntry | null>(null);
  let row = $state<Where | null>(null);
  let total = $state(0);
  let anchor: HTMLElement | null = null;
  let open = $state(false);
  /** The last close: a click on the button of the open team closes it, and must not open it again. */
  let closed = { key: '', at: 0 };

  const keyOf = (e: RankingEntry) => `${e.teamClub}|${e.team}`;
  const ratio = $derived(team && team.played ? Math.round((team.won / team.played) * 100) : null);

  /** Opens the widget for a team, next to `at` (the button). `of` = the number of teams. */
  export function show(at: HTMLElement, e: RankingEntry, r: Where, of: number) {
    if (closed.key === keyOf(e) && performance.now() - closed.at < 250) return;
    team = e;
    row = r;
    total = of;
    anchor = at;
    el?.showPopover();
    place();
  }

  /** Below the button, or above when there is no room; always inside the screen. */
  function place() {
    if (!el || !anchor || !open) return;
    const a = anchor.getBoundingClientRect();
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    const gap = 6;
    const edge = 8;
    const left = Math.min(Math.max(edge, a.left), window.innerWidth - w - edge);
    const below = a.bottom + gap;
    const top = below + h + edge <= window.innerHeight ? below : Math.max(edge, a.top - gap - h);
    el.style.left = `${Math.max(edge, left)}px`;
    el.style.top = `${top}px`;
  }

  /** `beforetoggle`, not `toggle`: that one comes after the click that closed the widget. */
  function onbeforetoggle(e: ToggleEvent) {
    open = e.newState === 'open';
    if (!open && team) closed = { key: keyOf(team), at: performance.now() };
  }

  $effect(() => {
    void [team, open];
    place();
  });

  // The popover does not follow the button: close it when the page moves.
  $effect(() => {
    if (!open) return;
    const away = (e: Event) => {
      if (e.target instanceof Node && el?.contains(e.target)) return;
      el?.hidePopover();
    };
    window.addEventListener('scroll', away, { capture: true, passive: true });
    window.addEventListener('resize', away);
    return () => {
      window.removeEventListener('scroll', away, { capture: true });
      window.removeEventListener('resize', away);
    };
  });

  const close = (e: MouseEvent) => {
    link(e);
    el?.hidePopover();
  };
</script>

<div class="pp tp" popover="auto" role="dialog" aria-label={team ? `Ploeginfo ${team.team}` : 'Ploeginfo'} bind:this={el} {onbeforetoggle}>
  {#if team && row}
    <header class="pp-h">
      <span class="rank-tag place {placeClass(team.position)}">{team.position}</span>
      <span class="pp-who">
        <span class="n">{team.team}</span>
        <span class="t">Plaats {team.position}{total ? ` van ${total}` : ''}{row.divisionName ? ` · ${shortDivision(row.divisionName)}` : ''}</span>
      </span>
    </header>

    <dl class="pp-stats">
      <div><dt>Punten</dt><dd>{team.points}</dd></div>
      <div><dt>Gespeeld</dt><dd>{team.played}</dd></div>
      <div><dt>Winst</dt><dd>{ratio === null ? '–' : `${ratio}%`}</dd></div>
      <div><dt><abbr title="Gewonnen, gelijk, verloren">W/G/V</abbr></dt><dd>{team.won}<span class="sep">/</span>{team.draw}<span class="sep">/</span>{team.lost}</dd></div>
      <div><dt><abbr title="Individuele wedstrijden: gewonnen en verloren">Wedstr.</abbr></dt><dd>{team.gamesWon}<span class="sep">/</span>{team.gamesLost}</dd></div>
      <div><dt>Sets</dt><dd>{team.setsWon}<span class="sep">/</span>{team.setsLost}</dd></div>
    </dl>

    <p class="tp-links">
      {#if row.divisionId}<a href={urls.division(row.divisionId, row.week)} onclick={close}>Afdeling</a>{/if}
      <a href={clubPath(team.teamClub)} onclick={close}>Clubavond</a>
    </p>
    <p class="pp-foot">Rangschikking van VTTL</p>
  {/if}
</div>
