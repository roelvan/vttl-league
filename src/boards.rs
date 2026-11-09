//! Scoreboards (Flippr / ESP32) over MQTT.
//!
//! Each club registers its boards (device id + alias). A captain links a board to one game of a
//! match with "Naar scorebord": the server sends the player names to the board (config topic)
//! and stores the link. Updates (`tabletennis/{device}/{boardMatch}/update`) count only for a
//! linked device; the first update after the link fixes the board's own match id, and updates
//! for any other board match are ignored. When the board match is finished, the server writes
//! the set scores into the match (the server then uploads it) and removes the link.
//! Player 1 on the board = home side.

use crate::api::ApiError;
use crate::app::App;
use crate::auth::User;
use crate::db::{self, Db};
use crate::scoreboard::{self, MatchState};
use crate::sheets::{Applied, Op};
use axum::Json;
use axum::extract::{Query, State};
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const MIGRATION: &str = "
    CREATE TABLE device(
        club        TEXT NOT NULL,
        id          TEXT NOT NULL,
        alias       TEXT NOT NULL,
        last_seen   INTEGER,
        PRIMARY KEY (club, id)
    );
    CREATE TABLE board_link(
        device      TEXT PRIMARY KEY,
        club        TEXT NOT NULL,
        sheet       TEXT NOT NULL,
        game        INTEGER NOT NULL,
        account     TEXT NOT NULL,
        board_match TEXT,
        linked_at   INTEGER NOT NULL
    );
    ALTER TABLE account ADD COLUMN can_upload INTEGER NOT NULL DEFAULT 0;";

/// What the browsers get for a board (web/src/lib/boards.svelte.ts `BoardState`).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BoardState {
    pub device: String,
    pub sheet: Option<String>,
    pub game: Option<i64>,
    pub current: (u8, u8),
    pub games: Vec<(u8, u8)>,
    pub server: u8,
    pub finished: bool,
    /// Epoch ms.
    pub at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub alias: String,
    /// Epoch ms of the last MQTT message.
    pub last_seen: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub device: String,
    pub club: String,
    pub sheet: String,
    pub game: i64,
    pub account: String,
    pub board_match: Option<String>,
}

/// The MQTT connection (None when MQTT_URL is not set) and the last state per board.
pub struct Boards {
    client: Option<AsyncClient>,
    live: Mutex<HashMap<String, BoardState>>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64
}

// ---------------------------------------------------------------- storage

