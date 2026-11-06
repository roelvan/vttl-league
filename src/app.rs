//! Shared state and the change events that are pushed to every browser.

use crate::db::Db;
use crate::mqtt::{DeviceMatch, Mqtt};
use crate::vttl::Vttl;
use serde::Serialize;
use tokio::sync::broadcast;

pub struct Config {
    pub club: String,
    pub dry_run: bool,
}

pub struct App {
    pub cfg: Config,
    pub vttl: Vttl,
    pub db: Db,
    pub mqtt: Mqtt,
    pub events: broadcast::Sender<String>,
}

/// Every change goes to every browser. A browser reloads what it shows when an
/// event touches it, so no one works on stale data.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Device(Box<DeviceMatch>),
    DeviceRemoved { device_id: String, mqtt_match_id: String },
    Lineup { match_id: String, side: String },
    Result { match_id: String, position: i64 },
    Links,
    /// The list of devices added by hand changed.
    Devices,
}

impl App {
    pub fn emit(&self, e: Event) {
        let _ = self.events.send(serde_json::to_string(&e).unwrap());
    }
}
