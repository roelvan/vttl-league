// Scoreboards (Flippr / ESP32) of the club and their live state.
//
// A board is linked to one game of one match when a captain taps "Naar scorebord": the server
// sends the player names to the board (MQTT config topic) and stores the link. Updates from a
// board without a link are ignored by the server. Player 1 on the board is always the home side.

import { api, ApiError } from './api';
import { ws } from './workspace.svelte';
import { publicClub } from './viewer.svelte';
import type { Sheet } from './sheet/types';
import type { BoardLive, GameView, LiveView, TableView } from './live';

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
  finished: boolean;
  /** Epoch ms of the last update. */
  at: number;
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

export function boardFor(matchId: string, device: string): BoardLive {
  const s = boards.live[device];
  const d = boards.devices.find((x) => x.id === device);
  const seen = Math.max(d?.lastSeen ?? 0, s?.at ?? 0);
  const linked = s && s.sheet === matchId ? s.game : null;
  return {
    alias: d?.alias || device,
    online: !!seen && Date.now() - seen < ONLINE_MS,
    linkedGame: linked,
    points: linked !== null && s && !s.finished ? s.current : null,
    server: linked !== null && s ? (s.server === 1 ? 'h' : 'a') : null,
  };
}

/** Live points per game index, for the tiles of the match order. */
export function livePointsFor(matchId: string, view: LiveView): Record<number, [number, number]> {
  const out: Record<number, [number, number]> = {};
  for (const t of view.tables) {
    const s = t.device ? boards.live[t.device] : undefined;
    if (s && s.sheet === matchId && s.game !== null && !s.finished) out[s.game] = s.current;
  }
  return out;
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
