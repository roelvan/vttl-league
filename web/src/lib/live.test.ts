import { describe, expect, it } from 'vitest';
import { custom } from './sheet/create';
import { liveView, person, tally, notation, letter } from './live';
import { nightCard } from './night';
import { getSystem } from './sheet/systems';
import type { Sheet } from './sheet/types';
import type { MatchRow } from './types';

function men(): Sheet {
  const s = custom(2, 1, '2026-11-07');
  s.home.name = 'Molenhoek B';
  s.away.name = 'Westrozebeke A';
  const names = [['VANDENBUSSCHE Pieter', 'C2'], ['DEBRUYNE Jonas', 'C4'], ['LEMAHIEU Wout', 'C6'], ['VERHAEGHE Bram', 'D0']];
  names.forEach(([n, r], i) => Object.assign(s.home.players[i], { name: n, ranking: r, computerNumber: String(100 + i) }));
  const away = [['OSTYN Kevin', 'C2'], ['CLAEYS Lander', 'C4'], ['VANNESTE Thomas', 'C4'], ['DEPREZ Sander', 'D0']];
  away.forEach(([n, r], i) => Object.assign(s.away.players[i], { name: n, ranking: r, computerNumber: String(200 + i) }));
  return s;
}
const win = (h: boolean) => (h ? [{ l: '11', r: '5' }, { l: '11', r: '5' }, { l: '11', r: '5' }] : [{ l: '5', r: '11' }, { l: '5', r: '11' }, { l: '5', r: '11' }]);
const pad = (sets: { l: string; r: string }[]) => [...sets, ...Array.from({ length: 5 - sets.length }, () => ({ l: '', r: '' }))];

describe('person', () => {
  it('splits VTTL-style names', () => {
    expect(person('DEBRUYNE Jonas', 'C4')).toEqual({ full: 'Jonas Debruyne', last: 'Debruyne', initial: 'J.', ranking: 'C4' });
    expect(person('VAN DEN BROECK Anne-Sophie').full).toBe('Anne-Sophie Van Den Broeck');
    expect(person('').full).toBe('');
    expect(person('Jan').last).toBe('Jan');
  });
});

describe('letters', () => {
  it('home A–D and away W–Z for 4 players, X–Z for 3', () => {
    const m = getSystem(2);
    expect([0, 1, 2, 3].map((i) => letter(m, i, 'home')).join('')).toBe('ABCD');
    expect([0, 1, 2, 3].map((i) => letter(m, i, 'away')).join('')).toBe('WXYZ');
    const w = getSystem(4);
    expect([0, 1, 2].map((i) => letter(w, i, 'away')).join('')).toBe('XYZ');
  });
});

