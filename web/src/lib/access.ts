// Who may change a match: any logged-in member of the home club (design decision), while VTTL
// has not validated or locked the result. Away matches are read-only (the other club keeps them).

import { session } from './session.svelte';
import type { Sheet } from './sheet/types';

export const normClub = (c: string | null | undefined) => (c ?? '').replace('-', '').toUpperCase();

export function canEdit(sheet: Sheet | undefined | null): boolean {
  const club = normClub(session.me?.account.club);
  if (!sheet || !club || sheet.vttlLocked) return false;
  return normClub(sheet.home.clubId) === club;
}
