//! Scoreboards (Flippr / ESP32) over MQTT.
//!
//! Each club registers its boards (device id + alias). A captain links a board to one game of a
//! match with "Naar scorebord": the server sends the player names to the board (config topic),
//! starts a new board match with the away player serving (command topic)
//! and stores the link. Updates (`tabletennis/{device}/{boardMatch}/update`) count only for a
//! linked device; the first update after the link fixes the board's own match id, and updates
//! for any other board match are ignored. When the board match is finished, the server writes
//! the set scores into the match (the server then uploads it). The link stays until the next
//! "Naar scorebord" for this board: an undo on the board after the end opens the game again,
//! and the next end writes the corrected set scores (see `effect`).
//! The last state of a linked board is also in the database, so a restart of the server keeps it.
//! Player 1 on the board = home side.

use crate::api::ApiError;
use crate::app::App;
use crate::auth::User;
use crate::db::{self, Db};
use crate::scoreboard::{self, MatchState, Timeout};
use crate::sheets::{Applied, Op};
use axum::Json;
use axum::http::StatusCode;
use axum::extract::{Query, State};
use rumqttc::v5::mqttbytes::QoS;
use rumqttc::v5::mqttbytes::v5::{Packet, PublishProperties};
use rumqttc::v5::{AsyncClient, Event, EventLoop, MqttOptions};
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

/// The last state of a linked board (JSON of `BoardState`), so a restart keeps it.
pub const MIGRATION_STATE: &str = "ALTER TABLE board_link ADD COLUMN state TEXT;";

/// The board match of the link before this one: its updates are not for the new game.
pub const MIGRATION_OLD_MATCH: &str = "ALTER TABLE board_link ADD COLUMN old_match TEXT;";

/// What the browsers get for a board (web/src/lib/boards.svelte.ts `BoardState`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BoardState {
    pub device: String,
    pub sheet: Option<String>,
    pub game: Option<i64>,
    pub current: (u8, u8),
    pub games: Vec<(u8, u8)>,
    pub server: u8,
    /// Player (1 or 2) who served first in game 1.
    pub first_server: u8,
    /// Every point in order: '0' = home (player 1) won it, '1' = away.
    pub points: String,
    /// Match time on the board at the last update.
    pub duration_ms: u32,
    /// None = the board did not send timeout data.
    pub timeout: Option<Timeout>,
    pub finished: bool,
    /// Epoch ms (server time) of the last update.
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
    /// The board match of the link before this one (see `decide`).
    pub old_match: Option<String>,
}

/// The MQTT connection (None when MQTT_HOST is not set) and the last state per board.
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

    /// Links a device to a game. A device has one link at a time: the newest wins. The board
    /// match of the old link is kept as `old_match` (`l.old_match` is not used).
    pub fn link_board(&self, l: &Link) -> anyhow::Result<()> {
        self.conn().execute(
            "INSERT INTO board_link(device, club, sheet, game, account, board_match, linked_at) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6)
             ON CONFLICT(device) DO UPDATE SET club = excluded.club, sheet = excluded.sheet, game = excluded.game,
               account = excluded.account, old_match = COALESCE(board_link.board_match, board_link.old_match),
               board_match = NULL, state = NULL, linked_at = excluded.linked_at",
            params![l.device, l.club, l.sheet, l.game, l.account, db::now()],
        )?;
        Ok(())
    }

    pub fn board_link(&self, device: &str) -> anyhow::Result<Option<Link>> {
        Ok(self
            .conn()
            .query_row("SELECT device, club, sheet, game, account, board_match, old_match FROM board_link WHERE device = ?1", params![device], |r| {
                Ok(Link { device: r.get(0)?, club: r.get(1)?, sheet: r.get(2)?, game: r.get(3)?, account: r.get(4)?, board_match: r.get(5)?, old_match: r.get(6)? })
            })
            .optional()?)
    }

    pub fn set_board_match(&self, device: &str, board_match: &str) -> anyhow::Result<()> {
        self.conn().execute("UPDATE board_link SET board_match = ?2 WHERE device = ?1", params![device, board_match])?;
        Ok(())
    }

    /// Keeps the last state of a linked board. Does nothing when the board has no link.
    pub fn save_board_state(&self, s: &BoardState) -> anyhow::Result<()> {
        self.conn().execute("UPDATE board_link SET state = ?2 WHERE device = ?1", params![s.device, serde_json::to_string(s)?])?;
        Ok(())
    }

    /// The kept states of all linked boards. A state that does not parse is skipped.
    pub fn board_states(&self) -> anyhow::Result<Vec<BoardState>> {
        let c = self.conn();
        let mut st = c.prepare("SELECT state FROM board_link WHERE state IS NOT NULL")?;
        let rows = st.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?.iter().filter_map(|s| serde_json::from_str(s).ok()).collect())
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
/// Board player that serves first in a linked match: player 2 = the away player.
const AWAY_SERVES: u8 = 2;