describe('liveView', () => {
  it('first men game is D–X', () => {
    const v = liveView(men());
    expect(v.games).toHaveLength(16);
    expect(v.needed).toBe(9);
    expect(v.games[0].home.label).toBe('D');
    expect(v.games[0].away.label).toBe('X');
    expect(v.games[0].home.people[0].last).toBe('Verhaeghe');
  });

  it('not started: no live games, first two are next', () => {
    const s = men();
    s.tables = [{ no: '7', device: 'b1' }, { no: '8', device: null }];
    const v = liveView(s);
    expect(v.started).toBe(false);
    expect(v.games.map((g) => g.state).slice(0, 3)).toEqual(['next', 'next', 'later']);
    expect(v.tables[0].current).toBeNull();
    expect(v.tables[0].next?.index).toBe(0);
  });

  it('fixed pairs: odd games on table 1, even games on table 2', () => {
    const s = men();
    s.tables = [{ no: '7', device: null }, { no: '8', device: null }];
    s.started = true;
    s.results[0] = pad(win(true));
    s.results[2] = pad(win(false));
    s.results[1] = pad([{ l: '11', r: '6' }]);
    const v = liveView(s);
    expect(v.homeWins).toBe(1);
    expect(v.awayWins).toBe(1);
    // Table 7: games 1, 3 done -> game 5 is live, game 7 next.
    expect(v.tables[0].current?.pos).toBe(5);
    expect(v.tables[0].next?.pos).toBe(7);
    // Table 8: game 2 is live (one set in), game 4 next.
    expect(v.tables[1].current?.pos).toBe(2);
    expect(v.tables[1].current?.currentBox).toBe(1);
    expect(v.tables[1].next?.pos).toBe(4);
    expect(v.games[4].state).toBe('live');
    expect(v.games[5].state).toBe('later');
  });

  it('one table takes all games', () => {
    const s = men();
    s.tables = [{ no: '3', device: null }];
    s.started = true;
    const v = liveView(s);
    expect(v.games.every((g) => g.table === 0)).toBe(true);
    expect(v.tables[0].current?.pos).toBe(1);
    expect(v.tables[0].next?.pos).toBe(2);
  });

  it('team forfeit gives every game to the other side', () => {
    const s = men();
    s.away.forfeit = true;
    const v = liveView(s);
    expect(v.homeWins).toBe(16);
    expect(v.complete).toBe(true);
  });

  it('doubles system: the double game shows the pair', () => {
    const s = custom(4, 1, '2026-11-07');
    ['A A', 'B B', 'C C'].forEach((n, i) => (s.home.players[i].name = `P${i} ${n}`));
    s.home.doubles[0] = '1/3';
    const v = liveView(s);
    const dbl = v.games.find((g) => g.double)!;
    expect(dbl).toBeTruthy();
    expect(dbl.home.label).toBe('1/3');
    expect(dbl.home.people).toHaveLength(2);
    expect(v.needed).toBe(6);
  });
});

describe('tally', () => {
  it('counts wins and losses per player', () => {
    const s = men();
    s.results[0] = pad(win(true)); // D beats X
    s.results[3] = pad(win(false)); // A loses to Y
    const v = liveView(s);
    const home = tally(s, v, 'home');
    expect(home.map((r) => r.label)).toEqual(['A', 'B', 'C', 'D']);
    expect(home[3]).toMatchObject({ wins: 1, losses: 0 });
    expect(home[0]).toMatchObject({ wins: 0, losses: 1 });
    expect(home[0].pips).toHaveLength(4);
  });
});

describe('notation', () => {
  it('loser points, negative when away wins', () => {
    expect(notation([{ h: 11, a: 7 }, { h: 4, a: 11 }, { h: 11, a: 8 }, { h: 11, a: 5 }])).toBe('7/-4/8/5');
  });
});

describe('nightCard', () => {
  const row = (over: Partial<MatchRow>): MatchRow =>
    ({ matchId: 'X/1', isHome: false, isBye: false, score: '', matchType: 2, matchSystem: null, divisionName: 'P3B', isValidated: false, isLocked: false, homeTeam: 'A', awayTeam: 'B', ...over }) as MatchRow;

  it('away match: VTTL score only', () => {
    const c = nightCard(row({ score: '10-6' }), undefined);
    expect(c.state).toBe('final');
    expect(c.homeScore).toBe(10);
    expect(c.segments.filter((x) => x === 'h')).toHaveLength(10);
    expect(c.outcome).toBe('lost');
  });

  it('away match: the bar follows the VTTL game order', () => {
    const c = nightCard(row({ score: '13-3', games: 'ahhhaahhhhhhhhhh' }), undefined);
    expect(c.segments.join('')).toBe('ahhhaahhhhhhhhhh');
    const partial = nightCard(row({ score: '1-1', games: 'ha--' }), undefined);
    expect(partial.segments.slice(0, 4)).toEqual(['h', 'a', '', '']);
  });

  it('home match: the stored record wins over VTTL', () => {
    const s = men();
    s.started = true;
    s.results[0] = pad(win(true));
    const c = nightCard(row({ isHome: true, score: '' }), s);
    expect(c.state).toBe('live');
    expect(c.homeScore).toBe(1);
    expect(c.played).toBe(1);
    expect(c.total).toBe(16);
  });
});
