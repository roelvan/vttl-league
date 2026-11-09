// The stored match records ("sheets", never shown as such in the UI) and their sync.
//
// A match that has no stored record yet gets a local *draft* made from VTTL, so it can be shown.
// The draft is sent to the server only at the first user edit (`change` with source "user").
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
import { publicClub } from './viewer.svelte';
import { importOps } from './sheet/vttl';
import { data, loadMembers, loadCategories, defaultCategory, clubName, memberKey } from './data.svelte';
import { fromVttl } from './sheet/create';
import type { Member, MatchRow } from './types';

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
  /** Made from VTTL for display only. Becomes a real record at the first user edit. */
  draft?: boolean;
}

const KEY = 'vl.workspace';

interface Persisted {
  entries: Record<string, Omit<Entry, 'view'>>;
  current: string | null;
  seq: number;
}

/** Records of older days are dropped from this device when they have nothing to send. */
const KEEP_DAYS = 4;

function restore(): Persisted {
  try {
    const v = localStorage.getItem(KEY);
    if (v) {
      const p = JSON.parse(v) as Persisted;
      const limit = new Date(Date.now() - KEEP_DAYS * 86400000).toISOString().slice(0, 10);
      for (const [id, e] of Object.entries(p.entries ?? {})) {
        if (!e.pending.length && !e.needsCreate && (e.base.day || '0') < limit) delete p.entries[id];
      }
      return { entries: p.entries ?? {}, current: p.current ?? null, seq: p.seq ?? 0 };
    }
  } catch {}
  return { entries: {}, current: null, seq: 0 };
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
  /** The match shown last (target of the "Wedstrijd" and "Opstelling" nav items). */
  current: saved.current,
  /** Short status line (toast). */
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
      entries[id] = { base: $state.snapshot(e.base) as Sheet, rev: e.rev, pending: $state.snapshot(e.pending) as Batch[], needsCreate: e.needsCreate, draft: e.draft };
    }
    try {
      localStorage.setItem(KEY, JSON.stringify({ entries, current: ws.current, seq } satisfies Persisted));
    } catch {}
  });
}

export const pendingCount = () =>
  Object.values(ws.entries).reduce((n, e) => (e.draft ? n : n + e.pending.reduce((m, b) => m + b.ops.length, 0) + (e.needsCreate ? 1 : 0)), 0);
export const hasPending = (id: string) => {
  const e = ws.entries[id];
  return !!e && !e.draft && (e.pending.length > 0 || e.needsCreate);
};

function recompute(e: Entry) {
  e.view = applyOps($state.snapshot(e.base) as Sheet, $state.snapshot(e.pending).flatMap((b) => b.ops));
}

