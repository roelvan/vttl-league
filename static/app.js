"use strict";

// ---------- state ----------
const st = {
  config: null,
  date: null,
  matches: [],
  matchId: null,
  sheet: null,
  devices: new Map(), // "device|mqttMatch" -> device match
  links: [],
  known: [], // devices added by hand: { device_id, alias }
  // Unsaved edits. key "row:<pos>" or "lineup:H|A" -> { rev: rev the edit started from, values }
  dirty: new Map(),
};

// ---------- helpers ----------
const $ = (id) => document.getElementById(id);

function h(tag, attrs = {}, ...kids) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v === false || v == null) continue;
    if (k.startsWith("on")) el.addEventListener(k.slice(2), v);
    else if (k === "value") el.value = v;
    else el.setAttribute(k, v === true ? "" : v);
  }
  for (const k of kids.flat()) if (k != null) el.append(k instanceof Node ? k : String(k));
  return el;
}

function status(msg, isError = false) {
  $("status").textContent = msg;
  $("status").className = isError ? "error" : "";
}

async function api(method, url, body) {
  const r = await fetch(url, {
    method,
    headers: body ? { "Content-Type": "application/json" } : {},
    body: body ? JSON.stringify(body) : undefined,
  });
  const data = await r.json().catch(() => ({}));
  if (!r.ok) {
    const e = new Error(data.error || r.statusText);
    e.status = r.status;
    throw e;
  }
  return data;
}

