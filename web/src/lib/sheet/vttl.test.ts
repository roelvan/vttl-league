import { describe, expect, it } from 'vitest';
import { getSystem } from './systems';
import { applyOps } from './ops';
import { boxesFromNotation, canonical, importOps, uploadOps, vttlStatus, type Lookup } from './vttl';
import { fromVttl as sheetFromRow } from './create';
import type { MatchRow, TeamMatch, Member } from '../types';

const men = getSystem(2);
const row: MatchRow = {
  matchId: 'PWVLH01/012', uniqueId: 878673, week: '01', date: '2026-09-19', time: '19:00', venue: null,
  homeClub: 'WVL129', homeTeam: 'Westhoek A', awayClub: 'WVL154', awayTeam: 'Zonnebeke A', score: '4-12',
  divisionId: 9533, divisionCategory: 37, isHomeForfeited: false, isAwayForfeited: false, isValidated: false, isLocked: false,
  isBye: false, isHome: false, teamLetter: 'A', divisionName: 'Afdeling 1 - Prov. West-Vl. - Heren', matchType: 2, matchSystem: 2, divisionLevel: 9,
};
const pl = (pos: number, id: number, last: string) => ({ position: pos, uniqueIndex: id, firstName: 'JAN', lastName: last, ranking: 'C0', victoryCount: 0, isForfeited: false });
function vttlMatch(scores: string[], extra: Partial<TeamMatch> = {}): TeamMatch {
  return {
    ...row, venueClub: '', isHomeWithdrawn: false, isAwayWithdrawn: false,
    details: {
      detailsCreated: true, matchSystem: 2, homeCaptain: 101, awayCaptain: 201, referee: 101,
      homePlayers: [pl(1, 101, 'A'), pl(2, 102, 'B'), pl(3, 103, 'C'), pl(4, 104, 'D')],
      awayPlayers: [pl(1, 201, 'W'), pl(2, 202, 'X'), pl(3, 203, 'Y'), pl(4, 204, 'Z')],
      homeDoubles: [], awayDoubles: [],
      results: scores.map((sc, i) => ({ position: i + 1, homeIndexes: [men.games[i].home[0] + 1], awayIndexes: [men.games[i].away[0] + 1], homeSets: 0, awaySets: 0, isHomeForfeited: sc === '8', isAwayForfeited: sc === '9', scores: sc === '8' || sc === '9' ? '' : sc.split('/').map((x, j) => `${j + 1}|${x}`).join(',') })),
      homeScore: 0, awayScore: 0,
    },
    ...extra,
  };
}
const mem = (id: number, pos: number): Member => ({ position: pos, uniqueIndex: id, rankingIndex: pos, firstName: 'JAN', lastName: 'A', ranking: 'C0', status: 'A', club: 'WVL129', gender: 'M', category: 'SEN' });
const look: Lookup = { member: (_c, id) => (id === 101 ? mem(101, 7) : undefined), clubName: () => 'Westhoek' };

describe('VTTL sync', () => {
  it('notation <-> boxes', () => {
    expect(boxesFromNotation('-3/7/10/-12', men).slice(0, 4)).toEqual([{ l: '3', r: '11' }, { l: '11', r: '7' }, { l: '12', r: '10' }, { l: '12', r: '14' }]);
    expect(boxesFromNotation('8', men)[0]).toEqual({ l: 'WO', r: '' });
  });
  it('imports a played match into an empty sheet, then it is synced', () => {
    const s0 = sheetFromRow(row, 27, []);
    const m = vttlMatch(['-3/-4/-7', '5/5/5', '9']);
    const s1 = applyOps(s0, importOps(s0, men, m, look));
    expect(s1.home.players[0]).toMatchObject({ name: 'A Jan', computerNumber: '101', position: '7', index: '7', captain: true });
    expect(s1.homeCaptain.computerNumber).toBe('101');
    expect(s1.results[0].slice(0, 3)).toEqual([{ l: '3', r: '11' }, { l: '4', r: '11' }, { l: '7', r: '11' }]);
    expect(s1.results[2][0]).toEqual({ l: '', r: 'WO' });
    expect(vttlStatus(s1, men)).toBe('synced');
    // Running it again changes nothing.
    expect(importOps(s1, men, m, look)).toEqual([]);
  });
  it('a local edit is kept until VTTL itself changes', () => {
    const s0 = sheetFromRow(row, 27, []);
    const m = vttlMatch(['-3/-4/-7']);
    const s1 = applyOps(s0, importOps(s0, men, m, look));
    const s2 = applyOps(s1, [{ p: ['results', 0, 2], v: { l: '11', r: '7' } }, { p: ['results', 0, 3], v: { l: '11', r: '5' } }, { p: ['results', 0, 4], v: { l: '11', r: '1' } }]);
    expect(vttlStatus(s2, men)).toBe('pending');
    expect(importOps(s2, men, m, look)).toEqual([]); // same VTTL data: local wins
    const s3 = applyOps(s2, importOps(s2, men, vttlMatch(['-5/-5/-5']), look)); // VTTL changed: VTTL wins
    expect(s3.results[0][0]).toEqual({ l: '5', r: '11' });
    expect(s3.results[0][3]).toEqual({ l: '', r: '' });
  });
  it('after a real upload the sheet is synced, after a dry run it is not', () => {
    const s0 = sheetFromRow(row, 27, []);
    const s1 = applyOps(s0, [0, 1, 2].map((b) => ({ p: ['results', 0, b], v: { l: '11', r: '3' } })));
    expect(vttlStatus(s1, men)).toBe('pending');
    const dry = applyOps(s1, uploadOps(s1, men, { at: 0, by: 'x', ok: true, partial: true, dryRun: true, errors: [] }));
    expect(vttlStatus(dry, men)).toBe('pending');
    const real = applyOps(s1, uploadOps(s1, men, { at: 0, by: 'x', ok: true, partial: true, dryRun: false, errors: [] }));
    expect(vttlStatus(real, men)).toBe('synced');
    const failed = applyOps(s1, uploadOps(s1, men, { at: 0, by: 'x', ok: false, partial: false, dryRun: false, errors: ['22'] }));
    expect(vttlStatus(failed, men)).toBe('error');
  });
  it('validated on VTTL makes the sheet read-only', () => {
    const s0 = sheetFromRow(row, 27, []);
    const s1 = applyOps(s0, importOps(s0, men, vttlMatch([], { isValidated: true }), look));
    expect(vttlStatus(s1, men)).toBe('locked');
  });
  it('canonical has a key per player, captain, referee and game', () => {
    expect(Object.keys(canonical(sheetFromRow(row, 27, []), men))).toHaveLength(4 * 2 + 2 + 2 + 1 + 16);
  });
});
