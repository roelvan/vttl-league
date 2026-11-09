// View model for the live screens: what each game, table and player looks like right now.
// Pure functions of (sheet, match system), so they work for every system (singles, doubles,
// Super substitutes, team forfeits) and can be tested without a browser.

import type { Sheet, SetScore } from './sheet/types';
import { gameResult, gameSlots, setWinner, computeScore, isEmptySet, type Won } from './sheet/score';
import { getSystem, slotLabel, type MatchSystem } from './sheet/systems';
import { titleCase } from './format';
import type { BoardTimeout, Moment } from './boardStats';

export type Side = 'home' | 'away';
export type GameState = 'final' | 'live' | 'next' | 'later' | 'error';

export interface Person {
  /** "Jonas Debruyne" */
  full: string;
  /** "Debruyne" */
  last: string;
  /** "J." */
  initial: string;
  ranking: string;
}

export interface GameSide {
  /** Slot letters: "A", "W", "1/2" … */
  label: string;
  people: Person[];
  /** One entry per box: the points of this side, or "" when empty. */
  cells: string[];
  /** Per box: did this side win that set? null = not decided. */
  won: (boolean | null)[];
  sets: number;
}

export interface GameView {
  /** 0-based index in sys.games */
  index: number;
  /** 1-based number on the sheet */
  pos: number;
  double: boolean;
  home: GameSide;
  away: GameSide;
  result: Won;
  winner: 'h' | 'a' | null;
  state: GameState;
  /** Index in sheet.tables (fixed pairs), or null when no tables are set. */
  table: number | null;
  /** The first box that is still empty, while the game is live; else null. */
  currentBox: number | null;
}

export interface TableView {
  index: number;
  no: string;
  device: string | null;
  current: GameView | null;
  next: GameView | null;
}

/** Live state of the scoreboard on this table, for the game that is linked to it. */
export interface BoardLive {
  alias: string;
  online: boolean;
  /** The game index this board is linked to, or null. */
  linkedGame: number | null;
  /** Points in the current set, [home, away]. */
  points: [number, number] | null;
  /** Sets the board finished in the linked game (the match record gets them at the end). */
  sets: [number, number][];
  /** 'h' | 'a' = who serves. */
  server: 'h' | 'a' | null;
  /** The board's timeout state while the linked game runs. */
  timeout: BoardTimeout | null;
  /** Set point, match point or deuce in the linked game. */
  moment: Moment;
}

export interface LiveView {
  sys: MatchSystem;
  games: GameView[];
  homeWins: number;
  awayWins: number;
  /** Wins needed to take the evening. */
  needed: number;
  started: boolean;
  complete: boolean;
  hasErrors: boolean;
  tables: TableView[];
}

// ---------------------------------------------------------------- names

/** "DEBRUYNE Jonas" (VTTL sheet style: surname in capitals first) -> parts. */
export function person(name: string, ranking = ''): Person {
  const t = name.trim().split(/\s+/).filter(Boolean);
  if (!t.length) return { full: '', last: '', initial: '', ranking };
  let i = 0;
  while (i < t.length - 1 && /\p{L}/u.test(t[i]) && t[i] === t[i].toUpperCase()) i++;
  const last = titleCase(i ? t.slice(0, i).join(' ') : t.at(-1)!);
  const first = i ? t.slice(i).join(' ') : t.slice(0, -1).join(' ');
  return { full: first ? `${first} ${last}` : last, last, initial: first ? `${first[0]}.` : '', ranking };
}

const HOME_LETTERS = 'ABCD';
const AWAY_LETTERS4 = 'WXYZ';
const AWAY_LETTERS3 = 'XYZ';

/** Letter of a single-player slot, as on the design (home A–D, away W–Z). */
export function letter(sys: MatchSystem, slot: number, side: Side): string {
  const s = sys.slots[slot];
  if (!s) return '?';
  if (s.kind === 'double') return 'D';
  if (s.kind !== 'single') return '';
  if (sys.id === 8) return slotLabel(sys, slot, side === 'away');
  if (side === 'home') return HOME_LETTERS[s.player] ?? s.label;
  return (sys.playerCount <= 3 ? AWAY_LETTERS3 : AWAY_LETTERS4)[s.player] ?? s.label;
}

function slotPeople(sheet: Sheet, sys: MatchSystem, side: Side, slot: number): Person[] {
  const s = sys.slots[slot];
  const team = sheet[side];
  if (!s) return [];
  if (s.kind === 'single') {
    const p = team.players[s.player];
    return [person(p?.name ?? '', p?.ranking ?? '')];
  }
  if (s.kind === 'double') {
    const pair = team.doubles[s.double] ?? '';
    return pair
      .split('/')
      .map((x) => Number(x) - 1)
      .filter((i) => i >= 0)
      .map((i) => person(team.players[i]?.name ?? '', team.players[i]?.ranking ?? ''));
  }
  return [];
}

// ---------------------------------------------------------------- games

const isHomeWin = (w: Won) => w === 'home' || w === 'homeFF' || w === 'homeWO';
const isAwayWin = (w: Won) => w === 'away' || w === 'awayFF' || w === 'awayWO';