/// Points of a set on the scoreboard: its table tennis rules count to 11 and nothing else.
const BOARD_SET_POINTS: u32 = 11;

/// A scoreboard can only count a match whose sets go to 11 points. The old match systems
/// (sets to 21) would get 11-point scores in a 21-point sheet, so they get no board.
/// A match without a known system is not refused: nothing says its sets are different.
pub fn board_can_count(doc: &Value) -> Result<(), String> {
    match doc["matchSystemId"].as_i64().and_then(crate::rules::system) {
        Some(sys) if sys.point_count != BOARD_SET_POINTS => Err(format!(
            "Het scorebord telt sets tot {BOARD_SET_POINTS} punten. Deze wedstrijd speelt sets tot {} punten: vul de uitslag met de hand in.",
            sys.point_count
        )),
        _ => Ok(()),
    }
}

/// Best-of of game `game` of a match: the league decides it, not the board. It is the number
/// of set boxes of the match system (3 sets to win = best of 5); a match without a known
/// system falls back to the boxes that are on the sheet, then to best of 5.
pub fn best_of(doc: &Value, game: i64) -> u8 {
    let boxes = doc["matchSystemId"]
        .as_i64()
        .and_then(crate::rules::system)
        .map(|s| s.boxes)
        .or_else(|| doc["results"][game as usize].as_array().map(|a| a.len()))
        .unwrap_or(5);
    if boxes % 2 == 1 && boxes <= 7 { boxes as u8 } else { 5 }
}

/// How long the broker keeps the retained names of a board (MQTT 5 Message Expiry Interval).
/// After this time a board that connects gets no names of a match that ended long ago.
const CONFIG_EXPIRY_SECS: u32 = 2 * 60 * 60;

/// The MQTT 5 properties of a config message.
fn config_properties() -> PublishProperties {
    PublishProperties { message_expiry_interval: Some(CONFIG_EXPIRY_SECS), ..Default::default() }
}

/// A new link waits for the first update of the NEW board match. The board repeats the last
/// message of its old match, and that message can come after the link (the board did not get
/// the "new" command yet, or it lost it): it must not fix the link to the old match, or the
/// result of the previous game would go into this game. A board with a topic without a match
/// id (old firmware) cannot be told apart this way.
pub fn decide(link: Option<&Link>, topic_match: &str) -> Decision {
    match link {
        None => Decision::Ignore,
        Some(l) => match &l.board_match {
            None if !topic_match.is_empty() && l.old_match.as_deref() == Some(topic_match) => Decision::Ignore,
            None => Decision::Show { fix_match: true },
            Some(m) if m == topic_match => Decision::Show { fix_match: false },
            Some(_) => Decision::Ignore,
        },
    }
}

/// What an update of a linked board does to the match record.
#[derive(Debug, PartialEq)]
pub enum Effect {
    /// Only the live state changes.
    Show,
    /// The board match is finished: write its set scores.
    Write,
    /// An undo after the end: the game runs again, so its set scores go out of the record.
    Reopen,
    /// The board changed a finished game that somebody corrected by hand since: the hand wins
    /// and the board loses its link.
    Detach,
}