const iso = (d) =>
  `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
// The alias ("Table 1"); the device ID when the device has no alias.
function deviceLabel(id) {
  return st.known.find((d) => d.device_id === id)?.alias || id;
}
const playerName = (p) => (p ? `${p.last_name} ${p.first_name}` : "?");

// ---------- week picker ----------
function seasonSaturdays(today) {
  const startYear = today.getMonth() >= 7 ? today.getFullYear() : today.getFullYear() - 1;
  const d = new Date(startYear, 8, 1); // 1 September
  while (d.getDay() !== 6) d.setDate(d.getDate() + 1);
  const end = new Date(startYear + 1, 4, 31); // 31 May
  const out = [];
  for (; d <= end; d.setDate(d.getDate() + 7)) out.push(new Date(d));
  return out;
}

function closestSaturday(list, today) {
  const t = new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime();
  return list.reduce((best, d) => (Math.abs(d - t) < Math.abs(best - t) ? d : best), list[0]);
}

function initWeeks() {
  const today = new Date();
  const list = seasonSaturdays(today);
  const hash = new URLSearchParams(location.hash.slice(1));
  const def = hash.get("date") || iso(closestSaturday(list, today));
  const fmt = new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric" });
  $("week").replaceChildren(...list.map((d) => h("option", { value: iso(d) }, fmt.format(d))));
  $("week").value = def;
  $("week").addEventListener("change", () => selectWeek($("week").value, null));
  selectWeek(def, hash.get("match"));
}

function saveHash() {
  const p = new URLSearchParams();
  if (st.date) p.set("date", st.date);
  if (st.matchId) p.set("match", st.matchId);
  history.replaceState(null, "", "#" + p);
}

async function selectWeek(date, matchId) {
  st.date = date;
  st.matchId = matchId;
  st.sheet = null;
  st.dirty.clear();
  saveHash();
  renderSheet();
  $("teams").replaceChildren(h("li", {}, "loading…"));
  try {
    st.matches = (await api("GET", `/api/matches?date=${date}`)).matches;
  } catch (e) {
    st.matches = [];
    status(e.message, true);
  }
  renderTeams();
  if (matchId) openMatch(matchId);
}

function renderTeams() {
  if (!st.matches.length) {
    $("teams").replaceChildren(h("li", {}, "No home matches on this day."));
    return;
  }
  $("teams").replaceChildren(
    ...st.matches.map((m) =>
      h("li", {},
        h("button", { "aria-current": m.match_id === st.matchId, onclick: () => openMatch(m.match_id) },
          `${m.home_team} – ${m.away_team}`, h("br"),
          m.live_score
            ? h("span", { class: "side-score" }, `${m.live_score.home} – ${m.live_score.away}`,
                h("small", {}, ` (${m.live_score.played}/${m.live_score.total} played)`))
            : null,
          h("br"),
          h("small", {}, `${m.time.slice(0, 5)} · ${m.match_id}${m.score ? " · VTTL " + m.score : ""}`)))
    )
  );
}

// ---------- sheet ----------
async function openMatch(matchId) {
  if (st.matchId !== matchId) st.dirty.clear();
  st.matchId = matchId;
  saveHash();
  renderTeams();
  await reloadSheet();
}

async function reloadSheet() {
  if (!st.matchId) return;
  const want = st.matchId;
  try {
    const s = await api("GET", `/api/sheet?match_id=${encodeURIComponent(want)}`);
    if (st.matchId !== want) return; // the user opened another match meanwhile
    st.sheet = s;
    st.links = s.links.concat(st.links.filter((l) => l.match_id !== want));
    renderSheet();
  } catch (e) {
    status(e.message, true);
  }
}

function renderSheet() {
  const s = st.sheet;
  if (!s) {
    $("sheet").replaceChildren(h("p", {}, st.matchId ? "loading…" : "Select a team."));
    return;
  }
  const m = s.match;
  const locked = m.is_validated || m.is_locked;
  const focused = $("sheet").contains(document.activeElement) ? document.activeElement : null;
  const focusId = focused?.id;
  const caret = focused?.selectionStart != null ? [focused.selectionStart, focused.selectionEnd] : null;
  $("sheet").replaceChildren(
    h("h1", {}, `${m.home_team} – ${m.away_team}`),
    h("p", {},
      `${m.match_id} · ${m.date} ${m.time.slice(0, 5)} · week ${m.week} · ${s.system.name}`,
      m.score ? ` · VTTL score ${m.score}` : "",
      locked ? " · VALIDATED/LOCKED on VTTL" : ""),
    h("p", { id: "team-score", class: "team-score" }, scoreText(s)),
    renderLineups(s),
    renderRows(s),
    renderUpload()
  );
  // Keep the cursor where the user types, also when another user's change reloads the sheet.
  const el = focusId && $(focusId);
  if (el) {
    el.focus();
    if (caret && el.setSelectionRange) el.setSelectionRange(...caret);
  }
}

// `taken`: players selected in the other slots of the same team. They are not shown,
// so a player cannot be selected twice.
function memberOptions(members, selected, taken = []) {
  return [h("option", { value: "" }, "—")].concat(
    members.filter((p) => p.unique_index === selected || !taken.includes(p.unique_index)).map((p) =>
      h("option", { value: p.unique_index, selected: selected === p.unique_index },
        `${p.last_name} ${p.first_name} (${p.ranking})`))
  );
}

// "WVL154 A 5 – 3 Team B · 8 of 16 played · 2 live"
function scoreText(s) {
  const sc = s.score;
  const live = s.rows.filter((r) => !r.outcome && liveFor(r.position) && !liveFor(r.position).state.finished).length;
  const lead = sc.home > sc.away ? s.match.home_team : sc.away > sc.home ? s.match.away_team : null;
  return `${s.match.home_team} ${sc.home} – ${sc.away} ${s.match.away_team}` +
    ` · ${sc.played} of ${sc.total} played` + (live ? ` · ${live} live` : "") +
    (sc.played === sc.total ? (lead ? ` · ${lead} wins` : " · draw") : lead ? ` · ${lead} leads` : "");
}

const outcomeText = (r, s) =>
  ({ home: s.match.home_team, away: s.match.away_team, void: "none (both forfeit)" })[r.outcome] ?? "";

// Lineup shown on screen: the unsaved edit, else the saved/VTTL lineup.
function lineupValues(side) {
  const d = st.dirty.get("lineup:" + side);
  if (d) return d.values;
  const sd = st.sheet[side === "H" ? "home" : "away"];
  return {
    players: Array.from({ length: st.sheet.system.players_per_team }, (_, i) => sd.players[i]?.unique_index ?? null),
    captain: sd.captain ?? null,
  };
}

function setLineup(side, values) {
  const key = "lineup:" + side;
  const base = st.dirty.get(key)?.rev ?? st.sheet[side === "H" ? "home" : "away"].rev;
  if (!values.players.includes(values.captain)) values.captain = null;
  st.dirty.set(key, { rev: base, values });
  renderSheet();
}

function renderLineups(s) {
  const n = s.system.players_per_team;
  const sides = { H: s.home, A: s.away };
  const vals = { H: lineupValues("H"), A: lineupValues("A") };
  const cell = (side, i) => {
    const v = vals[side];
    const cur = v.players[i];
    const sel = h("select", {
      id: `lineup-${side}-${i}`,
      onchange: () => {
        const players = [...v.players];
        players[i] = sel.value ? Number(sel.value) : null;
        setLineup(side, { players, captain: v.captain });
      },
    }, memberOptions(sides[side].members, cur, v.players.filter((_, j) => j !== i)));
    const cap = h("input", {
      type: "radio", name: "captain-" + side, title: "Captain", disabled: cur == null,
      checked: cur != null && v.captain === cur,
      onchange: () => setLineup(side, { players: [...v.players], captain: cur }),
    });
    const clear = cur != null
      ? h("button", {
          title: "Remove this player", tabindex: "-1",
          onclick: () => {
            const players = [...v.players];
            players[i] = null;
            setLineup(side, { players, captain: v.captain });
          },
        }, "×")
      : null;
    return h("td", {}, sel, " ", h("label", {}, cap, "C"), " ", clear);
  };
  const cls = (side) => {
    const d = st.dirty.get("lineup:" + side);
    if (!d) return null;
    return d.rev !== sides[side].rev ? "stale" : "dirty";
  };
  const info = (side) =>
    ({ local: "saved here", vttl: "from VTTL, not saved here", none: "not set" })[sides[side].source];
  return h("section", {},
    h("h2", {}, "Lineups"),
    h("table", {},
      h("thead", {}, h("tr", {}, h("th", {}, "#"),
        h("th", { class: cls("H") }, `Home: ${s.match.home_team}`),
        h("th", { class: cls("A") }, `Away: ${s.match.away_team}`))),
      h("tbody", {}, Array.from({ length: n }, (_, i) =>
        h("tr", {}, h("td", {}, i + 1), cell("H", i), cell("A", i)))),
      h("tfoot", {}, h("tr", {}, h("td"),
        ...["H", "A"].map((side) => h("td", {},
          h("button", { onclick: () => saveLineup(side) }, "Save"), " ",
          st.dirty.has("lineup:" + side) ? h("button", { onclick: () => discard("lineup:" + side) }, "Discard") : null, " ",
          vals[side].players.some((p) => p != null)
            ? h("button", { onclick: () => setLineup(side, { players: Array(n).fill(null), captain: null }) }, "Clear all")
            : null,
          " ", h("small", {}, info(side)),
          cls(side) === "stale" ? h("div", {}, h("small", {}, "Changed by another user. Discard to see it.")) : null))))
    ));
}

async function saveLineup(side) {
  const s = st.sheet;
  const members = (side === "H" ? s.home : s.away).members;
  const v = lineupValues(side);
  const players = v.players.map((id) => {
    const p = members.find((m) => m.unique_index === id);
    return p ? { unique_index: p.unique_index, first_name: p.first_name, last_name: p.last_name } : null;
  });
  const key = "lineup:" + side;
  const rev = st.dirty.get(key)?.rev ?? (side === "H" ? s.home.rev : s.away.rev);
  try {
    await api("POST", "/api/lineup", { match_id: s.match.match_id, side, players, captain: v.captain, rev });
    st.dirty.delete(key);
    status("Lineup saved.");
  } catch (e) {
    status(e.status === 409 ? "Not saved: another user changed this lineup first. Discard to see it." : e.message, true);
  }
  await reloadSheet();
}

function discard(key) {
  st.dirty.delete(key);
  renderSheet();
}

// One set box. Short form as on the score sheet, the points of the loser:
// "7" = 11-7, "-7" = 7-11, "12" = 14-12, "-0" = 0-11. "11-7" also works.
function parseSet(text) {
  const t = text.trim();
  if (!t) return null;
  let m = t.match(/^(\d{1,2})[-:](\d{1,2})$/);
  if (m) return [Number(m[1]), Number(m[2])];
  m = t.match(/^(-?)(\d{1,2})$/);
  if (!m) throw new Error(`"${t}" is not a set score (use 7, -7 or 11-7)`);
  const lo = Number(m[2]);
  const hi = lo >= 10 ? lo + 2 : 11;
  return m[1] ? [lo, hi] : [hi, lo];
}
const setToShort = ([a, b]) => (a > b ? String(b) : `-${a}`);

// Box texts -> sets. An empty box between two filled boxes is an error.
function parseBoxes(texts) {
  const sets = texts.map(parseSet);
  const last = sets.findLastIndex((x) => x);
  const gap = sets.slice(0, last + 1).findIndex((x) => !x);
  if (gap >= 0) throw new Error(`set ${gap + 1} is empty`);
  return sets.slice(0, last + 1);
}

function vttlEntryText(e) {
  if (e == null) return "";
  return { 8: "home forfeit", 9: "away forfeit", "-": "both forfeit" }[e] ?? e;
}

function liveFor(pos) {
  let best = null;
  for (const dm of st.devices.values())
    if (dm.match_id === st.matchId && dm.position === pos && (!best || dm.seq > best.seq)) best = dm;
  return best;
}

function liveText(dm) {
  if (!dm) return "";
  const s = dm.state;
  const games = s.games.map(([a, b]) => `${a}-${b}`).join(" ");
  const dev = deviceLabel(dm.device_id);
  if (s.finished) return `${dev}: finished ${s.games_won[0]}-${s.games_won[1]} (${games})`;
  return `${dev}: ${s.games_won[0]}-${s.games_won[1]} sets · ${s.current[0]}-${s.current[1]} · serve P${s.server}` +
    (games ? ` (${games})` : "");
}

function knownDevices() {
  const ids = new Set(st.known.map((d) => d.device_id));
  return [...ids].sort((a, b) => deviceLabel(a).localeCompare(deviceLabel(b)));
}

const setBoxCount = () => 2 * st.sheet.system.set_count - 1; // best of 5

function renderRows(s) {
  const devices = knownDevices();
  const sets = Array.from({ length: setBoxCount() }, (_, i) => `Set ${i + 1}`);
  return h("section", {},
    h("h2", {}, "Matches"),
    h("p", {}, h("small", {},
      "Type the points of the loser: 7 = 11-7, -7 = 7-11, 12 = 14-12. ",
      "Tab: next set. Enter: save and go to the next match.")),
    h("table", {},
      h("thead", {}, h("tr", {},
        ...["#", "Home", "Away", ...sets, "Sets", "Forfeit", "", "Source", "VTTL", "Won by", "Scoreboard", "Live"]
          .map((t) => h("th", {}, t)))),
      h("tbody", {}, s.rows.map((r) => renderRow(r, devices)))));
}

// "3-1" from the sets, home first.
function setsWon(sets) {
  const home = sets.filter(([a, b]) => a > b).length;
  return `${home}-${sets.length - home}`;
}

function renderRow(r, devices) {
  const key = "row:" + r.position;
  const d = st.dirty.get(key);
  const res = r.result;
  const n = setBoxCount();
  const saved = Array.from({ length: n }, (_, i) => (res?.sets[i] ? setToShort(res.sets[i]) : ""));
  const texts = d ? d.values.sets : saved;
  const forfeit = d ? d.values.forfeit : res?.forfeit ?? "";
  const capName = (p, side) =>
    p ? playerName(p) + (st.sheet[side].captain === p.unique_index ? " (C)" : "") : "?";

  const summary = h("td", {});
  const boxes = texts.map((t, i) =>
    h("input", {
      id: `set-${r.position}-${i}`, class: "set", size: 3, value: t, autocomplete: "off",
      oninput: () => mark(),
      onkeydown: (e) => {
        if (e.key !== "Enter") return;
        e.preventDefault();
        $(`set-${nextPosition(r.position)}-0`)?.focus();
        saveResult(r);
      },
    }));
  const ff = h("select", { tabindex: "-1", onchange: () => mark() },
    ...[["", "—"], ["home", "home"], ["away", "away"], ["both", "both"]].map(([v, t]) =>
      h("option", { value: v, selected: v === forfeit }, t)));

  // Shows the full score of each box and the sets won; marks a bad box.
  function check() {
    const full = [];
    for (const b of boxes) {
      try {
        const set = parseSet(b.value);
        b.classList.remove("bad");
        b.title = set ? `${set[0]}-${set[1]}` : "";
        if (set) full.push(set);
      } catch (e) {
        b.classList.add("bad");
        b.title = e.message;
      }
    }
    summary.replaceChildren(full.length ? h("span", { title: full.map(([a, b]) => `${a}-${b}`).join(" ") }, setsWon(full)) : "");
  }
  function mark() {
    const base = st.dirty.get(key)?.rev ?? r.rev;
    st.dirty.set(key, { rev: base, values: { sets: boxes.map((b) => b.value), forfeit: ff.value } });
    tr.className = base !== r.rev ? "stale" : "dirty";
    check();
  }

  const link = st.links.find((l) => l.match_id === st.matchId && l.position === r.position);
  const dev = h("select", { tabindex: "-1" }, h("option", { value: "" }, "—"),
    ...devices.map((id) => h("option", { value: id, selected: link?.device_id === id }, deviceLabel(id))));
  const single = r.kind === 1;

  const tr = h("tr", { class: d ? (d.rev !== r.rev ? "stale" : "dirty") : null },
    h("td", {}, r.position),
    h("td", {}, single ? `${r.home_index}. ${capName(r.home_player, "home")}` : "double"),
    h("td", {}, single ? `${r.away_index}. ${capName(r.away_player, "away")}` : "double"),
    ...boxes.map((b) => h("td", {}, b)),
    summary,
    h("td", {}, ff),
    h("td", {},
      h("button", { tabindex: "-1", onclick: () => saveResult(r) }, "Save"),
      d ? h("button", { tabindex: "-1", onclick: () => discard(key) }, "Discard") : null),
    h("td", {}, res?.source ?? ""),
    h("td", {}, vttlEntryText(r.vttl_entry)),
    h("td", {}, outcomeText(r, st.sheet)),
    h("td", {}, single
      ? [dev, " ", h("button", { tabindex: "-1", onclick: () => linkDevice(r.position, dev.value) }, "Link"),
         link ? h("button", { tabindex: "-1", onclick: () => unlinkDevice(link.device_id) }, "Unlink") : null]
      : "—"),
    h("td", { id: "live-" + r.position }, liveText(liveFor(r.position)))
  );
  check();
  return tr;
}

function nextPosition(pos) {
  const rows = st.sheet.rows;
  const i = rows.findIndex((x) => x.position === pos);
  return rows[i + 1]?.position;
}

async function saveResult(r) {
  const key = "row:" + r.position;
  const n = setBoxCount();
  const texts = Array.from({ length: n }, (_, i) => $(`set-${r.position}-${i}`).value);
  const forfeit = st.dirty.get(key)?.values.forfeit ?? r.result?.forfeit ?? "";
  let sets;
  try {
    sets = forfeit ? [] : parseBoxes(texts);
  } catch (e) {
    return status(`Match ${r.position}: ${e.message}`, true);
  }
  const rev = st.dirty.get(key)?.rev ?? r.rev;
  try {
    await api("POST", "/api/result", {
      match_id: st.matchId, position: r.position, sets, forfeit: forfeit || null, rev,
    });
    st.dirty.delete(key);
    status(`Match ${r.position} saved.`);
  } catch (e) {
    status(e.status === 409
      ? `Not saved: another user changed match ${r.position} first. Discard to see it.`
      : `Match ${r.position}: ${e.message}`, true);
  }
  await reloadSheet();
}

async function linkDevice(position, deviceId, force = false) {
  if (!deviceId) return status("Select a scoreboard first.", true);
  try {
    await api("POST", "/api/link", { device_id: deviceId, match_id: st.matchId, position, force });
    status(`${deviceLabel(deviceId)} linked to match ${position}. Names sent.`);
  } catch (e) {
    if (e.status === 409 && !force && confirm(`${e.message}. Take this scoreboard anyway?`))
      return linkDevice(position, deviceId, true);
    status(e.message, true);
  }
}

async function unlinkDevice(deviceId) {
  try {
    await api("DELETE", `/api/link/${encodeURIComponent(deviceId)}`);
    status(`${deviceLabel(deviceId)} unlinked.`);
  } catch (e) {
    status(e.message, true);
  }
}

function renderUpload() {
  const out = h("pre", { id: "upload-out" });
  return h("section", {},
    h("h2", {}, "VTTL upload"),
    h("p", {}, st.config?.dry_run
      ? "Dry run: the line is shown, not sent."
      : h("strong", {}, "LIVE: the line is sent to the VTTL database.")),
    h("button", { onclick: () => doUpload(out) }, st.config?.dry_run ? "Build I-line" : "Upload to VTTL"),
    out);
}

async function doUpload(out) {
  if (!st.config.dry_run && !confirm("Send the results to the LIVE VTTL database?")) return;
  out.textContent = "…";
  try {
    const r = await api("POST", "/api/upload", { match_id: st.matchId });
    out.textContent = [r.line, ...r.warnings.map((w) => "warning: " + w),
      r.dry_run ? "(dry run, not sent)" : `result: ${r.result}`, ...(r.errors || []).map((e) => "error: " + e)].join("\n");
  } catch (e) {
    out.textContent = "error: " + e.message;
  }
}

// ---------- scoreboards ----------
async function reloadDevices() {
  try {
    const d = await api("GET", "/api/devices");
    st.devices = new Map(d.matches.map((m) => [m.device_id + "|" + m.mqtt_match_id, m]));
    st.links = d.links;
    st.known = d.known;
  } catch (e) {
    status(e.message, true);
  }
  renderKnown();
  renderDevices();
}

function renderKnown() {
  const id = h("input", { size: 14, placeholder: "device ID" });
  const alias = h("input", { size: 14, placeholder: "alias, e.g. Table 1" });
  const seen = new Set([...st.devices.values()].map((dm) => dm.device_id));
  $("known").replaceChildren(
    h("table", {},
      h("thead", {}, h("tr", {}, ...["Device ID", "Alias", "Seen", ""].map((t) => h("th", {}, t)))),
      h("tbody", {},
        st.known.map((d) => {
          const a = h("input", { size: 14, value: d.alias });
          return h("tr", {},
            h("td", {}, d.device_id),
            h("td", {}, a),
            h("td", {}, seen.has(d.device_id) ? "yes" : "no"),
            h("td", {},
              h("button", { onclick: () => saveDevice(d.device_id, a.value) }, "Save"), " ",
              h("button", { onclick: () => deleteDevice(d.device_id) }, "Remove")));
        }),
        h("tr", {},
          h("td", {}, id), h("td", {}, alias), h("td"),
          h("td", {}, h("button", { onclick: () => saveDevice(id.value, alias.value) }, "Add"))))));
}

async function saveDevice(deviceId, alias) {
  if (!deviceId.trim()) return status("Enter a device ID.", true);
  try {
    await api("POST", "/api/devices", { device_id: deviceId.trim(), alias });
    status(`Device ${deviceId.trim()} saved.`);
  } catch (e) {
    status(e.message, true);
  }
}

async function deleteDevice(deviceId) {
  if (!confirm(`Remove device ${deviceLabel(deviceId)} from the list?`)) return;
  try {
    await api("DELETE", `/api/devices/${encodeURIComponent(deviceId)}`);
    status(`Device ${deviceId} removed.`);
  } catch (e) {
    status(e.message, true);
  }
}

function renderDevices() {
  if (!st.config?.mqtt) {
    $("devices").replaceChildren(h("p", {}, "MQTT is not configured on the server."));
    return;
  }
  const list = [...st.devices.values()].sort((a, b) => b.seq - a.seq);
  $("devices").replaceChildren(list.length
    ? h("table", {},
        h("thead", {}, h("tr", {}, ...["Device", "Device match", "Linked to", "State", ""].map((t) => h("th", {}, t)))),
        h("tbody", {}, list.map((dm) => h("tr", {},
          h("td", {}, deviceLabel(dm.device_id)),
          h("td", {}, dm.mqtt_match_id || "(old firmware)"),
          h("td", {}, dm.match_id ? `${dm.match_id} #${dm.position}` : ""),
          h("td", {}, liveText(dm)),
          h("td", {}, h("button", { onclick: () => clearDeviceMatch(dm) }, "Clear"))))))
    : h("p", {}, "No scoreboard data."));
  // Update the live column of the open sheet without touching any input.
  for (const r of st.sheet?.rows ?? []) {
    const cell = $("live-" + r.position);
    if (cell) cell.textContent = liveText(liveFor(r.position));
  }
  if (st.sheet && $("team-score")) $("team-score").textContent = scoreText(st.sheet);
}

