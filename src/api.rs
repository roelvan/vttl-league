use crate::app::{App, Event};
use crate::db::{Conflict, Device, Forfeit, Lineup, LineupPlayer, Link, ResultRow};
use crate::upload;
use crate::vttl::{Member, Player};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;

type S = State<Arc<App>>;
type R<T> = Result<Json<T>, ApiError>;

pub fn router(app: Arc<App>) -> Router {
    Router::new()
        .route("/api/config", get(config))
        .route("/api/matches", get(matches))
        .route("/api/sheet", get(sheet))
        .route("/api/lineup", post(save_lineup))
        .route("/api/result", post(save_result))
        .route("/api/link", post(link))
        .route("/api/link/{device_id}", delete(unlink))
        .route("/api/devices", get(devices).post(save_device))
        .route("/api/devices/{device_id}", delete(delete_device))
        .route("/api/devices/{device_id}/clear", post(clear_device_match))
        .route("/api/upload", post(do_upload))
        .route("/ws", get(ws))
        .with_state(app)
}

// ---------- errors ----------

pub struct ApiError(StatusCode, String);

#[derive(Debug)]
struct BadRequest(String);
impl std::fmt::Display for BadRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for BadRequest {}
fn bad(msg: impl Into<String>) -> anyhow::Error {
    BadRequest(msg.into()).into()
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        if e.downcast_ref::<Conflict>().is_some() {
            ApiError(StatusCode::CONFLICT, e.to_string())
        } else if e.downcast_ref::<BadRequest>().is_some() {
            ApiError(StatusCode::BAD_REQUEST, e.to_string())
        } else {
            tracing::error!("{e:#}");
            ApiError(StatusCode::BAD_GATEWAY, format!("{e:#}"))
        }
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

// ---------- reads ----------

async fn config(State(app): S) -> Json<Value> {
    Json(json!({ "club": app.cfg.club, "dry_run": app.cfg.dry_run, "mqtt": app.mqtt.client.is_some() }))
}

#[derive(Deserialize)]
struct DateQ {
    date: String,
}

/// Team score from the outcome of each position: { home, away, played, total }.
fn team_score<'a>(outcomes: impl Iterator<Item = Option<&'a str>>) -> Value {
    let (mut home, mut away, mut played, mut total) = (0, 0, 0, 0);
    for o in outcomes {
        total += 1;
        played += o.is_some() as i64;
        home += (o == Some("home")) as i64;
        away += (o == Some("away")) as i64;
    }
    json!({ "home": home, "away": away, "played": played, "total": total })
}

/// Home matches of the club on one day, each with the current team score.
async fn matches(State(app): S, Query(q): Query<DateQ>) -> R<Value> {
    let all = app.vttl.matches_on(&app.cfg.club, &q.date).await?;
    let mut home = vec![];
    for m in all.into_iter().filter(|m| m.home_club == app.cfg.club) {
        // No match system yet (no details on VTTL): no score.
        let live = match app.vttl.match_system(m.match_system).await {
            Ok(sys) if m.match_system != 0 => {
                let results = app.db.results(&m.match_id)?;
                Some(team_score(sys.defs.iter().map(|d| {
                    upload::outcome(
                        results.iter().find(|r| r.position == d.position),
                        m.results.iter().find(|x| x.position == d.position),
                        sys.set_count as usize,
                    )
                })))
            }
            _ => None,
        };
        home.push(json!({ "match_id": m.match_id, "time": m.time, "home_team": m.home_team,
                          "away_team": m.away_team, "away_club": m.away_club, "score": m.score,
                          "live_score": live }));
    }
    Ok(Json(json!({ "matches": home })))
}

#[derive(Deserialize)]
struct MatchQ {
    match_id: String,
}

#[derive(Serialize)]
struct Side {
    /// One entry per team position (index 0 = position 1).
    players: Vec<Option<LineupPlayer>>,
    /// Unique index of the captain (local, else VTTL).
    captain: Option<i64>,
    /// Rev to send back on save (0 = never saved locally).
    rev: i64,
    /// "local" | "vttl" | "none"
    source: &'static str,
    members: Vec<Member>,
}

