// The open match sheets and their sync with the server.
//
// Each sheet has a `base` (the last version the server confirmed, with its `rev`) and a queue
// of local batches of ops that the server has not confirmed yet. What the screen shows is
// `view` = base + all pending ops. Everything is kept in local storage, so a reload or a
// captive-portal login in the hall loses nothing.
//
// A batch gets a (client, seq) when it is first sent and keeps it for every retry: the server
// ignores a batch it has already applied. Ops only "set" a value, so applying one twice is
// harmless; the last write wins per field.

import { api, ApiError, OfflineError, type StoredSheet } from './api';
import { applyOps, setAt, type Op } from './sheet/ops';
import type { Sheet } from './sheet/types';
import { getSystem } from './sheet/systems';
import { computeScore, isByeClub } from './sheet/score';
import { session } from './session.svelte';
import { settings } from './settings.svelte';
import { importOps, uploadOps } from './sheet/vttl';
import { exportCsv } from './sheet/export';
import { data, loadMembers, defaultCategory, clubName, memberKey } from './data.svelte';
import type { Member } from './types';

interface Batch {
  client: string | null;
  seq: number | null;
  ops: Op[];
}

export interface Entry {
  base: Sheet;
  rev: number;
  view: Sheet;
  pending: Batch[];
  /** Made on this device while offline: the server does not have it yet. */
  needsCreate: boolean;
}

export type Screen = 'overview' | string;

const KEY = 'vl.workspace';

interface Persisted {
  entries: Record<string, Omit<Entry, 'view'>>;
  open: string[];
  active: Screen | null;
  seq: number;
}

function restore(): Persisted {
  try {
    const v = localStorage.getItem(KEY);
    if (v) return JSON.parse(v);
  } catch {}
  return { entries: {}, open: [], active: null, seq: 0 };
}

const saved = restore();
const tabClient = (() => {
  try {
    let id = sessionStorage.getItem('vl.client');
    if (!id) sessionStorage.setItem('vl.client', (id = crypto.randomUUID()));
    return id;
  } catch {
    return crypto.randomUUID();
  }
})();

export const ws = $state({
  entries: Object.fromEntries(
    Object.entries(saved.entries).map(([id, e]) => [id, { ...e, view: applyOps(e.base, e.pending.flatMap((b) => b.ops)) }]),
  ) as Record<string, Entry>,
  open: saved.open,
  active: saved.active as Screen | null,
  /** Short status for the status bar. */
  message: '',
  syncing: false,
  live: false,
});
let seq = saved.seq;

let persistQueued = false;
/**
 * Writes the workspace to local storage at the end of the current task. Not later: a reload or
 * a closed tab right after a score must not lose it.
 */
function persist() {
  if (persistQueued) return;
  persistQueued = true;
  queueMicrotask(() => {
    persistQueued = false;
    const entries: Persisted['entries'] = {};
    for (const [id, e] of Object.entries(ws.entries)) {
      entries[id] = { base: $state.snapshot(e.base) as Sheet, rev: e.rev, pending: $state.snapshot(e.pending) as Batch[], needsCreate: e.needsCreate };
    }
    try {
      localStorage.setItem(KEY, JSON.stringify({ entries, open: $state.snapshot(ws.open), active: ws.active, seq } satisfies Persisted));
    } catch {}
  });
}

export const pendingCount = () => Object.values(ws.entries).reduce((n, e) => n + e.pending.reduce((m, b) => m + b.ops.length, 0) + (e.needsCreate ? 1 : 0), 0);
export const hasPending = (id: string) => {
  const e = ws.entries[id];
  return !!e && (e.pending.length > 0 || e.needsCreate);
};

function recompute(e: Entry) {
  e.view = applyOps($state.snapshot(e.base) as Sheet, $state.snapshot(e.pending).flatMap((b) => b.ops));
}

function adopt(e: Entry, s: StoredSheet) {
  e.base = s.doc;
  e.rev = s.rev;
  recompute(e);
}

// ---------------------------------------------------------------- local changes

/**
 * Applies a change at once and queues it for the server.
 * `source: 'vttl'` = an import: no automatic end hour and no live upload.
 */
