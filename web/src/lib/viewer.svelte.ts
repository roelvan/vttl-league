// Whose club is on screen. Every page of a club is "/c/{club}/…": the same link for everyone.
// Without a login, or for another club, the page is read-only; a logged-in member of that
// club gets the editable app on the same link.

import { session } from './session.svelte';

export const viewer = $state({ club: null as string | null });

const norm = (c: string | null | undefined) => (c ?? '').replace('-', '').toUpperCase();

/** The club shown: from the URL, else the user's own club. */
export const viewClub = () => viewer.club ?? session.me?.account.club ?? null;

/** True when the page shows another club than the user's own, or nobody is logged in. */
export function isPublic() {
  const own = session.me?.account.club;
  if (!own) return true;
  return !!viewer.club && norm(viewer.club) !== norm(own);
}

/**
 * True when the user's own club is the away team. The screen then shows the away side first
 * and in the home colour: the own club keeps its place and its colour in every match.
 * Public pages and a match between two teams of the club keep the official order.
 */
export function ownIsAway(homeClub: string | null | undefined, awayClub: string | null | undefined) {
  const own = norm(session.me?.account.club);
  return !!own && !isPublic() && norm(awayClub) === own && norm(homeClub) !== own;
}

/** The colour class of a side: "h" is the first side on screen, thus the away team in a flipped match. */
export const tone = <T extends string>(cls: T, flip: boolean): T | 'h' | 'a' => (!flip ? cls : cls === 'h' ? 'a' : cls === 'a' ? 'h' : cls);

/** The club for a public API call, or null for the normal (logged-in) API. */
export const publicClub = () => (isPublic() ? viewClub() : null);
