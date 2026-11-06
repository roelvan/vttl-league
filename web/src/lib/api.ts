// Fetch wrapper for the server API.
//
// The hall WiFi has a captive portal that asks for a new login about every 30 minutes.
// While it does, every request gets a redirect or an HTML page instead of our JSON.
// Those answers are reported as `offline` (kind "portal"), never as data.

import type { Me, Member, ClubInfo, MatchList, TeamMatch, PlayerCategory, MatchSystem, Season, Club } from './types';
import type { Sheet } from './sheet/types';
import type { Op } from './sheet/ops';

export interface StoredSheet {
  id: string;
  rev: number;
  day: string;
  season: number;
  doc: Sheet;
}

export class ApiError extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string,
  ) {
    super(message);
  }
}

export class OfflineError extends Error {
  constructor(public kind: 'network' | 'portal') {
    super(kind === 'portal' ? 'De wifi vraagt om opnieuw aan te melden.' : 'Geen verbinding met de server.');
  }
}

type Listener = (online: boolean, kind?: 'network' | 'portal') => void;
const listeners = new Set<Listener>();
export function onConnectivity(l: Listener) {
  listeners.add(l);
  return () => listeners.delete(l);
}
function report(online: boolean, kind?: 'network' | 'portal') {
  for (const l of listeners) l(online, kind);
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  let res: Response;
  try {
    res = await fetch(path, {
      method,
      redirect: 'manual',
      credentials: 'same-origin',
      headers: body === undefined ? { accept: 'application/json' } : { 'content-type': 'application/json', accept: 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  } catch {
    report(false, 'network');
    throw new OfflineError('network');
  }
  if (res.type === 'opaqueredirect' || (res.status >= 300 && res.status < 400)) {
    report(false, 'portal');
    throw new OfflineError('portal');
  }
  if (res.status === 204) {
    report(true);
    return undefined as T;
  }
  const type = res.headers.get('content-type') ?? '';
  if (!type.includes('application/json')) {
    // A portal page, or a proxy error page in front of the server.
    report(false, res.ok ? 'portal' : 'network');
    throw new OfflineError(res.ok ? 'portal' : 'network');
  }
  report(true);
  const data = await res.json();
  if (!res.ok) throw new ApiError(res.status, data.error ?? 'error', data.message ?? res.statusText);
  return data as T;
}

const q = (season?: number) => (season ? `?season=${season}` : '');

export const api = {
  login: (account: string, password: string) => request<Me>('POST', '/api/login', { account, password }),
  logout: () => request<void>('POST', '/api/logout'),
  me: () => request<Me>('GET', '/api/me'),
  setMember: (uniqueIndex: number | null) => request<Me>('POST', '/api/me/member', { uniqueIndex }),
  searchMembers: (name: string) => request<Member[]>('GET', `/api/members/search?q=${encodeURIComponent(name)}`),
  seasons: () => request<Season[]>('GET', '/api/seasons'),
  club: (season?: number) => request<ClubInfo>('GET', `/api/club${q(season)}`),
  clubMatches: (season?: number) => request<MatchList>('GET', `/api/club/matches${q(season)}`),
  match: (matchId: string, season?: number) =>
    request<TeamMatch>('GET', `/api/match?id=${encodeURIComponent(matchId)}${season ? `&season=${season}` : ''}`),
  members: (category: number, club?: string, season?: number) => {
    const p = new URLSearchParams({ category: String(category) });
    if (club) p.set('club', club);
    if (season) p.set('season', String(season));
    return request<Member[]>('GET', `/api/club/members?${p}`);
  },
  playerCategories: (season?: number) => request<PlayerCategory[]>('GET', `/api/player-categories${q(season)}`),
  matchSystems: () => request<MatchSystem[]>('GET', '/api/match-systems'),
  clubs: (season?: number) => request<Club[]>('GET', `/api/clubs${q(season)}`),
  sheetsOn: (day: string) => request<StoredSheet[]>('GET', `/api/sheets?day=${encodeURIComponent(day)}`),
  sheet: (id: string) => request<StoredSheet>('GET', `/api/sheet?id=${encodeURIComponent(id)}`),
  createSheet: (doc: Sheet) =>
    request<{ sheet: StoredSheet; created: boolean }>('PUT', '/api/sheet', { id: doc.id, day: doc.day, season: doc.season, doc }),
  upload: (id: string, data: string, partial: boolean) =>
    request<{ ok: boolean; dryRun: boolean; errors: string[] }>('POST', '/api/upload', { id, data, partial }),
  sheetOps: (id: string, clientId: string, seq: number, ops: Op[]) =>
    request<{ rev: number; seq: number }>('POST', '/api/sheet/ops', { id, clientId, seq, ops }),
};