#[derive(Serialize)]
struct Row {
    position: i64,
    kind: i64,
    home_index: i64,
    away_index: i64,
    home_player: Option<LineupPlayer>,
    away_player: Option<LineupPlayer>,
    /// Local result (manual or scoreboard). rev is 0 when there is none.
    result: Option<ResultRow>,
    rev: i64,
    vttl_entry: Option<String>,
    /// "home" | "away" | "void" (both forfeit) | None (not played yet)
    outcome: Option<&'static str>,
}

fn side(local: Option<Lineup>, vttl: &[Player], vttl_captain: i64, count: i64, members: Vec<Member>) -> Side {
    let n = count as usize;
    let vttl_captain = (vttl_captain != 0).then_some(vttl_captain);
    match local {
        Some(mut l) => {
            l.players.resize(n, None);
            Side { players: l.players, captain: l.captain, rev: l.rev, source: "local", members }
        }
        None if !vttl.is_empty() => {
            let players = (1..=n)
                .map(|pos| {
                    vttl.iter().find(|p| p.position == pos as i64).map(|p| LineupPlayer {
                        unique_index: p.unique_index,
                        first_name: p.first_name.clone(),
                        last_name: p.last_name.clone(),
                    })
                })
                .collect();
            Side { players, captain: vttl_captain, rev: 0, source: "vttl", members }
        }
        None => Side { players: vec![None; n], captain: vttl_captain, rev: 0, source: "none", members },
    }
}

async fn load_sheet(app: &App, match_id: &str) -> anyhow::Result<Value> {
    let m = app.vttl.team_match(&app.cfg.club, match_id, false).await?;
    let sys = app.vttl.match_system(m.match_system).await?;
    let (hm, am) = tokio::join!(app.vttl.members(&m.home_club), async {
        if m.away_club.is_empty() || m.away_club == "-" {
            Ok(vec![])
        } else {
            app.vttl.members(&m.away_club).await
        }
    });
    let home = side(app.db.lineup(match_id, "H")?, &m.home_players, m.home_captain, sys.players_per_team, hm?);
    let away = side(app.db.lineup(match_id, "A")?, &m.away_players, m.away_captain, sys.players_per_team, am?);
    let results = app.db.results(match_id)?;
    let pick = |s: &Side, i: i64| s.players.get((i - 1).max(0) as usize).cloned().flatten();
    let rows: Vec<Row> = sys
        .defs
        .iter()
        .map(|d| {
            let r = results.iter().find(|r| r.position == d.position).cloned();
            let v = m.results.iter().find(|x| x.position == d.position);
            Row {
                position: d.position,
                kind: d.kind,
                home_index: d.home_index,
                away_index: d.away_index,
                home_player: (d.kind == 1).then(|| pick(&home, d.home_index)).flatten(),
                away_player: (d.kind == 1).then(|| pick(&away, d.away_index)).flatten(),
                rev: r.as_ref().map(|r| r.rev).unwrap_or(0),
                outcome: upload::outcome(r.as_ref(), v, sys.set_count as usize),
                result: r.filter(|r| !r.is_empty()),
                vttl_entry: v.map(|x| x.entry.clone()),
            }
        })
        .collect();
    let score = team_score(rows.iter().map(|r| r.outcome));
    let links: Vec<Link> = app.db.links()?.into_iter().filter(|l| l.match_id == match_id).collect();
    Ok(json!({ "match": m, "system": sys, "home": home, "away": away, "rows": rows, "links": links, "score": score }))
}

async fn sheet(State(app): S, Query(q): Query<MatchQ>) -> R<Value> {
    Ok(Json(load_sheet(&app, &q.match_id).await?))
}

async fn devices(State(app): S) -> R<Value> {
    Ok(Json(json!({ "matches": app.mqtt.snapshot(), "links": app.db.links()?, "known": app.db.devices()? })))
}

// ---------- writes (all checked against a rev, all broadcast) ----------

#[derive(Deserialize)]
struct LineupReq {
    match_id: String,
    side: String,
    players: Vec<Option<LineupPlayer>>,
    #[serde(default)]
    captain: Option<i64>,
    rev: i64,
}

