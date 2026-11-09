// Scoreboards (Flippr / ESP32) of the club and their live state.
//
// A board is linked to one game of one match when a captain taps "Naar scorebord": the server
// sends the player names to the board (MQTT config topic) and stores the link. Updates from a
// board without a link are ignored by the server. Player 1 on the board is always the home side.

import { api, ApiError } from './api';
import { ws } from './workspace.svelte';
import { publicClub } from './viewer.svelte';
import type { BoardPlay, Sheet } from './sheet/types';
import type { BoardLive, GameView, LiveView, TableView } from './live';
import { playStats, type BoardTimeout } from './boardStats';

export interface Device {
  id: string;
  alias: string;
  /** Epoch ms of the last MQTT message, or null. */
  lastSeen: number | null;
}

export interface BoardState {
  device: string;
  /** The linked match record and game index, or null. */
  sheet: string | null;
  game: number | null;
  /** Points of the set in progress: [player 1 = home, player 2 = away]. */
  current: [number, number];
  /** Finished sets. */
  games: [number, number][];
  /** 1 = home serves, 2 = away. */
  server: 1 | 2;
  /** Who served first in set 1: 1 = home, 2 = away. */
  firstServer: 1 | 2;
  /** Every point in order: '0' = home won it, '1' = away. */
  points: string;
  /** Match time on the board at the last update. */
  durationMs: number;
  /** null = the board did not send timeout data. */
  timeout: BoardTimeout | null;
  finished: boolean;
  /** Epoch ms (server time) of the last update. */
  at: number;
}

/** A game with a point sequence: live from the board, or stored after the game. */
export interface Play extends BoardPlay {
  live: boolean;
  /** Live only: server time of the last update, to let the duration run on. */
  at: number | null;
  /** Live only: the board's timeout state. */
  timeout: BoardTimeout | null;
}

/** A board counts as online when it sent something in the last 2 minutes. */
const ONLINE_MS = 2 * 60 * 1000;

export const boards = $state({
  devices: [] as Device[],
  live: {} as Record<string, BoardState>,
  loaded: false,
});

export async function loadDevices() {
  const pub = publicClub();
  const [devices, states] = pub
    ? await api.publicBoards(pub).then((r) => [r.devices, r.states] as const)
    : await Promise.all([api.devices(), api.boardStates().catch(() => [] as BoardState[])]);
  boards.devices = devices;
  for (const s of states) boards.live[s.device] = s;
  boards.loaded = true;
}

export async function saveDevice(id: string, alias: string) {
  await api.saveDevice(id.trim(), alias.trim());
  await loadDevices();
}

export async function removeDevice(id: string) {
  await api.deleteDevice(id);
  await loadDevices();
}

export const deviceAlias = (id: string) => boards.devices.find((d) => d.id === id)?.alias || id;

/** Called by the WebSocket for every board message. */
export function onBoardMessage(msg: { device: string; state?: BoardState | null; lastSeen?: number }) {
  if (msg.state) boards.live[msg.device] = msg.state;
  else if (msg.state === null) delete boards.live[msg.device];
  const d = boards.devices.find((x) => x.id === msg.device);
  if (d && msg.lastSeen) d.lastSeen = msg.lastSeen;
}

/** @param bestOf sets of a game in this match system (view.sys.boxes) */
export function boardFor(matchId: string, device: string, bestOf: number): BoardLive {
  const s = boards.live[device];
  const d = boards.devices.find((x) => x.id === device);
  const seen = Math.max(d?.lastSeen ?? 0, s?.at ?? 0);
  const linked = s && s.sheet === matchId ? s.game : null;
  const running = linked !== null && !!s && !s.finished;
  return {
    alias: d?.alias || device,
    online: !!seen && Date.now() - seen < ONLINE_MS,
    linkedGame: linked,
    points: running ? s.current : null,
    sets: running ? s.games : [],
    server: linked !== null && s ? (s.server === 1 ? 'h' : 'a') : null,
    timeout: running ? (s.timeout ?? null) : null,
    moment: running && s.points !== undefined ? playStats(s.points, s.firstServer, bestOf).moment : null,
  };
}

/** What the tiles of the match order show of a game on a board. */
export interface LiveGame {
  points: [number, number];
  /** Sets the board finished; the match record gets them when the game ends. */
  sets: [number, number][];
  timeout: BoardTimeout | null;
}

/** Live board data per game index, for the tiles of the match order. */
export function liveGamesFor(matchId: string, view: LiveView): Record<number, LiveGame> {
  const out: Record<number, LiveGame> = {};
  for (const t of view.tables) {
    const s = t.device ? boards.live[t.device] : undefined;
    if (s && s.sheet === matchId && s.game !== null && !s.finished) out[s.game] = { points: s.current, sets: s.games, timeout: s.timeout ?? null };
  }
  return out;
}

/** Per game index: [home, away] took their timeout. Live from the board while the game runs,
 *  else what the server stored when the game ended. Games without board data are left out. */
export function timeoutsUsedFor(matchId: string, sheet: Sheet, view: LiveView): Record<number, [boolean, boolean]> {
  const out: Record<number, [boolean, boolean]> = {};
  sheet.boardPlay?.forEach((p, i) => {
    if (p?.timeoutsUsed) out[i] = p.timeoutsUsed;
  });
  for (const t of view.tables) {
    const s = t.device ? boards.live[t.device] : undefined;
    if (s && s.sheet === matchId && s.game !== null && !s.finished && s.timeout) out[s.game] = [s.timeout.p1Used, s.timeout.p2Used];
  }
  return out;
}

/** The point sequence of a game: live from its board, else what the server stored when the
 *  game ended. Null when no scoreboard counted the game. */
export function playFor(matchId: string, sheet: Sheet, view: LiveView, game: number): Play | null {
  const stored = sheet.boardPlay?.[game];
  for (const t of view.tables) {
    const s = t.device ? boards.live[t.device] : undefined;
    if (!s || s.sheet !== matchId || s.game !== game || s.points === undefined) continue;
    // A finished game: the stored copy wins; the board state bridges the moment before it arrives.
    if (s.finished && stored) break;
    const used: [boolean, boolean] | null = s.timeout ? [s.timeout.p1Used, s.timeout.p2Used] : null;
    return { points: s.points, firstServer: s.firstServer, durationMs: s.durationMs, timeoutsUsed: used, live: !s.finished, at: s.at, timeout: s.timeout };
  }
  return stored ? { ...stored, live: false, at: null, timeout: null } : null;
}

const upper = (s: string) => s.toUpperCase();

/** Sends the players of a game to the board of its table and links the board to that game. */
export async function sendToBoard(matchId: string, _sheet: Sheet, table: TableView, game: GameView) {
  if (!table.device) return;
  const side = (g: GameView['home']) => ({
    first: upper(g.people.length === 1 ? g.people[0].full.slice(0, g.people[0].full.length - g.people[0].last.length).trim() : ''),
    last: upper(g.people.map((p) => p.last).join('/') || g.label),
  });
  try {
    const st = await api.linkBoard({
      device: table.device,
      sheet: matchId,
      game: game.index,
      player1: side(game.home),
      player2: side(game.away),
    });
    boards.live[table.device] = st;
    ws.message = `Spelers van wedstrijd ${game.pos} naar ${deviceAlias(table.device)} gestuurd`;
  } catch (e) {
    ws.message = e instanceof ApiError ? `Scorebord: ${e.message}` : 'Het scorebord is niet bereikbaar.';
  }
}
