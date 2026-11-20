// Player data for the player widget (ELO, results of the season).
//
// The server keeps each player for a day and refreshes in the background, so a request here
// is cheap. This store only keeps what the session saw, so a second look shows at once.

import { api } from './api';
import type { PlayerInfo } from './types';
import { isPublic, publicClub } from './viewer.svelte';

export interface PlayerEntry {
  /** Undefined: not loaded yet. Null: the server has no data of this player. */
  info?: PlayerInfo | null;
  loading: boolean;
  error: string;
  /** When this device last asked the server (ms). */
  asked: number;
}

/** Do not ask the server again for the same player within this time. */
const ASK_AGAIN_MS = 60_000;

export const players = $state<Record<number, PlayerEntry>>({});

/** Gets a player from the server. What the store has stays on screen until the answer is there. */
export async function loadPlayer(id: number) {
  players[id] ??= { loading: false, error: '', asked: 0 };
  // Read it back: this is the reactive copy.
  const e = players[id];
  if (e.loading || Date.now() - e.asked < ASK_AGAIN_MS) return;
  e.loading = true;
  e.asked = Date.now();
  try {
    const club = publicClub();
    e.info = club ? await api.publicPlayer(club, id) : await api.player(id);
    e.error = '';
  } catch (err) {
    e.error = err instanceof Error ? err.message : 'Laden mislukt';
    // A failure is not a reason to wait: the next look tries again.
    e.asked = 0;
  } finally {
    e.loading = false;
  }
}

const warmed = new Set<string>();

/**
 * Someone opened a match: the server gets its players from VTTL in the background, so the
 * widget has data at the first click. Once per line-up and session; the server skips a player
 * that it got less than a day ago. For a spectator, the server only gets the players of a
 * line-up of the club.
 */
export function warmPlayers(ids: number[]) {
  if (!ids.length) return;
  const club = publicClub();
  if (isPublic() && !club) return;
  const key = `${club ?? ''}|${[...ids].sort((a, b) => a - b).join(',')}`;
  if (warmed.has(key)) return;
  warmed.add(key);
  (club ? api.publicWarmPlayers(club, ids) : api.warmPlayers(ids)).catch(() => warmed.delete(key));
}