async fn save_lineup(State(app): S, Json(r): Json<LineupReq>) -> R<Value> {
    if r.side != "H" && r.side != "A" {
        return Err(bad("side must be H or A").into());
    }
    let ids: Vec<i64> = r.players.iter().flatten().map(|p| p.unique_index).collect();
    if (1..ids.len()).any(|i| ids[..i].contains(&ids[i])) {
        return Err(bad("a player is assigned twice").into());
    }
    if r.captain.is_some_and(|c| !ids.contains(&c)) {
        return Err(bad("the captain must be a player of the lineup").into());
    }
    let rev = app.db.save_lineup(&r.match_id, &r.side, &r.players, r.captain, r.rev)?;
    app.emit(Event::Lineup { match_id: r.match_id, side: r.side });
    Ok(Json(json!({ "rev": rev })))
}

#[derive(Deserialize)]
struct ResultReq {
    match_id: String,
    position: i64,
    sets: Vec<(u8, u8)>,
    forfeit: Option<Forfeit>,
    rev: i64,
}

async fn save_result(State(app): S, Json(r): Json<ResultReq>) -> R<Value> {
    if !r.sets.is_empty() && r.forfeit.is_none() {
        let m = app.vttl.team_match(&app.cfg.club, &r.match_id, false).await?;
        let sys = app.vttl.match_system(m.match_system).await?;
        upload::validate(&r.sets, sys.set_count as usize).map_err(bad)?;
    }
    let sets = if r.forfeit.is_some() { vec![] } else { r.sets };
    let rev = app.db.save_result_manual(&r.match_id, r.position, &sets, r.forfeit.as_ref(), r.rev)?;
    app.emit(Event::Result { match_id: r.match_id, position: r.position });
    Ok(Json(json!({ "rev": rev })))
}

#[derive(Deserialize)]
struct LinkReq {
    device_id: String,
    match_id: String,
    position: i64,
    /// Take the device even when it is linked to another position.
    #[serde(default)]
    force: bool,
}

async fn link(State(app): S, Json(r): Json<LinkReq>) -> R<Value> {
    if !app.db.devices()?.iter().any(|d| d.device_id == r.device_id) {
        return Err(bad(format!("device {} is not in the device list", r.device_id)).into());
    }
    if let Some(l) = app.db.link(&r.device_id)? {
        if !r.force && (l.match_id != r.match_id || l.position != r.position) {
            return Err(ApiError(
                StatusCode::CONFLICT,
                format!("device {} is in use for {} position {}", l.device_id, l.match_id, l.position),
            ));
        }
    }
    let sheet = load_sheet(&app, &r.match_id).await?;
    let row = sheet["rows"]
        .as_array()
        .and_then(|rows| rows.iter().find(|x| x["position"] == r.position))
        .ok_or_else(|| bad("unknown position"))?;
    let name = |p: &Value| {
        (p["first_name"].as_str().unwrap_or("").to_string(), p["last_name"].as_str().unwrap_or("?").to_string())
    };
    let (h, a) = (name(&row["home_player"]), name(&row["away_player"]));

    // Existing device matches: an unfinished one without points is the one about to be
    // played, so it joins this link. All others stay out of it.
    let existing = app.mqtt.snapshot();
    let mut old = vec![];
    for dm in existing.iter().filter(|d| d.device_id == r.device_id) {
        if !dm.state.finished && dm.state.point_count == 0 {
            app.db.bind(&r.device_id, &dm.mqtt_match_id, &r.match_id, r.position)?;
        } else {
            old.push(dm.mqtt_match_id.clone());
        }
    }
    app.db.mark_seen(&r.device_id, &old)?;
    app.db.set_link(&Link { device_id: r.device_id.clone(), match_id: r.match_id.clone(), position: r.position })?;
    {
        let mut st = app.mqtt.store.lock().unwrap();
        for dm in st.matches.values_mut() {
            if dm.device_id == r.device_id && !old.contains(&dm.mqtt_match_id) {
                dm.match_id = Some(r.match_id.clone());
                dm.position = Some(r.position);
            }
        }
    }
    // Player 1 = home player.
    app.mqtt.send_names(&r.device_id, (&h.0, &h.1), (&a.0, &a.1)).await?;
    app.emit(Event::Links);
    Ok(Json(json!({ "ok": true })))
}

