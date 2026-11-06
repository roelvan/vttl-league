// Score rules, ported from PieterP.ScoreSheet.ViewModels/Score/ScoreCalculator.cs.
//
// A box holds a number, "WO" (the player did not come: only in set 1) or "FF" (the player
// gave up: the remaining sets go to the opponent). Left = home, right = away.

import type { Sheet, SetScore } from './types';
import type { MatchSystem } from './systems';

/** 'partial' = some sets are entered and the game is not decided yet (still playing). */
export type Won = 'empty' | 'partial' | 'home' | 'homeFF' | 'homeWO' | 'away' | 'awayFF' | 'awayWO' | 'neither' | 'error';
export type Winner = 'home' | 'away' | 'draw' | 'error' | 'incomplete';

const isHome = (w: Won) => w === 'home' || w === 'homeFF' || w === 'homeWO';
const isAway = (w: Won) => w === 'away' || w === 'awayFF' || w === 'awayWO';

export const isEmptySet = (s: SetScore | undefined) => !s || (s.l === '' && s.r === '');

/** Who won one set. Team forfeits are handled by the caller. */
export function setWinner(index: number, s: SetScore, sys: MatchSystem): Won {
  const left = s.l.trim().toUpperCase();
  const right = s.r.trim().toUpperCase();
  if (index > 0 && (left === 'WO' || right === 'WO')) return 'error';
  if (left === 'WO' && right === 'WO') return 'neither';
  if (left === 'FF' || left === 'WO') {
    if (right.length > 0) return 'error';
    return left === 'FF' ? 'awayFF' : 'awayWO';
  }
  if (right === 'FF' || right === 'WO') {
    if (left.length > 0) return 'error';
    return right === 'FF' ? 'homeFF' : 'homeWO';
  }
  if (!/^\d+$/.test(left) || !/^\d+$/.test(right)) return 'error';
  const home = Number(left);
  const away = Number(right);
  const pc = sys.pointCount;
  if (home > away) {
    if (home < pc || (home > pc && home - away !== 2) || home - away < 2) return 'error';
    return 'home';
  }
  if (away < pc || (away > pc && away - home !== 2) || away - home < 2) return 'error';
  return 'away';
}

export interface GameResult {
  won: Won;
  homeSets: number;
  awaySets: number;
  /** Per box: false when the box is wrong (shown in red). */
  valid: boolean[];
}

export function gameResult(sets: SetScore[], sys: MatchSystem, homeForfeit: boolean, awayForfeit: boolean): GameResult {
  const valid = Array.from({ length: sys.boxes }, () => true);
  if (homeForfeit) return { won: 'awayWO', homeSets: 0, awaySets: 0, valid };
  if (awayForfeit) return { won: 'homeWO', homeSets: 0, awaySets: 0, valid };
  const boxes = Array.from({ length: sys.boxes }, (_, i) => sets[i] ?? { l: '', r: '' });
  if (boxes.every(isEmptySet)) return { won: 'empty', homeSets: 0, awaySets: 0, valid };

  let result: Won = 'empty';
  let hs = 0;
  let as = 0;
  for (let i = 0; i < boxes.length; i++) {
    const s = boxes[i];
    if (result !== 'empty') {
      // The game is decided: later boxes must be empty.
      if (!isEmptySet(s)) {
        valid[i] = false;
        result = 'error';
      }
      continue;
    }
    if (isEmptySet(s) && boxes.slice(i).every(isEmptySet)) {
      // Not finished yet. (ScoreSheet marks the next box as wrong here; while a game
      // is entered live, that is only noise.)
      result = 'partial';
      break;
    }
    const w = setWinner(i, s, sys);
    if (w === 'error') {
      valid[i] = false;
      result = 'error';
    } else if (isHome(w)) {
      if (hs < sys.setCount) {
        hs++;
        if (hs === sys.setCount || w === 'homeFF' || w === 'homeWO') result = w;
      } else {
        valid[i] = false;
        result = 'error';
      }
    } else if (isAway(w)) {
      if (as < sys.setCount) {
        as++;
        if (as === sys.setCount || w === 'awayFF' || w === 'awayWO') result = w;
      } else {
        valid[i] = false;
        result = 'error';
      }
    } else if (w === 'neither') {
      result = 'neither';
    }
  }
  if (result === 'homeWO') [hs, as] = [sys.setCount, 0];
  else if (result === 'awayWO') [hs, as] = [0, sys.setCount];
  else if (result === 'homeFF') hs = sys.setCount;
  else if (result === 'awayFF') as = sys.setCount;
  return { won: result, homeSets: hs, awaySets: as, valid };
}

/** Slot indexes that play a game on one side, with the Super substitute applied. */
export function gameSlots(sheet: Sheet, sys: MatchSystem, gameIndex: number, side: 'home' | 'away'): number[] {
  const g = sys.games[gameIndex];
  const team = sheet[side];
  for (const slot of sys.slots) {
    if (slot.kind === 'sub' && team.substitutes[slot.sub] === g.pos && g.allowSub) return [slot.transferable];
  }
  return side === 'home' ? g.home : g.away;
}

