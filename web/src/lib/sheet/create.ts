// A new sheet from a VTTL match, ported from TabTUpdater.CreateMatch / GuessTypeInfo
// and CompetitiveMatchViewModel.Initialize(MatchStartInfo).

import type { MatchRow, PlayerCategory, Venue } from '../types';
import { getSystem } from './systems';
import { emptyPerson, emptyPlayer, type Level, type Sheet, type SheetTeam } from './types';
import { isByeClub } from './score';

/** TabT division level (region) -> sheet level. */
export function levelOf(divisionLevel: number | null): Level {
  if (divisionLevel === 6) return 'super';
  if (divisionLevel === 1) return 'national';
  if (divisionLevel === 15 || divisionLevel === 16) return 'regional';
  return 'provincial';
}

/** "Afdeling 5C - Prov. West-Vl. - Heren" -> "5C". Super has no series. */
export function seriesOf(divisionName: string | null, level: Level): string {
  if (!divisionName || level === 'super') return '';
  let from = -1;
  if (divisionName.startsWith('Afdeling ') || divisionName.startsWith('Division ')) from = 9;
  else if (divisionName.startsWith('Reeks ') || divisionName.startsWith('Série ') || divisionName.startsWith('Serie ')) from = 6;
  if (from < 0) return '';
  const end = divisionName.indexOf(' ', from);
  return end === -1 ? divisionName : divisionName.slice(from, end);
}

interface TypeInfo {
  men: boolean;
  women: boolean;
  youth: boolean;
  veterans: boolean;
}

function typeOfCategory(c: PlayerCategory): TypeInfo {
  return {
    youth: c.maxAge !== null && c.maxAge <= 21,
    veterans: c.minAge !== null && c.minAge >= 39,
    women: c.sex === 'F',
    men: c.sex !== 'F',
  };
}

export function typeInfo(categoryId: number, categories: PlayerCategory[]): TypeInfo {
  const c = categories.find((x) => x.id === categoryId);
  if (!c) return { men: false, women: false, youth: false, veterans: false };
  if (!c.isGroup) return typeOfCategory(c);
  // A group: every member category must agree.
  const t = { men: true, women: true, youth: true, veterans: true };
  for (const id of c.groupMembers) {
    const m = categories.find((x) => x.id === id);
    if (!m) continue;
    const x = typeOfCategory(m);
    t.men &&= x.men;
    t.women &&= x.women;
    t.youth &&= x.youth;
    t.veterans &&= x.veterans;
  }
  if (!c.groupMembers.length) return { men: false, women: false, youth: false, veterans: false };
  return t;
}

/** The default "all members" category: no age limits, sex "*" (else "M"). */
export function menCategory(categories: PlayerCategory[]): number {
  const open = (c: PlayerCategory) => (c.minAge ?? 0) <= 0 && (c.maxAge ?? 100) >= 100;
  return categories.find((c) => open(c) && c.sex === '*')?.id ?? categories.find((c) => open(c) && c.sex === 'M')?.id ?? 37;
}

export function womenCategory(categories: PlayerCategory[]): number {
  const open = (c: PlayerCategory) => (c.minAge ?? 0) <= 0 && (c.maxAge ?? 100) >= 100;
  return categories.find((c) => open(c) && c.sex === 'F')?.id ?? 38;
}

export const venueText = (v: Venue | null | undefined) => (v ? [v.name, v.street, v.town].filter(Boolean).join('\n') : '');

/** "2026-11-14" -> "14/11/2026" */
export function isoToDate(iso: string) {
  if (!iso) return '';
  const [y, m, d] = iso.split('-');
  return `${d}/${m}/${y}`;
}

function team(name: string, clubId: string, sysId: number): SheetTeam {
  const sys = getSystem(sysId);
  const bye = isByeClub(clubId);
  return {
    name: name.trim(), // Super team names have a trailing space
    clubId,
    forfeit: false,
    players: Array.from({ length: sys.playerCount }, () => ({ ...emptyPlayer(), computerNumber: bye ? '?' : '' })),
    doubles: sys.slots.filter((s) => s.kind === 'double').map((s) => (s.kind === 'double' ? s.options[0] : '')),
    substitutes: Array.from({ length: sys.subCount }, () => null),
  };
}

export function fromVttl(m: MatchRow, season: number, categories: PlayerCategory[]): Sheet {
  const sysId = m.matchType ?? m.matchSystem;
  if (sysId == null) throw new Error(`Geen matchsysteem bekend voor ${m.matchId}`);
  const sys = getSystem(sysId);
  const level = levelOf(m.divisionLevel);
  const series = seriesOf(m.divisionName, level);
  const t = typeInfo(m.divisionCategory, categories);
  const cup = (m.divisionName ?? '').toLowerCase().includes('beker');
  const isSuper = level === 'super';
  const date = isoToDate(m.date);
  return {
    v: 1,
    id: m.matchId,
    season,
    day: m.date,
    isOfficial: true,
    matchSystemId: sys.id,
    playerCategory: m.divisionCategory,
    official: { matchId: m.matchId, date, series, homeTeam: m.homeTeam.trim(), awayTeam: m.awayTeam.trim() },
    address: venueText(m.venue),
    date,
    startHour: m.time,
    endHour: '',
    matchId: m.matchId,
    series,
    level,
    men: t.men,
    women: t.women,
    interclub: !isSuper && !cup && !t.youth && !t.veterans,
    super: isSuper,
    cup,
    youth: t.youth,
    veterans: t.veterans,
    article632: !isSuper, // ScoreSheet default: on, except for Super
    comments: '',
    home: team(m.homeTeam, m.homeClub, sys.id),
    away: team(m.awayTeam, m.awayClub, sys.id),
    results: sys.games.map(() => Array.from({ length: sys.boxes }, () => ({ l: '', r: '' }))),
    chiefReferee: emptyPerson(),
    homeCaptain: emptyPerson(),
    awayCaptain: emptyPerson(),
    roomCommissioner: emptyPerson(),
  };
}

/** A blank, unofficial sheet ("Nieuw wedstrijdblad"). */
export function custom(sysId: number, menCat: number, day: string): Sheet {
  const sys = getSystem(sysId);
  const id = `custom-${crypto.randomUUID()}`;
  return {
    v: 1,
    id,
    season: 0,
    day,
    isOfficial: false,
    matchSystemId: sys.id,
    playerCategory: menCat,
    official: null,
    address: '',
    date: isoToDate(day),
    startHour: '',
    endHour: '',
    matchId: '',
    series: '',
    level: 'provincial',
    men: true,
    women: false,
    interclub: true,
    super: false,
    cup: false,
    youth: false,
    veterans: false,
    article632: true,
    comments: '',
    home: team('', '', sys.id),
    away: team('', '', sys.id),
    results: sys.games.map(() => Array.from({ length: sys.boxes }, () => ({ l: '', r: '' }))),
    chiefReferee: emptyPerson(),
    homeCaptain: emptyPerson(),
    awayCaptain: emptyPerson(),
    roomCommissioner: emptyPerson(),
  };
}