async fn unlink(State(app): S, Path(device_id): Path<String>) -> R<Value> {
    if app.db.delete_link(&device_id)? {
        if app.mqtt.client.is_some() {
            app.mqtt.send_names(&device_id, ("", ""), ("", "")).await?;
        }
        app.emit(Event::Links);
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct DeviceReq {
    device_id: String,
    #[serde(default)]
    alias: String,
}

async fn save_device(State(app): S, Json(r): Json<DeviceReq>) -> R<Value> {
    let device_id = r.device_id.trim().to_string();
    // The id is one MQTT topic level: tabletennis/<id>/...
    if device_id.is_empty() || device_id.contains(['/', '+', '#']) || device_id.chars().any(char::is_whitespace) {
        return Err(bad("the device ID must not be empty and must not contain spaces, /, + or #").into());
    }
    app.db.save_device(&Device { device_id: device_id.clone(), alias: r.alias.trim().to_string() })?;
    app.mqtt.listen(&device_id).await?;
    app.emit(Event::Devices);
    Ok(Json(json!({ "ok": true })))
}

async fn delete_device(State(app): S, Path(device_id): Path<String>) -> R<Value> {
    if app.db.delete_device(&device_id)? {
        // Not listened to any more, so a link cannot give results.
        if app.db.delete_link(&device_id)? {
            app.emit(Event::Links);
        }
        for (device_id, mqtt_match_id) in app.mqtt.forget(&device_id).await? {
            app.emit(Event::DeviceRemoved { device_id, mqtt_match_id });
        }
        app.emit(Event::Devices);
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct ClearReq {
    mqtt_match_id: String,
}

async fn clear_device_match(State(app): S, Path(device_id): Path<String>, Json(r): Json<ClearReq>) -> R<Value> {
    app.mqtt.clear_match(&device_id, &r.mqtt_match_id).await?;
    Ok(Json(json!({ "ok": true })))
}

async fn do_upload(State(app): S, Json(q): Json<MatchQ>) -> R<Value> {
    // Always read VTTL fresh right before an upload: it replaces all results.
    let m = app.vttl.team_match(&app.cfg.club, &q.match_id, true).await?;
    if m.is_validated || m.is_locked {
        return Err(bad("the match is validated or locked on VTTL").into());
    }
    let sys = app.vttl.match_system(m.match_system).await?;
    let season = app.vttl.current_season().await?;
    let (line, warnings) = upload::i_line(
        &season,
        &m,
        &sys,
        app.db.lineup(&q.match_id, "H")?.as_ref(),
        app.db.lineup(&q.match_id, "A")?.as_ref(),
        &app.db.results(&q.match_id)?,
    );
    tracing::info!("I-line ({}): {line}", if app.cfg.dry_run { "dry run" } else { "upload" });
    if app.cfg.dry_run {
        return Ok(Json(json!({ "dry_run": true, "line": line, "warnings": warnings })));
    }
    let (ok, errors) = app.vttl.upload(&line).await?;
    Ok(Json(json!({ "dry_run": false, "line": line, "warnings": warnings, "result": ok, "errors": errors })))
}

// ---------- push ----------

async fn ws(State(app): S, up: WebSocketUpgrade) -> Response {
    up.on_upgrade(move |sock| push(app, sock))
}

async fn push(app: Arc<App>, mut sock: WebSocket) {
    let mut rx = app.events.subscribe();
    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                Ok(text) => if sock.send(Message::Text(text.into())).await.is_err() { break },
                // Too slow: tell the browser to reload everything.
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    if sock.send(Message::Text(r#"{"type":"resync"}"#.into())).await.is_err() { break }
                }
                Err(_) => break,
            },
            msg = sock.recv() => match msg {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                _ => {}
            },
        }
    }
}
