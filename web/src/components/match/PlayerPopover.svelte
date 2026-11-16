<script module lang="ts">
  export interface Who {
    id: number;
    name: string;
    ranking: string;
    /** Team name, as on the match ("Zonnebeke A"). */
    team: string;
    /** Colour class of the side: "h" or "a". */
    cls: string;
  }
</script>

<script lang="ts">
  // The player widget: ELO and results of the season, in a popover next to the info button.
  // The name and the ranking come from the match and show at once; the rest comes from the
  // server (lib/players.svelte.ts), which keeps each player for a day.
  import { loadPlayer, players } from '../../lib/players.svelte';
  import { titleCase } from '../../lib/format';

  let el: HTMLDivElement | undefined = $state();
  let who = $state<Who | null>(null);
  let anchor: HTMLElement | null = null;
  let open = $state(false);
  /** The last close: a click on the button of the open player closes it, and must not open it again. */
  let closed = { id: 0, at: 0 };

  const entry = $derived(who ? players[who.id] : undefined);
  const info = $derived(entry?.info);
  const games = $derived(info ? info.wins + info.losses : 0);
  const ratio = $derived(info && games ? Math.round((info.wins / games) * 100) : null);

  /** Opens the widget for a player, next to `at` (the button). */
  export function show(at: HTMLElement, w: Who) {
    if (closed.id === w.id && performance.now() - closed.at < 250) return;
    who = w;
    anchor = at;
    void loadPlayer(w.id);
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
    const left = Math.min(Math.max(edge, a.right - w), window.innerWidth - w - edge);
    const below = a.bottom + gap;
    const top = below + h + edge <= window.innerHeight ? below : Math.max(edge, a.top - gap - h);
    el.style.left = `${Math.max(edge, left)}px`;
    el.style.top = `${top}px`;
  }

  /** `beforetoggle`, not `toggle`: that one comes after the click that closed the widget. */
  function onbeforetoggle(e: ToggleEvent) {
    open = e.newState === 'open';
    if (!open && who) closed = { id: who.id, at: performance.now() };
  }

  // The size changes when the data comes in.
  $effect(() => {
    void [info, entry?.loading, entry?.error, open];
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

  const opponent = (r: { firstName: string; lastName: string }) => `${titleCase(r.firstName)} ${titleCase(r.lastName)}`.trim();

  /** "3 u geleden": how old the VTTL data is. */
  function ago(at: number, now: number) {
    const min = Math.floor((now - at) / 60_000);
    if (min < 2) return 'zojuist';
    if (min < 60) return `${min} min geleden`;
    if (min < 48 * 60) return `${Math.floor(min / 60)} u geleden`;
    return `${Math.floor(min / 1440)} dagen geleden`;
  }
</script>

<div class="pp" popover="auto" role="dialog" aria-label={who ? `Spelerinfo ${who.name}` : 'Spelerinfo'} bind:this={el} {onbeforetoggle}>
  {#if who}
    <header class="pp-h">
      <span class="rank-tag {who.cls}">{info?.ranking || who.ranking || '–'}</span>
      <span class="pp-who">
        <span class="n">{who.name}</span>
        {#if who.team}<span class="t">{who.team}</span>{/if}
      </span>
    </header>

    {#if info}
      <dl class="pp-stats">
        <div><dt>ELO</dt><dd>{info.elo ?? '–'}</dd></div>
        <div><dt>Winst</dt><dd>{ratio === null ? '–' : `${ratio}%`}</dd></div>
        <div><dt>W–V</dt><dd>{info.wins}<span class="sep">-</span>{info.losses}</dd></div>
      </dl>

      {#if games}
        <h3>Tegen klassement</h3>
        <ul class="pp-ranks">
          {#each info.byRank as r (r.ranking)}
            <li><span class="rk">{r.ranking || '–'}</span><span class="wl">{r.wins}<span class="sep">-</span>{r.losses}</span></li>
          {/each}
        </ul>

        <h3>Laatste {info.last.length === 1 ? 'wedstrijd' : `${info.last.length} wedstrijden`}</h3>
        <ul class="pp-last">
          {#each info.last as r, i (i)}
            <li>
              <i class="pip {r.won ? 'w' : 'l'}"></i>
              <span class="sr-only">{r.won ? 'Gewonnen van' : 'Verloren van'}</span>
              <span class="n">{opponent(r)}</span>
              <span class="rk">{r.ranking}</span>
              <span class="sets">{r.setsFor}<span class="sep">-</span>{r.setsAgainst}</span>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="pp-note">Nog geen enkelwedstrijden dit seizoen.</p>
      {/if}
      <p class="pp-foot">VTTL · {ago(info.fetchedAt, Date.now())}</p>
    {:else if entry?.error}
      <p class="pp-note">De gegevens van VTTL laden niet. {entry.error}</p>
    {:else if info === null}
      <p class="pp-note">Nog geen gegevens van VTTL voor deze speler.</p>
    {:else}
      <div class="pp-load" aria-busy="true" aria-label="Laden">
        <span class="sk row"></span><span class="sk row"></span><span class="sk row"></span>
      </div>
    {/if}
  {/if}
</div>