async function clearDeviceMatch(dm) {
  if (!confirm(`Remove match ${dm.mqtt_match_id} of ${deviceLabel(dm.device_id)} from the broker?`)) return;
  try {
    await api("POST", `/api/devices/${encodeURIComponent(dm.device_id)}/clear`, { mqtt_match_id: dm.mqtt_match_id });
  } catch (e) {
    status(e.message, true);
  }
}

// ---------- live sync ----------
let reloadTimer = null;
function reloadSheetSoon() {
  // Several events often come at once (e.g. a result and a link). Reload once.
  clearTimeout(reloadTimer);
  reloadTimer = setTimeout(reloadSheet, 150);
}

// Reload the sidebar scores (the server reads VTTL from its 60 s cache).
let matchesTimer = null;
function reloadMatchesSoon() {
  clearTimeout(matchesTimer);
  matchesTimer = setTimeout(async () => {
    const date = st.date;
    try {
      const list = (await api("GET", `/api/matches?date=${date}`)).matches;
      if (st.date !== date) return; // the user picked another week meanwhile
      st.matches = list;
      renderTeams();
    } catch (e) {
      status(e.message, true);
    }
  }, 300);
}

function connect() {
  const ws = new WebSocket(`${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/ws`);
  ws.onopen = () => {
    $("conn").textContent = "live";
    // Events can be missed while disconnected: load everything again.
    reloadDevices();
    reloadSheet();
    if (st.date) reloadMatchesSoon();
  };
  ws.onclose = () => {
    $("conn").textContent = "disconnected, retrying…";
    setTimeout(connect, 1000);
  };
  ws.onmessage = (msg) => {
    const ev = JSON.parse(msg.data);
    switch (ev.type) {
      case "device": {
        const { type, ...dm } = ev;
        st.devices.set(dm.device_id + "|" + dm.mqtt_match_id, dm);
        renderDevices();
        break;
      }
      case "device_removed":
        st.devices.delete(ev.device_id + "|" + ev.mqtt_match_id);
        renderDevices();
        break;
      case "lineup":
        if (ev.match_id === st.matchId) reloadSheetSoon();
        break;
      case "result":
        if (ev.match_id === st.matchId) reloadSheetSoon();
        if (st.matches.some((m) => m.match_id === ev.match_id)) reloadMatchesSoon();
        break;
      case "links":
      case "devices":
        reloadDevices().then(() => st.sheet && renderSheet());
        break;
      case "resync":
        reloadDevices();
        reloadSheet();
        reloadMatchesSoon();
        break;
    }
  };
}

(async function main() {
  st.config = await api("GET", "/api/config");
  initWeeks();
  connect();
})();
