<script lang="ts">
  // The line-ups of a match, one column per team. A player in focus (pointer, keyboard or a
  // click that keeps it on) shows the result against each opponent: green = won, red = lost.
  // Used in the club night cards (compact) and in the Spelers card of the match page (`full`).
  import type { LineRow, Person, Side } from '../lib/live';
  import { tone } from '../lib/viewer.svelte';
  import Icon from './Icon.svelte';

  let {
    lineup,
    names,
    flip = false,
    full = false,
    pinned = $bindable(null),
    onfocusrow,
    oninfo,
  }: {
    lineup: Record<Side, LineRow[]>;
    names: Record<Side, string>;
    /** The own club plays away: the away team comes first, in the home colour. */
    flip?: boolean;
    /** The match page: full names and a dot per game. */
    full?: boolean;
    /** The row under the pointer or with the keyboard focus; null when it leaves. */
    onfocusrow?: (r: LineRow | null) => void;
    /** Key of the row that a tap or a click keeps on. */
    pinned?: string | null;
    /** Opens the player widget next to `at` (the ranking). Without it, the ranking is not a button. */
    oninfo?: (at: HTMLElement, p: Person, side: Side) => void;
  } = $props();

  /** Key of the row under the pointer or with the keyboard focus. */
  let hovered = $state<string | null>(null);

  const rows = $derived([...lineup.home, ...lineup.away]);
  const active = $derived(rows.find((r) => r.key === (hovered ?? pinned)) ?? null);
  const sides = $derived<Side[]>(flip ? ['away', 'home'] : ['home', 'away']);

  function hover(r: LineRow | null) {
    hovered = r?.key ?? null;
    onfocusrow?.(r);
  }

  /** "R. Vaneyghen": the initial and the surname. */
  const short = (r: LineRow) =>
    (r.double || full ? r.person.full : [r.person.initial, r.person.last].filter(Boolean).join(' ')) || '–';
  const fullName = (r: LineRow) => (r.double ? `Dubbel ${r.person.full}` : r.person.full || 'Nog niet opgesteld');

  /** For a screen reader: the same data as the colours. */
  function summary(r: LineRow) {
    const parts = rows.flatMap((o) => {
      const d = r.vs[o.key];
      if (!d || !d.result) return [];
      const who = fullName(o);
      if (d.result === 'live') return [`speelt nu tegen ${who}`];
      return [`${d.result === 'w' ? 'won van' : 'verloor van'} ${who} met ${d.own}–${d.other}`];
    });
    return `${fullName(r)}${r.person.ranking ? `, ${r.person.ranking}` : ''}, ${r.wins} gewonnen, ${r.losses} verloren${parts.length ? `: ${parts.join(', ')}` : ''}`;
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && (pinned = null)} />

<div class="nl" class:full class:has-active={!!active}>
  {#each sides as side (side)}
    {@const cls = tone(side === 'home' ? 'h' : 'a', flip)}
    <div class="nl-col">
      <div class="nl-h"><span class="n">{names[side]}</span><span>W–V</span></div>
      {#each lineup[side] as r (r.key)}
        {@const duel = active && active !== r ? (active.vs[r.key] ?? null) : null}
        <!-- The row is not a button: the info button is in it. The pick button lies over the row. -->
        <div
          class="nl-row {duel ? `is-${duel.result || 'todo'}` : ''}"
          class:on={active === r}
          class:pinned={pinned === r.key}
          class:off={!!active && active !== r && !duel}
          role="presentation"
          onpointerenter={(e) => e.pointerType === 'mouse' && hover(r)}
          onpointerleave={(e) => e.pointerType === 'mouse' && hover(null)}
        >
          <button
            class="nl-pick"
            type="button"
            aria-pressed={pinned === r.key}
            aria-label={summary(r)}
            title={fullName(r)}
            onclick={() => (pinned = pinned === r.key ? null : r.key)}
            onfocus={(e) => e.currentTarget.matches(':focus-visible') && hover(r)}
            onblur={() => hover(null)}
          ></button>
          <!-- The ranking opens the player widget. -->
          {#if oninfo && !r.double && r.person.id}
            <button
              class="rank-tag {cls}"
              type="button"
              aria-label="Spelerinfo {r.person.full}, {r.person.ranking || 'geen klassement'}"
              aria-haspopup="dialog"
              onclick={(e) => oninfo(e.currentTarget, r.person, side)}
            >{r.person.ranking || '–'}</button>
          {:else}
            <span class="rank-tag {cls}" class:pair={r.double}>{r.double ? 'D' : r.person.ranking || '–'}</span>
          {/if}
          <span class="n">{short(r)}</span>
          {#if full}<span class="pips">{#each r.pips as p, i (i)}<i class="pip {p}"></i>{/each}</span>{/if}
          {#if duel}
            <span class="res">
              {#if duel.result === 'w'}
                {#if duel.upset}<span class="upset"><Icon name="up" size={13} /></span>{/if}{duel.own}–{duel.other}
              {:else if duel.result === 'l'}
                {duel.own}–{duel.other}
              {:else if duel.result === 'live'}
                bezig
              {:else}
                later
              {/if}
            </span>
          {:else}
            <span class="wl" class:clean={r.wins > 0 && !r.losses}>
              {#if !active && r.upsets}<span class="upset" title="Won van een hoger klassement"><Icon name="up" size={13} /></span>{/if}
              {r.wins}<span class="sep">-</span>{r.losses}</span>
          {/if}
        </div>
      {/each}
    </div>
  {/each}
</div>
