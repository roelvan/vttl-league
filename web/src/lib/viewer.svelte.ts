// Whose club is on screen. Public pages (/c/{club}/…) work without a login and are read-only;
// a logged-in member of that club gets the normal, editable app on the same link.

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

/** The club for a public API call, or null for the normal (logged-in) API. */
export const publicClub = () => (isPublic() ? viewClub() : null);
