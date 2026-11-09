import { describe, expect, it } from 'vitest';
import { clock, playStats, servePct, serverAt, timeoutLeft, type BoardTimeout } from './boardStats';

const rep = (s: string, n: number) => s.repeat(n);

describe('serverAt', () => {
  it('changes every 2 points and every point from 10-10 (as src/scoreboard.rs)', () => {
    expect(serverAt(0, 0, 0, 0)).toBe(0);
    expect(serverAt(0, 0, 1, 0)).toBe(0);
    expect(serverAt(0, 0, 2, 1)).toBe(1); // 3 points: changed once
    expect(serverAt(0, 0, 10, 10)).toBe(0); // 10 changes
    expect(serverAt(0, 0, 11, 10)).toBe(1);
    expect(serverAt(1, 1, 0, 0)).toBe(0); // set 2 starts with the other player
  });
});

describe('playStats', () => {
  it('counts a 3-0 game', () => {
    const s = playStats(rep('0', 33), 2, 5);
    expect(s.finished).toBe(true);
    expect(s.setsWon).toEqual([3, 0]);
    expect(s.sets.map((x) => x.score)).toEqual([[11, 0], [11, 0], [11, 0]]);
    expect(s.match[0].points).toBe(33);
    expect(s.match[0].bestStreak).toBe(33); // streaks carry across sets
    expect(s.sets[1].sides[0].bestStreak).toBe(11);
    expect(s.match[0].biggestLead).toBe(11);
    expect(s.moment).toBeNull();
    expect(s.current).toEqual([0, 0]);
  });

  it('ignores points after the end', () => {
    expect(playStats(rep('0', 40), 1, 5).match[0].points).toBe(33);
  });

  it('serve stats: away serves first, wins its own serves', () => {
    // Away serves points 1-2, home 3-4. The server wins every point.
    const s = playStats('1100', 2, 5);
    expect(s.match[1]).toMatchObject({ served: 2, serveWon: 2 });
    expect(s.match[0]).toMatchObject({ served: 2, serveWon: 2 });
    expect(servePct(s.match[0])).toBe('100.0%');
    expect(servePct(playStats('', 2, 5).match[0])).toBe('–');
    expect(s.server).toBe(1);
  });

  it('a new set starts empty with the progression at 0-0', () => {
    const s = playStats(rep('1', 11), 1, 5);
    expect(s.sets).toHaveLength(2);
    expect(s.sets[1]).toMatchObject({ score: [0, 0], finished: false, progression: [[0, 0]] });
    expect(s.sets[0].progression).toHaveLength(12);
  });

  it('moments: set point, deuce, match point', () => {
    expect(playStats(rep('0', 10), 1, 5).moment).toBe('Setpunt');
    expect(playStats(rep('01', 10), 1, 5).moment).toBe('Deuce');
    expect(playStats(rep('0', 22) + rep('0', 10), 1, 5).moment).toBe('Matchpunt');
    // Best of 7 needs 4 sets: 2-0 up and 10-0 is a set point only.
    expect(playStats(rep('0', 22) + rep('0', 10), 1, 7).moment).toBe('Setpunt');
    expect(playStats('', 1, 5).moment).toBeNull();
  });

  it('keeps the last 20 points', () => {
    const s = playStats('1' + rep('0', 20), 1, 5);
    expect(s.last).toEqual(rep('0', 20).split('').map(Number));
  });

  it('biggest lead is per set', () => {
    // Home leads 5-0, then away wins 11-5.
    const s = playStats(rep('0', 5) + rep('1', 11), 1, 5);
    expect(s.sets[0].sides[0].biggestLead).toBe(5);
    expect(s.sets[0].sides[1].biggestLead).toBe(6);
    expect(s.match[1].bestStreak).toBe(11);
  });
});

describe('clock and timeouts', () => {
  it('formats durations', () => {
    expect(clock(0)).toBe('0:00');
    expect(clock(61_500)).toBe('1:01');
    expect(clock(3_725_000)).toBe('1:02:05');
  });

  it('counts down from the deadline', () => {
    const t: BoardTimeout = { active: true, owner: 1, p1Used: true, p2Used: false, expired: false, deadline: 100_000 };
    expect(timeoutLeft(t, 40_001)).toBe(60);
    expect(timeoutLeft(t, 120_000)).toBe(0);
    expect(timeoutLeft({ ...t, deadline: 0 }, 1)).toBeNull();
    expect(timeoutLeft({ ...t, expired: true }, 1)).toBe(0);
  });
});
