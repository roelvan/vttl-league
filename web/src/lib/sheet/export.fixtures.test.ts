// Writes tests/upload-fixtures.json: sample matches of every system with the upload lines,
// score and canonical map that the TypeScript rules give. The server (src/rules.rs) builds
// the uploads itself and checks that it gives the same output on these matches.
import { describe, expect, it } from 'vitest';
import { writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { SYSTEMS } from './systems';
import { custom } from './create';
import { iLine } from './export';
import { computeScore } from './score';
import { canonical } from './vttl';
import type { Sheet } from './types';

/** Small seeded random generator, so the file only changes when the rules change. */
function rng(seed: number) {
  return () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
}

function fill(s: Sheet, r: () => number, played: number) {
  const sys = SYSTEMS[s.matchSystemId];
  const pc = sys.pointCount;
  s.results = sys.games.map((_, gi) => {
    const boxes = Array.from({ length: sys.boxes }, () => ({ l: '', r: '' }));
    if (gi >= played) return boxes;
    let h = 0, a = 0, i = 0;
    while (h < sys.setCount && a < sys.setCount) {
      const loser = Math.floor(r() * (pc - 1));
      const deuce = r() < 0.15;
      const homeWins = r() < 0.5;
      const w = deuce ? pc + 1 + Math.floor(r() * 3) : pc;
      const l = deuce ? w - 2 : loser;
      boxes[i++] = homeWins ? { l: String(w), r: String(l) } : { l: String(l), r: String(w) };
      if (homeWins) h++;
      else a++;
    }
    return boxes;
  });
}

function sample(sysId: number, n: number): Sheet {
  const s = custom(sysId, 1, '2026-11-07');
  const sys = SYSTEMS[sysId];
  s.id = s.matchId = `T${sysId}/${n}`;
  s.isOfficial = true;
  s.season = 27;
  s.home.clubId = 'Vl-B123';
  s.away.clubId = 'WVL154';
  s.home.players.forEach((p, i) => (p.computerNumber = String(500100 + i)));
  s.away.players.forEach((p, i) => (p.computerNumber = String(600100 + i)));
  s.home.doubles = s.home.doubles.map((_, i) => sys.slots.flatMap((x) => (x.kind === 'double' ? [x.options] : []))[i]?.[n % 3] ?? '1/2');
  s.away.doubles = [...s.home.doubles];
  s.homeCaptain.computerNumber = '500100';
  s.awayCaptain.computerNumber = '600101';
  s.chiefReferee.computerNumber = n % 2 ? '500102' : '';
  s.startHour = '19:45';
  s.endHour = n % 3 ? '23:10' : '';
  return s;
}

describe('upload fixtures for the server', () => {
  it('writes tests/upload-fixtures.json', () => {
    const out: { name: string; sheet: Sheet; full: string; partial: string; score: number[]; canonical: Record<string, string> }[] = [];
    const add = (name: string, s: Sheet) => {
      const sys = SYSTEMS[s.matchSystemId];
      const sc = computeScore(s, sys);
      out.push({ name, sheet: s, full: iLine(s, sys, false), partial: iLine(s, sys, true), score: [sc.homeWins, sc.awayWins], canonical: canonical(s, sys) });
    };
    for (const id of Object.keys(SYSTEMS).map(Number)) {
      const r = rng(1000 + id);
      const n = SYSTEMS[id].games.length;
      for (const [k, played] of [[0, 0], [1, Math.floor(n / 2)], [2, n]] as const) {
        const s = sample(id, k);
        fill(s, r, played);
        add(`sys${id}-${played}of${n}`, s);
      }
      // Specials on a complete match.
      const wo = sample(id, 3);
      fill(wo, r, n);
      wo.results[0] = wo.results[0].map((_, i) => (i === 0 ? { l: 'WO', r: '' } : { l: '', r: '' }));
      wo.results[1] = wo.results[1].map((_, i) => (i === 0 ? { l: '11', r: '3' } : i === 1 ? { l: '', r: 'FF' } : { l: '', r: '' }));
      wo.comments = 'Tafel 7 "kapot"\nVerplaatst naar tafel 8\\9';
      add(`sys${id}-wo-ff-comments`, wo);
      const bad = sample(id, 4);
      fill(bad, r, n);
      bad.results[2][0] = { l: '11', r: '10' };
      add(`sys${id}-error`, bad);
      const ff = sample(id, 5);
      ff.away.forfeit = true;
      add(`sys${id}-team-forfeit`, ff);
    }
    const bye = sample(2, 6);
    bye.home.clubId = 'Vrij';
    add('bye', bye);
    const sub = sample(8, 7);
    fill(sub, rng(7), 6);
    sub.home.substitutes = [5];
    sub.home.players[3].computerNumber = '500199';
    add('super-substitute', sub);
    const odd = sample(2, 8);
    fill(odd, rng(8), 16);
    odd.home.players[1].computerNumber = '-';
    odd.home.players[2].computerNumber = 'abc';
    odd.startHour = '19u45';
    odd.endHour = '23.05';
    add('odd-numbers-and-hours', odd);

    const file = fileURLToPath(new URL('../../../../tests/upload-fixtures.json', import.meta.url));
    writeFileSync(file, JSON.stringify(out, null, 1) + '\n');
    expect(out.length).toBeGreaterThan(40);
  });
});
