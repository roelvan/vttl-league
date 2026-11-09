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

use crate::app::App;
use crate::db::Db;
use crate::rules::{self, Sheet};
use crate::sheets::{Applied, Op};
use crate::tabt::Creds;
use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Wait this long after a change: a captain often corrects a score right after typing it.
const DELAY: Duration = Duration::from_secs(3);
/// The client id of the changes the uploader writes (they do not schedule a new upload).
pub const CLIENT: &str = "server:upload";

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
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64
}

// ---------------------------------------------------------------- storage

impl Db {
    pub fn sheet_updated_by(&self, club: &str, id: &str) -> anyhow::Result<Option<String>> {
        Ok(self.conn().query_row("SELECT updated_by FROM sheet WHERE club = ?1 AND id = ?2", params![club, id], |r| r.get(0)).optional()?)
    }

    /// The upload file of the last successful upload of a match (dry runs included).
    pub fn last_upload(&self, club: &str, id: &str) -> anyhow::Result<Option<String>> {
        Ok(self
            .conn()
            .query_row("SELECT data FROM upload WHERE club = ?1 AND id = ?2 AND ok = 1 ORDER BY at DESC, rowid DESC LIMIT 1", params![club, id], |r| r.get(0))
            .optional()?)
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

/// The ops that record an upload in the match (web/src/lib/sheet/vttl.ts `uploadOps`).
pub fn record_ops(s: &Sheet, o: &Outcome) -> Vec<Op> {
    let mut ops = vec![Op {
        p: vec![json!("upload")],
        v: json!({ "at": now_ms(), "by": o.account, "ok": o.ok, "partial": o.partial, "dryRun": o.dry_run, "errors": o.errors }),
    }];
    if o.ok && !o.dry_run {
        let Some(sys) = rules::system(s.match_system_id) else { return ops };
        let mut sent = rules::canonical(s, sys);
        if o.partial {
            // A partial upload has no captains and no referee.
            for k in ["home.cap", "away.cap", "ref"] {
                sent.insert(k.into(), s.vttl_seen.get(k).cloned().unwrap_or_default());
            }
        }
        ops.push(Op { p: vec![json!("vttlSeen")], v: json!(sent) });
        if !o.partial {
            ops.push(Op { p: vec![json!("disablePartialUpload")], v: json!(true) });
        }
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
        if let Err(e) = upload(&app, &key.0, &key.1, false, None).await {
            tracing::warn!(club = %key.0, id = %key.1, "upload: {e:#}");
        }
    });
}

/// Uploads one match now. `first` = try this login first (a manual upload by a user).
/// Returns None when there is nothing to send.
pub async fn upload(app: &Arc<App>, club: &str, id: &str, force: bool, first: Option<Creds>) -> anyhow::Result<Option<Outcome>> {
    let key = (club.to_string(), id.to_string());
    if !app.uploads.busy.lock().unwrap().insert(key.clone()) {
        // Another upload of this match is running: try again after it.
        if !force {
            schedule(app, club, id);
        }
        return Ok(None);
    }
    let r = upload_locked(app, club, id, force, first).await;
    app.uploads.busy.lock().unwrap().remove(&key);
    r
}

async fn upload_locked(app: &Arc<App>, club: &str, id: &str, force: bool, first: Option<Creds>) -> anyhow::Result<Option<Outcome>> {
    let Some(stored) = app.db.sheet(club, id)? else { return Ok(None) };
    let s: Sheet = serde_json::from_value(stored.doc)?;
    let last = app.db.last_upload(club, id)?;
    let Some((data, partial)) = plan(&s, last.as_deref(), force) else { return Ok(None) };

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
    let mut outcome = Outcome { ok: false, dry_run, partial, errors: vec!["Geen VTTL-login van deze club beschikbaar".into()], data: data.clone(), account: String::new() };
    if dry_run {
        let account = logins.first().map(|c| c.account.clone()).unwrap_or_else(|| "server".into());
        outcome = Outcome { ok: true, errors: vec![], account, ..outcome };
    } else {
        for c in &logins {
            let (ok, errors, no_rights) = send(app, c, &data).await?;
            outcome = Outcome { ok, errors, account: c.account.clone(), ..outcome.clone() };
            if !no_rights {
                break;
            }
        }
    }

    app.db.log_upload(club, id, &outcome.account, partial, dry_run, outcome.ok, &data, &outcome.errors)?;
    tracing::info!(club, id, partial, dry_run, ok = outcome.ok, account = %outcome.account, "upload");
    if outcome.ok && !dry_run {
        app.db.mark_can_upload(&outcome.account)?;
        app.tabt.invalidate(club);
        app.tabt.invalidate(&club.replace('-', ""));
    }
    // Record it in the match, so every device shows the VTTL status.
    let ops = record_ops(&s, &outcome);
    if let Some(Applied::Done(rev)) = app.db.apply_ops(club, id, CLIENT, now_ms(), &ops, &outcome.account)? {
        app.push(club, json!({ "type": "ops", "id": id, "rev": rev, "clientId": CLIENT, "ops": ops }));
    }
    app.push(club, json!({ "type": "upload", "id": id, "ok": outcome.ok, "dryRun": dry_run, "partial": partial, "errors": outcome.errors }));
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

    #[test]
    fn recording() {
        let s = sheet(json!([won()]));
        let o = Outcome { ok: true, dry_run: false, partial: true, errors: vec![], data: String::new(), account: "jan".into() };
        let ops = record_ops(&s, &o);
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[1].v["home.cap"], "9"); // a partial upload keeps the captains VTTL had
        assert_eq!(ops[1].v["g.0"], "5/5/5");
        let dry = Outcome { dry_run: true, ..o.clone() };
        assert_eq!(record_ops(&s, &dry).len(), 1);
        let full = Outcome { partial: false, ..o };
        assert_eq!(record_ops(&s, &full).len(), 3);
    }
}
