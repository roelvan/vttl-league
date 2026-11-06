//! Match sheets: one JSON document per (club, sheet id), changed with small "set" operations.
//!
//! The client owns the shape of the document (web/src/lib/sheet). The server only stores it,
//! applies operations in arrival order (last write wins per field) and pushes every change to
//! the other browsers of the club.
//!
//! Each browser tab has a client id and numbers its batches. A batch with a sequence number
//! that the server has already seen is a retry (the answer got lost, for example behind the
//! captive portal): it is acknowledged again but not applied twice.

use crate::api::ApiError;
use crate::app::App;
use crate::auth::User;
use crate::db::{self, Db};
use axum::Json;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::Response;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;

pub const MIGRATION: &str = "
    CREATE TABLE sheet(
        club        TEXT NOT NULL,
        id          TEXT NOT NULL,
        season      INTEGER NOT NULL,
        day         TEXT NOT NULL,
        doc         TEXT NOT NULL,
        rev         INTEGER NOT NULL,
        updated_at  INTEGER NOT NULL,
        updated_by  TEXT NOT NULL,
        PRIMARY KEY (club, id)
    );
    CREATE INDEX sheet_day ON sheet(club, day);
    CREATE TABLE sheet_client(
        club        TEXT NOT NULL,
        id          TEXT NOT NULL,
        client_id   TEXT NOT NULL,
        last_seq    INTEGER NOT NULL,
        PRIMARY KEY (club, id, client_id)
    );";

