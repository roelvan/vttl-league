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

import { api, ApiError, OfflineError, type StoredSheet, type UploadOutcome } from './api';
import { applyOps, setAt, type Op } from './sheet/ops';
import type { Sheet } from './sheet/types';
import { getSystem } from './sheet/systems';
import { computeScore, isByeClub, isEmptySet } from './sheet/score';
import { session } from './session.svelte';
import { publicClub } from './viewer.svelte';
import { normClub } from './access';
import { importOps, keyLabel, olderThanUpload } from './sheet/vttl';
import { data, loadMembers, loadCategories, loadMatches, loadPublicMatches, mergeMatchRows, defaultCategory, clubName, memberKey } from './data.svelte';
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

type StoredEntry = Omit<Entry, 'view'>;

interface Persisted {
  /** Scope key -> match id -> entry. See `Ctx`. */
  stores: Record<string, Record<string, StoredEntry>>;
  current: string | null;
}

/**
 * Whose data an entry is. The records of the user's own club ("own:WVL154") and the records
 * that a public page of a club shows ("pub:WVL094") have the same match ids, but they come
 * from different API calls and only the first kind is ever sent to the server. Each kind has
 * its own store, so a visit to the page of another club never puts a record of that club in
 * the queue of the own club.
 *
 * A function that waits for the server takes the context at its start and keeps it: the user
 * can go to another club before the answer comes.
 */
interface Ctx {
  key: string;
  /** The club for the public API, or null for the API of the logged-in member. */
  pub: string | null;
}

const NONE = 'none';
const ownKey = () => {
  const club = session.me?.account.club;
  return club ? `own:${normClub(club)}` : NONE;
};
const ctx = (): Ctx => {
  const pub = publicClub();
  return pub ? { key: `pub:${normClub(pub)}`, pub } : { key: ownKey(), pub: null };
};
const EMPTY: Record<string, Entry> = Object.freeze({});

/** Records of older days are dropped from this device when they have nothing to send. */
const KEEP_DAYS = 4;

function restore(): Persisted {
  try {
    const v = localStorage.getItem(KEY);
    if (v) {
      const p = JSON.parse(v) as Partial<Persisted> & { entries?: Record<string, StoredEntry> };
      const stores = p.stores ?? legacy(p.entries ?? {});
      const limit = new Date(Date.now() - KEEP_DAYS * 86400000).toISOString().slice(0, 10);
      for (const [key, store] of Object.entries(stores)) {
        for (const [id, e] of Object.entries(store)) {
          // Only a record of the own club can have changes that wait for the server.
          const waits = key.startsWith('own:') && !e.draft && (e.pending.length > 0 || e.needsCreate);
          if (!waits && (e.base.day || '0') < limit) delete store[id];
        }
        if (!Object.keys(store).length) delete stores[key];
      }
      return { stores, current: p.current ?? null };
    }
  } catch {}
  return { stores: {}, current: null };
}

/**
 * The workspace of an older version had one store for all clubs. Only the changes of the own
 * club that still wait for the server are kept: a member can only change a home match, so a
 * record with another home club is not ours to send. The rest comes from the server again.
 */
function legacy(entries: Record<string, StoredEntry>): Persisted['stores'] {
  let club = '';
  try {
    club = normClub(JSON.parse(localStorage.getItem('vl.me') ?? 'null')?.account?.club);
  } catch {}
  if (!club) return {};
  const own: Record<string, StoredEntry> = {};
  for (const [id, e] of Object.entries(entries)) {
    if (!e.draft && (e.pending.length > 0 || e.needsCreate) && normClub(e.base.home?.clubId) === club) own[id] = e;
  }
  return Object.keys(own).length ? { [`own:${club}`]: own } : {};
}

const saved = restore();
/**
 * The client id of this page load, with its own batch numbers from 1. The server takes a batch
 * with a number that is not higher than the last one of the same client for a retry, and does
 * not apply it. So a client id must never continue with numbers from somewhere else: a
 * reload, a second tab or a copy of a tab each get a new id. A batch that was numbered by an
 * earlier page load keeps its own (client, seq) in the queue for its retries.
 */
const tabClient = crypto.randomUUID();

const withView = (store: Record<string, StoredEntry>) =>
  Object.fromEntries(Object.entries(store).map(([id, e]) => [id, { ...e, view: applyOps(e.base, e.pending.flatMap((b) => b.ops)) }])) as Record<string, Entry>;

