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
    if !matches!(t, "ops" | "sheet" | "board" | "resync" | "vttl") {
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

/// All clubs of the season, for the club search page: the id and the names only.
pub async fn clubs(State(app): State<Arc<App>>) -> Result<Json<Vec<Value>>, ApiError> {
    let season = app.tabt.current_season(None).await.map_err(|e| ApiError::Vttl(e.to_string()))?;
    let clubs = app.tabt.clubs(None, season).await.map_err(|e| ApiError::Vttl(e.to_string()))?;
    Ok(Json(clubs.iter().map(club_line).collect()))
}

fn club_line(c: &crate::tabt::Club) -> Value {
    json!({ "id": c.id, "name": c.name, "longName": c.long_name, "categoryName": c.category_name })
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

#[derive(Deserialize)]
pub struct PlayerQuery {
    club: String,
    id: i64,
}

/// A player for the player widget, or null. A visitor without a login cannot make this
/// server ask VTTL for any member number: only for a player in a line-up of the club, and
/// then with the guest session of VTTL, one call at a time (`queued_player`). The answer
/// goes in the cache, so a player costs one call per day for all visitors together.
pub async fn player(State(app): State<Arc<App>>, Query(q): Query<PlayerQuery>) -> Result<Json<Option<crate::tabt::PlayerInfo>>, ApiError> {
    let club = club_id(&q.club)?;
    if known_players(&app, club, vec![q.id]).await?.is_empty() {
        return Ok(Json(None));
    }
    Ok(Json(crate::api::queued_player(&app, q.id).await.map_err(|e| ApiError::Vttl(e.to_string()))?))
}

#[derive(Deserialize)]
pub struct WarmBody {
    club: String,
    ids: Vec<i64>,
}

/// A spectator opened a match: get its players in the background, as for a member, with the
/// guest session of VTTL. Only the players that `player` gives too.
pub async fn warm_players(State(app): State<Arc<App>>, Json(b): Json<WarmBody>) -> Result<axum::http::StatusCode, ApiError> {
    let club = club_id(&b.club)?;
    let ids = known_players(&app, club, b.ids.into_iter().take(crate::api::WARM_MAX).collect()).await?;
    crate::api::warm(app, None, ids);
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// The players of `ids` that a spectator can ask for: those that the cache has, and those in
/// a line-up of the club (in a match sheet, or in a match of the season as VTTL has it).
/// From the database and the cache only: this never calls VTTL for the matches.
async fn known_players(app: &App, club: &str, ids: Vec<i64>) -> Result<Vec<i64>, ApiError> {
    let mut known = Vec::new();
    let mut rest = Vec::new();
    for id in ids.into_iter().filter(|id| *id > 0) {
        if app.tabt.has_player(id) || app.db.sheet_has_player(club, id)? {
            known.push(id);
        } else {
            rest.push(id);
        }
    }
    if rest.is_empty() {
        return Ok(known);
    }
    let season = app.tabt.current_season(None).await.map_err(|e| ApiError::Vttl(e.to_string()))?;
    if let Some((_, matches)) = app.tabt.club_matches_cached(season, club) {
        let played: std::collections::HashSet<i64> = matches
            .iter()
            .filter_map(|m| m.details.as_ref())
            .flat_map(|d| d.home_players.iter().chain(&d.away_players))
            .map(|p| p.unique_index)
            .collect();
        known.extend(rest.into_iter().filter(|id| played.contains(id)));
    }
    Ok(known)
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
    let _watching = app.watchers.enter(&club);
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
    fn club_list_has_no_venues() {
        let c = crate::tabt::Club { id: "WVL154".into(), name: "Zonnebeke".into(), long_name: "TTC Zonnebeke".into(), category: 9, category_name: "West-Vlaanderen".into(), venues: vec![] };
        assert_eq!(club_line(&c), json!({ "id": "WVL154", "name": "Zonnebeke", "longName": "TTC Zonnebeke", "categoryName": "West-Vlaanderen" }));
    }

    #[test]
    fn club_ids() {
        assert!(club_id("WVL154").is_ok());
        assert!(club_id("Vl-B234").is_ok());
        assert!(club_id("x'; drop").is_err());
        assert!(club_id("").is_err());
    }
}
