import { describe, expect, it } from 'vitest';
import { SYSTEMS, getSystem } from './systems';
import { computeScore, gameResult, setWinner, isPlayerWO } from './score';
import { fromVttl, seriesOf, levelOf, typeInfo } from './create';
import { validate, type ValidationContext } from './validate';
import { applyOps } from './ops';
import type { MatchRow, PlayerCategory, Member } from '../types';
import type { Sheet } from './types';

const men = getSystem(2);
const old = getSystem(1);
const S = (l: string, r: string) => ({ l, r });

describe('setWinner (ScoreSheet rules)', () => {
  it('normal sets', () => {
    expect(setWinner(0, S('11', '9'), men)).toBe('home');
    expect(setWinner(0, S('9', '11'), men)).toBe('away');
    expect(setWinner(0, S('12', '10'), men)).toBe('home');
    expect(setWinner(0, S('13', '10'), men)).toBe('error'); // above 11 the gap must be exactly 2
    expect(setWinner(0, S('11', '10'), men)).toBe('error');
    expect(setWinner(0, S('10', '8'), men)).toBe('error');
    expect(setWinner(0, S('21', '19'), old)).toBe('home');
    expect(setWinner(0, S('11', '5'), old)).toBe('error');
  });
  it('WO and FF', () => {
    expect(setWinner(0, S('wo', ''), men)).toBe('awayWO');
    expect(setWinner(0, S('', 'WO'), men)).toBe('homeWO');
    expect(setWinner(0, S('WO', 'WO'), men)).toBe('neither');
    expect(setWinner(1, S('WO', ''), men)).toBe('error'); // WO only in set 1
    expect(setWinner(2, S('ff', ''), men)).toBe('awayFF');
    expect(setWinner(0, S('FF', '3'), men)).toBe('error');
  });
});

describe('gameResult', () => {
  it('3-1 game', () => {
    const r = gameResult([S('11', '5'), S('9', '11'), S('11', '7'), S('11', '3')], men, false, false);
    expect(r).toMatchObject({ won: 'home', homeSets: 3, awaySets: 1 });
    expect(r.valid.every(Boolean)).toBe(true);
  });
  it('a box after the decision is wrong', () => {
    const r = gameResult([S('11', '5'), S('11', '5'), S('11', '5'), S('11', '5')], men, false, false);
    expect(r.won).toBe('error');
    expect(r.valid).toEqual([true, true, true, false, true]);
  });
  it('unfinished game is partial, not an error', () => {
    const r = gameResult([S('11', '5'), S('8', '11')], men, false, false);
    expect(r).toMatchObject({ won: 'partial', homeSets: 1, awaySets: 1 });
    expect(r.valid.every(Boolean)).toBe(true);
  });
  it('FF gives the remaining sets', () => {
    expect(gameResult([S('11', '5'), S('', 'ff')], men, false, false)).toMatchObject({ won: 'homeFF', homeSets: 3, awaySets: 0 });
  });
  it('WO counts as 3-0', () => {
    expect(gameResult([S('WO', '')], men, false, false)).toMatchObject({ won: 'awayWO', homeSets: 0, awaySets: 3 });
  });
});

const cats: PlayerCategory[] = [
  { id: 37, name: 'Seniors Heren', shortName: 'SEN', rankingCategory: 1, sex: '*', minAge: 0, maxAge: 999, isGroup: false, groupMembers: [] },
  { id: 38, name: 'Seniors Dames', shortName: 'SEN', rankingCategory: 2, sex: 'F', minAge: 0, maxAge: 999, isGroup: false, groupMembers: [] },
  { id: 3, name: 'Veteranen', shortName: 'VET', rankingCategory: 1, sex: '', minAge: null, maxAge: null, isGroup: true, groupMembers: [17, 19] },
  { id: 17, name: 'V40', shortName: 'V40', rankingCategory: 1, sex: '*', minAge: 39, maxAge: 999, isGroup: false, groupMembers: [] },
  { id: 19, name: 'V50', shortName: 'V50', rankingCategory: 1, sex: '*', minAge: 49, maxAge: 999, isGroup: false, groupMembers: [] },
  { id: 41, name: 'Jeugd', shortName: 'YOU', rankingCategory: 1, sex: '', minAge: null, maxAge: null, isGroup: true, groupMembers: [15] },
  { id: 15, name: 'Benjamins', shortName: 'BEN', rankingCategory: 1, sex: '*', minAge: 0, maxAge: 8, isGroup: false, groupMembers: [] },
];

const row: MatchRow = {
  matchId: 'PWVLH08/122', uniqueId: 0, week: '08', date: '2026-11-14', time: '14:00',
  venue: { name: 'SPORTHAL', street: 'Kerkstraat 1', town: '8980 ZONNEBEKE', phone: '', comment: '' },
  homeClub: 'WVL154', homeTeam: 'Zonnebeke I', awayClub: 'WVL023', awayTeam: 'Tielt D', score: '',
  divisionId: 9541, divisionCategory: 37, isHomeForfeited: false, isAwayForfeited: false, isValidated: false, isLocked: false,
  isBye: false, isHome: true, teamLetter: 'I', divisionName: 'Afdeling 5C - Prov. West-Vl. - Heren', matchType: 2, matchSystem: null, divisionLevel: 9,
};

