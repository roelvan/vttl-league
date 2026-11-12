// Two-way sync with VTTL.
//
// Both sides are reduced to the same "canonical" key -> text map (what an upload sends and
// what GetMatches returns): player numbers, doubles, forfeits, captains, referee, substitutes
// and one TabT notation per game ("4/-7/9/0", "8", "9", "-").
//
// - Import: a VTTL value wins when it is new or changed since the last import (`vttlSeen`).
//   An unchanged VTTL value never overwrites a newer local edit, and an empty one never erases.
// - After a real upload the server reads the match back from VTTL (src/uploader.rs): `vttlSeen`
//   becomes what VTTL really holds, and `upload.confirmed` tells if that is what we sent.
// - Status: the sheet is "synced" when its canonical map equals `vttlSeen`.

import type { Member, TeamMatch } from '../types';
import type { Op } from './ops';
import type { MatchSystem } from './systems';
import type { Person, SetScore, Sheet } from './types';
import { gameNotation } from './export';
import { gameResult } from './score';
import { titleCase } from '../format';

type Side = 'home' | 'away';
const SIDES: Side[] = ['home', 'away'];

/** TabT DoubleTeams.Team code -> sheet option (checked on 2025-26 data). */
const DOUBLE_CODES: Record<number, string> = { 1: '1/2', 2: '1/3', 3: '2/3' };

export function canonical(s: Sheet, sys: MatchSystem): Record<string, string> {
  const out: Record<string, string> = {};
  for (const side of SIDES) {
    const t = s[side];
    sys.slots.forEach((slot) => {
      if (slot.kind === 'single') out[`${side}.p.${slot.player}`] = (t.players[slot.player]?.computerNumber ?? '').trim();
      else if (slot.kind === 'double') out[`${side}.d.${slot.double}`] = t.doubles[slot.double] ?? '';
      else out[`${side}.sub.${slot.sub}`] = t.substitutes[slot.sub] == null ? '' : String(t.substitutes[slot.sub]);
    });
    out[`${side}.ff`] = t.forfeit ? '1' : '';
    out[`${side}.cap`] = (side === 'home' ? s.homeCaptain : s.awayCaptain).computerNumber.trim();
  }
  out.ref = s.chiefReferee.computerNumber.trim();
  sys.games.forEach((_, gi) => {
    const sets = s.results[gi] ?? [];
    out[`g.${gi}`] = gameNotation(gameResult(sets, sys, s.home.forfeit, s.away.forfeit).won, sets, sys);
  });
  return out;
}

/** The same map from VTTL data. Keys without data on VTTL are left out (never erase). */
export function fromVttl(m: TeamMatch, sys: MatchSystem): Record<string, string> {
  const out: Record<string, string> = {};
  const d = m.details;
  if (m.isHomeForfeited) out['home.ff'] = '1';
  if (m.isAwayForfeited) out['away.ff'] = '1';
  if (!d) return out;
  for (const side of SIDES) {
    const players = side === 'home' ? d.homePlayers : d.awayPlayers;
    const doubles = side === 'home' ? d.homeDoubles : d.awayDoubles;
    sys.slots.forEach((slot) => {
      if (slot.kind === 'single') {
        const p = players.find((x) => x.position === slot.player + 1);
        if (p) out[`${side}.p.${slot.player}`] = String(p.uniqueIndex);
      } else if (slot.kind === 'double') {
        const code = doubles[slot.double]?.team ?? 0;
        if (DOUBLE_CODES[code] && slot.options.includes(DOUBLE_CODES[code])) out[`${side}.d.${slot.double}`] = DOUBLE_CODES[code];
      }
    });
    const cap = side === 'home' ? d.homeCaptain : d.awayCaptain;
    if (cap) out[`${side}.cap`] = String(cap);
  }
  if (d.referee) out.ref = String(d.referee);
  for (const r of d.results) {
    const gi = r.position - 1;
    if (gi < 0 || gi >= sys.games.length) continue;
    let n = '';
    if (r.isHomeForfeited && r.isAwayForfeited) n = '-';
    else if (r.isHomeForfeited) n = '8';
    else if (r.isAwayForfeited) n = '9';
    else n = r.scores.split(',').map((x) => x.split('|')[1]).filter((x) => x !== undefined && x !== '').join('/');
    if (n) out[`g.${gi}`] = n;
    // The Super substitute: the 4th player ("V") in a game where a substitution is allowed.
    const g = sys.games[gi];
    for (const slot of sys.slots) {
      if (slot.kind !== 'sub' || !g.allowSub) continue;
      const vPos = (sys.slots[slot.transferable] as { player: number }).player + 1;
      if (r.homeIndexes[0] === vPos) out[`home.sub.${slot.sub}`] = String(g.pos);
      if (r.awayIndexes[0] === vPos) out[`away.sub.${slot.sub}`] = String(g.pos);
    }
  }
  return out;
}