export interface GameRow {
  result: GameResult;
  /** Running team score after this game; null when the game is empty or wrong. */
  homeTotal: number | null;
  awayTotal: number | null;
}

export interface Score {
  games: GameRow[];
  homeWins: number;
  awayWins: number;
  winner: Winner;
  /** Per slot index: "3", "2+WO", "WO" … (singles and double slots). */
  homeIndividual: string[];
  awayIndividual: string[];
  homeIsWO: boolean[];
  awayIsWO: boolean[];
}

export const isByeClub = (c: string) => ['-', 'bye', 'vrij'].includes(c.trim().toLowerCase());

export function computeScore(sheet: Sheet, sys: MatchSystem): Score {
  const hf = sheet.home.forfeit;
  const af = sheet.away.forfeit;
  const bye = isByeClub(sheet.home.clubId) || isByeClub(sheet.away.clubId);
  const n = sys.slots.length;
  const wins = { home: new Array(n).fill(0), away: new Array(n).fill(0) };
  const woWins = { home: new Array(n).fill(0), away: new Array(n).fill(0) };
  let home = 0;
  let away = 0;
  let errors = false;
  let incomplete = false;
  const games: GameRow[] = [];

  const emptyRow = (): GameRow => ({
    result: { won: 'empty', homeSets: 0, awaySets: 0, valid: Array.from({ length: sys.boxes }, () => true) },
    homeTotal: null,
    awayTotal: null,
  });

  if (bye || hf || af) {
    // No games are played: every box must stay empty.
    if (!bye) {
      home = hf ? 0 : sys.games.length;
      away = hf ? sys.games.length : 0;
    }
    sys.games.forEach((_, i) => {
      const row = emptyRow();
      const sets = sheet.results[i] ?? [];
      row.result.valid = row.result.valid.map((_, b) => isEmptySet(sets[b]));
      if (i === sys.games.length - 1) {
        row.homeTotal = home;
        row.awayTotal = away;
      }
      games.push(row);
    });
  } else {
    sys.games.forEach((_, i) => {
      const r = gameResult(sheet.results[i] ?? [], sys, false, false);
      const row: GameRow = { result: r, homeTotal: null, awayTotal: null };
      if (r.won === 'empty' || r.won === 'partial') {
        incomplete = true;
      } else if (r.won === 'error') {
        errors = true;
      } else {
        if (isHome(r.won)) {
          addWinner(r.won === 'homeWO' ? woWins.home : wins.home, gameSlots(sheet, sys, i, 'home'));
          home++;
        } else if (isAway(r.won)) {
          addWinner(r.won === 'awayWO' ? woWins.away : wins.away, gameSlots(sheet, sys, i, 'away'));
          away++;
        }
        row.homeTotal = home;
        row.awayTotal = away;
      }
      games.push(row);
    });
  }

  const winner: Winner = errors ? 'error' : incomplete ? 'incomplete' : home === away ? 'draw' : home > away ? 'home' : 'away';
  const homeIsWO = sys.slots.map((s, i) => s.kind === 'single' && isPlayerWO(sheet, sys, 'home', i));
  const awayIsWO = sys.slots.map((s, i) => s.kind === 'single' && isPlayerWO(sheet, sys, 'away', i));
  const individual = (side: 'home' | 'away', isWO: boolean[]) =>
    sys.slots.map((s, i) => {
      if (s.kind === 'sub') return '';
      if (isWO[i]) return 'WO';
      const wo = woWins[side][i];
      return `${wins[side][i]}${wo === 0 ? '' : wo === 1 ? '+WO' : `+${wo}WO`}`;
    });
  return {
    games,
    homeWins: home,
    awayWins: away,
    winner,
    homeIndividual: individual('home', homeIsWO),
    awayIndividual: individual('away', awayIsWO),
    homeIsWO,
    awayIsWO,
  };
}

function addWinner(counts: number[], slots: number[]) {
  if (slots.length === 1) counts[slots[0]]++; // only individual games count
}

/**
 * A player is WO when the team forfeits, or when every game of that player was lost by
 * WO in set 1 (or nobody won). A player without games (a Super substitute) is not WO.
 */
export function isPlayerWO(sheet: Sheet, sys: MatchSystem, side: 'home' | 'away', slot: number): boolean {
  if (sheet[side].forfeit) return true;
  let played = 0;
  for (let i = 0; i < sys.games.length; i++) {
    if (!gameSlots(sheet, sys, i, side).includes(slot)) continue;
    played++;
    const s = sheet.results[i]?.[0] ?? { l: '', r: '' };
    const w = sheet.home.forfeit ? 'awayWO' : sheet.away.forfeit ? 'homeWO' : setWinner(0, s, sys);
    const lostByWO = side === 'home' ? w === 'awayWO' : w === 'homeWO';
    if (!lostByWO && w !== 'neither') return false;
  }
  return played !== 0;
}