class Workspace {
  /** All records on this device: scope key -> match id -> entry. */
  stores = $state<Record<string, Record<string, Entry>>>(Object.fromEntries(Object.entries(saved.stores).map(([key, store]) => [key, withView(store)])));
  /** The match shown last (target of the "Wedstrijd" and "Opstelling" nav items). */
  current = $state<string | null>(saved.current);
  /** Short status line (toast). */
  message = $state('');
  syncing = $state(false);
  live = $state(false);
  /** Matches that still wait for the server and VTTL (a match that is new on this device). */
  loading = $state<Record<string, boolean>>({});

  /** The records of the club on screen. Read-only: the functions of this module write. */
  get entries(): Record<string, Entry> {
    return this.stores[ctx().key] ?? EMPTY;
  }
}

export const ws = new Workspace();
let seq = 0;

/** The store of a context, made when it is not there. Not for use in a `$derived`. */
const bucket = (c: Ctx) => {
  // Read it back: the assignment gives the plain object, the store holds its reactive proxy.
  if (!ws.stores[c.key]) ws.stores[c.key] = {};
  return ws.stores[c.key];
};
/** The records of the user's own club: the only ones with changes for the server. */
const ownEntries = () => ws.stores[ownKey()] ?? EMPTY;

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
    const stores: Persisted['stores'] = {};
    for (const [key, store] of Object.entries(ws.stores)) {
      if (key === NONE) continue;
      const entries: Record<string, StoredEntry> = (stores[key] = {});
      for (const [id, e] of Object.entries(store)) {
        entries[id] = { base: $state.snapshot(e.base) as Sheet, rev: e.rev, pending: $state.snapshot(e.pending) as Batch[], needsCreate: e.needsCreate, draft: e.draft };
      }
    }
    try {
      localStorage.setItem(KEY, JSON.stringify({ stores, current: ws.current } satisfies Persisted));
    } catch {}
  });
}

export const pendingCount = () =>
  Object.values(ownEntries()).reduce((n, e) => (e.draft ? n : n + e.pending.reduce((m, b) => m + b.ops.length, 0) + (e.needsCreate ? 1 : 0)), 0);
