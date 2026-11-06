// TabT upload lines ("I" lines), ported from PieterP.ScoreSheet.ViewModels/Score/Export/CsvExporter.cs.
// Tests: export.test.ts (the cases of ScoreSheet's CsvExporterTests.cs).

import type { Sheet, SetScore } from './types';
import type { MatchSystem } from './systems';
import { computeScore, gameResult, isByeClub, isEmptySet, isPlayerWO, setWinner, type Won } from './score';

export const HEADER = 'C;Match result exported by Scoresheet, Copyright (c) PieterP.be\r\n';

/** int.TryParse: an optional sign and digits. */
const isNumber = (s: string) => /^[+-]?\d+$/.test(s.trim());

/** "Vl-B234" -> "VLB234" (the TabT specification uses VLB); byes become "bye". */
export function clubIndex(c: string): string {
  if (isByeClub(c)) return 'bye';
  if (c.startsWith('Vl-B')) return 'VLB' + c.slice(4);
  return c;
}

/** One game in TabT notation: "4/-7/9/0", "9" (home WO), "8" (away WO), "-" (nobody). "" when it cannot be exported. */
export function gameNotation(won: Won, sets: SetScore[], sys: MatchSystem): string {
  switch (won) {
    case 'neither':
      return '-';
    case 'homeWO':
      return '9';
    case 'awayWO':
      return '8';
    case 'home':
    case 'away':
    case 'homeFF':
    case 'awayFF': {
      const parts: string[] = [];
      let wonSets = 0;
      let lostSets = 0;
      for (let i = 0; i < sets.length; i++) {
        if (isEmptySet(sets[i])) break;
        const w = setWinner(i, sets[i], sys);
        if (w === 'home') {
          parts.push(sets[i].r.trim());
          wonSets++;
        } else if (w === 'away') {
          parts.push('-' + sets[i].l.trim());
          lostSets++;
        } else if (w === 'homeFF') {
          // The remaining sets go to home with 0 points for the opponent.
          parts.push('0');
          wonSets++;
          while (wonSets < sys.setCount) {
            parts.push('0');
            wonSets++;
          }
        } else if (w === 'awayFF') {
          parts.push('-0');
          lostSets++;
          while (lostSets < sys.setCount) {
            parts.push('-0');
            lostSets++;
          }
        }
      }
      return parts.join('/');
    }
    default:
      return '';
  }
}

function team(s: Sheet, sys: MatchSystem, side: 'home' | 'away'): string {
  const t = s[side];
  const out: string[] = [];
  sys.slots.forEach((slot, si) => {
    if (slot.kind === 'single') {
      const cn = (t.players[slot.player]?.computerNumber ?? '').trim();
      if (cn === '' && slot.optional) return; // the Super 4th player may be empty
      if (cn !== '?' && cn !== '-' && !isNumber(cn)) {
        out.push(''); // invalid number: ScoreSheet writes nothing (and logs a warning)
        return;
      }
      let v = (isPlayerWO(s, sys, side, si) ? '-' : '') + (cn === '-' ? '?' : cn);
      for (const sub of sys.slots) {
        if (sub.kind === 'sub' && sub.transferable === si && t.substitutes[sub.sub] != null) v += '+' + t.substitutes[sub.sub];
      }
      out.push(v);
    } else if (slot.kind === 'double') {
      out.push((t.doubles[slot.double] ?? slot.options[0]).replace(/[/\\+]/g, '-'));
    }
  });
  return out.join(',');
}

/** "18:30" / "18u30" / "18.30" -> "18:30"; null when not a time. */
function hhmm(s: string): string | null {
  const m = s.trim().match(/^([01]?\d|2[0-3])[:.hu]([0-5]\d)$/);
  return m ? `${m[1].padStart(2, '0')}:${m[2]}` : null;
}

/**
 * The I line of one sheet (with \r\n). `partial`: games that are not finished become "?" and the
 * people/hours/comments columns are left out, unless every game is known.
 */
export function iLine(s: Sheet, sys: MatchSystem, partial = false): string {
  const score = computeScore(s, sys);
  const cols: string[] = ['I', String(s.season), s.matchId, s.home.clubId ? clubIndex(s.home.clubId) : '', s.away.clubId ? clubIndex(s.away.clubId) : '', team(s, sys, 'home'), team(s, sys, 'away')];
  let complete = true;
  const bye = isByeClub(s.home.clubId) || isByeClub(s.away.clubId);
  if (bye) {
    cols.push(`U${score.homeWins}-${score.awayWins}`);
  } else {
    const games = sys.games.map((_, gi) => {
      const sets = s.results[gi] ?? [];
      const r = gameResult(sets, sys, s.home.forfeit, s.away.forfeit);
      if (partial && (r.won === 'empty' || r.won === 'partial' || r.won === 'error')) {
        complete = false;
        return '?';
      }
      return gameNotation(r.won, sets, sys);
    });
    cols.push('P' + games.join(','));
  }
  if (complete) partial = false;
  if (!partial) {
    const empty = s.home.forfeit || s.away.forfeit || bye;
    const person = (cn: string, optional = false) => (empty ? '-' : optional && cn === '' ? '-' : isNumber(cn) ? cn.trim() : '-');
    cols.push([person(s.homeCaptain.computerNumber), person(s.awayCaptain.computerNumber), person(s.chiefReferee.computerNumber), person(s.roomCommissioner.computerNumber, true)].join(','));
    const start = hhmm(s.startHour);
    const end = hhmm(s.endHour);
    cols.push(start && end ? `${start},${end}` : '');
    if (s.comments.length > 0) cols.push('"' + s.comments.replace(/\\/g, '\\\\').replace(/\r?\n/g, '\\n').replace(/"/g, "'") + '"');
  }
  return cols.join(';') + '\r\n';
}

/** A full upload file: the comment header and one I line per sheet. */
export function exportCsv(sheets: { sheet: Sheet; sys: MatchSystem }[], partial = false): string {
  return HEADER + sheets.map(({ sheet, sys }) => iLine(sheet, sys, partial)).join('');
}