/** TabT game notation -> score boxes. "-3" = away won the set 11-3, "10" = home won 12-10. */
export function boxesFromNotation(n: string, sys: MatchSystem): SetScore[] {
  const boxes = Array.from({ length: sys.boxes }, () => ({ l: '', r: '' }));
  if (n === '8') boxes[0] = { l: 'WO', r: '' };
  else if (n === '9') boxes[0] = { l: '', r: 'WO' };
  else if (n === '-') boxes[0] = { l: 'WO', r: 'WO' };
  else
    n.split('/').forEach((x, i) => {
      if (i >= sys.boxes) return;
      const away = x.trim().startsWith('-');
      const loser = Math.abs(Number(x));
      if (!Number.isFinite(loser)) return;
      const winner = loser <= sys.pointCount - 2 ? sys.pointCount : loser + 2;
      boxes[i] = away ? { l: String(loser), r: String(winner) } : { l: String(winner), r: String(loser) };
    });
  return boxes;
}

export interface Lookup {
  /** A member of a club (strength list of the sheet's category, else the default one). */
  member: (club: string, uniqueIndex: number) => Member | undefined;
  clubName: (club: string) => string;
}

const memberName = (m: { lastName: string; firstName: string }) => `${m.lastName.toUpperCase()} ${titleCase(m.firstName)}`;

/**
 * Ops that bring VTTL changes into the sheet. Every changed key is applied and remembered in
 * `vttlSeen`; when VTTL locked the match, the sheet becomes read-only.
 */
export function importOps(s: Sheet, sys: MatchSystem, m: TeamMatch, look: Lookup): Op[] {
  const ops: Op[] = [];
  const vttl = fromVttl(m, sys);
  const mine = canonical(s, sys);
  const seen = s.vttlSeen ?? {};
  const d = m.details;
  const person = (club: string, cn: number): Person => {
    const p = [...(d?.homePlayers ?? []), ...(d?.awayPlayers ?? [])].find((x) => x.uniqueIndex === cn);
    const mem = look.member(club, cn);
    const name = p ? memberName(p) : mem ? memberName(mem) : '';
    return { name, computerNumber: String(cn), clubId: club, clubName: look.clubName(club) };
  };

  for (const [key, v] of Object.entries(vttl)) {
    if (v === (seen[key] ?? '')) continue; // VTTL did not change this: keep the local value
    ops.push({ p: ['vttlSeen', key], v });
    if (v === mine[key]) continue;
    const [a, b, c] = key.split('.');
    if (a === 'g') {
      ops.push({ p: ['results', Number(b)], v: boxesFromNotation(v, sys) });
    } else if (a === 'ref') {
      ops.push({ p: ['chiefReferee'], v: person(s.home.clubId, Number(v)) });
    } else {
      const side = a as Side;
      const team = s[side];
      if (b === 'ff') ops.push({ p: [side, 'forfeit'], v: v === '1' });
      else if (b === 'd') ops.push({ p: [side, 'doubles', Number(c)], v });
      else if (b === 'sub') ops.push({ p: [side, 'substitutes', Number(c)], v: Number(v) });
      else if (b === 'cap') {
        ops.push({ p: [side === 'home' ? 'homeCaptain' : 'awayCaptain'], v: person(team.clubId, Number(v)) });
        team.players.forEach((p, i) => {
          const isCap = p.computerNumber.trim() === v || vttl[`${side}.p.${i}`] === v;
          if (p.captain !== isCap) ops.push({ p: [side, 'players', i, 'captain'], v: isCap });
        });
      } else if (b === 'p') {
        const players = side === 'home' ? d?.homePlayers : d?.awayPlayers;
        const vp = players?.find((x) => String(x.uniqueIndex) === v);
        const mem = look.member(team.clubId, Number(v));
        ops.push({
          p: [side, 'players', Number(c)],
          v: {
            name: vp ? memberName(vp) : mem ? memberName(mem) : '',
            computerNumber: v,
            // VTTL has no strength-list position or index: take them from the strength list.
            position: mem ? String(mem.position) : '',
            index: mem ? String(mem.rankingIndex) : '',
            ranking: vp?.ranking ?? mem?.ranking ?? '',
            captain: vttl[`${side}.cap`] === v,
          },
        });
      }
    }
  }
  const locked = m.isValidated || m.isLocked;
  if (locked !== !!s.vttlLocked) ops.push({ p: ['vttlLocked'], v: locked });
  return ops;
}

