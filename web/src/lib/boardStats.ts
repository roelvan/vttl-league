// Statistics of one game counted on a scoreboard, from its point sequence.
//
// Same formulas as vttl-api (lib/mqtt/matchMessage.js `deriveMatchStats` and
// app/components/ScoreStats.js / GameDetail.js), so both apps show the same numbers.
// Side 0 = home = player 1 on the board, side 1 = away = player 2.

export type Side = 0 | 1;

export interface SideStats {
  /** Points won when this side served / points this side served. */
  serveWon: number;
  served: number;
  /** Points won. */
  points: number;
  /** Longest run of points in a row. */
  bestStreak: number;
  /** Largest lead within one set. */
  biggestLead: number;
}

export interface SetStats {
  /** 0-based set number. */
  index: number;
  score: [number, number];
  finished: boolean;
  sides: [SideStats, SideStats];
  /** Score after each point, starting at 0-0. */
  progression: [number, number][];
}

export type Moment = 'Matchpunt' | 'Setpunt' | 'Deuce' | null;

export interface PlayStats {
  sets: SetStats[];
  /** Whole game. Streaks carry across sets, as in vttl-api. */
  match: [SideStats, SideStats];
  /** The last 20 points, oldest first. */
  last: Side[];
  current: [number, number];
  setsWon: [number, number];
  /** Who serves the next point. */
  server: Side;
  finished: boolean;
  moment: Moment;
}

const blank = (): SideStats => ({ serveWon: 0, served: 0, points: 0, bestStreak: 0, biggestLead: 0 });

/** Who serves at score a-b in set `set`: the first server alternates per set; the serve
 *  changes every 2 points, and every point from 10-10 (same rule as src/scoreboard.rs). */
export function serverAt(firstServer: Side, set: number, a: number, b: number): Side {
  const setFirst = (firstServer ^ (set & 1)) as Side;
  const n = a + b;
  const switches = a >= 10 && b >= 10 ? 10 + (n - 20) : Math.floor(n / 2);
  return (setFirst ^ (switches & 1)) as Side;
}

/**
 * @param points '0' = home won the point, '1' = away (the board's bit sequence)
 * @param firstServer the board player (1 or 2) who served first in set 1
 * @param bestOf 5 or 7 sets
 */
export function playStats(points: string, firstServer: 1 | 2, bestOf: number): PlayStats {
  const need = Math.floor(bestOf / 2) + 1;
  const first = (firstServer - 1) as Side;
  const match: [SideStats, SideStats] = [blank(), blank()];
  const sets: SetStats[] = [];
  const setsWon: [number, number] = [0, 0];
  const all: Side[] = [];
  let finished = false;

  let set: SetStats = newSet(0);
  let matchStreak = 0;
  let setStreak = 0;
  let prev: Side | null = null;
  let setPrev: Side | null = null;

  for (const ch of points) {
    if (finished) break;
    const w: Side = ch === '1' ? 1 : 0;
    const [a, b] = set.score;
    const srv = serverAt(first, set.index, a, b);
    for (const s of [match[srv], set.sides[srv]]) {
      s.served++;
      if (w === srv) s.serveWon++;
    }
    match[w].points++;
    set.sides[w].points++;
    all.push(w);

    matchStreak = w === prev ? matchStreak + 1 : 1;
    prev = w;
    match[w].bestStreak = Math.max(match[w].bestStreak, matchStreak);
    setStreak = w === setPrev ? setStreak + 1 : 1;
    setPrev = w;
    set.sides[w].bestStreak = Math.max(set.sides[w].bestStreak, setStreak);

    set.score = w === 0 ? [a + 1, b] : [a, b + 1];
    set.progression.push(set.score);
    const lead = set.score[w] - set.score[1 - w];
    set.sides[w].biggestLead = Math.max(set.sides[w].biggestLead, lead);
    match[w].biggestLead = Math.max(match[w].biggestLead, lead);

    const [x, y] = set.score;
    if ((x >= 11 || y >= 11) && Math.abs(x - y) >= 2) {
      set.finished = true;
      sets.push(set);
      setsWon[x > y ? 0 : 1]++;
      finished = setsWon[0] === need || setsWon[1] === need;
      set = newSet(sets.length);
      setPrev = null;
      setStreak = 0;
    }
  }
  // The set in progress (also an empty one at 0-0 right after a set ended).
  if (!finished) sets.push(set);

  const current: [number, number] = finished ? [0, 0] : set.score;
  return {
    sets,
    match,
    last: all.slice(-20),
    current,
    setsWon,
    server: serverAt(first, set.index, current[0], current[1]),
    finished,
    moment: finished ? null : moment(current, setsWon, need),
  };
}

function newSet(index: number): SetStats {
  return { index, score: [0, 0], finished: false, sides: [blank(), blank()], progression: [[0, 0]] };
}

/** Same order as vttl-api `matchMoment`: match point, game point, deuce. */
function moment([a, b]: [number, number], won: [number, number], need: number): Moment {
  const leader = a >= 10 && a > b ? 0 : b >= 10 && b > a ? 1 : null;
  if (leader !== null) return won[leader] + 1 >= need ? 'Matchpunt' : 'Setpunt';
  if (a >= 10 && b >= 10) return 'Deuce';
  return null;
}

/** "54.5%", or "–" when the side did not serve yet (vttl-api shows one decimal). */
export const servePct = (s: SideStats) => (s.served ? `${((s.serveWon / s.served) * 100).toFixed(1)}%` : '–');

/** m:ss, or h:mm:ss from one hour. */
export function clock(ms: number): string {
  const t = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(t / 3600);
  const m = Math.floor((t % 3600) / 60);
  const s = String(t % 60).padStart(2, '0');
  return h ? `${h}:${String(m).padStart(2, '0')}:${s}` : `${m}:${s}`;
}

// ---------------------------------------------------------------- timeouts

/** The board's timeout state (src/scoreboard.rs `Timeout`). */
export interface BoardTimeout {
  active: boolean;
  /** 0 = general (referee), 1 = home, 2 = away. */
  owner: 0 | 1 | 2;
  p1Used: boolean;
  p2Used: boolean;
  expired: boolean;
  /** UTC epoch ms; 0 = the board does not know the time. */
  deadline: number;
}

/** Seconds left of a running timeout, or null when the board sent no deadline. Always from
 *  the absolute deadline (as vttl-api), so a phone that sleeps does not drift. */
export function timeoutLeft(t: BoardTimeout, now: number): number | null {
  if (t.expired) return 0;
  if (!t.deadline) return null;
  return Math.max(0, Math.ceil((t.deadline - now) / 1000));
}