export function change(id: string, ops: Op[], source: 'user' | 'vttl' = 'user') {
  const e = ws.entries[id];
  if (!e || !ops.length) return;
  const before = teamScore(e.view);
  for (const op of ops) setAt(e.view, op.p, op.v);
  const last = e.pending.at(-1);
  if (last && last.seq === null) last.ops.push(...ops);
  else e.pending.push({ client: null, seq: null, ops: [...ops] });
  persist();
  scheduleFlush(100);
  if (source !== 'user') return;

  const after = teamScore(e.view);
  // ScoreSheet fills in the end hour when the result becomes complete.
  if (!before.complete && after.complete && !e.view.endHour && !isByeClub(e.view.home.clubId) && !isByeClub(e.view.away.clubId)) {
    const now = new Date();
    const hh = `${String(now.getHours()).padStart(2, '0')}:${String(now.getMinutes()).padStart(2, '0')}`;
    change(id, [{ p: ['endHour'], v: hh }], 'vttl');
  }
  // ScoreSheet LiveUpdateService: upload when the team score changes.
  if (before.home !== after.home || before.away !== after.away) scheduleLive(id);
}

function teamScore(s: Sheet) {
  try {
    const sc = computeScore(s, getSystem(s.matchSystemId));
    return { home: sc.homeWins, away: sc.awayWins, complete: sc.winner !== 'error' && sc.winner !== 'incomplete' };
  } catch {
    return { home: 0, away: 0, complete: false };
  }
}

/** Shortcut for one field. */
export const set = (id: string, p: Op['p'], v: unknown) => change(id, [{ p, v }]);

// ---------------------------------------------------------------- open / close

/** Adds sheets (from the server, or new local ones) and opens them in this order. */
export function openSheets(sheets: { stored?: StoredSheet; local?: Sheet }[], activate = true) {
  for (const s of sheets) {
    const id = s.stored?.id ?? s.local!.id;
    if (!ws.entries[id]) {
      const doc = s.stored?.doc ?? s.local!;
      ws.entries[id] = { base: doc, rev: s.stored?.rev ?? 0, view: structuredClone(doc), pending: [], needsCreate: !s.stored };
    }
    if (!ws.open.includes(id)) ws.open.push(id);
  }
  sortOpen();
  if (activate && sheets.length) ws.active = sheets[0].stored?.id ?? sheets[0].local!.id;
  persist();
  scheduleFlush(0);
  for (const s of sheets) importFromVttl(s.stored?.id ?? s.local!.id).catch(() => {});
}

/** Tiles from best team to worst: A, B, C … (the letter of our own team), then by hour. */
function sortOpen() {
  const club = (session.me?.account.club ?? '').replace('-', '').toUpperCase();
  const key = (id: string) => {
    const s = ws.entries[id]?.view;
    if (!s) return ['~', ''];
    const ours = s.home.clubId.replace('-', '').toUpperCase() === club ? s.home.name : s.away.name;
    const letter = ours.split(' ').at(-1) ?? '';
    return [String(letter.length).padStart(2, '0') + letter, s.startHour];
  };
  ws.open.sort((a, b) => {
    const [la, ha] = key(a);
    const [lb, hb] = key(b);
    return la.localeCompare(lb) || ha.localeCompare(hb);
  });
}

export function close(id: string) {
  ws.open = ws.open.filter((x) => x !== id);
  if (ws.active === id) ws.active = ws.open[0] ?? 'overview';
  const e = ws.entries[id];
  // Keep unsent changes until they reach the server.
  if (e && !e.pending.length && !e.needsCreate) delete ws.entries[id];
  persist();
}

export function closeAll() {
  for (const id of [...ws.open]) close(id);
  ws.active = null;
  persist();
}

export function activate(s: Screen) {
  ws.active = s;
  persist();
}

// ---------------------------------------------------------------- sync

let flushTimer: ReturnType<typeof setTimeout> | undefined;
let flushing = false;
let backoff = 0;

export function scheduleFlush(ms: number) {
  clearTimeout(flushTimer);
  flushTimer = setTimeout(flush, ms);
}