impl Db {
    pub fn devices(&self, club: &str) -> anyhow::Result<Vec<Device>> {
        let c = self.conn();
        let mut st = c.prepare("SELECT id, alias, last_seen FROM device WHERE club = ?1 ORDER BY alias, id")?;
        let rows = st.query_map(params![club], |r| Ok(Device { id: r.get(0)?, alias: r.get(1)?, last_seen: r.get(2)? }))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn save_device(&self, club: &str, id: &str, alias: &str) -> anyhow::Result<()> {
        self.conn().execute(
            "INSERT INTO device(club, id, alias) VALUES (?1, ?2, ?3) ON CONFLICT(club, id) DO UPDATE SET alias = excluded.alias",
            params![club, id, alias],
        )?;
        Ok(())
    }

    pub fn delete_device(&self, club: &str, id: &str) -> anyhow::Result<()> {
        let c = self.conn();
        c.execute("DELETE FROM device WHERE club = ?1 AND id = ?2", params![club, id])?;
        c.execute("DELETE FROM board_link WHERE club = ?1 AND device = ?2", params![club, id])?;
        Ok(())
    }

    pub fn has_device(&self, club: &str, id: &str) -> anyhow::Result<bool> {
        Ok(self.conn().query_row("SELECT 1 FROM device WHERE club = ?1 AND id = ?2", params![club, id], |_| Ok(())).optional()?.is_some())
    }

    /// Records that a device sent something. Returns the clubs that know this device.
    pub fn touch_device(&self, id: &str, at: i64) -> anyhow::Result<Vec<String>> {
        let c = self.conn();
        c.execute("UPDATE device SET last_seen = ?2 WHERE id = ?1", params![id, at])?;
        let mut st = c.prepare("SELECT club FROM device WHERE id = ?1")?;
        let rows = st.query_map(params![id], |r| r.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Links a device to a game. A device has one link at a time: the newest wins.
    pub fn link_board(&self, l: &Link) -> anyhow::Result<()> {
        self.conn().execute(
            "INSERT INTO board_link(device, club, sheet, game, account, board_match, linked_at) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6)
             ON CONFLICT(device) DO UPDATE SET club = excluded.club, sheet = excluded.sheet, game = excluded.game,
               account = excluded.account, board_match = NULL, linked_at = excluded.linked_at",
            params![l.device, l.club, l.sheet, l.game, l.account, db::now()],
        )?;
        Ok(())
    }

    pub fn board_link(&self, device: &str) -> anyhow::Result<Option<Link>> {
        Ok(self
            .conn()
            .query_row("SELECT device, club, sheet, game, account, board_match FROM board_link WHERE device = ?1", params![device], |r| {
                Ok(Link { device: r.get(0)?, club: r.get(1)?, sheet: r.get(2)?, game: r.get(3)?, account: r.get(4)?, board_match: r.get(5)? })
            })
            .optional()?)
    }

    pub fn set_board_match(&self, device: &str, board_match: &str) -> anyhow::Result<()> {
        self.conn().execute("UPDATE board_link SET board_match = ?2 WHERE device = ?1", params![device, board_match])?;
        Ok(())
    }

    pub fn unlink_board(&self, device: &str) -> anyhow::Result<()> {
        self.conn().execute("DELETE FROM board_link WHERE device = ?1", params![device])?;
        Ok(())
    }

    pub fn links_of(&self, club: &str) -> anyhow::Result<Vec<String>> {
        let c = self.conn();
        let mut st = c.prepare("SELECT device FROM board_link WHERE club = ?1")?;
        let rows = st.query_map(params![club], |r| r.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn mark_can_upload(&self, account: &str) -> anyhow::Result<()> {
        self.conn().execute("UPDATE account SET can_upload = 1 WHERE account = ?1", params![account])?;
        Ok(())
    }
}

// ---------------------------------------------------------------- what an update means

/// What to do with one update message.
#[derive(Debug, PartialEq)]
pub enum Decision {
    /// No link, or the update belongs to another board match.
    Ignore,
    /// Show the state; `fix_match` = remember this board match for the link.
    Show { fix_match: bool },
}

/// `topic_match` = the board's own match id from the topic ("" when the topic has none).
pub fn decide(link: Option<&Link>, topic_match: &str) -> Decision {
    match link {
        None => Decision::Ignore,
        Some(l) => match &l.board_match {
            None => Decision::Show { fix_match: true },
            Some(m) if m == topic_match => Decision::Show { fix_match: false },
            Some(_) => Decision::Ignore,
        },
    }
}

/// The set scores of a finished board match as sheet boxes ("l" = home = player 1).
pub fn result_boxes(s: &MatchState, boxes: usize) -> Value {
    let mut v: Vec<Value> = s.games.iter().map(|(a, b)| json!({ "l": a.to_string(), "r": b.to_string() })).collect();
    while v.len() < boxes {
        v.push(json!({ "l": "", "r": "" }));
    }
    Value::Array(v)
}

/// `tabletennis/{device}/update` or `tabletennis/{device}/{boardMatch}/update`.
pub fn parse_topic(topic: &str) -> Option<(&str, &str, &str)> {
    let p: Vec<&str> = topic.split('/').collect();
    match p.as_slice() {
        ["tabletennis", dev, kind] => Some((dev, "", kind)),
        ["tabletennis", dev, m, kind] => Some((dev, m, kind)),
        _ => None,
    }
}

// ---------------------------------------------------------------- MQTT

impl Boards {
    /// Connects when MQTT_URL is set (mqtt://host:port), with MQTT_USER / MQTT_PASSWORD.
    pub fn connect() -> (Self, Option<rumqttc::EventLoop>) {
        let Some(url) = std::env::var("MQTT_URL").ok().filter(|u| !u.trim().is_empty()) else {
            tracing::warn!("MQTT_URL not set: scoreboards are off");
            return (Self { client: None, live: Mutex::new(HashMap::new()) }, None);
        };
        let rest = url.trim_start_matches("mqtt://").trim_start_matches("tcp://");
        let (host, port) = rest.trim_end_matches('/').rsplit_once(':').map(|(h, p)| (h.to_string(), p.parse().unwrap_or(1883))).unwrap_or((rest.to_string(), 1883));
        let mut opts = MqttOptions::new(format!("vttl-league-{}", rand::random::<u32>()), host, port);
        opts.set_keep_alive(Duration::from_secs(30));
        if let (Ok(u), Ok(p)) = (std::env::var("MQTT_USER"), std::env::var("MQTT_PASSWORD")) {
            opts.set_credentials(u, p);
        }
        let (client, eventloop) = AsyncClient::new(opts, 64);
        (Self { client: Some(client), live: Mutex::new(HashMap::new()) }, Some(eventloop))
    }

    pub fn enabled(&self) -> bool {
        self.client.is_some()
    }

    pub fn state(&self, device: &str) -> Option<BoardState> {
        self.live.lock().unwrap().get(device).cloned()
    }

    fn set_state(&self, s: BoardState) {
        self.live.lock().unwrap().insert(s.device.clone(), s);
    }

    /// Sends the names to the board (retained, like vttl-api `publishDeviceConfig`).
    async fn send_names(&self, device: &str, p1: &Names, p2: &Names) -> anyhow::Result<()> {
        let c = self.client.as_ref().ok_or_else(|| anyhow::anyhow!("MQTT is not configured"))?;
        let payload = json!({
            "player1FirstName": scoreboard::display_name(&p1.first),
            "player1FamilyName": scoreboard::display_name(&p1.last),
            "player2FirstName": scoreboard::display_name(&p2.first),
            "player2FamilyName": scoreboard::display_name(&p2.last),
        });
        c.publish(format!("tabletennis/{device}/config"), QoS::AtLeastOnce, true, payload.to_string()).await?;
        Ok(())
    }
}

/// Runs the MQTT event loop for the life of the server; reconnects on errors.
pub async fn run(app: Arc<App>, mut eventloop: rumqttc::EventLoop) {
    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Packet::ConnAck(_))) => {
                tracing::info!("mqtt connected");
                if let Some(c) = &app.boards.client
                    && let Err(e) = c.subscribe("tabletennis/#", QoS::AtMostOnce).await {
                        tracing::warn!("mqtt subscribe: {e}");
                    }
            }
            Ok(Event::Incoming(Packet::Publish(p))) => {
                // A retained message is an old state the broker kept, not a new update.
                if !p.retain
                    && let Err(e) = on_publish(&app, &p.topic, &p.payload) {
                        tracing::warn!(topic = %p.topic, "board update: {e:#}");
                    }
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!("mqtt: {e}");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

fn on_publish(app: &Arc<App>, topic: &str, payload: &[u8]) -> anyhow::Result<()> {
    let Some((device, topic_match, kind)) = parse_topic(topic) else { return Ok(()) };
    if kind != "update" {
        return Ok(());
    }
    let at = now_ms();
    let clubs = app.db.touch_device(device, at)?;
    let link = app.db.board_link(device)?;
    let fix = match decide(link.as_ref(), topic_match) {
        Decision::Ignore => {
            for c in &clubs {
                app.push(c, json!({ "type": "board", "device": device, "lastSeen": at }));
            }
            return Ok(());
        }
        Decision::Show { fix_match } => fix_match,
    };
    let link = link.expect("decide shows only linked boards");
    let s = scoreboard::decode(payload)?;
    if fix {
        app.db.set_board_match(device, topic_match)?;
    }
    let state = BoardState {
        device: device.to_string(),
        sheet: Some(link.sheet.clone()),
        game: Some(link.game),
        current: s.current,
        games: s.games.clone(),
        server: s.server,
        finished: s.finished,
        at,
    };
    app.boards.set_state(state.clone());
    app.push(&link.club, json!({ "type": "board", "device": device, "lastSeen": at, "state": state }));
    if s.finished {
        write_result(app, &link, &s)?;
        app.db.unlink_board(device)?;
    }
    Ok(())
}

/// Puts the set scores of a finished board match into the match record, like a client edit.
fn write_result(app: &Arc<App>, link: &Link, s: &MatchState) -> anyhow::Result<()> {
    let Some(sheet) = app.db.sheet(&link.club, &link.sheet)? else { return Ok(()) };
    let boxes = sheet.doc["results"][link.game as usize].as_array().map(|a| a.len()).unwrap_or(s.best_of as usize).max(s.games.len());
    let ops = vec![Op { p: vec![json!("results"), json!(link.game)], v: result_boxes(s, boxes) }];
    let client = format!("board:{}", link.device);
    let seq = now_ms();
    if let Some(Applied::Done(rev)) = app.db.apply_ops(&link.club, &link.sheet, &client, seq, &ops, &link.account)? {
        app.push(&link.club, json!({ "type": "ops", "id": link.sheet, "rev": rev, "clientId": client, "ops": ops }));
        crate::uploader::schedule(app, &link.club, &link.sheet);
        tracing::info!(club = %link.club, sheet = %link.sheet, game = link.game, "board result saved");
    }
    Ok(())
}

// ---------------------------------------------------------------- HTTP

pub async fn list_devices(State(app): State<Arc<App>>, u: User) -> Result<Json<Vec<Device>>, ApiError> {
    Ok(Json(app.db.devices(u.club()?)?))
}

#[derive(Deserialize)]
pub struct DeviceBody {
    id: String,
    alias: String,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || "-_.:".contains(c))
}

pub async fn save_device(State(app): State<Arc<App>>, u: User, Json(b): Json<DeviceBody>) -> Result<(), ApiError> {
    if !valid_id(b.id.trim()) || b.alias.len() > 80 {
        return Err(ApiError::BadRequest("Ongeldig ID: gebruik letters, cijfers, - _ . of :".into()));
    }
    Ok(app.db.save_device(u.club()?, b.id.trim(), b.alias.trim())?)
}

#[derive(Deserialize)]
pub struct IdQuery {
    id: String,
}

pub async fn delete_device(State(app): State<Arc<App>>, u: User, Query(q): Query<IdQuery>) -> Result<(), ApiError> {
    Ok(app.db.delete_device(u.club()?, &q.id)?)
}

/// The last state of every linked board of the club (for a page that opens mid-game).
pub async fn states(State(app): State<Arc<App>>, u: User) -> Result<Json<Vec<BoardState>>, ApiError> {
    let links = app.db.links_of(u.club()?)?;
    Ok(Json(links.iter().filter_map(|d| app.boards.state(d)).collect()))
}

#[derive(Deserialize)]
pub struct Names {
    first: String,
    last: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkBody {
    device: String,
    sheet: String,
    game: i64,
    player1: Names,
    player2: Names,
}

pub async fn link(State(app): State<Arc<App>>, u: User, Json(b): Json<LinkBody>) -> Result<Json<BoardState>, ApiError> {
    let club = u.club()?;
    if !app.boards.enabled() {
        return Err(ApiError::BadRequest("De server heeft geen MQTT-verbinding (MQTT_URL).".into()));
    }
    if !app.db.has_device(club, &b.device)? {
        return Err(ApiError::NotFound(format!("Scorebord {} hoort niet bij deze club", b.device)));
    }
    let sheet = app.db.sheet(club, &b.sheet)?.ok_or_else(|| ApiError::NotFound(format!("Wedstrijd {} bestaat niet", b.sheet)))?;
    if b.game < 0 || b.game as usize >= sheet.doc["results"].as_array().map(|a| a.len()).unwrap_or(0) {
        return Err(ApiError::BadRequest("Ongeldige wedstrijd".into()));
    }
    app.boards.send_names(&b.device, &b.player1, &b.player2).await.map_err(|e| ApiError::BadRequest(format!("Scorebord: {e}")))?;
    app.db.link_board(&Link {
        device: b.device.clone(),
        club: club.to_string(),
        sheet: b.sheet.clone(),
        game: b.game,
        account: u.account.account.clone(),
        board_match: None,
    })?;
    let state = BoardState { device: b.device.clone(), sheet: Some(b.sheet), game: Some(b.game), current: (0, 0), games: vec![], server: 1, finished: false, at: now_ms() };
    app.boards.set_state(state.clone());
    app.push(club, json!({ "type": "board", "device": b.device, "state": state }));
    Ok(Json(state))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(m: Option<&str>) -> Link {
        Link { device: "b1".into(), club: "W1".into(), sheet: "S/1".into(), game: 3, account: "jan".into(), board_match: m.map(String::from) }
    }

    #[test]
    fn topics() {
        assert_eq!(parse_topic("tabletennis/b1/m9/update"), Some(("b1", "m9", "update")));
        assert_eq!(parse_topic("tabletennis/b1/update"), Some(("b1", "", "update")));
        assert_eq!(parse_topic("tabletennis/b1/config"), Some(("b1", "", "config")));
        assert_eq!(parse_topic("other/b1/update"), None);
    }

    #[test]
    fn only_the_linked_board_match_counts() {
        assert_eq!(decide(None, "m1"), Decision::Ignore);
        assert_eq!(decide(Some(&link(None)), "m1"), Decision::Show { fix_match: true });
        assert_eq!(decide(Some(&link(Some("m1"))), "m1"), Decision::Show { fix_match: false });
        assert_eq!(decide(Some(&link(Some("m1"))), "m2"), Decision::Ignore);
    }

    #[test]
    fn boxes_from_board() {
        let s = MatchState { duration_ms: 0, best_of: 5, point_count: 0, games: vec![(11, 7), (4, 11), (12, 10), (11, 5)], current: (0, 0), games_won: (3, 1), server: 1, finished: true, timeout: None };
        let v = result_boxes(&s, 5);
        assert_eq!(v[1], json!({ "l": "4", "r": "11" }));
        assert_eq!(v[4], json!({ "l": "", "r": "" }));
    }

    #[test]
    fn devices_and_links() {
        let db = db::memory();
        db.save_device("W1", "b1", "Tafel 7").unwrap();
        db.save_device("W1", "b1", "Tafel 8").unwrap();
        assert_eq!(db.devices("W1").unwrap()[0].alias, "Tafel 8");
        assert_eq!(db.touch_device("b1", 5).unwrap(), vec!["W1".to_string()]);
        db.link_board(&link(None)).unwrap();
        db.set_board_match("b1", "m1").unwrap();
        assert_eq!(db.board_link("b1").unwrap().unwrap().board_match.as_deref(), Some("m1"));
        // A new link resets the board match.
        db.link_board(&Link { game: 4, ..link(None) }).unwrap();
        let l = db.board_link("b1").unwrap().unwrap();
        assert_eq!((l.game, l.board_match), (4, None));
        db.delete_device("W1", "b1").unwrap();
        assert!(db.board_link("b1").unwrap().is_none());
    }
}