export type VttlStatus = 'locked' | 'error' | 'synced' | 'pending' | 'none';

export function vttlStatus(s: Sheet, sys: MatchSystem): VttlStatus {
  if (s.vttlLocked) return 'locked';
  const mine = canonical(s, sys);
  const seen = s.vttlSeen ?? {};
  const keys = new Set([...Object.keys(mine), ...Object.keys(seen)]);
  const differs = [...keys].some((k) => (mine[k] ?? '') !== (seen[k] ?? ''));
  if (!differs) return Object.values(mine).some(Boolean) ? 'synced' : 'none';
  return uploadFailed(s) ? 'error' : 'pending';
}

/** The last real upload was refused, got no answer, or VTTL holds other data. */
const uploadFailed = (s: Sheet) => !!s.upload && !s.upload.dryRun && (!s.upload.ok || s.upload.confirmed === false);

/** Status of one game row: is its result on VTTL? `null` = nothing to show. */
export function gameStatuses(s: Sheet, sys: MatchSystem): (VttlStatus | null)[] {
  if (!s.isOfficial || s.vttlLocked) return sys.games.map(() => null);
  const mine = canonical(s, sys);
  const seen = s.vttlSeen ?? {};
  return sys.games.map((_, gi) => {
    const m = mine[`g.${gi}`] ?? '';
    const v = seen[`g.${gi}`] ?? '';
    if (m === v) return m ? 'synced' : null;
    return uploadFailed(s) ? 'error' : 'pending';
  });
}

export const GAME_STATUS_TEXT: Partial<Record<VttlStatus, string>> = {
  synced: 'Deze uitslag staat op VTTL',
  pending: 'Deze uitslag staat nog niet op VTTL',
  error: 'Deze uitslag staat nog niet op VTTL: de laatste upload is mislukt',
};

export const STATUS_TEXT: Record<VttlStatus, string> = {
  locked: 'Gevalideerd door VTTL',
  error: 'De laatste upload naar VTTL is niet gelukt',
  synced: 'VTTL heeft dezelfde gegevens als dit wedstrijdblad',
  pending: 'Dit wedstrijdblad heeft wijzigingen die nog niet op VTTL staan',
  none: 'Nog niets ingevuld',
};

/** A canonical key in words, e.g. "g.3" -> "wedstrijd 4", "home.p.0" -> "speler 1 thuis". */
export function keyLabel(key: string): string {
  const [a, b, c] = key.split('.');
  if (a === 'g') return `wedstrijd ${Number(b) + 1}`;
  if (a === 'ref') return 'scheidsrechter';
  const side = a === 'home' ? 'thuis' : 'bezoekers';
  const n = Number(c) + 1;
  if (b === 'p') return `speler ${n} ${side}`;
  if (b === 'd') return `dubbel ${n} ${side}`;
  if (b === 'sub') return `invaller ${side}`;
  if (b === 'cap') return `kapitein ${side}`;
  if (b === 'ff') return `forfait ${side}`;
  return key;
}
