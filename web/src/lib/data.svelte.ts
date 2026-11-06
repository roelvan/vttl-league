// Reference data from VTTL, cached in memory and in local storage so the sheet keeps
// working (and validating) when the hall WiFi drops.

import { api } from './api';
import type { Club, MatchList, Member, PlayerCategory } from './types';
import { menCategory } from './sheet/create';

function load<T>(key: string): T | undefined {
  try {
    const v = localStorage.getItem(key);
    return v ? (JSON.parse(v) as T) : undefined;
  } catch {
    return undefined;
  }
}
function save(key: string, v: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(v));
  } catch {}
}

export const data = $state({
  categories: load<PlayerCategory[]>('vl.cats') ?? [],
  clubs: load<Club[]>('vl.clubs') ?? [],
  matches: load<MatchList>('vl.matches.current') ?? null,
  /** "club|category" -> strength list */
  members: load<Record<string, Member[]>>('vl.members') ?? {},
});

export const defaultCategory = () => menCategory(data.categories);

const inflight = new Map<string, Promise<unknown>>();
function once<T>(key: string, f: () => Promise<T>): Promise<T> {
  let p = inflight.get(key) as Promise<T> | undefined;
  if (!p) {
    p = f().finally(() => inflight.delete(key));
    inflight.set(key, p);
  }
  return p;
}

export function loadCategories() {
  return once('cats', async () => {
    data.categories = await api.playerCategories();
    save('vl.cats', data.categories);
    return data.categories;
  });
}

export function loadClubs() {
  return once('clubs', async () => {
    data.clubs = await api.clubs();
    save('vl.clubs', data.clubs);
    return data.clubs;
  });
}

export function loadMatches(season?: number) {
  return once(`matches.${season ?? 'current'}`, async () => {
    const m = await api.clubMatches(season);
    if (!season) {
      data.matches = m;
      save('vl.matches.current', m);
    }
    return m;
  });
}

export const memberKey = (club: string, category: number) => `${club.replace('-', '').toUpperCase()}|${category}`;

export function loadMembers(club: string, category: number) {
  const key = memberKey(club, category);
  return once(`members.${key}`, async () => {
    const m = await api.members(category, club);
    data.members[key] = m;
    save('vl.members', $state.snapshot(data.members));
    return m;
  });
}

/** Cached strength list, or undefined (and a background load) when not known yet. */
export function members(club: string, category: number): Member[] | undefined {
  if (!club || club === '-') return undefined;
  const m = data.members[memberKey(club, category)];
  if (!m) loadMembers(club, category).catch(() => {});
  return m;
}

export function clubName(id: string): string {
  const norm = id.replace('-', '').toUpperCase();
  return data.clubs.find((c) => c.id.replace('-', '').toUpperCase() === norm)?.name ?? '';
}

export function clubExists(id: string): boolean | undefined {
  if (!data.clubs.length) return undefined;
  const norm = id.replace('-', '').toUpperCase();
  return data.clubs.some((c) => c.id.replace('-', '').toUpperCase() === norm);
}
