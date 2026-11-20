import { viewClub } from './viewer.svelte';

// URLs of the app. A VTTL match id has a slash ("PWVL03/045"); in a URL it becomes a dash.

export const matchSlug = (matchId: string) => matchId.replace('/', '-');

export function matchIdFromSlug(slug: string) {
  const i = slug.lastIndexOf('-');
  return i < 0 ? slug : `${slug.slice(0, i)}/${slug.slice(i + 1)}`;
}

/** The pages of a club: the same URL for everyone, with or without a login. */
export const clubPath = (club: string, rest = '') => `/c/${encodeURIComponent(club)}${rest === '/' ? '' : rest}`;

/** "/c/{club}" of the club on screen; "" while no club is known (the club search page). */
const prefix = () => {
  const c = viewClub();
  return c ? clubPath(c) : '';
};

/** Where a login returns to: only a path of this app. */
export const safeReturn = (to: string | null | undefined) => (to && to.startsWith('/') && !to.startsWith('//') && !to.startsWith('/login') ? to : '/');
export const loginUrl = (back: string) => `/login?terug=${encodeURIComponent(back)}`;

/** On a match URL: the match opened from the division page, so the sidebar shows the division. */
export const VIA_DIVISION = { key: 'lijst', value: 'afdeling' };
const via = (on: boolean) => (on ? `?${VIA_DIVISION.key}=${VIA_DIVISION.value}` : '');

export const urls = {
  home: () => prefix() || '/',
  day: (day: string) => `${prefix()}/dag/${day}`,
  match: (matchId: string, viaDivision = false) => `${prefix()}/wedstrijd/${matchSlug(matchId)}${via(viaDivision)}`,
  lineup: (matchId: string, viaDivision = false) => `${prefix()}/wedstrijd/${matchSlug(matchId)}/opstelling${via(viaDivision)}`,
  /** All matches of a division in a week; without a week, the week in progress. */
  division: (id: number, week?: string | null) => `${prefix()}/afdeling/${id}${week ? `/week/${encodeURIComponent(week)}` : ''}`,
  /** A match on the pages of another club (the home club has the live data). */
  matchAt: (club: string, matchId: string, viaDivision = false) => clubPath(club, `/wedstrijd/${matchSlug(matchId)}`) + via(viaDivision),
  settings: () => '/instellingen',
};

/** ISO day plus or minus n days. */
export function addDays(iso: string, n: number) {
  const [y, m, d] = iso.split('-').map(Number);
  const t = new Date(y, m - 1, d + n);
  return `${t.getFullYear()}-${String(t.getMonth() + 1).padStart(2, '0')}-${String(t.getDate()).padStart(2, '0')}`;
}

export type MenuAction = 'print' | 'slips' | 'validate' | 'upload' | 'share';