const MAX_DOC_BYTES: usize = 512 * 1024;
const MAX_OPS: usize = 2000;
const MAX_PATH: usize = 8;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sheet {
    pub id: String,
    pub rev: i64,
    pub day: String,
    pub season: i64,
    pub doc: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Op {
    /// Path into the document: object keys and array indexes.
    pub p: Vec<Value>,
    pub v: Value,
}

// ---------------------------------------------------------------- operations

/// Sets `v` at path `p`. Missing objects are created; arrays grow with nulls.
pub fn apply(doc: &mut Value, op: &Op) -> Result<(), String> {
    if op.p.is_empty() || op.p.len() > MAX_PATH {
        return Err("bad path length".into());
    }
    let mut cur = doc;
    for (i, key) in op.p.iter().enumerate() {
        let last = i == op.p.len() - 1;
        match key {
            Value::String(k) => {
                if !cur.is_object() {
                    *cur = json!({});
                }
                let obj = cur.as_object_mut().unwrap();
                if last {
                    obj.insert(k.clone(), op.v.clone());
                    return Ok(());
                }
                cur = obj.entry(k.clone()).or_insert(Value::Null);
            }
            Value::Number(n) => {
                let idx = n.as_u64().filter(|i| *i < 1000).ok_or("bad index")? as usize;
                if !cur.is_array() {
                    *cur = json!([]);
                }
                let arr = cur.as_array_mut().unwrap();
                if arr.len() <= idx {
                    arr.resize(idx + 1, Value::Null);
                }
                if last {
                    arr[idx] = op.v.clone();
                    return Ok(());
                }
                cur = &mut arr[idx];
            }
            _ => return Err("bad path element".into()),
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- storage

pub enum Applied {
    /// New revision after applying.
    Done(i64),
    /// The batch was seen before; current revision.
    Duplicate(i64),
}

impl Db {
    pub fn sheets_on(&self, club: &str, day: &str) -> anyhow::Result<Vec<Sheet>> {
        let c = self.conn();
        let mut st = c.prepare("SELECT id, rev, day, season, doc FROM sheet WHERE club = ?1 AND day = ?2")?;
        let rows = st.query_map(params![club, day], row_sheet)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn sheet(&self, club: &str, id: &str) -> anyhow::Result<Option<Sheet>> {
        let c = self.conn();
        Ok(c.query_row("SELECT id, rev, day, season, doc FROM sheet WHERE club = ?1 AND id = ?2", params![club, id], row_sheet)
            .optional()?)
    }

    /// Creates the sheet unless it exists. Returns the stored sheet and whether it is new.
    pub fn create_sheet(&self, club: &str, s: &Sheet, by: &str) -> anyhow::Result<(Sheet, bool)> {
        let c = self.conn();
        let n = c.execute(
            "INSERT INTO sheet(club, id, season, day, doc, rev, updated_at, updated_by) VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7)
             ON CONFLICT(club, id) DO NOTHING",
            params![club, s.id, s.season, s.day, s.doc.to_string(), db::now(), by],
        )?;
        let stored = c.query_row("SELECT id, rev, day, season, doc FROM sheet WHERE club = ?1 AND id = ?2", params![club, s.id], row_sheet)?;
        Ok((stored, n == 1))
    }

    pub fn apply_ops(&self, club: &str, id: &str, client: &str, seq: i64, ops: &[Op], by: &str) -> anyhow::Result<Option<Applied>> {
        let mut c = self.conn();
        let tx = c.transaction()?;
        let Some((doc, rev)): Option<(String, i64)> = tx
            .query_row("SELECT doc, rev FROM sheet WHERE club = ?1 AND id = ?2", params![club, id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?
        else {
            return Ok(None);
        };
        let last: Option<i64> = tx
            .query_row(
                "SELECT last_seq FROM sheet_client WHERE club = ?1 AND id = ?2 AND client_id = ?3",
                params![club, id, client],
                |r| r.get(0),
            )
            .optional()?;
        if last.is_some_and(|l| seq <= l) {
            return Ok(Some(Applied::Duplicate(rev)));
        }
        let mut doc: Value = serde_json::from_str(&doc)?;
        for op in ops {
            apply(&mut doc, op).map_err(|e| anyhow::anyhow!("{e}"))?;
        }
        let text = doc.to_string();
        if text.len() > MAX_DOC_BYTES {
            anyhow::bail!("sheet too large");
        }
        tx.execute(
            "UPDATE sheet SET doc = ?3, rev = rev + 1, updated_at = ?4, updated_by = ?5 WHERE club = ?1 AND id = ?2",
            params![club, id, text, db::now(), by],
        )?;
        tx.execute(
            "INSERT INTO sheet_client(club, id, client_id, last_seq) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(club, id, client_id) DO UPDATE SET last_seq = excluded.last_seq",
            params![club, id, client, seq],
        )?;
        tx.commit()?;
        Ok(Some(Applied::Done(rev + 1)))
    }
}

fn row_sheet(r: &rusqlite::Row) -> rusqlite::Result<Sheet> {
    let doc: String = r.get(4)?;
    Ok(Sheet {
        id: r.get(0)?,
        rev: r.get(1)?,
        day: r.get(2)?,
        season: r.get(3)?,
        doc: serde_json::from_str(&doc).unwrap_or(Value::Null),
    })
}

// ---------------------------------------------------------------- HTTP

#[derive(Deserialize)]
pub struct DayQuery {
    day: String,
}

pub async fn list(State(app): State<Arc<App>>, u: User, Query(q): Query<DayQuery>) -> Result<Json<Vec<Sheet>>, ApiError> {
    Ok(Json(app.db.sheets_on(u.club()?, &q.day)?))
}

#[derive(Deserialize)]
pub struct IdQuery {
    id: String,
}

pub async fn get_one(State(app): State<Arc<App>>, u: User, Query(q): Query<IdQuery>) -> Result<Json<Sheet>, ApiError> {
    app.db
        .sheet(u.club()?, &q.id)?
        .map(Json)
        .ok_or_else(|| ApiError::NotFound(format!("Wedstrijdblad {} bestaat niet", q.id)))
}

#[derive(Deserialize)]
pub struct CreateBody {
    id: String,
    day: String,
    season: i64,
    doc: Value,
}

pub async fn create(State(app): State<Arc<App>>, u: User, Json(b): Json<CreateBody>) -> Result<Json<Value>, ApiError> {
    let club = u.club()?;
    if b.id.is_empty() || b.id.len() > 64 || b.doc.to_string().len() > MAX_DOC_BYTES {
        return Err(ApiError::BadRequest("Ongeldig wedstrijdblad".into()));
    }
    let s = Sheet { id: b.id, rev: 1, day: b.day, season: b.season, doc: b.doc };
    let (stored, created) = app.db.create_sheet(club, &s, &u.account.account)?;
    if created {
        app.push(club, json!({ "type": "sheet", "id": stored.id, "day": stored.day, "rev": stored.rev }));
    }
    Ok(Json(json!({ "sheet": stored, "created": created })))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsBody {
    id: String,
    client_id: String,
    seq: i64,
    ops: Vec<Op>,
}

pub async fn ops(State(app): State<Arc<App>>, u: User, Json(b): Json<OpsBody>) -> Result<Json<Value>, ApiError> {
    let club = u.club()?;
    if b.ops.len() > MAX_OPS || b.client_id.is_empty() || b.client_id.len() > 64 {
        return Err(ApiError::BadRequest("Ongeldige wijziging".into()));
    }
    let r = app
        .db
        .apply_ops(club, &b.id, &b.client_id, b.seq, &b.ops, &u.account.account)
        .map_err(|e| ApiError::BadRequest(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound(format!("Wedstrijdblad {} bestaat niet", b.id)))?;
    let rev = match r {
        Applied::Duplicate(rev) => rev,
        Applied::Done(rev) => {
            app.push(club, json!({ "type": "ops", "id": b.id, "rev": rev, "clientId": b.client_id, "ops": b.ops }));
            rev
        }
    };
    Ok(Json(json!({ "rev": rev, "seq": b.seq })))
}

// ---------------------------------------------------------------- WebSocket

pub async fn ws(State(app): State<Arc<App>>, u: User, upgrade: WebSocketUpgrade) -> Result<Response, ApiError> {
    let club = u.club()?.to_string();
    Ok(upgrade.on_upgrade(move |socket| run_ws(app, club, socket)))
}

async fn run_ws(app: Arc<App>, club: String, mut socket: WebSocket) {
    let mut rx = app.events.subscribe();
    let mut ping = tokio::time::interval(std::time::Duration::from_secs(25));
    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                Ok((c, text)) if c == club => {
                    if socket.send(Message::Text(text.as_str().into())).await.is_err() { break; }
                }
                Ok(_) => {}
                // Too slow: tell the client to reload what it shows.
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    if socket.send(Message::Text(r#"{"type":"resync"}"#.into())).await.is_err() { break; }
                }
                Err(_) => break,
            },
            _ = ping.tick() => {
                if socket.send(Message::Ping(Vec::new().into())).await.is_err() { break; }
            }
            msg = socket.recv() => match msg {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                _ => {}
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(p: Value, v: Value) -> Op {
        Op { p: serde_json::from_value(p).unwrap(), v }
    }

    #[test]
    fn set_paths() {
        let mut d = json!({ "home": { "players": [] } });
        apply(&mut d, &op(json!(["home", "players", 2, "name"]), json!("JANSSENS Jan"))).unwrap();
        apply(&mut d, &op(json!(["results", 0, 1]), json!({ "l": "11", "r": "9" }))).unwrap();
        assert_eq!(d["home"]["players"][2]["name"], "JANSSENS Jan");
        assert_eq!(d["home"]["players"][0], Value::Null);
        assert_eq!(d["results"][0][1]["l"], "11");
        assert!(apply(&mut d, &op(json!([]), json!(1))).is_err());
        assert!(apply(&mut d, &op(json!(["a", 5000]), json!(1))).is_err());
    }

    #[test]
    fn ops_are_applied_once() {
        let db = db::memory();
        let s = Sheet { id: "PWVLH08/012".into(), rev: 1, day: "2026-11-14".into(), season: 27, doc: json!({ "comments": "" }) };
        let (_, created) = db.create_sheet("WVL154", &s, "jan").unwrap();
        assert!(created);
        let (_, created) = db.create_sheet("WVL154", &s, "jan").unwrap();
        assert!(!created);
        let ops = vec![op(json!(["comments"]), json!("ok"))];
        assert!(matches!(db.apply_ops("WVL154", "PWVLH08/012", "c1", 1, &ops, "jan").unwrap(), Some(Applied::Done(2))));
        // Retry of the same batch.
        assert!(matches!(db.apply_ops("WVL154", "PWVLH08/012", "c1", 1, &ops, "jan").unwrap(), Some(Applied::Duplicate(2))));
        assert!(matches!(db.apply_ops("WVL154", "PWVLH08/012", "c2", 1, &ops, "jan").unwrap(), Some(Applied::Done(3))));
        // Other clubs do not see it.
        assert!(db.sheet("WVL129", "PWVLH08/012").unwrap().is_none());
        assert_eq!(db.sheets_on("WVL154", "2026-11-14").unwrap().len(), 1);
    }
}
