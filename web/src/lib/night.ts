// Club night: one card per team match of the club on a day.

import type { MatchRow } from './types';
import type { Sheet } from './sheet/types';
import { liveView, lineup, type LineRow, type TableView } from './live';
import { byTeam } from './format';
import { SYSTEMS } from './sheet/systems';

function gameCount(row: MatchRow) {
  const id = row.matchType ?? row.matchSystem;
  return id != null ? (SYSTEMS[id]?.games.length ?? 0) : 0;
}

/** "Afdeling 1 - Prov. West-Vl. - Heren" -> "Afdeling 1 · Heren" */
export function shortDivision(name: string) {
  const p = name.split(' - ').map((x) => x.trim()).filter(Boolean);
  return p.length > 2 ? `${p[0]} · ${p.at(-1)}` : p.join(' · ');
}

export type NightState = 'live' | 'final' | 'soon' | 'bye';

export interface NightCard {
  row: MatchRow;
  division: string;
  isHome: boolean;
  homeScore: number | null;
  awayScore: number | null;
  /** One entry per game: who won it, or live. Empty when the match system is unknown. */
  segments: ('h' | 'a' | 'live' | '')[];
  played: number;
  total: number;
  state: NightState;
  tables: string;
  /** From our point of view. */
  outcome: 'won' | 'lost' | 'draw' | null;
  /** The tables with a game in progress (own home matches only). */
  live: TableView[];
  /** Sets of a game in this match system. */
  boxes: number;
  /** The players of the two teams with their games; null when no player is known. */
  lineup: { home: LineRow[]; away: LineRow[] } | null;
}

function parseScore(s: string): [number, number] | null {
  const m = /^\s*(\d+)\s*-\s*(\d+)/.exec(s);
  return m ? [Number(m[1]), Number(m[2])] : null;
}

/** The line-ups of a match record, when it has a player. */
function lineupOf(sheet: Sheet | undefined): NightCard['lineup'] {
  if (!sheet) return null;
  try {
    const v = liveView(sheet);
    const l = { home: lineup(sheet, v, 'home'), away: lineup(sheet, v, 'away') };
    return [...l.home, ...l.away].some((r) => !r.double && r.person.full) ? l : null;
  } catch {
    return null; // unknown match system
  }
}

/**
 * `sheet` is the stored record: the live truth of a home match. `known` is any record of the
 * match on this device (also a copy of the VTTL data): it gives the line-ups.
 */
export function nightCard(row: MatchRow, sheet: Sheet | undefined, known: Sheet | undefined = sheet): NightCard {
  const division = shortDivision(row.divisionName ?? '');
  if (row.isBye) {
    return { row, division, isHome: row.isHome, homeScore: null, awayScore: null, segments: [], played: 0, total: 0, state: 'bye', tables: '', outcome: null, live: [], boxes: 0, lineup: null };
  }
  let homeScore: number | null = null;
  let awayScore: number | null = null;
  let segments: NightCard['segments'] = [];
  let played = 0;
  let total = 0;
  let state: NightState = 'soon';
  let tables = row.isHome ? '' : 'Uit';
  let live: TableView[] = [];
  let boxes = 0;

  // Our own home matches: the stored record is the live truth. Away matches: VTTL only.
  if (sheet && row.isHome) {
    try {
      const v = liveView(sheet);
      homeScore = v.homeWins;
      awayScore = v.awayWins;
      total = v.games.length;
      played = v.games.filter((g) => g.state === 'final').length;
      segments = v.games.map((g) => (g.winner ?? (g.state === 'live' ? 'live' : '')));
      state = v.complete ? 'final' : v.started ? 'live' : 'soon';
      if (state === 'live') live = v.tables.filter((t) => t.current);
      boxes = v.sys.boxes;
    } catch {
      // Unknown match system: fall back to the VTTL row.
    }
  }
  if (homeScore === null) {
    const s = parseScore(row.score);
    total = gameCount(row);
    if (s) {
      [homeScore, awayScore] = s;
      played = s[0] + s[1];
      state = row.isValidated || row.isLocked || (total && played >= total) ? 'final' : 'live';
      // The real game order when VTTL has the individual results, else home wins first.
      const order = row.games ?? '';
      segments = Array.from({ length: Math.max(total, played, order.length) }, (_, i) =>
        order ? (order[i] === 'h' ? 'h' : order[i] === 'a' ? 'a' : '') : i < s[0] ? 'h' : i < played ? 'a' : '',
      );
    } else {
      // No score yet: 0–0 and one empty segment per game, as for a home match that did not start.
      homeScore = awayScore = 0;
      segments = Array.from({ length: total }, () => '' as const);
    }
  }
  if (homeScore !== null && state === 'soon' && (homeScore || awayScore)) state = 'live';

  const ours = row.isHome ? homeScore : awayScore;
  const theirs = row.isHome ? awayScore : homeScore;
  const outcome = state === 'final' && ours !== null && theirs !== null ? (ours > theirs ? 'won' : ours < theirs ? 'lost' : 'draw') : null;
  return { row, division, isHome: row.isHome, homeScore, awayScore, segments, played, total, state, tables, outcome, live, boxes, lineup: lineupOf(known) };
}

/** The matches of a day, best team first. */
export function matchesOn(rows: MatchRow[], day: string) {
  return rows.filter((m) => m.date === day).sort(byTeam);
}

/** The match days of the season, sorted. */
export function matchDays(rows: MatchRow[]) {
  return [...new Set(rows.filter((m) => m.date && !m.isBye).map((m) => m.date))].sort();
}
