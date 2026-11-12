//! Uploads to VTTL, done by the server (VTTL wants results as soon as possible).
//!
//! Every stored change of a match (an edit from any device, or a scoreboard result) schedules
//! an upload of that match a moment later. The server builds the upload lines itself
//! (src/rules.rs) from the stored match:
//! - complete result: the full upload, once per change (also when every phone is closed);
//! - otherwise: a partial upload when a game was decided since the last upload.
//!
//! The VTTL login: the account that made the last change, then the other accounts of the club
//! (those with a successful real upload first). VTTL fault 22 (no rights) tries the next one.
//! Uploads stay dry runs unless UPLOAD_ENABLED=true. The result is written into the match
//! (`upload`, `vttlSeen`) like any other change, so every device shows it.
//!
//! Read-back: VTTL only answers "Result true/false". So after a real upload (and after a network
//! error, which may hide a stored upload) the server reads the match back from VTTL and compares
//! it with what it sent. Only then is the upload "confirmed". `vttlSeen` becomes what VTTL really
//! holds, never what we only hoped it holds.
//!
//! Retry queue: every minute, the server checks the matches of yesterday, today and tomorrow. A
//! match with data that VTTL does not have (yet) is uploaded again, with a longer wait after each
//! failure (1, 2, 4, 8, then every 15 minutes). This also catches uploads lost by a restart.
//!
//! Every attempt is kept in the `upload` table: the VTTL log of a match.

use crate::app::App;
use crate::db::Db;
use crate::rules::{self, MatchSystem, Sheet, Slot};
use crate::sheets::{Applied, Op};
use crate::tabt::{Creds, TeamMatch};
use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use serde_json::json;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Wait this long after a change: a captain often corrects a score right after typing it.
const DELAY: Duration = Duration::from_secs(3);
/// Wait this long before the read-back: give VTTL a moment to store the upload.
const CHECK_DELAY: Duration = Duration::from_secs(4);
/// The retry queue looks at the recent matches this often.
const SWEEP_EVERY: Duration = Duration::from_secs(60);
/// Longest wait between two retries of the same match.
const MAX_BACKOFF: i64 = 15 * 60;
/// The client id of the changes the uploader writes (they do not schedule a new upload).
pub const CLIENT: &str = "server:upload";

/// What started an upload (shown in the VTTL log).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A change of the match.
    Auto,
    /// The upload button.
    Manual,
    /// The retry queue.
    Retry,
}

impl Source {
    fn as_str(self) -> &'static str {
        match self {
            Source::Auto => "auto",
            Source::Manual => "manual",
            Source::Retry => "retry",
        }
    }
}

#[derive(Default)]
pub struct Uploads {
    generation: Mutex<HashMap<(String, String), u64>>,
    busy: Mutex<HashSet<(String, String)>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub ok: bool,
    pub dry_run: bool,
    pub partial: bool,
    pub errors: Vec<String>,
    pub data: String,
    pub account: String,
    /// The read-back: Some(true) = VTTL holds what we sent, Some(false) = VTTL differs,
    /// None = not checked (dry run, refused upload, or the read-back failed).
    pub confirmed: Option<bool>,
    /// Canonical keys where VTTL differs from what we sent (e.g. "g.3", "home.p.0").
    pub diff: Vec<String>,
    /// Why the read-back failed.
    pub check_error: Option<String>,
}

/// One row of the VTTL log of a match.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    /// Milliseconds since 1970.
    pub at: i64,
    pub account: String,
    pub source: String,
    pub partial: bool,
    pub dry_run: bool,
    pub ok: bool,
    pub confirmed: Option<bool>,
    pub errors: Vec<String>,
    pub diff: Vec<String>,
    pub check_error: Option<String>,
    pub data: String,
}

pub const MIGRATION: &str = "
    ALTER TABLE upload ADD COLUMN source TEXT NOT NULL DEFAULT 'auto';
    ALTER TABLE upload ADD COLUMN confirmed INTEGER;
    ALTER TABLE upload ADD COLUMN diff TEXT NOT NULL DEFAULT '[]';
    ALTER TABLE upload ADD COLUMN check_error TEXT;";

/// A good upload: VTTL holds it (or a dry run, which cannot be checked).
const GOOD: &str = "ok = 1 AND (dry_run = 1 OR confirmed = 1)";

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64
}