/// `prev` = the last state of this board for the linked game, `stored` = `results[game]` of
/// the match record. A board that finished the game can still change it (undo, then other
/// points), but only while the record holds what the board wrote.
pub fn effect(prev: Option<&BoardState>, s: &MatchState, stored: &Value) -> Effect {
    let Some(prev) = prev.filter(|p| p.finished) else {
        return if s.finished { Effect::Write } else { Effect::Show };
    };
    // The same end again (the board repeats its last message): nothing new.
    if s.finished && s.games == prev.games && s.points == prev.points {
        return Effect::Show;
    }
    if !same_sets(stored, &prev.games) {
        return Effect::Detach;
    }
    if s.finished { Effect::Write } else { Effect::Reopen }
}

/// Are the set boxes of a game in the match record the sets `games` of a board?
/// Empty boxes after the last set do not count.
pub fn same_sets(stored: &Value, games: &[(u8, u8)]) -> bool {
    let Some(boxes) = stored.as_array() else { return false };
    let text = |b: &Value, k: &str| b[k].as_str().unwrap_or("").trim().to_string();
    let mut sets: Vec<(String, String)> = boxes.iter().map(|b| (text(b, "l"), text(b, "r"))).collect();
    while sets.last().is_some_and(|(l, r)| l.is_empty() && r.is_empty()) {
        sets.pop();
    }
    sets.len() == games.len() && sets.iter().zip(games).all(|((l, r), (a, b))| *l == a.to_string() && *r == b.to_string())
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
    /// Connects when MQTT_HOST is set, with MQTT_PORT (default 1883, or 8883 with TLS),
    /// MQTT_TLS (true/false) and MQTT_USER / MQTT_PASSWORD.
    /// The connection uses MQTT 5: the broker needs it for the expiry of the config messages.
    pub fn connect() -> (Self, Option<EventLoop>) {
        let env = |k: &str| std::env::var(k).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        let Some(host) = env("MQTT_HOST") else {
            tracing::warn!("MQTT_HOST not set: scoreboards are off");
            return (Self { client: None, live: Mutex::new(HashMap::new()) }, None);
        };
        let tls = env("MQTT_TLS").is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"));
        let port = env("MQTT_PORT").and_then(|p| p.parse().ok()).unwrap_or(if tls { 8883 } else { 1883 });
        tracing::info!("MQTT: {host}:{port}{}", if tls { " (TLS)" } else { "" });
        let mut opts = MqttOptions::new(format!("vttl-league-{}", rand::random::<u32>()), host, port);
        opts.set_keep_alive(Duration::from_secs(30));
        if tls {
            opts.set_transport(rumqttc::Transport::tls_with_default_config());
        }
        if let (Some(u), Some(p)) = (env("MQTT_USER"), env("MQTT_PASSWORD")) {
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

    /// Puts back the states that the database kept (after a restart of the server).
    pub fn restore(&self, states: Vec<BoardState>) {
        for s in states {
            self.set_state(s);
        }
    }

    fn set_state(&self, s: BoardState) {
        self.live.lock().unwrap().insert(s.device.clone(), s);
    }

    /// Sends the names to the board (retained, like vttl-api `publishDeviceConfig`).
    /// The broker drops the retained message after `CONFIG_EXPIRY_SECS`. The board shows two
    /// lines per side: line 1 (top) gets `last`, line 2 (bottom) gets `first`.
    async fn send_names(&self, device: &str, p1: &Names, p2: &Names) -> anyhow::Result<()> {
        let c = self.client.as_ref().ok_or_else(|| anyhow::anyhow!("MQTT is not configured"))?;
        let payload = json!({
            "player1NameLine1": scoreboard::display_name(&p1.last),
            "player1NameLine2": scoreboard::display_name(&p1.first),
            "player2NameLine1": scoreboard::display_name(&p2.last),
            "player2NameLine2": scoreboard::display_name(&p2.first),
        });
        c.publish_with_properties(format!("tabletennis/{device}/config"), QoS::AtLeastOnce, true, payload.to_string(), config_properties())
            .await?;
        Ok(())
    }

    /// Starts a new match on the board (like vttl-api `publishDeviceCommand`). Not retained:
    /// a board that reconnects later must not reset its score. `sport` makes a board that was
    /// left in another sport (badminton) play table tennis, and `bestOf` sets the format of the
    /// match system on the board; older firmware ignores both fields and keeps its own format.
    async fn start_match(&self, device: &str, starting_server: u8, best_of: u8) -> anyhow::Result<()> {
        let c = self.client.as_ref().ok_or_else(|| anyhow::anyhow!("MQTT is not configured"))?;
        let payload = json!({ "command": "new", "payload": { "startingServer": starting_server, "sport": scoreboard::SPORT_TABLE_TENNIS, "bestOf": best_of } });
        c.publish(format!("tabletennis/{device}/command"), QoS::AtLeastOnce, false, payload.to_string()).await?;
        Ok(())
    }
}

/// Runs the MQTT event loop for the life of the server; reconnects on errors.
pub async fn run(app: Arc<App>, mut eventloop: EventLoop) {
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
                // MQTT 5 gives the topic as bytes; a topic that is not UTF-8 is not ours.
                if !p.retain
                    && let Ok(topic) = std::str::from_utf8(&p.topic)
                    && let Err(e) = on_publish(&app, topic, &p.payload) {
                        tracing::warn!(topic = %topic, "board update: {e:#}");
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
    // The board must play the format of the match: a board with older firmware, or one whose
    // operator changed Best Of in its menu, would otherwise finish too early or too late and
    // the wrong result would go into the match (and be uploaded).
    let sheet = app.db.sheet(&link.club, &link.sheet)?;
    if let Some(sheet) = &sheet {
        let want = best_of(&sheet.doc, link.game);
        if s.best_of != want {
            anyhow::bail!("board plays best of {}, the match is best of {want}", s.best_of);
        }
    }
    let stored = sheet.map(|x| x.doc["results"][link.game as usize].clone()).unwrap_or(Value::Null);
    let prev = app.boards.state(device).filter(|p| p.sheet.as_deref() == Some(link.sheet.as_str()) && p.game == Some(link.game));
    let effect = effect(prev.as_ref(), &s, &stored);
    if effect == Effect::Detach {
        app.db.unlink_board(device)?;
        tracing::info!(club = %link.club, sheet = %link.sheet, game = link.game, "board changed a game that was corrected by hand: link removed");
        app.push(&link.club, json!({ "type": "board", "device": device, "lastSeen": at }));
        return Ok(());
    }
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
        first_server: s.first_server,
        points: s.points.clone(),
        duration_ms: s.duration_ms,
        timeout: s.timeout.clone(),
        finished: s.finished,
        at,
    };
    app.boards.set_state(state.clone());
    // Also the finished state: after a restart, an undo on the board must still find it.
    app.db.save_board_state(&state)?;
    app.push(&link.club, json!({ "type": "board", "device": device, "lastSeen": at, "state": state }));
    match effect {
        Effect::Write => write_result(app, &link, &s)?,
        Effect::Reopen => reopen(app, &link, &s, &stored)?,
        Effect::Show | Effect::Detach => {}
    }
    Ok(())
}

/// What the match record keeps of a board game, for the stats after the game
/// (web/src/lib/sheet/types.ts `BoardPlay`).
pub fn board_play(s: &MatchState) -> Value {
    let used = s.timeout.as_ref().map(|t| json!([t.p1_used, t.p2_used])).unwrap_or(Value::Null);
    json!({ "points": s.points, "firstServer": s.first_server, "durationMs": s.duration_ms, "timeoutsUsed": used })
}

/// Puts the set scores of a finished board match into the match record, like a client edit.
/// The point sequence goes into `boardPlay[game]` for the stats.
fn write_result(app: &Arc<App>, link: &Link, s: &MatchState) -> anyhow::Result<()> {
    let Some(sheet) = app.db.sheet(&link.club, &link.sheet)? else { return Ok(()) };
    let boxes = sheet.doc["results"][link.game as usize].as_array().map(|a| a.len()).unwrap_or(s.best_of as usize).max(s.games.len());
    let ops = vec![
        Op { p: vec![json!("results"), json!(link.game)], v: result_boxes(s, boxes) },
        Op { p: vec![json!("boardPlay"), json!(link.game)], v: board_play(s) },
    ];
    save_ops(app, link, ops, "board result saved")
}

/// The ops that take a board result out of the match record: empty set boxes, no point sequence.
pub fn reopen_ops(game: i64, boxes: usize) -> Vec<Op> {
    let empty: Vec<Value> = (0..boxes).map(|_| json!({ "l": "", "r": "" })).collect();
    vec![
        Op { p: vec![json!("results"), json!(game)], v: Value::Array(empty) },
        Op { p: vec![json!("boardPlay"), json!(game)], v: Value::Null },
    ]
}

/// An undo on the board after the end of the game: the game is not decided any more, so the
/// result of the board goes out of the match record. The browsers show the game live again.
fn reopen(app: &Arc<App>, link: &Link, s: &MatchState, stored: &Value) -> anyhow::Result<()> {
    let boxes = stored.as_array().map(|a| a.len()).unwrap_or(s.best_of as usize);
    save_ops(app, link, reopen_ops(link.game, boxes), "board game opened again")
}

/// Stores a change of the board like a client edit, tells the browsers and plans the upload.
fn save_ops(app: &Arc<App>, link: &Link, ops: Vec<Op>, what: &str) -> anyhow::Result<()> {
    let client = format!("board:{}", link.device);
    let seq = now_ms();
    if let Some(Applied::Done(rev)) = app.db.apply_ops(&link.club, &link.sheet, &client, seq, &ops, &link.account)? {
        app.push(&link.club, json!({ "type": "ops", "id": link.sheet, "rev": rev, "clientId": client, "ops": ops }));
        crate::uploader::schedule(app, &link.club, &link.sheet);
        tracing::info!(club = %link.club, sheet = %link.sheet, game = link.game, "{what}");
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

pub async fn save_device(State(app): State<Arc<App>>, u: User, Json(b): Json<DeviceBody>) -> Result<StatusCode, ApiError> {
    if !valid_id(b.id.trim()) || b.alias.len() > 80 {
        return Err(ApiError::BadRequest("Ongeldig ID: gebruik letters, cijfers, - _ . of :".into()));
    }
    app.db.save_device(u.club()?, b.id.trim(), b.alias.trim())?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct IdQuery {
    id: String,
}

pub async fn delete_device(State(app): State<Arc<App>>, u: User, Query(q): Query<IdQuery>) -> Result<StatusCode, ApiError> {
    app.db.delete_device(u.club()?, &q.id)?;
    Ok(StatusCode::NO_CONTENT)
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
        return Err(ApiError::BadRequest("De server heeft geen MQTT-verbinding (MQTT_HOST).".into()));
    }
    if !app.db.has_device(club, &b.device)? {
        return Err(ApiError::NotFound(format!("Scorebord {} hoort niet bij deze club", b.device)));
    }
    let sheet = app.db.sheet(club, &b.sheet)?.ok_or_else(|| ApiError::NotFound(format!("Wedstrijd {} bestaat niet", b.sheet)))?;
    if b.game < 0 || b.game as usize >= sheet.doc["results"].as_array().map(|a| a.len()).unwrap_or(0) {
        return Err(ApiError::BadRequest("Ongeldige wedstrijd".into()));
    }
    // Before anything goes to the board: names on a board that cannot count the match
    // would only mislead the players.
    board_can_count(&sheet.doc).map_err(ApiError::BadRequest)?;
    app.boards.send_names(&b.device, &b.player1, &b.player2).await.map_err(|e| ApiError::BadRequest(format!("Scorebord: {e}")))?;
    // The away player (player 2) always serves first.
    app.boards.start_match(&b.device, AWAY_SERVES, best_of(&sheet.doc, b.game)).await.map_err(|e| ApiError::BadRequest(format!("Scorebord: {e}")))?;
    app.db.link_board(&Link {
        device: b.device.clone(),
        club: club.to_string(),
        sheet: b.sheet.clone(),
        game: b.game,
        account: u.account.account.clone(),
        board_match: None,
        old_match: None,
    })?;
    let state = BoardState {
        device: b.device.clone(),
        sheet: Some(b.sheet),
        game: Some(b.game),
        current: (0, 0),
        games: vec![],
        server: AWAY_SERVES,
        first_server: AWAY_SERVES,
        points: String::new(),
        duration_ms: 0,
        timeout: None,
        finished: false,
        at: now_ms(),
    };
    app.boards.set_state(state.clone());
    app.db.save_board_state(&state)?;
    app.push(club, json!({ "type": "board", "device": b.device, "state": state }));
    Ok(Json(state))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(m: Option<&str>) -> Link {
        Link { device: "b1".into(), club: "W1".into(), sheet: "S/1".into(), game: 3, account: "jan".into(), board_match: m.map(String::from), old_match: None }
    }

    #[test]
    fn a_board_counts_only_sets_to_11() {
        // the current systems: sets to 11
        for id in [2, 4, 5, 6, 9, -2] {
            assert_eq!(board_can_count(&json!({ "matchSystemId": id })), Ok(()), "system {id}");
        }
        // the old systems 1 and 3: sets to 21
        for id in [1, 3] {
            let e = board_can_count(&json!({ "matchSystemId": id })).unwrap_err();
            assert!(e.contains("21 punten") && e.contains("11 punten"), "{e}");
        }
        // no system, or one the server does not know: not refused
        assert_eq!(board_can_count(&json!({})), Ok(()));
        assert_eq!(board_can_count(&json!({ "matchSystemId": 999 })), Ok(()));
        // every system of the server is covered by one of the two cases
        for id in -5..40 {
            if let Some(sys) = crate::rules::system(id) {
                assert_eq!(board_can_count(&json!({ "matchSystemId": id })).is_ok(), sys.point_count == 11, "system {id}");
            }
        }
    }

    #[test]
    fn best_of_comes_from_the_match_system() {
        // system 2: 3 sets to win = best of 5; system -2: 4 sets = best of 7; system 1: 2 sets = best of 3
        assert_eq!(best_of(&json!({ "matchSystemId": 2 }), 0), 5);
        assert_eq!(best_of(&json!({ "matchSystemId": -2 }), 0), 7);
        assert_eq!(best_of(&json!({ "matchSystemId": 1 }), 0), 3);
        // the system wins over the boxes on the sheet
        assert_eq!(best_of(&json!({ "matchSystemId": 2, "results": [[null, null, null]] }), 0), 5);
        // unknown system: the boxes of that game, else best of 5
        assert_eq!(best_of(&json!({ "matchSystemId": 999, "results": [null, [null, null, null]] }), 1), 3);
        assert_eq!(best_of(&json!({ "results": [[null]] }), 0), 1);
        assert_eq!(best_of(&json!({ "results": [null] }), 0), 5);
        assert_eq!(best_of(&json!({}), 0), 5);
        // a format no board can play falls back to best of 5
        assert_eq!(best_of(&json!({ "results": [[null, null]] }), 0), 5);
        assert_eq!(best_of(&json!({ "results": [[null, null, null, null, null, null, null, null, null]] }), 0), 5);
    }

    #[test]
    fn topics() {
        assert_eq!(parse_topic("tabletennis/b1/m9/update"), Some(("b1", "m9", "update")));
        assert_eq!(parse_topic("tabletennis/b1/update"), Some(("b1", "", "update")));
        assert_eq!(parse_topic("tabletennis/b1/config"), Some(("b1", "", "config")));
        assert_eq!(parse_topic("other/b1/update"), None);
    }

    #[test]
    fn config_expires_after_two_hours() {
        assert_eq!(config_properties().message_expiry_interval, Some(7200));
    }

    #[test]
    fn only_the_linked_board_match_counts() {
        assert_eq!(decide(None, "m1"), Decision::Ignore);
        assert_eq!(decide(Some(&link(None)), "m1"), Decision::Show { fix_match: true });
        assert_eq!(decide(Some(&link(Some("m1"))), "m1"), Decision::Show { fix_match: false });
        assert_eq!(decide(Some(&link(Some("m1"))), "m2"), Decision::Ignore);
    }

    #[test]
    fn a_new_link_ignores_the_old_board_match() {
        let db = db::memory();
        db.save_device("W1", "b1", "Tafel 7").unwrap();
        db.link_board(&link(None)).unwrap();
        db.set_board_match("b1", "m1").unwrap();
        // The next game goes to the board. The board repeats the end of m1: not for this game.
        db.link_board(&Link { game: 4, ..link(None) }).unwrap();
        let l = db.board_link("b1").unwrap().unwrap();
        assert_eq!((l.board_match.as_deref(), l.old_match.as_deref()), (None, Some("m1")));
        assert_eq!(decide(Some(&l), "m1"), Decision::Ignore);
        assert_eq!(decide(Some(&l), "m2"), Decision::Show { fix_match: true });
        // Sent again before the board started a new match: m1 stays the old match.
        db.link_board(&Link { game: 4, ..link(None) }).unwrap();
        let l = db.board_link("b1").unwrap().unwrap();
        assert_eq!(decide(Some(&l), "m1"), Decision::Ignore);
        // Old firmware has no match id in the topic: every update counts, as before.
        let old = Link { old_match: Some(String::new()), ..link(None) };
        assert_eq!(decide(Some(&old), ""), Decision::Show { fix_match: true });
    }

    fn played(games: &[(u8, u8)], points: &str, finished: bool) -> MatchState {
        MatchState { duration_ms: 0, best_of: 5, point_count: 0, games: games.to_vec(), current: (0, 0), games_won: (0, 0), server: 1, first_server: 2, points: points.into(), finished, timeout: None }
    }
    fn shown(s: &MatchState) -> BoardState {
        BoardState { device: "b1".into(), sheet: Some("S/1".into()), game: Some(3), current: s.current, games: s.games.clone(), server: 1, first_server: 2, points: s.points.clone(), duration_ms: 0, timeout: None, finished: s.finished, at: 0 }
    }

    #[test]
    fn sets_of_the_record_against_the_board() {
        let games = [(11, 7), (4, 11), (12, 10), (11, 5)];
        let boxes = result_boxes(&played(&games, "", true), 5);
        assert!(same_sets(&boxes, &games));
        // a set that somebody changed, a set less, a set more
        let mut other = boxes.clone();
        other[3] = json!({ "l": "11", "r": "6" });
        assert!(!same_sets(&other, &games));
        assert!(!same_sets(&boxes, &games[..3]));
        assert!(!same_sets(&result_boxes(&played(&games[..3], "", true), 5), &games));
        // no boxes at all, and boxes that are not objects
        assert!(!same_sets(&Value::Null, &games));
        assert!(same_sets(&json!([null, null]), &[]));
    }

    #[test]
    fn an_undo_after_the_end_opens_the_game_again() {
        let end = played(&[(11, 7), (11, 5), (11, 9)], "000", true);
        let wrote = result_boxes(&end, 5);
        let running = played(&[(11, 7), (11, 5)], "00", false);
        // a running game: the end writes, a point only shows
        assert_eq!(effect(None, &running, &Value::Null), Effect::Show);
        assert_eq!(effect(Some(&shown(&running)), &running, &Value::Null), Effect::Show);
        assert_eq!(effect(Some(&shown(&running)), &end, &Value::Null), Effect::Write);
        assert_eq!(effect(None, &end, &Value::Null), Effect::Write);
        // the board repeats its end: nothing to write again
        assert_eq!(effect(Some(&shown(&end)), &end, &wrote), Effect::Show);
        // an undo after the end, then the end with another score
        assert_eq!(effect(Some(&shown(&end)), &running, &wrote), Effect::Reopen);
        let empty = Value::Array(reopen_ops(3, 5)[0].v.as_array().unwrap().clone());
        let end2 = played(&[(11, 7), (11, 5), (12, 10)], "001", true);
        assert_eq!(effect(Some(&shown(&running)), &end2, &empty), Effect::Write);
        // the undo message was lost: the new end follows the old end
        assert_eq!(effect(Some(&shown(&end)), &end2, &wrote), Effect::Write);
        // somebody corrected the result by hand: the board does not touch it
        let mut hand = wrote.clone();
        hand[2] = json!({ "l": "11", "r": "8" });
        assert_eq!(effect(Some(&shown(&end)), &running, &hand), Effect::Detach);
        assert_eq!(effect(Some(&shown(&end)), &end2, &hand), Effect::Detach);
    }

    #[test]
    fn a_game_opened_again_has_no_board_result() {
        let ops = reopen_ops(3, 5);
        assert_eq!(ops[0].p, vec![json!("results"), json!(3)]);
        assert_eq!(ops[0].v, json!([{ "l": "", "r": "" }, { "l": "", "r": "" }, { "l": "", "r": "" }, { "l": "", "r": "" }, { "l": "", "r": "" }]));
        assert_eq!((ops[1].p.clone(), ops[1].v.clone()), (vec![json!("boardPlay"), json!(3)], Value::Null));
    }

    #[test]
    fn boxes_from_board() {
        let s = MatchState { duration_ms: 0, best_of: 5, point_count: 0, games: vec![(11, 7), (4, 11), (12, 10), (11, 5)], current: (0, 0), games_won: (3, 1), server: 1, first_server: 2, points: "01".into(), finished: true, timeout: None };
        let v = result_boxes(&s, 5);
        assert_eq!(v[1], json!({ "l": "4", "r": "11" }));
        assert_eq!(v[4], json!({ "l": "", "r": "" }));
    }

    #[test]
    fn board_play_for_the_record() {
        let mut s = MatchState { duration_ms: 90_000, best_of: 5, point_count: 2, games: vec![], current: (1, 1), games_won: (0, 0), server: 2, first_server: 2, points: "01".into(), finished: false, timeout: None };
        assert_eq!(board_play(&s), json!({ "points": "01", "firstServer": 2, "durationMs": 90_000, "timeoutsUsed": null }));
        s.timeout = Some(Timeout { active: false, owner: 0, p1_used: true, p2_used: false, expired: false, deadline: 0 });
        assert_eq!(board_play(&s)["timeoutsUsed"], json!([true, false]));
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

    #[test]
    fn the_state_of_a_linked_board_survives_a_restart() {
        let db = db::memory();
        let state = BoardState {
            device: "b1".into(),
            sheet: Some("s1".into()),
            game: Some(2),
            current: (5, 3),
            games: vec![(11, 7)],
            server: 1,
            first_server: 2,
            points: "0010".into(),
            duration_ms: 90_000,
            timeout: Some(Timeout { active: false, owner: 0, p1_used: true, p2_used: false, expired: false, deadline: 0 }),
            finished: false,
            at: 7,
        };
        // No link: nothing to keep.
        db.save_board_state(&state).unwrap();
        assert!(db.board_states().unwrap().is_empty());
        db.link_board(&link(None)).unwrap();
        db.save_board_state(&state).unwrap();
        assert_eq!(db.board_states().unwrap(), vec![state.clone()]);
        // A new link starts without a state; the end of the link removes it.
        db.link_board(&link(None)).unwrap();
        assert!(db.board_states().unwrap().is_empty());
        db.save_board_state(&state).unwrap();
        db.unlink_board("b1").unwrap();
        assert!(db.board_states().unwrap().is_empty());
    }
}
