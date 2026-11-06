// Validations, ported from PieterP.ScoreSheet.ViewModels/Score/Validations/*.cs and
// VMMatchSystem.GetValidations, in the same order and with the same Dutch texts.
// They warn; they never block an upload.

import type { Member } from '../types';
import type { MatchSystem } from './systems';
import type { Sheet, SheetTeam } from './types';
import { computeScore, isByeClub, type Score } from './score';

export interface ValidationContext {
  today: Date;
  /** Strength list of a club for a category, or undefined when not loaded (check skipped). */
  members: (clubId: string, category: number) => Member[] | undefined;
  /** true/false when the club list is loaded, undefined when not (check skipped). */
  clubExists: (clubId: string) => boolean | undefined;
  /** The "all members" category (men). */
  defaultCategory: number;
  /** Other sheets of the same day (for the chief-referee check). */
  sameDay: Sheet[];
  systemOf: (id: number) => MatchSystem;
}

const fmt = (s: string, ...a: string[]) => s.replace(/\{(\d)\}/g, (_, i) => a[Number(i)] ?? '');
const isNumber = (s: string) => /^\d+$/.test(s.trim());

/** "14/11/2026" or "14-11-2026" -> Date (local), else null. */
export function parseDate(s: string): Date | null {
  const m = s.trim().match(/^(\d{1,2})[/.-](\d{1,2})[/.-](\d{4})$/);
  if (!m) return null;
  const d = new Date(Number(m[3]), Number(m[2]) - 1, Number(m[1]));
  return d.getMonth() === Number(m[2]) - 1 ? d : null;
}
export const parseTime = (s: string) => /^([01]?\d|2[0-3])[:.hu]([0-5]\d)$/.test(s.trim());
const sameDay = (a: Date, b: Date) => a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();

export function mustBePlayed(s: Sheet) {
  return !(isByeClub(s.home.clubId) || isByeClub(s.away.clubId) || s.home.forfeit || s.away.forfeit);
}

