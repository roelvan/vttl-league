<script lang="ts">
  import type { LineRow, Side } from '../lib/live';
  import type { NightCard } from '../lib/night';
  import { tone } from '../lib/viewer.svelte';
  import Icon from './Icon.svelte';

  let {
    lineup,
    names,
    flip = false,
  }: {
    lineup: NonNullable<NightCard['lineup']>;
    names: Record<Side, string>;
    /** The own club plays away: the away team comes first, in the home colour. */
    flip?: boolean;
  } = $props();

  /** Key of the row under the pointer or with the keyboard focus. */
  let hovered = $state<string | null>(null);
  /** Key of the row that a tap or a click keeps on. */
  let pinned = $state<string | null>(null);

  const rows = $derived([...lineup.home, ...lineup.away]);
  const active = $derived(rows.find((r) => r.key === (hovered ?? pinned)) ?? null);
  const sides = $derived<Side[]>(flip ? ['away', 'home'] : ['home', 'away']);
  const sideOf = (r: LineRow) => r.key.split(':')[0] as Side;

  /** "R. Vaneyghen": the initial and the surname. */
  const short = (r: LineRow) =>
    (r.double ? r.person.full : [r.person.initial, r.person.last].filter(Boolean).join(' ')) || '–';
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

<div class="nl" class:has-active={!!active}>
  {#each sides as side (side)}
    {@const cls = tone(side === 'home' ? 'h' : 'a', flip)}
    <div class="nl-col">
      <div class="nl-h"><span class="n">{names[side]}</span><span>W–V</span></div>
      {#each lineup[side] as r (r.key)}
        {@const duel = active && active !== r ? (active.vs[r.key] ?? null) : null}
        <button
          class="nl-row {duel ? `is-${duel.result || 'todo'}` : ''}"
          class:on={active === r}
          class:off={!!active && active !== r && !duel}
          type="button"
          aria-pressed={pinned === r.key}
          aria-label={summary(r)}
          title={fullName(r)}
          onclick={() => (pinned = pinned === r.key ? null : r.key)}
          onpointerenter={(e) => e.pointerType === 'mouse' && (hovered = r.key)}
          onpointerleave={(e) => e.pointerType === 'mouse' && (hovered = null)}
          onfocus={(e) => e.currentTarget.matches(':focus-visible') && (hovered = r.key)}
          onblur={() => (hovered = null)}
        >
          <span class="rank-tag {cls}" class:pair={r.double}>{r.double ? 'D' : r.person.ranking || '–'}</span>
          <span class="n">{short(r)}</span>
          {#if duel}
            <span class="res">
              {#if duel.result === 'w'}
                {#if duel.upset}<Icon name="up" size={13} />{/if}{duel.own}–{duel.other}
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
              {#if !active && r.upsets}<span class="upset" title="Won van een hoger klassement"><Icon name="up" size={14} /></span>{/if}
              {r.wins}<span class="sep">-</span>{r.losses}</span>
          {/if}
        </button>
      {/each}
    </div>
  {/each}
</div>