describe('create', () => {
  it('parses series and level', () => {
    expect(seriesOf('Afdeling 5C - Prov. West-Vl. - Heren', 'provincial')).toBe('5C');
    expect(seriesOf('Super Afdeling', 'super')).toBe('');
    expect(levelOf(6)).toBe('super');
    expect(levelOf(15)).toBe('regional');
    expect(levelOf(9)).toBe('provincial');
  });
  it('guesses the type from the category', () => {
    expect(typeInfo(3, cats)).toEqual({ men: true, women: false, youth: false, veterans: true });
    expect(typeInfo(41, cats)).toMatchObject({ youth: true, veterans: false });
  });
  it('builds an official sheet', () => {
    const s = fromVttl(row, 27, cats);
    expect(s).toMatchObject({ date: '14/11/2026', series: '5C', level: 'provincial', men: true, interclub: true, article632: true });
    expect(s.address).toBe('SPORTHAL\nKerkstraat 1\n8980 ZONNEBEKE');
    expect(s.home.players).toHaveLength(4);
    expect(s.results).toHaveLength(16);
    expect(s.results[0]).toHaveLength(5);
  });
});

function filled(): Sheet {
  let s = fromVttl(row, 27, cats);
  const names = ['A', 'B', 'C', 'D'];
  s = applyOps(s, names.flatMap((n, i) => [
    { p: ['home', 'players', i], v: { name: `HOME ${n}`, computerNumber: String(100 + i), position: String(i + 1), index: String(i + 1), ranking: 'C0', captain: i === 0 } },
    { p: ['away', 'players', i], v: { name: `AWAY ${n}`, computerNumber: String(200 + i), position: String(i + 1), index: String(i + 1), ranking: 'C2', captain: i === 0 } },
  ]));
  return s;
}

describe('computeScore', () => {
  it('counts games and individual wins', () => {
    let s = filled();
    // Home wins the first 10 games 3-0, away the last 6.
    s = applyOps(s, men.games.flatMap((_, g) => [0, 1, 2].map((b) => ({ p: ['results', g, b], v: g < 10 ? S('11', '3') : S('3', '11') }))));
    const sc = computeScore(s, men);
    expect([sc.homeWins, sc.awayWins, sc.winner]).toEqual([10, 6, 'home']);
    expect(sc.games[15]).toMatchObject({ homeTotal: 10, awayTotal: 6 });
    // Home slot 3 (player 4) plays games 1, 5, 9 (wins) and 13 (loss).
    expect(sc.homeIndividual[3]).toBe('3');
  });
  it('a player who lost every game by WO is WO', () => {
    let s = filled();
    men.games.forEach((g, i) => {
      if (g.home[0] === 3) s = applyOps(s, [{ p: ['results', i, 0], v: S('WO', '') }]);
    });
    expect(isPlayerWO(s, men, 'home', 3)).toBe(true);
    expect(computeScore(s, men).homeIndividual[3]).toBe('WO');
    expect(computeScore(s, men).awayIndividual[1]).toBe('0+WO');
  });
  it('team forfeit gives all games', () => {
    const s = applyOps(filled(), [{ p: ['away', 'forfeit'], v: true }]);
    expect(computeScore(s, men)).toMatchObject({ homeWins: 16, awayWins: 0, winner: 'home' });
  });
  it('Super substitute replaces the player in the chosen game', () => {
    const sup = SYSTEMS[8];
    let s = { ...filled(), matchSystemId: 8 } as Sheet;
    s = applyOps(s, [{ p: ['home', 'substitutes', 0], v: 4 }, { p: ['results', 3, 0], v: S('11', '1') }, { p: ['results', 3, 1], v: S('11', '1') }, { p: ['results', 3, 2], v: S('11', '1') }]);
    const sc = computeScore(s, sup);
    expect(sc.homeIndividual[3]).toBe('1'); // V won game 4
    expect(sc.homeIndividual[0]).toBe('0');
  });
});

describe('validate', () => {
  const members: Member[] = ['A', 'B', 'C', 'D'].map((n, i) => ({
    position: i + 1, uniqueIndex: 100 + i, rankingIndex: i + 1, firstName: n, lastName: 'HOME', ranking: 'C0', status: 'A', club: 'WVL154', gender: 'M', category: 'SEN',
  }));
  const ctx: ValidationContext = {
    today: new Date(2026, 10, 14),
    members: (c) => (c === 'WVL154' ? members : undefined),
    clubExists: () => true,
    defaultCategory: 37,
    sameDay: [],
    systemOf: getSystem,
  };
  it('a fresh sheet has the expected warnings', () => {
    const e = validate(fromVttl(row, 27, cats), men, ctx);
    expect(e).toContain('Het einduur is niet ingevuld');
    expect(e).toContain('De lijst met setstanden is onvolledig');
    expect(e).toContain('De naam van thuisspeler 1 is niet ingevuld');
    expect(e).not.toContain('Het adres van het lokaal is niet ingevuld');
  });
  it('checks the strength list and the order', () => {
    let s = filled();
    s = applyOps(s, [{ p: ['home', 'players', 1, 'ranking'], v: 'B6' }, { p: ['home', 'players', 2, 'index'], v: '9' }, { p: ['home', 'players', 3, 'index'], v: '2' }]);
    const e = validate(s, men, ctx);
    expect(e).toContain('Het klassement van thuisspeler 2 komt niet overeen met de gegevens in de database');
    expect(e).toContain('De spelers van de thuisploeg staan niet in de juiste volgorde');
  });
  it('wrong date', () => {
    const e = validate(filled(), men, { ...ctx, today: new Date(2026, 10, 20) });
    expect(e).toContain('De datum op het wedstrijdblad komt niet overeen met de datum van vandaag');
  });
});