function sideOf(sheet: Sheet, sys: MatchSystem, gi: number, side: Side, sets: SetScore[]): GameSide {
  const slots = gameSlots(sheet, sys, gi, side);
  const people = slots.flatMap((s) => slotPeople(sheet, sys, side, s));
  const label =
    slots.length === 1 && sys.slots[slots[0]]?.kind === 'double'
      ? sheet[side].doubles[(sys.slots[slots[0]] as { double: number }).double] || 'Dubbel'
      : slots.map((s) => letter(sys, s, side)).join('+');
  const cells: string[] = [];
  const won: (boolean | null)[] = [];
  let count = 0;
  for (let b = 0; b < sys.boxes; b++) {
    const s = sets[b] ?? { l: '', r: '' };
    cells.push(side === 'home' ? s.l : s.r);
    if (isEmptySet(s)) {
      won.push(null);
      continue;
    }
    const w = setWinner(b, s, sys);
    const mine = side === 'home' ? isHomeWin(w) : isAwayWin(w);
    const theirs = side === 'home' ? isAwayWin(w) : isHomeWin(w);
    won.push(mine ? true : theirs ? false : null);
    if (mine) count++;
  }
  return { label, people, cells, won, sets: count };
}

export function liveView(sheet: Sheet): LiveView {
  const sys = getSystem(sheet.matchSystemId);
  const score = computeScore(sheet, sys);
  const tables = sheet.tables?.filter((t) => t.no.trim()) ?? [];
  const anyResult = sheet.results.some((g) => g?.some((s) => !isEmptySet(s)));
  const started = !!sheet.started || anyResult;

  const games: GameView[] = sys.games.map((g, gi) => {
    const sets = sheet.results[gi] ?? [];
    const r = gameResult(sets, sys, sheet.home.forfeit, sheet.away.forfeit);
    const home = sideOf(sheet, sys, gi, 'home', sets);
    const away = sideOf(sheet, sys, gi, 'away', sets);
    if (r.won === 'homeWO' || r.won === 'awayWO') {
      // A WO game: show the sets as the result, no boxes.
      home.sets = r.homeSets;
      away.sets = r.awaySets;
    }
    const decided = r.won !== 'empty' && r.won !== 'partial' && r.won !== 'error';
    const winner = isHomeWin(r.won) ? 'h' : isAwayWin(r.won) ? 'a' : null;
    const double = gameSlots(sheet, sys, gi, 'home').some((s) => sys.slots[s]?.kind === 'double') || g.home.length > 1;
    return {
      index: gi,
      pos: g.pos,
      double,
      home,
      away,
      result: r.won,
      winner: decided ? winner : null,
      state: r.won === 'error' ? 'error' : decided ? 'final' : r.won === 'partial' ? 'live' : 'later',
      table: tables.length ? gi % tables.length : null,
      currentBox: null,
    };
  });

  // Fixed pairs: per table, the first undecided game is live and the one after it is next.
  const tableViews: TableView[] = tables.map((t, ti) => {
    const queue = games.filter((g) => g.table === ti && g.state !== 'final');
    const current = started ? (queue[0] ?? null) : null;
    const next = started ? (queue[1] ?? null) : (queue[0] ?? null);
    return { index: ti, no: t.no, device: t.device, current, next };
  });
  for (const t of tableViews) {
    if (t.current && t.current.state === 'later') t.current.state = 'live';
    if (t.next && t.next.state === 'later') t.next.state = 'next';
  }
  for (const g of games) {
    if (g.state === 'live') {
      const sets = sheet.results[g.index] ?? [];
      const b = Array.from({ length: sys.boxes }, (_, i) => i).find((i) => isEmptySet(sets[i]));
      g.currentBox = b ?? null;
    }
  }

  const n = sys.games.length;
  return {
    sys,
    games,
    homeWins: score.homeWins,
    awayWins: score.awayWins,
    needed: Math.floor(n / 2) + 1,
    started,
    complete: score.winner === 'home' || score.winner === 'away' || score.winner === 'draw',
    hasErrors: score.winner === 'error',
    tables: tableViews,
  };
}

// ---------------------------------------------------------------- players tonight

export type Pip = 'w' | 'l' | 'live' | '';

export interface TallyRow {
  label: string;
  person: Person;
  pips: Pip[];
  wins: number;
  losses: number;
}

/** One row per single-player slot (and one per double slot), with a pip per game. */
export function tally(sheet: Sheet, view: LiveView, side: Side): TallyRow[] {
  const { sys } = view;
  const mine = side === 'home' ? 'h' : 'a';
  const rows: TallyRow[] = [];
  sys.slots.forEach((slot, si) => {
    if (slot.kind === 'sub') return;
    const games = view.games.filter((g) => gameSlots(sheet, sys, g.index, side).includes(si));
    if (!games.length) return;
    const pips: Pip[] = games.map((g) => (g.winner ? (g.winner === mine ? 'w' : 'l') : g.state === 'live' ? 'live' : ''));
    const people = slotPeople(sheet, sys, side, si);
    const p =
      slot.kind === 'double'
        ? { full: people.map((x) => x.last).join(' / ') || 'Dubbel', last: 'Dubbel', initial: '', ranking: '' }
        : (people[0] ?? person(''));
    rows.push({
      label: letter(sys, si, side),
      person: p,
      pips,
      wins: pips.filter((x) => x === 'w').length,
      losses: pips.filter((x) => x === 'l').length,
    });
  });
  return rows;
}

// ---------------------------------------------------------------- VTTL notation

/** Loser points per set, negative when the away side wins the set: "7/-4/8/5". */
export function notation(sets: { h: number; a: number }[]): string {
  return sets
    .filter((s) => Number.isFinite(s.h) && Number.isFinite(s.a) && s.h !== s.a)
    .map((s) => (s.h > s.a ? String(s.a) : `-${s.h}`))
    .join('/');
}
