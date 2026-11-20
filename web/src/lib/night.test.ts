import { describe, expect, it } from 'vitest';
import { placeClass, standingOf } from './night';
import type { RankingEntry } from './types';

const entry = (position: number, team: string, teamClub: string): RankingEntry => ({
  position, team, teamClub, played: 0, won: 0, lost: 0, draw: 0, gamesWon: 0, gamesLost: 0, setsWon: 0, setsLost: 0, points: 0,
});

describe('standingOf', () => {
  const ranking = [entry(1, 'T.T.C. Torhout A', 'WVL115'), entry(3, 'T.T.C. Zonnebeke A', 'WVL154'), entry(12, 'T.T.C. Zonnebeke B', 'WVL154')];

  it('finds a team by club and letter, not by name', () => {
    expect(standingOf(ranking, 'WVL154', 'Zonnebeke B')?.position).toBe(12);
    expect(standingOf(ranking, 'WVL154', 'Zonnebeke A ')?.position).toBe(3);
  });
  it('knows a club id with and without a dash', () => {
    expect(standingOf([entry(2, 'Diest A', 'Vl-B293')], 'VLB293', 'P.W. Diest A')?.position).toBe(2);
  });
  it('takes the only team of a club when the letter differs', () => {
    expect(standingOf(ranking, 'WVL115', 'Torhout')?.position).toBe(1);
  });
  it('gives nothing for an unknown team or no standings', () => {
    expect(standingOf(ranking, 'WVL154', 'Zonnebeke C')).toBeUndefined();
    expect(standingOf(ranking, 'WVL999', 'Elders A')).toBeUndefined();
    expect(standingOf(undefined, 'WVL154', 'Zonnebeke A')).toBeUndefined();
  });
});

describe('placeClass', () => {
  it('colours the first three places only', () => {
    expect([1, 2, 3, 4, 0].map(placeClass)).toEqual(['p1', 'p2', 'p3', '', '']);
  });
});