export const hasPending = (id: string) => {
  const e = ownEntries()[id];
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
export function change(id: string, ops: Op[], source: 'user' | 'vttl' = 'user', c: Ctx = ctx()) {
  const e = ws.stores[c.key]?.[id];
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
    change(id, [{ p: ['endHour'], v: hh }], 'vttl', c);
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

function getSheet(id: string, c: Ctx) {
  return c.pub ? api.publicSheet(c.pub, id) : api.sheet(id);
}

/** Puts a stored record from the server into the workspace (keeps local pending ops on top). */
export function addStored(stored: StoredSheet, c: Ctx = ctx()) {
  const entries = bucket(c);
  const e = entries[stored.id];
  if (!e) entries[stored.id] = { base: stored.doc, rev: stored.rev, view: structuredClone(stored.doc), pending: [], needsCreate: false };
  else if (stored.rev > e.rev || e.draft || e.needsCreate) adopt(e, stored);
  persist();
}

/** The stored records of the club for one day (club night). Offline: keep what we have. */
export async function loadDay(day: string) {
  const c = ctx();
  try {
    for (const s of await (c.pub ? api.publicSheets(c.pub, day) : api.sheetsOn(day))) addStored(s, c);
  } catch (e) {
    if (!(e instanceof OfflineError)) throw e;
  }
}

/**
 * Makes sure a match is in the workspace. A match that is not on this device gets a draft from
 * the VTTL calendar row at once, so the page shows without a wait for the server. Then the
 * stored record of the server replaces the draft (if there is one), and what VTTL knows
 * (players, results) comes in.
 */
export async function ensureMatch(row: MatchRow, season: number) {
  const id = row.matchId;
  const c = ctx();
  const e = ws.stores[c.key]?.[id];
  // Only a match without a stored record can still change much: the page shows a skeleton.
  if (!e || e.draft) ws.loading[id] = true;
  try {
    await bringIn(row, season, c);
  } finally {
    delete ws.loading[id];
  }
}

const hasResult = (row: MatchRow) => /[1-9]/.test(row.score) || !!row.games;

/** VTTL has a result, but the record on this device has no players or no set scores yet. */
const lacksResult = (s: Sheet) =>
  ![...s.home.players, ...s.away.players].some((p) => p?.name) || !s.results.some((g) => g?.some((x) => !isEmptySet(x)));

/**
 * Brings the played matches of a day onto this device in the background, three at a time, so
 * the club night shows the players and a click on a match shows it complete at once. A match
 * that is here with its players and results is left alone: the server sends its changes. A
 * record without them (an old draft, or a stored record that was never filled) gets the VTTL
 * data. A match without a result has nothing to load.
 */
export async function preloadDay(rows: MatchRow[], season: number) {
  const c = ctx();
  const entries = bucket(c);
  const todo = rows.filter((row) => {
    if (row.isBye || !hasResult(row)) return false;
    const e = entries[row.matchId];
    return !e || lacksResult(e.view);
  });
  const next = async (): Promise<void> => {
    const row = todo.shift();
    if (!row) return;
    const e = entries[row.matchId];
    await (e && !e.draft ? importFromVttl(row.matchId, c) : bringIn(row, season, c)).catch(() => {});
    return next();
  };
  await Promise.all([next(), next(), next()]);
}

async function bringIn(row: MatchRow, season: number, c: Ctx) {
  const id = row.matchId;
  const entries = bucket(c);
  let draftError: unknown;
  if (!entries[id]) {
    try {
      if (!data.categories.length) await loadCategories().catch(() => {});
      const doc = fromVttl(row, season, data.categories);
      if (!entries[id]) {
        entries[id] = { base: doc, rev: 0, view: structuredClone(doc), pending: [], needsCreate: false, draft: true };
        persist();
      }
    } catch (e) {
      draftError = e; // the stored record can still be there
    }
  }
  if (!entries[id] || entries[id].draft) {
    try {
      addStored(await getSheet(id, c), c);
    } catch (e) {
      if (!(e instanceof ApiError && e.status === 404) && !(e instanceof OfflineError)) throw e;
      if (!entries[id]) throw draftError ?? e;
    }
  }
  scheduleFlush(0);
  await importFromVttl(id, c).catch(() => {});
}

export function setCurrent(id: string | null) {
  if (ws.current === id) return;
  ws.current = id;
  persist();
}

// ---------------------------------------------------------------- sync

let flushTimer: ReturnType<typeof setTimeout> | undefined;
let flushing = false;
/** Somebody asked for a flush while one was running: its loop can be past their match. */
let flushAgain = false;
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
  // Only the records of the user's own club go to the server, with the API of the member.
  const c = ctx();
  if (c.pub || c.key === NONE) return;
  if (flushing) {
    flushAgain = true;
    return;
  }
  flushing = true;
  flushAgain = false;
  ws.syncing = true;
  let failed = false;
  const refetch = new Set<string>();
  try {
    for (const [id, e] of Object.entries(bucket(c))) {
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
    for (const id of refetch) await refresh(id, c);
    backoff = 0;
    ws.message = `Alle wijzigingen opgeslagen (${stamp()})`;
  } catch (err) {
    failed = true;
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
    // After a failure the retry is planned already, and it sends everything.
    if (flushAgain && !failed) scheduleFlush(0);
  }
}

/** Reloads one sheet from the server and keeps the local pending ops on top. */
export async function refresh(id: string, c: Ctx = ctx()) {
  const e = ws.stores[c.key]?.[id];
  if (!e) return;
  try {
    const s = await getSheet(id, c);
    // A push or the answer to our own change came while this request was on its way: the
    // record here is newer than this answer.
    if (s.rev < e.rev && !e.draft && !e.needsCreate) return;
    adopt(e, s);
    persist();
  } catch (err) {
    if (err instanceof ApiError && err.status === 404) {
      // A spectator has nothing to send: the record is gone, the VTTL data stays on screen.
      if (e.draft || c.pub) return;
      e.needsCreate = true;
      scheduleFlush(0);
    } else throw err;
  }
}

async function refreshAll(c: Ctx) {
  const entries = bucket(c);
  for (const id of Object.keys(entries)) {
    if (entries[id]?.draft) continue;
    try {
      await refresh(id, c);
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
/** How many times the connection of this path opened: a later open is a reconnect. */
let opens = 0;

/** Opens the live connection: the club's own, or the public one for a spectator. Safe to call again. */
export function connect() {
  // The messages of this connection are for the club it was opened for.
  const c = ctx();
  const pub = c.pub;
  const path = pub ? `/api/public/ws?club=${encodeURIComponent(pub)}` : session.me?.account.club ? '/api/ws' : '';
  if (socket && socketPath === path) return;
  if (socket) {
    const old = socket;
    socket = null;
    old.onclose = null;
    old.close();
  }
  if (!path) return;
  if (socketPath !== path) opens = 0;
  socketPath = path;
  const url = `${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}${path}`;
  const s = new WebSocket(url);
  socket = s;
  s.onopen = async () => {
    retry = 0;
    ws.live = true;
    // Catch up on what we missed while away, then send our own changes.
    if (opens++) catchUpVttl(c);
    await refreshAll(c);
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
      const e = ws.stores[c.key]?.[msg.id];
      if (e?.draft) refresh(msg.id, c).catch(() => {});
      else if (e) {
        if (msg.rev === e.rev + 1 && !e.draft) {
          e.base = applyOps($state.snapshot(e.base) as Sheet, msg.ops);
          e.rev = msg.rev;
          recompute(e);
          persist();
        } else if (msg.rev > e.rev) refresh(msg.id, c).catch(() => {});
      }
    } else if (msg.type === 'upload' && !c.pub) {
      // A good upload shows in the top bar and on the match; only a failure gets a message.
      const o = msg as unknown as UploadOutcome;
      if (!o.dryRun && (!o.ok || o.confirmed === false)) ws.message = uploadMessage(o);
    } else if (msg.type === 'vttl') {
      // The server found new VTTL data: the match list and the open matches follow.
      const rows = msg.rows as MatchRow[];
      mergeMatchRows(c.pub, msg.season, rows);
      for (const r of rows) importFromVttl(r.matchId, c).catch(() => {});
    } else if (msg.type === 'resync') {
      catchUpVttl(c);
      refreshAll(c);
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
export async function importFromVttl(id: string, c: Ctx = ctx()) {
  const e = ws.stores[c.key]?.[id];
  if (!e || !e.view.isOfficial) return;
  const s = e.view;
  const pub = c.pub;
  // A spectator takes the club's stored record as it is; only a draft is filled from VTTL.
  if (pub && !e.draft) return;
  const m = pub ? await api.publicMatch(pub, s.matchId) : await api.match(s.matchId, s.season || undefined);
  const sys = getSystem(s.matchSystemId);
  const def = defaultCategory();
  // Strength lists, for the position and index that VTTL does not give.
  if (!pub) await Promise.allSettled(
    [s.home.clubId, s.away.clubId].filter((c) => c && !isByeClub(c)).flatMap((c) => [loadMembers(c, s.playerCategory), loadMembers(c, def)]),
  );
  const cur = ws.stores[c.key]?.[id]?.view;
  // Data from before the last upload: the read-back of that upload is newer.
  if (!cur || olderThanUpload(cur, m)) return;
  const ops = importOps(cur, sys, m, {
    member: (club, n) => memberIn(club, cur.playerCategory, n) ?? memberIn(club, def, n),
    clubName,
  });
  if (ops.length) change(id, ops, 'vttl', c);
}

const isRecent = (day: string) => {
  const d = new Date(day + 'T12:00:00');
  const diff = (Date.now() - d.getTime()) / 86400000;
  return diff > -1 && diff < 2; // today or yesterday
};

/**
 * The server sends new VTTL data as it finds it ("vttl" message), so there is no timer here.
 * After a break in the connection we ask once for what we missed: the match list, and the
 * VTTL data of every match of today and yesterday on this device.
 */
function catchUpVttl(c: Ctx) {
  (c.pub ? loadPublicMatches(c.pub) : loadMatches()).catch(() => {});
  const entries = bucket(c);
  for (const id of Object.keys(entries)) {
    const s = entries[id]?.view;
    if (s && isRecent(s.day)) importFromVttl(id, c).catch(() => {});
  }
}

// ---------------------------------------------------------------- VTTL: upload
// The server uploads by itself after every stored change (src/uploader.rs). This is the manual
// upload: first send our waiting changes, so the server uploads what this screen shows.

export type UploadResult = UploadOutcome;

/** One line about an upload and its read-back from VTTL. */
export function uploadMessage(o: UploadOutcome): string {
  const what = o.partial ? 'Tussenstand' : 'Uitslag';
  if (o.dryRun) return `${what} (proef) klaar, niets naar VTTL gestuurd`;
  if (o.confirmed === true) return `${what} staat op VTTL (gecontroleerd)`;
  if (o.confirmed === false) return `${what} gestuurd, maar VTTL toont andere gegevens: ${(o.diff ?? []).map(keyLabel).join(', ')}. De server probeert opnieuw.`;
  if (!o.ok) return `Upload naar VTTL mislukt: ${(o.errors ?? []).join('; ')}. De server probeert opnieuw.`;
  return `${what} naar VTTL gestuurd, nog niet gecontroleerd. De server probeert opnieuw.`;
}

export async function uploadSheet(id: string): Promise<UploadResult> {
  await flush();
  if (hasPending(id)) throw new Error('Nog niet alle wijzigingen zijn bij de server. Probeer opnieuw als er verbinding is.');
  return api.upload(id);
}
