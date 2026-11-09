//! Shared state.

use crate::boards::Boards;
use crate::crypto::Crypto;
use crate::db::Db;
use crate::tabt::Tabt;
use tokio::sync::broadcast;

pub struct App {
    pub tabt: Tabt,
    pub db: Db,
    pub crypto: Crypto,
    /// Send the session cookie with the `Secure` flag (off only for local http).
    pub cookie_secure: bool,
    /// Real uploads to VTTL. Off (dry run) unless UPLOAD_ENABLED=true: VTTL has no test database.
    pub upload_enabled: bool,
    /// Changes for the browsers: (club, JSON message). Each WebSocket keeps its own club.
    pub events: broadcast::Sender<(String, String)>,
    /// Scoreboards: MQTT connection and last state per board.
    pub boards: Boards,
    /// Server-side uploads to VTTL (debounce and lock per match).
    pub uploads: crate::uploader::Uploads,
}

impl App {
    pub fn push(&self, club: &str, msg: serde_json::Value) {
        let _ = self.events.send((club.to_string(), msg.to_string()));
    }
}