// ---------------------------------------------------------------- storage

impl Db {
    pub fn sheet_updated_by(&self, club: &str, id: &str) -> anyhow::Result<Option<String>> {
        Ok(self.conn().query_row("SELECT updated_by FROM sheet WHERE club = ?1 AND id = ?2", params![club, id], |r| r.get(0)).optional()?)
    }

    /// The upload file of the last good upload of a match: confirmed by VTTL, or a dry run.
    pub fn last_upload(&self, club: &str, id: &str) -> anyhow::Result<Option<String>> {
        Ok(self
            .conn()
            .query_row(
                &format!("SELECT data FROM upload WHERE club = ?1 AND id = ?2 AND {GOOD} ORDER BY at DESC, rowid DESC LIMIT 1"),
                params![club, id],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Attempts since the last good upload, and the time (s) of the newest attempt.
    pub fn failed_uploads(&self, club: &str, id: &str) -> anyhow::Result<(u32, i64)> {
        let c = self.conn();
        let mut st = c.prepare(&format!("SELECT at, {GOOD} FROM upload WHERE club = ?1 AND id = ?2 ORDER BY at DESC, rowid DESC LIMIT 20"))?;
        let rows: Vec<(i64, bool)> = st.query_map(params![club, id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
        let n = rows.iter().take_while(|(_, good)| !good).count() as u32;
        Ok((n, rows.first().map(|r| r.0).unwrap_or(0)))
    }

    pub fn log_upload(&self, club: &str, id: &str, source: Source, o: &Outcome) -> anyhow::Result<()> {
        self.conn().execute(
            "INSERT INTO upload(club, id, at, account, partial, dry_run, ok, data, errors, source, confirmed, diff, check_error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                club,
                id,
                crate::db::now(),
                o.account,
                o.partial,
                o.dry_run,
                o.ok,
                o.data,
                serde_json::to_string(&o.errors)?,
                source.as_str(),
                o.confirmed,
                serde_json::to_string(&o.diff)?,
                o.check_error
            ],
        )?;
        Ok(())
    }

    /// The VTTL log of a match, newest first.
    pub fn upload_log(&self, club: &str, id: &str) -> anyhow::Result<Vec<LogEntry>> {
        let c = self.conn();
        let mut st = c.prepare(
            "SELECT at, account, source, partial, dry_run, ok, confirmed, errors, diff, check_error, data
             FROM upload WHERE club = ?1 AND id = ?2 ORDER BY at DESC, rowid DESC LIMIT 100",
        )?;
        let list = |t: String| serde_json::from_str::<Vec<String>>(&t).unwrap_or_default();
        let rows = st.query_map(params![club, id], |r| {
            Ok(LogEntry {
                at: r.get::<_, i64>(0)? * 1000,
                account: r.get(1)?,
                source: r.get(2)?,
                partial: r.get(3)?,
                dry_run: r.get(4)?,
                ok: r.get(5)?,
                confirmed: r.get(6)?,
                errors: list(r.get(7)?),
                diff: list(r.get(8)?),
                check_error: r.get(9)?,
                data: r.get(10)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// (club, id) of the stored matches of yesterday, today and tomorrow (UTC, so a wide window).
    pub fn recent_sheets(&self) -> anyhow::Result<Vec<(String, String)>> {
        let c = self.conn();
        let mut st = c.prepare("SELECT club, id FROM sheet WHERE day BETWEEN date('now', '-1 day') AND date('now', '+1 day')")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Accounts of the club with a session (so the server has their password): the ones with a
    /// successful real upload first, then the most recently used.
    pub fn upload_accounts(&self, club: &str) -> anyhow::Result<Vec<String>> {
        let c = self.conn();
        let mut st = c.prepare(
            "SELECT a.account FROM account a JOIN session s ON s.account = a.account
             WHERE a.club = ?1 GROUP BY a.account ORDER BY a.can_upload DESC, MAX(s.last_seen) DESC LIMIT 5",
        )?;
        let rows = st.query_map(params![club], |r| r.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The (encrypted) password of the newest session of an account.
    pub fn account_secret(&self, account: &str) -> anyhow::Result<Option<String>> {
        Ok(self
            .conn()
            .query_row("SELECT secret FROM session WHERE account = ?1 ORDER BY last_seen DESC LIMIT 1", params![account], |r| r.get(0))
            .optional()?)
    }
}

// ---------------------------------------------------------------- what to send

/// The upload file to send now, or None. `force` = a manual upload (also when nothing changed).
pub fn plan(s: &Sheet, last_sent: Option<&str>, force: bool) -> Option<(String, bool)> {
    let sys = rules::system(s.match_system_id)?;
    if !s.is_official || !sys.competitive || s.vttl_locked {
        return None;
    }
    let score = rules::compute_score(s, sys);
    let full = score.complete();
    if !full && s.disable_partial_upload && !force {
        return None;
    }
    // Nothing decided yet: a partial upload would only send "?".
    if !full && score.home_wins + score.away_wins == 0 && !force {
        return None;
    }
    let data = rules::export_csv(s, sys, !full);
    if !force && last_sent == Some(data.as_str()) {
        return None;
    }
    Some((data, !full))
}

// ---------------------------------------------------------------- read-back

/// TabT DoubleTeams.Team code -> sheet option (web/src/lib/sheet/vttl.ts `DOUBLE_CODES`).
fn double_code(code: i64) -> Option<&'static str> {
    match code {
        1 => Some("1/2"),
        2 => Some("1/3"),
        3 => Some("2/3"),
        _ => None,
    }
}

/// The canonical map from VTTL data (web/src/lib/sheet/vttl.ts `fromVttl`). Keys without
/// data on VTTL are left out.
pub fn from_vttl(m: &TeamMatch, sys: &MatchSystem) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if m.is_home_forfeited {
        out.insert("home.ff".into(), "1".into());
    }
    if m.is_away_forfeited {
        out.insert("away.ff".into(), "1".into());
    }
    let Some(d) = &m.details else { return out };
    for (name, players, doubles, cap) in
        [("home", &d.home_players, &d.home_doubles, d.home_captain), ("away", &d.away_players, &d.away_doubles, d.away_captain)]
    {
        for slot in &sys.slots {
            match slot {
                Slot::Single { player, .. } => {
                    if let Some(p) = players.iter().find(|x| x.position == *player as i64 + 1) {
                        out.insert(format!("{name}.p.{player}"), p.unique_index.to_string());
                    }
                }
                Slot::Double { options, double } => {
                    let code = doubles.get(*double).map(|x| x.team).unwrap_or(0);
                    if let Some(c) = double_code(code).filter(|c| options.contains(c)) {
                        out.insert(format!("{name}.d.{double}"), c.to_string());
                    }
                }
                Slot::Sub { .. } => {}
            }
        }
        if cap != 0 {
            out.insert(format!("{name}.cap"), cap.to_string());
        }
    }
    if d.referee != 0 {
        out.insert("ref".into(), d.referee.to_string());
    }
    for r in &d.results {
        let gi = r.position - 1;
        if gi < 0 || gi as usize >= sys.games.len() {
            continue;
        }
        let gi = gi as usize;
        let n = if r.is_home_forfeited && r.is_away_forfeited {
            "-".to_string()
        } else if r.is_home_forfeited {
            "8".to_string()
        } else if r.is_away_forfeited {
            "9".to_string()
        } else {
            r.scores.split(',').filter_map(|x| x.split('|').nth(1)).filter(|x| !x.is_empty()).collect::<Vec<_>>().join("/")
        };
        if !n.is_empty() {
            out.insert(format!("g.{gi}"), n);
        }
        // The Super substitute: the 4th player ("V") in a game where a substitution is allowed.
        let g = &sys.games[gi];
        for slot in &sys.slots {
            let Slot::Sub { sub, transferable } = slot else { continue };
            if !g.allow_sub {
                continue;
            }
            let Some(Slot::Single { player, .. }) = sys.slots.get(*transferable) else { continue };
            let v_pos = *player as i64 + 1;
            if r.home_indexes.first() == Some(&v_pos) {
                out.insert(format!("home.sub.{sub}"), g.pos.to_string());
            }
            if r.away_indexes.first() == Some(&v_pos) {
                out.insert(format!("away.sub.{sub}"), g.pos.to_string());
            }
        }
    }
    out
}

/// Keys where VTTL does not hold what the upload sent. A partial upload has no captains and
/// no referee, so those are not compared.
pub fn compare(s: &Sheet, sys: &MatchSystem, partial: bool, m: &TeamMatch) -> Vec<String> {
    let vttl = from_vttl(m, sys);
    rules::canonical(s, sys)
        .into_iter()
        .filter(|(k, _)| !(partial && matches!(k.as_str(), "home.cap" | "away.cap" | "ref")))
        .filter(|(k, v)| vttl.get(k).map(String::as_str).unwrap_or("") != v)
        .map(|(k, _)| k)
        .collect()
}

/// The ops that record an upload in the match (web/src/lib/sheet/types.ts `UploadInfo`).
/// `vttl` = the match as read back from VTTL: `vttlSeen` becomes exactly what VTTL holds.
pub fn record_ops(s: &Sheet, o: &Outcome, vttl: Option<&TeamMatch>) -> Vec<Op> {
    let mut ops = vec![Op {
        p: vec![json!("upload")],
        v: json!({
            "at": now_ms(), "by": o.account, "ok": o.ok, "partial": o.partial, "dryRun": o.dry_run, "errors": o.errors,
            "confirmed": o.confirmed, "diff": o.diff, "checkError": o.check_error,
        }),
    }];
    let Some(sys) = rules::system(s.match_system_id) else { return ops };
    if let Some(m) = vttl {
        let held = from_vttl(m, sys);
        let mut seen: BTreeMap<String, String> = rules::canonical(s, sys).into_keys().map(|k| (k, String::new())).collect();
        seen.extend(held);
        ops.push(Op { p: vec![json!("vttlSeen")], v: json!(seen) });
        if (m.is_validated || m.is_locked) && !s.vttl_locked {
            ops.push(Op { p: vec![json!("vttlLocked")], v: json!(true) });
        }
    }
    if o.ok && !o.dry_run && !o.partial {
        ops.push(Op { p: vec![json!("disablePartialUpload")], v: json!(true) });
    }
    ops
}

// ---------------------------------------------------------------- running

/// Schedules an upload of a match after a stored change.
pub fn schedule(app: &Arc<App>, club: &str, id: &str) {
    let key = (club.to_string(), id.to_string());
    let this = {
        let mut g = app.uploads.generation.lock().unwrap();
        let e = g.entry(key.clone()).or_insert(0);
        *e += 1;
        *e
    };
    let app = app.clone();
    tokio::spawn(async move {
        tokio::time::sleep(DELAY).await;
        if app.uploads.generation.lock().unwrap().get(&key) != Some(&this) {
            return; // a newer change will upload
        }
        if let Err(e) = upload(&app, &key.0, &key.1, Source::Auto, None).await {
            tracing::warn!(club = %key.0, id = %key.1, "upload: {e:#}");
        }
    });
}

/// Wait before retry number `n` (n = failed attempts since the last good upload).
fn backoff(n: u32) -> i64 {
    if n == 0 { 0 } else { (60i64 << (n - 1).min(10)).min(MAX_BACKOFF) }
}

/// The retry queue: runs for as long as the server runs.
pub async fn run(app: Arc<App>) {
    // A short wait after a start, then look at once: uploads planned before a restart are lost.
    tokio::time::sleep(Duration::from_secs(10)).await;
    loop {
        if let Err(e) = sweep(&app) {
            tracing::warn!("upload sweep: {e:#}");
        }
        tokio::time::sleep(SWEEP_EVERY).await;
    }
}

fn sweep(app: &Arc<App>) -> anyhow::Result<()> {
    for (club, id) in app.db.recent_sheets()? {
        let Some(stored) = app.db.sheet(&club, &id)? else { continue };
        let Ok(s) = serde_json::from_value::<Sheet>(stored.doc) else { continue };
        let last = app.db.last_upload(&club, &id)?;
        if plan(&s, last.as_deref(), false).is_none() {
            continue; // VTTL has it, or there is nothing to send
        }
        let (failed, last_at) = app.db.failed_uploads(&club, &id)?;
        if crate::db::now() - last_at < backoff(failed) {
            continue;
        }
        let app = app.clone();
        tokio::spawn(async move {
            if let Err(e) = upload(&app, &club, &id, Source::Retry, None).await {
                tracing::warn!(club, id, "retry upload: {e:#}");
            }
        });
    }
    Ok(())
}

/// Uploads one match now. A manual upload (`Source::Manual`) sends also when nothing changed,
/// and tries `first` (the login of the user) first. Returns None when there is nothing to send.
pub async fn upload(app: &Arc<App>, club: &str, id: &str, source: Source, first: Option<Creds>) -> anyhow::Result<Option<Outcome>> {
    let key = (club.to_string(), id.to_string());
    if !app.uploads.busy.lock().unwrap().insert(key.clone()) {
        // Another upload of this match is running: try again after it.
        if source == Source::Auto {
            schedule(app, club, id);
        }
        return Ok(None);
    }
    let r = upload_locked(app, club, id, source, first).await;
    app.uploads.busy.lock().unwrap().remove(&key);
    r
}

async fn upload_locked(app: &Arc<App>, club: &str, id: &str, source: Source, first: Option<Creds>) -> anyhow::Result<Option<Outcome>> {
    let Some(stored) = app.db.sheet(club, id)? else { return Ok(None) };
    let s: Sheet = serde_json::from_value(stored.doc)?;
    let Some(sys) = rules::system(s.match_system_id) else { return Ok(None) };
    let last = app.db.last_upload(club, id)?;
    let Some((data, partial)) = plan(&s, last.as_deref(), source == Source::Manual) else { return Ok(None) };

    // Logins to try, in order.
    let mut logins: Vec<Creds> = first.into_iter().collect();
    let mut names: Vec<String> = app.db.sheet_updated_by(club, id)?.into_iter().collect();
    names.extend(app.db.upload_accounts(club)?);
    for n in names {
        if logins.iter().any(|c| c.account == n) {
            continue;
        }
        if let Some(sealed) = app.db.account_secret(&n)?
            && let Some(password) = app.crypto.decrypt(&sealed)
        {
            logins.push(Creds { account: n, password });
        }
    }

    let dry_run = !app.upload_enabled;
    let mut outcome = Outcome {
        ok: false,
        dry_run,
        partial,
        errors: vec!["Geen VTTL-login van deze club beschikbaar".into()],
        data: data.clone(),
        account: String::new(),
        confirmed: None,
        diff: vec![],
        check_error: None,
    };
    let mut held: Option<TeamMatch> = None;
    if dry_run {
        let account = logins.first().map(|c| c.account.clone()).unwrap_or_else(|| "server".into());
        outcome = Outcome { ok: true, errors: vec![], account, ..outcome };
    } else {
        let mut used: Option<&Creds> = None;
        let mut unreachable = false;
        for c in &logins {
            used = Some(c);
            outcome.account = c.account.clone();
            match send(app, c, &data).await {
                Ok((ok, errors, no_rights)) => {
                    outcome.ok = ok;
                    outcome.errors = errors;
                    if !no_rights {
                        break;
                    }
                }
                Err(e) => {
                    // No answer: VTTL may still have stored it. The read-back tells.
                    outcome.ok = false;
                    outcome.errors = vec![format!("Geen antwoord van VTTL: {e:#}")];
                    unreachable = true;
                    break;
                }
            }
        }
        if let Some(c) = used
            && (outcome.ok || unreachable)
        {
            tokio::time::sleep(CHECK_DELAY).await;
            match app.tabt.team_match(c, stored.season, club, &s.match_id, true).await {
                Ok(Some(m)) => {
                    outcome.diff = compare(&s, sys, partial, &m);
                    outcome.confirmed = Some(outcome.diff.is_empty());
                    if unreachable && outcome.diff.is_empty() {
                        // The answer got lost, but VTTL stored the upload.
                        outcome.ok = true;
                        outcome.errors.clear();
                    }
                    held = Some(m);
                }
                Ok(None) => outcome.check_error = Some(format!("Wedstrijd {} niet gevonden bij VTTL", s.match_id)),
                Err(e) => outcome.check_error = Some(format!("Controle bij VTTL mislukt: {e:#}")),
            }
        }
    }

    app.db.log_upload(club, id, source, &outcome)?;
    tracing::info!(club, id, partial, dry_run, ok = outcome.ok, confirmed = ?outcome.confirmed, account = %outcome.account, source = source.as_str(), "upload");
    if outcome.ok && !dry_run {
        app.db.mark_can_upload(&outcome.account)?;
        app.tabt.invalidate(club);
        app.tabt.invalidate(&club.replace('-', ""));
    }
    // Record it in the match, so every device shows the VTTL status.
    let ops = record_ops(&s, &outcome, held.as_ref());
    if let Some(Applied::Done(rev)) = app.db.apply_ops(club, id, CLIENT, now_ms(), &ops, &outcome.account)? {
        app.push(club, json!({ "type": "ops", "id": id, "rev": rev, "clientId": CLIENT, "ops": ops }));
    }
    app.push(
        club,
        json!({ "type": "upload", "id": id, "ok": outcome.ok, "dryRun": dry_run, "partial": partial, "errors": outcome.errors,
            "confirmed": outcome.confirmed, "diff": outcome.diff, "checkError": outcome.check_error }),
    );
    Ok(Some(outcome))
}

/// (ok, error lines, "no rights for this club")
async fn send(app: &App, c: &Creds, data: &str) -> anyhow::Result<(bool, Vec<String>, bool)> {
    match app.tabt.upload(c, data).await {
        Ok((ok, lines)) => Ok((ok, lines, false)),
        Err(e) => match e.downcast_ref::<crate::xml::Fault>() {
            Some(f) if f.code == "22" => Ok((false, vec![format!("Geen uploadrechten voor deze club (VTTL-fout 22: {})", f.message)], true)),
            Some(f) => Ok((false, vec![format!("VTTL-fout {}: {}", f.code, f.message)], false)),
            None => Err(e),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn sheet(results: Value) -> Sheet {
        serde_json::from_value(json!({
            "season": 27, "matchId": "T/1", "isOfficial": true, "matchSystemId": 6,
            "home": { "clubId": "W1", "players": [{ "computerNumber": "1" }, { "computerNumber": "2" }], "doubles": ["1/2"] },
            "away": { "clubId": "W2", "players": [{ "computerNumber": "3" }, { "computerNumber": "4" }], "doubles": ["1/2"] },
            "results": results,
            "vttlSeen": { "home.cap": "9" },
        }))
        .unwrap()
    }
    fn won() -> Value {
        json!([{ "l": "11", "r": "5" }, { "l": "11", "r": "5" }, { "l": "11", "r": "5" }, { "l": "", "r": "" }, { "l": "", "r": "" }])
    }
    fn empty() -> Value {
        json!([])
    }

    #[test]
    fn when_to_upload() {
        // Nothing decided: nothing to send.
        assert!(plan(&sheet(json!([empty(), empty(), empty(), empty(), empty()])), None, false).is_none());
        // One game decided: partial.
        let s = sheet(json!([won(), empty(), empty(), empty(), empty()]));
        let (data, partial) = plan(&s, None, false).unwrap();
        assert!(partial && data.contains(";P5/5/5,?,?,?,?"));
        // Same data again: no upload, unless forced.
        assert!(plan(&s, Some(&data), false).is_none());
        assert!(plan(&s, Some(&data), true).is_some());
        // Complete: full.
        let s = sheet(json!([won(), won(), won(), won(), won()]));
        let (_, partial) = plan(&s, None, false).unwrap();
        assert!(!partial);
    }

    #[test]
    fn not_official_or_locked() {
        let mut s = sheet(json!([won()]));
        s.is_official = false;
        assert!(plan(&s, None, true).is_none());
        let mut s = sheet(json!([won()]));
        s.vttl_locked = true;
        assert!(plan(&s, None, true).is_none());
    }

    fn outcome(partial: bool) -> Outcome {
        Outcome {
            ok: true,
            dry_run: false,
            partial,
            errors: vec![],
            data: String::new(),
            account: "jan".into(),
            confirmed: Some(true),
            diff: vec![],
            check_error: None,
        }
    }

    /// The match as VTTL returns it after the upload of `sheet(json!([won()]))`.
    fn vttl_match(scores: &str) -> TeamMatch {
        use crate::tabt::{DoubleTeam, IndividualResult, MatchDetails, Player};
        let player = |position, unique_index| Player {
            position,
            unique_index,
            first_name: String::new(),
            last_name: String::new(),
            ranking: String::new(),
            victory_count: 0,
            is_forfeited: false,
        };
        let double = vec![DoubleTeam { position: 1, team: 1, is_forfeited: false }];
        TeamMatch {
            match_id: "T/1".into(),
            unique_id: 1,
            week: String::new(),
            date: String::new(),
            time: String::new(),
            venue: None,
            venue_club: String::new(),
            home_club: "W1".into(),
            home_team: String::new(),
            away_club: "W2".into(),
            away_team: String::new(),
            score: String::new(),
            division_id: 0,
            division_category: 0,
            is_home_forfeited: false,
            is_away_forfeited: false,
            is_home_withdrawn: false,
            is_away_withdrawn: false,
            is_validated: false,
            is_locked: false,
            is_bye: false,
            details: Some(MatchDetails {
                details_created: true,
                match_system: 6,
                home_captain: 0,
                away_captain: 0,
                referee: 0,
                home_players: vec![player(1, 1), player(2, 2)],
                away_players: vec![player(1, 3), player(2, 4)],
                home_doubles: double.clone(),
                away_doubles: double,
                results: vec![IndividualResult {
                    position: 1,
                    home_indexes: vec![2],
                    away_indexes: vec![1],
                    home_sets: 3,
                    away_sets: 0,
                    is_home_forfeited: false,
                    is_away_forfeited: false,
                    scores: scores.into(),
                }],
                home_score: 1,
                away_score: 0,
            }),
        }
    }

    #[test]
    fn read_back() {
        let s = sheet(json!([won()]));
        let sys = rules::system(6).unwrap();
        let m = vttl_match("1|5,2|5,3|5");
        let held = from_vttl(&m, sys);
        assert_eq!(held["g.0"], "5/5/5");
        assert_eq!(held["home.p.1"], "2");
        assert_eq!(held["away.d.0"], "1/2");
        // VTTL holds what we sent. The captain "9" is not compared after a partial upload.
        assert!(compare(&s, sys, true, &m).is_empty());
        // VTTL has another score for game 1.
        assert_eq!(compare(&s, sys, true, &vttl_match("1|5,2|5,3|7")), vec!["g.0"]);
        // VTTL has nothing: every value we sent differs.
        let mut empty = m.clone();
        empty.details = None;
        assert!(compare(&s, sys, true, &empty).contains(&"home.p.0".to_string()));
    }

    #[test]
    fn recording() {
        let s = sheet(json!([won()]));
        let m = vttl_match("1|5,2|5,3|5");
        // Confirmed partial upload: vttlSeen is what VTTL holds (no captain there: "").
        let ops = record_ops(&s, &outcome(true), Some(&m));
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].v["confirmed"], true);
        assert_eq!(ops[1].v["g.0"], "5/5/5");
        assert_eq!(ops[1].v["home.cap"], "");
        // No read-back (dry run, or VTTL could not be read): vttlSeen does not change.
        let dry = Outcome { dry_run: true, confirmed: None, ..outcome(true) };
        assert_eq!(record_ops(&s, &dry, None).len(), 1);
        // A full upload stops the partial uploads.
        assert_eq!(record_ops(&s, &outcome(false), Some(&m)).len(), 3);
        // A validated match becomes read-only.
        let locked = TeamMatch { is_validated: true, ..m };
        assert!(record_ops(&s, &outcome(true), Some(&locked)).iter().any(|o| o.p[0] == "vttlLocked"));
    }

    #[test]
    fn retry_waits() {
        assert_eq!([0, 1, 2, 3, 4, 5, 9, 40].map(backoff), [0, 60, 120, 240, 480, 900, 900, 900]);
    }

    #[test]
    fn log_and_failures() {
        let db = crate::db::memory();
        let bad = Outcome { confirmed: Some(false), diff: vec!["g.0".into()], ..outcome(true) };
        db.log_upload("C", "S/1", Source::Auto, &outcome(true)).unwrap();
        db.log_upload("C", "S/1", Source::Retry, &bad).unwrap();
        db.log_upload("C", "S/1", Source::Retry, &Outcome { ok: false, confirmed: None, ..outcome(true) }).unwrap();
        assert_eq!(db.failed_uploads("C", "S/1").unwrap().0, 2);
        let log = db.upload_log("C", "S/1").unwrap();
        assert_eq!(log.len(), 3);
        assert_eq!(log[1].diff, vec!["g.0"]);
        assert_eq!(log[1].source, "retry");
        assert_eq!(log[2].confirmed, Some(true));
        // An upload that VTTL did not confirm is not "last sent": the retry sends it again.
        let unchecked = Outcome { data: "x".into(), confirmed: None, ..outcome(true) };
        db.log_upload("C", "S/2", Source::Auto, &unchecked).unwrap();
        assert_eq!(db.last_upload("C", "S/2").unwrap(), None);
    }
}
