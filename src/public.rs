//! Public, read-only pages: anyone with a link follows a club night or a match live, without
//! a login. Everything here is read-only and leaves out who did what (account names).
//!
//! VTTL data is read without credentials; the TabT cache is shared, so spectators mostly hit
//! the cache that the club's own users fill.

use crate::api::{ApiError, match_list, same_club};
use crate::app::App;
use crate::boards::{BoardState, Device};
use crate::sheets::Sheet;
use crate::tabt::TeamMatch;
use axum::Json;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::Response;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Deserialize)]
pub struct ClubQuery {
    club: String,
}

#[derive(Deserialize)]
pub struct DayQuery {
    club: String,
    day: String,
}

#[derive(Deserialize)]
pub struct IdQuery {
    club: String,
    id: String,
}

/// VTTL club ids: letters, digits and a dash ("WVL154", "Vl-B234").
fn club_id(c: &str) -> Result<&str, ApiError> {
    if c.is_empty() || c.len() > 16 || !c.chars().all(|x| x.is_ascii_alphanumeric() || x == '-') {
        return Err(ApiError::BadRequest("Ongeldige club".into()));
    }
    Ok(c)
}

/// The stored record without account names.
pub fn scrub(mut s: Sheet) -> Sheet {
    if let Some(u) = s.doc.get_mut("upload").and_then(Value::as_object_mut) {
        u.remove("by");
    }
    s
}

/// A push message as spectators get it: no account names, no client ids.
pub fn public_message(text: &str) -> Option<String> {
    let mut v: Value = serde_json::from_str(text).ok()?;
    let t = v.get("type")?.as_str()?;
    if !matches!(t, "ops" | "sheet" | "board" | "resync") {
        return None;
    }
    if let Some(o) = v.as_object_mut() {
        o.remove("by");
        if let Some(Value::String(c)) = o.get("clientId") {
            // Keep only the kind of writer ("board:"), so the client can tell a board result.
            let kind = if c.starts_with("board:") { "board" } else { "user" };
            o.insert("clientId".into(), Value::String(kind.into()));
        }
    }
    Some(v.to_string())
}

pub async fn club(State(app): State<Arc<App>>, Query(q): Query<ClubQuery>) -> Result<Json<Value>, ApiError> {
    let club = club_id(&q.club)?;
    let season = app.tabt.current_season(None).await.map_err(|e| ApiError::Vttl(e.to_string()))?;
    let (info, list) = tokio::join!(app.tabt.club(None, season, club), match_list(&app, None, season, club));
    let mut list = list?;
    list["club"] = json!(info.ok().flatten());
    Ok(Json(list))
}

pub async fn sheets(State(app): State<Arc<App>>, Query(q): Query<DayQuery>) -> Result<Json<Vec<Sheet>>, ApiError> {
    Ok(Json(app.db.sheets_on(club_id(&q.club)?, &q.day)?.into_iter().map(scrub).collect()))
}

pub async fn sheet(State(app): State<Arc<App>>, Query(q): Query<IdQuery>) -> Result<Json<Sheet>, ApiError> {
    app.db
        .sheet(club_id(&q.club)?, &q.id)?
        .map(|s| Json(scrub(s)))
        .ok_or_else(|| ApiError::NotFound(format!("Wedstrijd {} heeft nog geen live gegevens", q.id)))
}

pub async fn team_match(State(app): State<Arc<App>>, Query(q): Query<IdQuery>) -> Result<Json<TeamMatch>, ApiError> {
    let club = club_id(&q.club)?;
    let season = app.tabt.current_season(None).await.map_err(|e| ApiError::Vttl(e.to_string()))?;
    let matches = app.tabt.club_matches(None, season, club).await.map_err(|e| ApiError::Vttl(e.to_string()))?;
    matches
        .into_iter()
        .find(|m| m.match_id == q.id && (same_club(&m.home_club, club) || same_club(&m.away_club, club)))
        .map(Json)
        .ok_or_else(|| ApiError::NotFound(format!("Wedstrijd {} niet gevonden", q.id)))
}

pub async fn boards(State(app): State<Arc<App>>, Query(q): Query<ClubQuery>) -> Result<Json<Value>, ApiError> {
    let club = club_id(&q.club)?;
    let devices: Vec<Device> = app.db.devices(club)?;
    let states: Vec<BoardState> = app.db.links_of(club)?.iter().filter_map(|d| app.boards.state(d)).collect();
    Ok(Json(json!({ "devices": devices, "states": states })))
}

pub async fn ws(State(app): State<Arc<App>>, Query(q): Query<ClubQuery>, upgrade: WebSocketUpgrade) -> Result<Response, ApiError> {
    let club = club_id(&q.club)?.to_string();
    Ok(upgrade.on_upgrade(move |socket| run_ws(app, club, socket)))
}

async fn run_ws(app: Arc<App>, club: String, mut socket: WebSocket) {
    let mut rx = app.events.subscribe();
    let mut ping = tokio::time::interval(std::time::Duration::from_secs(25));
    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                Ok((c, text)) if c == club => {
                    if let Some(t) = public_message(&text)
                        && socket.send(Message::Text(t.into())).await.is_err() { break; }
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    if socket.send(Message::Text(r#"{"type":"resync"}"#.into())).await.is_err() { break; }
                }
                Err(_) => break,
            },
            _ = ping.tick() => {
                if socket.send(Message::Ping(Vec::new().into())).await.is_err() { break; }
            }
            // Spectators only listen: anything they send is ignored.
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

    #[test]
    fn messages_lose_account_names() {
        let t = public_message(r#"{"type":"ops","id":"S/1","rev":3,"clientId":"abc-123","ops":[],"by":"jan"}"#).unwrap();
        let v: Value = serde_json::from_str(&t).unwrap();
        assert!(v.get("by").is_none());
        assert_eq!(v["clientId"], "user");
        let t = public_message(r#"{"type":"ops","clientId":"board:b1","by":"jan"}"#).unwrap();
        assert!(t.contains(r#""clientId":"board""#));
        assert!(public_message(r#"{"type":"upload","id":"S/1"}"#).is_none());
    }

    #[test]
    fn scrub_upload_account() {
        let s = Sheet { id: "S/1".into(), rev: 1, day: "d".into(), season: 1, doc: json!({ "upload": { "by": "jan", "ok": true } }) };
        let s = scrub(s);
        assert!(s.doc["upload"].get("by").is_none());
        assert_eq!(s.doc["upload"]["ok"], true);
    }

    #[test]
    fn club_ids() {
        assert!(club_id("WVL154").is_ok());
        assert!(club_id("Vl-B234").is_ok());
        assert!(club_id("x'; drop").is_err());
        assert!(club_id("").is_err());
    }
}