function adopt(e: Entry, s: StoredSheet) {
  e.base = s.doc;
  e.rev = s.rev;
  e.draft = false;
  e.needsCreate = false;
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
  if (e.draft && source === 'user') {
    e.draft = false;
    e.needsCreate = true;
  }
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

// ---------------------------------------------------------------- load matches

function getSheet(id: string) {
  const pub = publicClub();
  return pub ? api.publicSheet(pub, id) : api.sheet(id);
}

/** Puts a stored record from the server into the workspace (keeps local pending ops on top). */
export function addStored(stored: StoredSheet) {
  const e = ws.entries[stored.id];
  if (!e) ws.entries[stored.id] = { base: stored.doc, rev: stored.rev, view: structuredClone(stored.doc), pending: [], needsCreate: false };
  else if (stored.rev > e.rev || e.draft || e.needsCreate) adopt(e, stored);
  persist();
}

/** The stored records of the club for one day (club night). Offline: keep what we have. */
export async function loadDay(day: string) {
  try {
    const pub = publicClub();
    for (const s of await (pub ? api.publicSheets(pub, day) : api.sheetsOn(day))) addStored(s);
  } catch (e) {
    if (!(e instanceof OfflineError)) throw e;
  }
}

/**
 * Makes sure a match is in the workspace: the stored record from the server, or else a draft
 * made from the VTTL calendar row. Then brings in what VTTL knows (players, results).
 */
export async function ensureMatch(row: MatchRow, season: number) {
  const id = row.matchId;
  if (!ws.entries[id] || ws.entries[id].draft) {
    try {
      addStored(await getSheet(id));
    } catch (e) {
      if (!(e instanceof ApiError && e.status === 404) && !(e instanceof OfflineError)) throw e;
      if (!ws.entries[id]) {
        if (!data.categories.length) await loadCategories().catch(() => {});
        const doc = fromVttl(row, season, data.categories);
        ws.entries[id] = { base: doc, rev: 0, view: structuredClone(doc), pending: [], needsCreate: false, draft: true };
        persist();
      }
    }
  }
  scheduleFlush(0);
  await importFromVttl(id).catch(() => {});
}

export function setCurrent(id: string | null) {
  if (ws.current === id) return;
  ws.current = id;
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
  if (flushing || publicClub()) return;
  flushing = true;
  ws.syncing = true;
  const refetch = new Set<string>();
  try {
    for (const [id, e] of Object.entries(ws.entries)) {
      if (e.draft) continue;
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
    adopt(e, await getSheet(id));
    persist();
  } catch (err) {
    if (err instanceof ApiError && err.status === 404) {
      if (e.draft) return;
      e.needsCreate = true;
      scheduleFlush(0);
    } else throw err;
  }
}

async function refreshAll() {
  for (const id of Object.keys(ws.entries)) {
    if (ws.entries[id]?.draft) continue;
    try {
      await refresh(id);
    } catch {
      return;
    }
  }
}

// ---------------------------------------------------------------- live updates

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Listener = (msg: { type: string; id?: string; day?: string; [k: string]: any }) => void;
const listeners = new Set<Listener>();
export const onServerEvent = (l: Listener) => (listeners.add(l), () => listeners.delete(l));

let socket: WebSocket | null = null;
let socketPath = '';
let retry = 0;

/** Opens the live connection: the club's own, or the public one for a spectator. Safe to call again. */
export function connect() {
  const pub = publicClub();
  const path = pub ? `/api/public/ws?club=${encodeURIComponent(pub)}` : session.me?.account.club ? '/api/ws' : '';
  if (socket && socketPath === path) return;
  if (socket) {
    const old = socket;
    socket = null;
    old.onclose = null;
    old.close();
  }
  if (!path) return;
  socketPath = path;
  const url = `${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}${path}`;
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
      if (e?.draft) refresh(msg.id).catch(() => {});
      else if (e) {
        if (msg.rev === e.rev + 1 && !e.draft) {
          e.base = applyOps($state.snapshot(e.base) as Sheet, msg.ops);
          e.rev = msg.rev;
          recompute(e);
          persist();
        } else if (msg.rev > e.rev) refresh(msg.id).catch(() => {});
      }
    } else if (msg.type === 'upload' && !publicClub()) {
      const what = msg.partial ? 'Tussenstand' : 'Uitslag';
      ws.message = msg.ok ? `${what} ${msg.dryRun ? '(proef) ' : ''}naar VTTL gestuurd` : `Upload naar VTTL mislukt: ${(msg.errors ?? []).join('; ')}`;
    } else if (msg.type === 'resync') {
      refreshAll();
    }
    for (const l of listeners) l(msg);
  };
  s.onclose = () => {
    if (socket !== s) return;
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
  const pub = publicClub();
  // A spectator takes the club's stored record as it is; only a draft is filled from VTTL.
  if (pub && !e.draft) return;
  const m = pub ? await api.publicMatch(pub, s.matchId) : await api.match(s.matchId, s.season || undefined);
  const sys = getSystem(s.matchSystemId);
  const def = defaultCategory();
  // Strength lists, for the position and index that VTTL does not give.
  if (!pub) await Promise.allSettled(
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

/** ScoreSheet checks away matches every 5 minutes; we check every match of today/yesterday. */
setInterval(() => {
  for (const id of Object.keys(ws.entries)) {
    const s = ws.entries[id]?.view;
    if (s && isRecent(s.day)) importFromVttl(id).catch(() => {});
  }
}, 5 * 60 * 1000);

// ---------------------------------------------------------------- VTTL: upload
// The server uploads by itself after every stored change (src/uploader.rs). This is the manual
// upload: first send our waiting changes, so the server uploads what this screen shows.

export interface UploadResult {
  ok: boolean;
  dryRun: boolean;
  partial: boolean;
  errors: string[];
  data: string;
}

export async function uploadSheet(id: string): Promise<UploadResult> {
  await flush();
  if (hasPending(id)) throw new Error('Nog niet alle wijzigingen zijn bij de server. Probeer opnieuw als er verbinding is.');
  return api.upload(id);
}