export function validate(s: Sheet, sys: MatchSystem, ctx: ValidationContext, score: Score = computeScore(s, sys)): string[] {
  const out: string[] = [];
  const add = (e: string | null) => e && out.push(e);
  const played = mustBePlayed(s);
  const bye = isByeClub(s.home.clubId) || isByeClub(s.away.clubId);

  // AddressValidation
  if (!s.address.trim()) add('Het adres van het lokaal is niet ingevuld');
  // ChiefRefereeValidValidation
  add(chiefReferee(s, ctx));
  // ClubIdValidation x2
  add(clubId(s.home.clubId, 'de thuisploeg', ctx));
  add(clubId(s.away.clubId, 'de uitploeg', ctx));
  // DateValidation
  add(date(s, bye, ctx));
  // EndHourValidation
  if (played) {
    if (!s.endHour) add('Het einduur is niet ingevuld');
    else if (!parseTime(s.endHour)) add('Het einduur is ongeldig');
  }
  // GenderValidation
  if (s.isOfficial) {
    if (!s.men && !s.women) add('Er is niet aangeduid of het om een mannen- of vrouwenwedstrijd gaat');
    if (s.men && s.women) add('Er is aangeduid dat het om een mannen- én vrouwenwedstrijd gaat');
  }
  // LevelValidation
  if (s.level === 'super' && !s.super) add("De afdeling staat op 'super', maar het vinkje bij 'Super Afdeling' staat niet aan");
  // MatchIdValidation
  if (!s.matchId) add('Het matchnummer is niet ingevuld');
  else if (s.isOfficial && s.matchId !== s.id) add(fmt('Het matchnummer komt niet overeen met de officiële gegevens ({0})', s.id));
  // MatchTypeValidation
  if (s.isOfficial) {
    const n = [s.interclub, s.super, s.youth, s.cup, s.veterans].filter(Boolean).length;
    if (n === 0) add('Er is niet aangeduid om welk type wedstrijd het gaat');
    if (n > 1) add('Er zijn meerdere wedstrijdtypes aangeduid');
  }
  // PersonNameValidation / PersonNumberValidation
  if (played) {
    const people: [string, { name: string; computerNumber: string }][] = [
      ['de hoofscheidsrechter', s.chiefReferee],
      ['de thuiskapitein', s.homeCaptain],
      ['de uitkapitein', s.awayCaptain],
    ];
    for (const [t, p] of people) if (!p.name) add(fmt('De naam van {0} is niet ingevuld', t));
    for (const [t, p] of people) if (!isNumber(p.computerNumber)) add(fmt('Het aansluitingsnummer van {0} is ongeldig', t));
  }
  // RoomCommissionerName/NumberValidation (only required for Super men)
  const superMen = s.level === 'super' && s.playerCategory === ctx.defaultCategory;
  if (superMen && !s.roomCommissioner.name) add('De naam van de zaalcommissaris is niet ingevuld');
  if (s.roomCommissioner.computerNumber) {
    if (!isNumber(s.roomCommissioner.computerNumber)) add('Het aansluitingsnummer van de zaalcommissaris is ongeldig');
    else {
      const m = ctx.members(s.home.clubId, ctx.defaultCategory);
      if (m && !m.some((x) => x.uniqueIndex === Number(s.roomCommissioner.computerNumber)))
        add('Het computernummer van de zaalcommissaris is ongeldig (of die persoon is niet aangesloten bij de thuisclub)');
    }
  } else if (superMen) add('Het aansluitingsnummer van de zaalcommissaris is niet ingevuld');
  // ScoreValidation
  if (score.winner === 'error') add('Er zit een fout in de lijst met setstanden');
  else if (score.winner === 'incomplete') {
    const min = Math.floor(sys.games.length / 2) + 1;
    if (!(s.cup && (score.homeWins === min || score.awayWins === min))) add('De lijst met setstanden is onvolledig');
  }
  // SeriesValidation
  if (s.official?.series != null) {
    if (s.series !== s.official.series) add(fmt('De reeks komt niet overeen met de officiële gegevens ({0})', s.official.series));
  } else if (!s.series) add('De reeks is niet ingevuld');
  // StartHourValidation
  if (played) {
    if (!s.startHour) add('Het startuur is niet ingevuld');
    else if (!parseTime(s.startHour)) add('Het startuur is ongeldig');
  }
  // TeamIndexValidation x2
  add(teamIndex(s, s.home, 'de thuisploeg'));
  add(teamIndex(s, s.away, 'de uitploeg'));
  // TeamNameValidation x2
  add(teamName(s, s.home.name, 'de thuisploeg'));
  add(teamName(s, s.away.name, 'de uitploeg'));
  // TeamWOValidation x2
  add(tooManyWO(s.home, score.homeIsWO, 'de thuisploeg'));
  add(tooManyWO(s.away, score.awayIsWO, 'de uitploeg'));
  // CaptainNotWOValidation x2
  if (played) {
    add(captainWO(s.home, sys, score.homeIsWO, 'de thuisploeg'));
    add(captainWO(s.away, sys, score.awayIsWO, 'de uitploeg'));
  }
  // PlayerValidation per single slot
  for (const side of ['home', 'away'] as const) {
    sys.slots.forEach((slot, i) => {
      if (slot.kind !== 'single') return;
      const title = fmt(side === 'home' ? 'thuisspeler {0}' : 'uitspeler {0}', String(i + 1));
      add(player(s, side, slot.player, slot.optional, title, ctx));
    });
  }
  return out;
}

function clubId(id: string, title: string, ctx: ValidationContext) {
  if (!id) return fmt('Het aanwijzingsnummer van {0} is niet ingevuld', title);
  if (isByeClub(id)) return null;
  if (ctx.clubExists(id) === false) return fmt('Kan het aanwijzingsnummer van {0} niet in de clubs-database vinden', title);
  return null;
}

function date(s: Sheet, bye: boolean, ctx: ValidationContext) {
  if (!s.date) return bye ? null : 'De datum van de match is niet ingevuld';
  const d = parseDate(s.date);
  if (!d) return 'De datum van de match is ongeldig';
  const yesterday = new Date(ctx.today);
  yesterday.setDate(yesterday.getDate() - 1);
  if (!sameDay(d, ctx.today) && !sameDay(d, yesterday)) return 'De datum op het wedstrijdblad komt niet overeen met de datum van vandaag';
  const org = s.official?.date ? parseDate(s.official.date) : null;
  if (org && !sameDay(d, org)) return fmt('De datum komt niet overeen met de officiële gegevens ({0})', s.official!.date);
  return null;
}

