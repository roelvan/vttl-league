const dayFmt = new Intl.DateTimeFormat('nl-BE', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' });
const shortFmt = new Intl.DateTimeFormat('nl-BE', { weekday: 'short', day: 'numeric', month: 'short' });

function parse(iso: string) {
  const [y, m, d] = iso.split('-').map(Number);
  return new Date(y, m - 1, d);
}

export const longDate = (iso: string) => (iso ? dayFmt.format(parse(iso)) : 'Zonder datum');
export const shortDate = (iso: string) => (iso ? shortFmt.format(parse(iso)) : 'Zonder datum');

export function todayIso() {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

/** "ROEL" -> "Roel" */
export function titleCase(s: string) {
  return s.toLowerCase().replace(/(^|[^\p{L}])(\p{L})/gu, (_, a, b) => a + b.toUpperCase());
}

export const playerName = (p: { lastName: string; firstName: string }) => `${p.lastName} ${titleCase(p.firstName)}`;

/** Our team letter in a match: "Zonnebeke AB" -> "AB". */
export const ownLetter = (m: { isHome: boolean; homeTeam: string; awayTeam: string; teamLetter: string | null }) =>
  m.teamLetter ?? (m.isHome ? m.homeTeam : m.awayTeam).split(' ').at(-1) ?? '';

/** Best team first: A, B, …, Z, AA, AB … (shorter letters first), then by start hour. */
export function byTeam<T extends { isHome: boolean; homeTeam: string; awayTeam: string; teamLetter: string | null; time: string }>(a: T, b: T) {
  const la = ownLetter(a), lb = ownLetter(b);
  return la.length - lb.length || la.localeCompare(lb) || a.time.localeCompare(b.time);
}
