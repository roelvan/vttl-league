// Match systems, ported from PieterP.ScoreSheet.ViewModels/Score/MatchSystems/*.cs.
// The id is the TabT MatchType / MatchSystem id.

export type Slot =
  | { kind: 'single'; label: string; optional: boolean; player: number }
  | { kind: 'double'; options: string[]; double: number }
  | { kind: 'sub'; sub: number; /** slot index of the player who comes in */ transferable: number };

export interface Game {
  pos: number;
  /** Slot indexes. One slot for a single (or a double slot), two for a free-time double. */
  home: number[];
  away: number[];
  allowSub: boolean;
}

export interface MatchSystem {
  id: number;
  name: string;
  /** Sets needed to win a game. */
  setCount: number;
  pointCount: number;
  /** Score boxes per game: setCount * 2 - 1. */
  boxes: number;
  competitive: boolean;
  slots: Slot[];
  games: Game[];
  playerCount: number;
  doubleCount: number;
  subCount: number;
}

const P123 = ['1/2', '1/3', '2/3'];
const P12 = ['1/2'];
const P1234 = ['1/2', '1/3', '1/4', '2/3', '2/4', '3/4'];

type GameDef = [number | number[], number | number[], boolean?];

function build(
  id: number,
  name: string,
  setCount: number,
  pointCount: number,
  competitive: boolean,
  slots: Slot[],
  games: GameDef[],
): MatchSystem {
  const arr = (x: number | number[]) => (Array.isArray(x) ? x : [x]);
  return {
    id,
    name,
    setCount,
    pointCount,
    boxes: setCount * 2 - 1,
    competitive,
    slots,
    games: games.map(([h, a, sub], i) => ({ pos: i + 1, home: arr(h), away: arr(a), allowSub: !!sub })),
    playerCount: slots.filter((s) => s.kind === 'single').length,
    doubleCount: slots.filter((s) => s.kind === 'double').length,
    subCount: slots.filter((s) => s.kind === 'sub').length,
  };
}

const singles = (n: number, labels?: string[], optionalFrom = Infinity): Slot[] =>
  Array.from({ length: n }, (_, i) => ({ kind: 'single', label: labels?.[i] ?? String(i + 1), optional: i >= optionalFrom, player: i }));
const dbl = (options: string[], double = 0): Slot => ({ kind: 'double', options, double });

// Game order of the 4-player interclub (h = home slot, a = away slot, 0-based).
const MEN: GameDef[] = [
  [3, 1], [2, 0], [1, 3], [0, 2], [3, 0], [2, 1], [1, 2], [0, 3],
  [3, 2], [2, 3], [1, 0], [0, 1], [3, 3], [2, 2], [1, 1], [0, 0],
];
const WOMEN: GameDef[] = [[2, 1], [1, 0], [0, 2], [2, 0], [1, 2], [0, 1], [3, 3], [2, 2], [1, 1], [0, 0]];

export const SYSTEMS: Record<number, MatchSystem> = {};
for (const s of [
  build(1, 'Interclub Heren (oud systeem)', 2, 21, true, singles(4), MEN),
  build(2, 'Interclub Heren', 3, 11, true, singles(4), MEN),
  build(3, 'Interclub Dames/Veteranen (oud systeem)', 2, 21, true, [...singles(3), dbl(P123)], WOMEN),
  build(4, 'Interclub Dames/Veteranen', 3, 11, true, [...singles(3), dbl(P123)], WOMEN),
  build(5, 'Beker van België', 3, 11, true, [...singles(3), dbl(P123)], [[0, 1], [1, 0], [2, 2], [3, 3], [0, 0], [2, 1], [1, 2]]),
  build(6, 'Jeugd (2x2)', 3, 11, true, [...singles(2), dbl(P12)], [[1, 0], [0, 1], [2, 2], [1, 1], [0, 0]]),
  build(7, 'Vrijetijdscompetitie', 3, 11, false, singles(3), [
    [[0, 1], [0, 1]], [2, 2], [0, 1], [1, 0], [[0, 2], [0, 2]], [1, 1], [2, 0], [0, 2], [[1, 2], [1, 2]], [0, 0], [2, 1], [1, 2],
  ]),
  build(
    8,
    'Super',
    3,
    11,
    true,
    [...singles(4, ['A', 'B', 'C', 'V'], 3), { kind: 'sub', sub: 0, transferable: 3 }],
    [[0, 1], [1, 0], [2, 2], [0, 0, true], [2, 1, true], [1, 2, true]],
  ),
  build(9, 'Beker van Vlaanderen', 3, 11, true, singles(3), [[2, 1], [1, 0], [0, 2], [2, 0], [1, 2], [0, 1], [2, 2], [1, 1], [0, 0]]),
  build(-2, 'Beker van de AFTT', 4, 11, false, [...singles(4), dbl(P1234)], [[0, 0], [1, 1], [4, 4], [0, 1], [1, 0]]),
]) {
  SYSTEMS[s.id] = s;
}

/** The away side of the Super system uses X/Y/Z/V. */
export function slotLabel(sys: MatchSystem, slot: number, away: boolean): string {
  const s = sys.slots[slot];
  if (s.kind !== 'single') return '';
  if (sys.id === 8 && away) return ['X', 'Y', 'Z', 'V'][s.player];
  return s.label;
}

export function getSystem(id: number): MatchSystem {
  const s = SYSTEMS[id];
  if (!s) throw new Error(`Het vereiste matchsysteem (${id}) kon niet gevonden worden.`);
  return s;
}