function chiefReferee(s: Sheet, ctx: ValidationContext) {
  const r = s.chiefReferee;
  if (!r.computerNumber || !r.name || !s.isOfficial || !isNumber(r.computerNumber)) return null;
  const cn = Number(r.computerNumber);
  if (s.home.clubId !== s.away.clubId) {
    const away = ctx.members(s.away.clubId, ctx.defaultCategory);
    if (away?.some((m) => m.uniqueIndex === cn)) return 'De hoofdscheidsrechter mag geen lid van de uit-club zijn.';
  }
  const home = ctx.members(s.home.clubId, ctx.defaultCategory);
  if (!home?.some((m) => m.uniqueIndex === cn)) return null; // probably an official referee
  if (isNumber(s.homeCaptain.computerNumber) && Number(s.homeCaptain.computerNumber) === cn) return null;
  const plays = (x: Sheet) => [...x.home.players, ...x.away.players].some((p) => p.computerNumber === r.computerNumber);
  if (plays(s)) return 'De hoofdscheidsrechter mag geen andere speler zijn dan de thuiskapitein';
  for (const o of ctx.sameDay) {
    if (o.id === s.id || o.matchId === s.matchId) continue;
    if (plays(o)) return fmt('De hoofdscheidsrecther van deze match speelt ook al in een andere gelijktijdige match ({0}).', `${o.home.name} - ${o.away.name}`);
  }
  return null;
}

function teamIndex(s: Sheet, t: SheetTeam, title: string) {
  if (s.level === 'super') return null; // Super may line up as they like
  let prev = -1;
  for (const p of t.players) {
    if (!isNumber(p.index)) continue;
    const i = Number(p.index);
    if (i < prev) return fmt('De spelers van {0} staan niet in de juiste volgorde', title);
    prev = i;
  }
  return null;
}

function teamName(s: Sheet, name: string, title: string) {
  if (s.official?.series != null) {
    if (name !== s.official.homeTeam && name !== s.official.awayTeam) return fmt('De naam van {0} komt niet overeen met de officiële gegevens', title);
  } else if (!name) return fmt('De naam van {0} is niet ingevuld', title);
  return null;
}

function tooManyWO(t: SheetTeam, isWO: boolean[], title: string) {
  if (t.forfeit) return null;
  return isWO.filter(Boolean).length > 1 ? fmt('Er kan maximum één speler WO zijn in {0}', title) : null;
}

function captainWO(t: SheetTeam, sys: MatchSystem, isWO: boolean[], title: string) {
  for (const [i, slot] of sys.slots.entries()) {
    if (slot.kind === 'single' && t.players[slot.player]?.captain && isWO[i]) return fmt('De kapitein van {0} mag niet een walkover zijn', title);
  }
  return null;
}

function player(s: Sheet, side: 'home' | 'away', idx: number, optional: boolean, title: string, ctx: ValidationContext) {
  const t = s[side];
  const p = t.players[idx];
  if (!p) return null;
  const v = [p.name, p.computerNumber, p.position, p.index, p.ranking];
  if (v.every((x) => x !== '') && optional) return null;
  if (isByeClub(t.clubId)) {
    if (v[1] !== '?') return fmt('Het computernummer van {0} moet een vraagteken zijn', title);
    if (v.filter((x) => x !== '').length !== 1) return fmt('Alle veldjes van {0}, behalve het computernummer, moeten leeg zijn bij een bye', title);
    return null;
  }
  if (t.forfeit && v[1] === '?') {
    if (v.filter((x) => x !== '').length !== 1)
      return fmt('Alle veldjes van {0}, behalve het computernummer, moeten leeg zijn als u een vraagteken als computernummer gebruikt', title);
    return null;
  }
  // The optional Super player (V) may stay empty.
  if (optional && v.every((x) => x === '')) return null;
  if (!v[0]) return fmt('De naam van {0} is niet ingevuld', title);
  if (!isNumber(v[1])) return fmt('Het computernummer van {0} is niet ingevuld of ongeldig', title);
  if (!isNumber(v[2])) return fmt('Het volgnummer van {0} is niet ingevuld of ongeldig', title);
  if (!isNumber(v[3])) return fmt('De index van {0} is niet ingevuld of ongeldig', title);
  if (!v[4]) return fmt('Het klassement van {0} is niet ingevuld', title);
  const members = ctx.members(t.clubId, s.playerCategory);
  if (members) {
    const m = members.find((x) => x.uniqueIndex === Number(v[1]));
    if (!m) return fmt('Kan {0} met computernummer {1} niet terugvinden in de sterktelijst', title, v[1]);
    if (v[0].toLowerCase() !== `${m.lastName} ${m.firstName}`.toLowerCase()) return fmt('De naam van {0} komt niet overeen met de gegevens in de database', title);
    if (v[2] !== String(m.position)) return fmt('Het volgnummer van {0} komt niet overeen met de gegevens in de database', title);
    if (v[3] !== String(m.rankingIndex)) return fmt('De index van {0} komt niet overeen met de gegevens in de database', title);
    if (v[4] !== m.ranking) return fmt('Het klassement van {0} komt niet overeen met de gegevens in de database', title);
  }
  return null;
}
