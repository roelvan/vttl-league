// URLs of the app. A VTTL match id has a slash ("PWVL03/045"); in a URL it becomes a dash.

export const matchSlug = (matchId: string) => matchId.replace('/', '-');

export function matchIdFromSlug(slug: string) {
  const i = slug.lastIndexOf('-');
  return i < 0 ? slug : `${slug.slice(0, i)}/${slug.slice(i + 1)}`;
}

export const urls = {
  day: (day: string) => `/dag/${day}`,
  match: (matchId: string) => `/wedstrijd/${matchSlug(matchId)}`,
  lineup: (matchId: string) => `/wedstrijd/${matchSlug(matchId)}/opstelling`,
  settings: () => '/instellingen',
};

/** ISO day plus or minus n days. */
export function addDays(iso: string, n: number) {
  const [y, m, d] = iso.split('-').map(Number);
  const t = new Date(y, m - 1, d + n);
  return `${t.getFullYear()}-${String(t.getMonth() + 1).padStart(2, '0')}-${String(t.getDate()).padStart(2, '0')}`;
}

export type MenuAction = 'print' | 'slips' | 'validate' | 'upload' | 'share' | 'about';
