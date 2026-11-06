// The cases of ScoreSheet's CsvExporterTests.cs, on the same one-game test system.
import { describe, expect, it } from 'vitest';
import { HEADER, exportCsv, iLine } from './export';
import type { MatchSystem } from './systems';
import type { Sheet } from './types';
import { emptyPerson, emptyPlayer } from './types';

const sys: MatchSystem = {
  id: 0, name: 'CSV exporter test system', setCount: 3, pointCount: 11, boxes: 5, competitive: true,
  slots: [{ kind: 'single', label: '1', optional: false, player: 0 }],
  games: [{ pos: 1, home: [0], away: [0], allowSub: false }],
  playerCount: 1, doubleCount: 0, subCount: 0,
};

function sets(encoded: string) {
  const out = Array.from({ length: 5 }, () => ({ l: '', r: '' }));
  if (encoded) encoded.split('|').forEach((s, i) => { const [l, r] = s.split(':'); out[i] = { l, r }; });
  return out;
}

function complete(): Sheet {
  const p = (cn: string) => ({ ...emptyPlayer(), computerNumber: cn });
  return {
    v: 1, id: 'M-1', season: 42, day: '', isOfficial: true, matchSystemId: 0, playerCategory: 37, official: null,
    address: '', date: '', startHour: '18:30', endHour: '21:45', matchId: 'M-1', series: '', level: 'provincial',
    men: true, women: false, interclub: true, super: false, cup: false, youth: false, veterans: false, article632: true, comments: '',
    home: { name: '', clubId: 'Vl-B123', forfeit: false, players: [p('1001')], doubles: [], substitutes: [] },
    away: { name: '', clubId: 'AFTT456', forfeit: false, players: [p('2001')], doubles: [], substitutes: [] },
    results: [sets('11:4|7:11|11:9|11:0')],
    chiefReferee: { ...emptyPerson(), computerNumber: '3003' },
    homeCaptain: { ...emptyPerson(), computerNumber: '3001' },
    awayCaptain: { ...emptyPerson(), computerNumber: '3002' },
    roomCommissioner: emptyPerson(),
  };
}
const RESULT = '4/-7/9/0';
const line = (o: { matchId?: string; result?: string; officials?: string; times?: string; comment?: string } = {}) =>
  `I;42;${o.matchId ?? 'M-1'};VLB123;AFTT456;1001;2001;P${o.result ?? RESULT};${o.officials ?? '3001,3002,3003,-'};${o.times ?? '18:30,21:45'}${o.comment === undefined ? '' : ';' + o.comment}\r\n`;

describe('iLine (ScoreSheet CsvExporterTests)', () => {
  it('header and match', () => {
    expect(exportCsv([{ sheet: complete(), sys }])).toBe(HEADER + line());
    const second = { ...complete(), matchId: 'M-2' };
    expect(exportCsv([{ sheet: complete(), sys }, { sheet: second, sys }])).toBe(HEADER + line() + line({ matchId: 'M-2' }));
    expect(exportCsv([])).toBe(HEADER);
  });
  it('missing identifiers leave the columns empty', () => {
    const s = { ...complete(), matchId: '' };
    s.home = { ...s.home, clubId: '' };
    s.away = { ...s.away, clubId: '' };
    expect(iLine(s, sys).startsWith('I;42;;;;1001;2001;')).toBe(true);
  });
  it.each([
    ['11:4|7:11|11:9|11:0', '4/-7/9/0'],
    ['4:11|11:7|9:11|0:11', '-4/7/-9/-0'],
    [':WO', '9'],
    ['WO:', '8'],
    [':FF', '0/0/0'],
    ['FF:', '-0/-0/-0'],
    ['WO:WO', '-'],
  ])('game %s -> %s', (enc, expected) => {
    const s = complete();
    s.results = [sets(enc)];
    expect(iLine(s, sys)).toContain(`;P${expected};`);
  });
  it.each(['', '1:1'])('partial with an unfinished or wrong game (%s)', (enc) => {
    const s = complete();
    s.results = [sets(enc)];
    expect(iLine(s, sys, true)).toBe('I;42;M-1;VLB123;AFTT456;1001;2001;P?\r\n');
    expect(iLine(s, sys, false)).toBe(line({ result: '' }));
  });
  it('partial with a complete result keeps all columns', () => {
    expect(iLine(complete(), sys, true)).toBe(line());
  });
  it('bye', () => {
    const s = complete();
    s.home = { ...s.home, clubId: 'Vrij' };
    expect(iLine(s, sys, true)).toBe('I;42;M-1;bye;AFTT456;1001;2001;U0-0;-,-,-,-;18:30,21:45\r\n');
  });
  it('team forfeit', () => {
    const s = complete();
    s.home = { ...s.home, forfeit: true };
    expect(iLine(s, sys)).toContain(';-1001;2001;P8;-,-,-,-;');
  });
  it('invalid and signed people numbers', () => {
    const s = complete();
    s.homeCaptain = { ...s.homeCaptain, computerNumber: 'home' };
    s.awayCaptain = { ...s.awayCaptain, computerNumber: 'away' };
    s.chiefReferee = { ...s.chiefReferee, computerNumber: 'referee' };
    s.roomCommissioner = { ...s.roomCommissioner, computerNumber: 'commissioner' };
    expect(iLine(s, sys)).toContain(`;P${RESULT};-,-,-,-;18:30,21:45`);
    s.homeCaptain.computerNumber = '-1';
    s.awayCaptain.computerNumber = '+2';
    s.chiefReferee.computerNumber = '003';
    s.roomCommissioner.computerNumber = '4';
    expect(iLine(s, sys)).toContain(`;P${RESULT};-1,+2,003,4;`);
  });
  it('an invalid hour leaves the hours empty', () => {
    expect(iLine({ ...complete(), endHour: 'invalid' }, sys)).toBe(line({ times: '' }));
  });
  it('comments are escaped', () => {
    expect(iLine({ ...complete(), comments: 'Folder\\file\r\n"quoted"' }, sys)).toBe(line({ comment: `"Folder\\\\file\\n'quoted'"` }));
  });
  it('singles, optional, "-", invalid number, doubles and substitutes', () => {
    const sx: MatchSystem = {
      ...sys,
      slots: [
        { kind: 'single', label: '1', optional: false, player: 0 },
        { kind: 'single', label: '2', optional: true, player: 1 },
        { kind: 'single', label: '3', optional: false, player: 2 },
        { kind: 'single', label: '4', optional: false, player: 3 },
        { kind: 'double', options: ['1/2+3\\4'], double: 0 },
        { kind: 'sub', sub: 0, transferable: 0 },
      ],
    };
    const s = complete();
    s.home = {
      ...s.home,
      players: [s.home.players[0], emptyPlayer(), { ...emptyPlayer(), computerNumber: '-' }, { ...emptyPlayer(), name: 'Invalid player', computerNumber: 'not-a-number' }],
      doubles: ['1/2+3\\4'],
      substitutes: [1],
    };
    expect(iLine(s, sx)).toContain(';1001+1,?,,1-2-3-4;');
  });
});
