import { publicClub } from './viewer.svelte';

// URLs of the app. A VTTL match id has a slash ("PWVL03/045"); in a URL it becomes a dash.

export const matchSlug = (matchId: string) => matchId.replace('/', '-');

export function matchIdFromSlug(slug: string) {
  const i = slug.lastIndexOf('-');
  return i < 0 ? slug : `${slug.slice(0, i)}/${slug.slice(i + 1)}`;
}

/** "/c/{club}" on public pages (another club, or no login), else "". */
const prefix = () => {
  const c = publicClub();
  return c ? `/c/${encodeURIComponent(c)}` : '';
};

export const urls = {
  home: () => prefix() || '/',
  day: (day: string) => `${prefix()}/dag/${day}`,
  match: (matchId: string) => `${prefix()}/wedstrijd/${matchSlug(matchId)}`,
  lineup: (matchId: string) => `${prefix()}/wedstrijd/${matchSlug(matchId)}/opstelling`,
  settings: () => '/instellingen',
  /** The link to share: always with the club, so it works for everyone without a login. */
  share: (club: string, matchId: string | null, day: string) =>
    `/c/${encodeURIComponent(club)}${matchId ? `/wedstrijd/${matchSlug(matchId)}` : `/dag/${day}`}`,
};

/** ISO day plus or minus n days. */
export function addDays(iso: string, n: number) {
  const [y, m, d] = iso.split('-').map(Number);
  const t = new Date(y, m - 1, d + n);
  return `${t.getFullYear()}-${String(t.getMonth() + 1).padStart(2, '0')}-${String(t.getDate()).padStart(2, '0')}`;
}

export type MenuAction = 'print' | 'slips' | 'validate' | 'upload' | 'share';