function stamp() {
  const d = new Date();
  return d.toLocaleTimeString('nl-BE', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

export async function flush() {
  if (flushing) return;
  flushing = true;
  ws.syncing = true;
  const refetch = new Set<string>();
  try {
    for (const [id, e] of Object.entries(ws.entries)) {
      if (e.needsCreate) {
        const r = await api.createSheet($state.snapshot(e.base) as Sheet);
        e.needsCreate = false;
        if (r.created) e.rev = r.sheet.rev;
        else refetch.add(id); // somebody else made it first: take theirs, keep our pending ops on top
        persist();
      }
      while (e.pending.length) {
        const b = e.pending[0];
        if (b.seq === null) {
          b.client = tabClient;
          b.seq = ++seq;
          persist();
        }
        let r;
        try {
          r = await api.sheetOps(id, b.client!, b.seq, $state.snapshot(b.ops) as Op[]);
        } catch (err) {
          if (err instanceof ApiError && err.status === 404) {
            e.needsCreate = true; // the server lost it (or never had it): send it again
            persist();
          }
          throw err;
        }
        e.pending.shift();
        if (r.rev === e.rev + 1) {
          e.base = applyOps($state.snapshot(e.base) as Sheet, $state.snapshot(b.ops) as Op[]);
          e.rev = r.rev;
        } else if (r.rev > e.rev) {
          refetch.add(id);
        }
        persist();
      }
    }
    for (const id of refetch) await refresh(id);
    backoff = 0;
    ws.message = `Alle wijzigingen opgeslagen (${stamp()})`;
  } catch (err) {
    if (err instanceof OfflineError) {
      backoff = Math.min(backoff ? backoff * 2 : 2000, 15000);
      ws.message = `${pendingCount()} wijziging(en) wachten op verbinding`;
      scheduleFlush(backoff);
    } else if (err instanceof ApiError && err.status === 404) {
      scheduleFlush(500);
    } else {
      ws.message = err instanceof Error ? `Opslaan mislukt: ${err.message}` : 'Opslaan mislukt';
      backoff = Math.min(backoff ? backoff * 2 : 5000, 30000);
      scheduleFlush(backoff);
    }
  } finally {
    flushing = false;
    ws.syncing = false;
  }
}

/** Reloads one sheet from the server and keeps the local pending ops on top. */
export async function refresh(id: string) {
  const e = ws.entries[id];
  if (!e) return;
  try {
    adopt(e, await api.sheet(id));
    persist();
  } catch (err) {
    if (err instanceof ApiError && err.status === 404) {
      e.needsCreate = true;
      scheduleFlush(0);
    } else throw err;
  }
}

async function refreshAll() {
  for (const id of Object.keys(ws.entries)) {
    try {
      await refresh(id);
    } catch {
      return;
    }
  }
}

// ---------------------------------------------------------------- live updates

type Listener = (msg: { type: string; id?: string; day?: string }) => void;
const listeners = new Set<Listener>();
export const onServerEvent = (l: Listener) => (listeners.add(l), () => listeners.delete(l));

let socket: WebSocket | null = null;
let retry = 0;

export function connect() {
  if (socket || !session.me?.account.club) return;
  const url = `${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/api/ws`;
  const s = new WebSocket(url);
  socket = s;
  s.onopen = async () => {
    retry = 0;
    ws.live = true;
    // Catch up on what we missed while away, then send our own changes.
    await refreshAll();
    scheduleFlush(0);
  };
  s.onmessage = (m) => {
    let msg;
    try {
      msg = JSON.parse(m.data);
    } catch {
      return;
    }
    if (msg.type === 'ops') {
      const e = ws.entries[msg.id];
      if (e) {
        if (msg.rev === e.rev + 1) {
          e.base = applyOps($state.snapshot(e.base) as Sheet, msg.ops);
          e.rev = msg.rev;
          recompute(e);
          persist();
        } else if (msg.rev > e.rev) refresh(msg.id).catch(() => {});
      }
    } else if (msg.type === 'resync') {
      refreshAll();
    }
    for (const l of listeners) l(msg);
  };
  s.onclose = () => {
    socket = null;
    ws.live = false;
    retry = Math.min(retry ? retry * 2 : 1000, 15000);
    setTimeout(connect, retry);
  };
}

addEventListener('online', () => {
  scheduleFlush(0);
  if (!socket) connect();
});
document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'visible') scheduleFlush(0);
});


