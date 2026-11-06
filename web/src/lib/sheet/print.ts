// Data for the printed sheet and the referee slips, ported from
// PieterP.ScoreSheet.ViewModels/Templates/OfficialScoreTemplate.cs, RefereeTemplate.cs and RefereeLayouts.cs.

import type { Sheet } from './types';
import { slotLabel, type MatchSystem } from './systems';
import { gameSlots } from './score';

/** The ✗ boxes of the VTTL sheet. */
export function matchTypeMarks(s: Sheet) {
  return {
    interclubMen: (s.interclub || s.veterans) && s.men,
    interclubWomen: (s.interclub || s.veterans) && s.women,
    interclubYouth: s.youth,
    cupMen: s.cup && s.men,
    cupWomen: s.cup && s.women,
    super: s.super,
  };
}

export const LEVEL_NAME = { super: 'Super', national: 'Nationaal', regional: 'Landelijk', provincial: 'Provinciaal' } as const;

/** An empty comments box is crossed out on paper, so nothing can be added after signing. */
export const printedComments = (c: string) => (c ? c : 'X'.repeat(45));

/** "4", "Y", "D" (double) or "1/2" (free-time double): the players column of the game list. */
export function playersLabel(s: Sheet, sys: MatchSystem, gi: number, side: 'home' | 'away') {
  return gameSlots(s, sys, gi, side)
    .map((i) => (sys.slots[i].kind === 'double' ? 'D' : slotLabel(sys, i, side === 'away')))
    .join('/');
}

export interface Slip {
  title: string | null;
  home: string;
  away: string;
  number: string;
  referee: string | null;
}

function playerText(s: Sheet, sys: MatchSystem, gi: number, side: 'home' | 'away') {
  const parts: string[] = [];
  for (const i of gameSlots(s, sys, gi, side)) {
    const slot = sys.slots[i];
    if (slot.kind === 'single') {
      const p = s[side].players[slot.player];
      parts.push(`${p?.name ?? ''} (${p?.ranking ?? ''})`);
    } else if (slot.kind === 'double') parts.push('Dubbel');
  }
  return parts.join(' / ');
}

/** RefereeTemplate.TuutTuut: who referees each game of a 16-game match. */
function referee(pos: number) {
  return [1, 2, 7, 8, 9, 10, 15, 16].includes(pos) ? 'Scheidsrechter: thuis' : 'Scheidsrechter: uit';
}

export type SlipLayout = '2x4' | '1x8' | 'table';
export const SLIP_LAYOUTS: [SlipLayout, string][] = [
  ['2x4', 'Layout om te knippen (2x4)'],
  ['1x8', 'Layout om te knippen (1x8)'],
  ['table', 'Één blad per tafel'],
];

export interface SlipPage {
  /** Only for the per-table layout. */
  title: string | null;
  slips: Slip[];
}

/** Pages of 8 slips. Per table: odd games on table 1, even games on table 2 (more than 8 games). */
export function slipPages(s: Sheet, sys: MatchSystem, layout: SlipLayout): SlipPage[] {
  const sixteen = sys.games.length === 16;
  const teams = `${s.home.name} - ${s.away.name}`;
  const slip = (gi: number): Slip => ({
    title: layout === 'table' ? null : teams,
    home: playerText(s, sys, gi, 'home'),
    away: playerText(s, sys, gi, 'away'),
    number: String(sys.games[gi].pos),
    referee: sixteen ? referee(sys.games[gi].pos) : null,
  });
  const all = sys.games.map((_, i) => i);
  const groups = layout === 'table' && all.length > 8 ? [all.filter((i) => i % 2 === 0), all.filter((i) => i % 2 === 1)] : [all];
  const pages: SlipPage[] = [];
  for (const g of groups) {
    for (let i = 0; i < g.length; i += 8) pages.push({ title: layout === 'table' ? teams : null, slips: g.slice(i, i + 8).map(slip) });
  }
  return pages;
}

export interface PrintJob {
  id: string;
  what: 'sheet' | 'slips';
  layout: SlipLayout;
}