// ---------------------------------------------------------------- VTTL: import

const memberIn = (club: string, category: number, id: number): Member | undefined =>
  data.members[memberKey(club, category)]?.find((m) => m.uniqueIndex === id);

/** Brings new or changed VTTL data into one sheet (see sheet/vttl.ts). */
export async function importFromVttl(id: string) {
  const e = ws.entries[id];
  if (!e || !e.view.isOfficial) return;
  const s = e.view;
  const m = await api.match(s.matchId, s.season || undefined);
  const sys = getSystem(s.matchSystemId);
  const def = defaultCategory();
  // Strength lists, for the position and index that VTTL does not give.
  await Promise.allSettled(
    [s.home.clubId, s.away.clubId].filter((c) => c && !isByeClub(c)).flatMap((c) => [loadMembers(c, s.playerCategory), loadMembers(c, def)]),
  );
  const cur = ws.entries[id]?.view;
  if (!cur) return;
  const ops = importOps(cur, sys, m, {
    member: (club, n) => memberIn(club, cur.playerCategory, n) ?? memberIn(club, def, n),
    clubName,
  });
  if (ops.length) {
    change(id, ops, 'vttl');
    ws.message = `Gegevens van VTTL binnengehaald voor ${cur.home.name} – ${cur.away.name}`;
  }
}

const isRecent = (day: string) => {
  const d = new Date(day + 'T12:00:00');
  const diff = (Date.now() - d.getTime()) / 86400000;
  return diff > -1 && diff < 2; // today or yesterday
};

/** ScoreSheet checks away matches every 5 minutes; we check every open sheet of today/yesterday. */
setInterval(() => {
  for (const id of ws.open) {
    const s = ws.entries[id]?.view;
    if (s && isRecent(s.day)) importFromVttl(id).catch(() => {});
  }
}, 5 * 60 * 1000);

// ---------------------------------------------------------------- VTTL: upload

export interface UploadResult {
  ok: boolean;
  dryRun: boolean;
  errors: string[];
  data: string;
}

/** Uploads one sheet (full or partial) and records the result in the sheet. */
export async function uploadSheet(id: string, partial = false): Promise<UploadResult> {
  const s = $state.snapshot(ws.entries[id].view) as Sheet;
  const sys = getSystem(s.matchSystemId);
  const csv = exportCsv([{ sheet: s, sys }], partial);
  const r = await api.upload(id, csv, partial);
  change(id, uploadOps(s, sys, { at: Date.now(), by: session.me?.account.account ?? '', ok: r.ok, partial, dryRun: r.dryRun, errors: r.errors }), 'vttl');
  return { ...r, data: csv };
}

const liveTimers = new Map<string, ReturnType<typeof setTimeout>>();
const liveBusy = new Set<string>();

function scheduleLive(id: string) {
  clearTimeout(liveTimers.get(id));
  liveTimers.set(id, setTimeout(() => liveUpload(id), 1500));
}

/** ScoreSheet LiveUpdateService rules. */
async function liveUpload(id: string) {
  const s = ws.entries[id]?.view;
  if (!s || !session.online || settings.liveUpdates === 'off') return;
  if (settings.liveUpdates === 'super' && s.level !== 'super') return;
  const sys = getSystem(s.matchSystemId);
  if (!s.isOfficial || !sys.competitive || s.disablePartialUpload || s.vttlLocked) return;
  // A complete result is never sent as a live update: people would think it was the real upload.
  if (teamScore(s).complete) return;
  if (liveBusy.has(id)) return scheduleLive(id);
  liveBusy.add(id);
  try {
    const r = await uploadSheet(id, true);
    ws.message = r.ok
      ? `Tussenstand ${r.dryRun ? '(proef) ' : ''}naar VTTL gestuurd (${new Date().toLocaleTimeString('nl-BE', { hour: '2-digit', minute: '2-digit' })})`
      : `Tussenstand naar VTTL mislukt: ${r.errors.join('; ')}`;
  } catch {
    // Offline: ScoreSheet does not retry a live update either; the next score change sends it.
  } finally {
    liveBusy.delete(id);
  }
}
