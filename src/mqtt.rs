//! MQTT client for the scoreboards. Keeps the latest state per device and device match,
//! binds device matches to team match positions, and writes results to the sheet.

use crate::app::{App, Event};
use crate::db::Link;
use crate::scoreboard::{self, MatchState};
use anyhow::{Result, bail};
use rumqttc::{AsyncClient, Event as MqEvent, MqttOptions, Packet, QoS, Transport};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
pub struct DeviceMatch {
    pub device_id: String,
    pub mqtt_match_id: String,
    pub state: MatchState,
    /// Team match position this device match belongs to.
    pub match_id: Option<String>,
    pub position: Option<i64>,
    /// Arrival order; the highest is the most recent.
    pub seq: u64,
}

#[derive(Default)]
pub struct Store {
    pub matches: HashMap<(String, String), DeviceMatch>,
    seq: u64,
}

pub struct Mqtt {
    pub client: Option<AsyncClient>,
    pub store: Mutex<Store>,
}

impl Mqtt {
    pub fn snapshot(&self) -> Vec<DeviceMatch> {
        let mut v: Vec<_> = self.store.lock().unwrap().matches.values().cloned().collect();
        v.sort_by(|a, b| (&a.device_id, a.seq).cmp(&(&b.device_id, b.seq)));
        v
    }

    async fn publish(&self, topic: String, retain: bool, payload: Vec<u8>) -> Result<()> {
        let Some(c) = &self.client else { bail!("MQTT is not configured") };
        c.publish(topic, QoS::AtLeastOnce, retain, payload).await?;
        Ok(())
    }

    /// Player 1 must be the home player.
    pub async fn send_names(&self, device_id: &str, p1: (&str, &str), p2: (&str, &str)) -> Result<()> {
        let n = scoreboard::display_name;
        let body = serde_json::json!({
            "player1FirstName": n(p1.0), "player1FamilyName": n(p1.1),
            "player2FirstName": n(p2.0), "player2FamilyName": n(p2.1),
        });
        self.publish(format!("tabletennis/{device_id}/config"), true, body.to_string().into_bytes())
            .await
    }

    /// Starts to listen to one scoreboard. The broker sends its retained state at once.
    pub async fn listen(&self, device_id: &str) -> Result<()> {
        let Some(c) = &self.client else { return Ok(()) };
        c.subscribe(device_topic(device_id), QoS::AtLeastOnce).await?;
        Ok(())
    }

    /// Stops listening to one scoreboard and forgets its device matches.
    /// Returns the forgotten (device, device match) keys.
    pub async fn forget(&self, device_id: &str) -> Result<Vec<(String, String)>> {
        if let Some(c) = &self.client {
            c.unsubscribe(device_topic(device_id)).await?;
        }
        let mut st = self.store.lock().unwrap();
        let keys: Vec<_> = st.matches.keys().filter(|k| k.0 == device_id).cloned().collect();
        for k in &keys {
            st.matches.remove(k);
        }
        Ok(keys)
    }

    /// Removes a retained match from the broker.
    pub async fn clear_match(&self, device_id: &str, mqtt_match_id: &str) -> Result<()> {
        let topic = if mqtt_match_id.is_empty() {
            format!("tabletennis/{device_id}/update")
        } else {
            format!("tabletennis/{device_id}/{mqtt_match_id}/update")
        };
        self.publish(topic, true, vec![]).await
    }
}

fn device_topic(device_id: &str) -> String {
    format!("tabletennis/{device_id}/#")
}

pub struct MqttConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub tls: bool,
}

/// Creates the client. The event loop runs in a background task.
pub fn start(cfg: Option<MqttConfig>) -> (Mqtt, Option<rumqttc::EventLoop>) {
    let Some(cfg) = cfg else {
        tracing::warn!("MQTT_HOST not set: scoreboards disabled");
        return (Mqtt { client: None, store: Default::default() }, None);
    };
    let id = format!("vttl-league-{}", std::process::id());
    let mut o = MqttOptions::new(id, cfg.host, cfg.port);
    o.set_credentials(cfg.user, cfg.password);
    o.set_keep_alive(Duration::from_secs(30));
    o.set_clean_session(true);
    o.set_max_packet_size(64 * 1024, 64 * 1024);
    if cfg.tls {
        o.set_transport(Transport::tls_with_default_config());
    }
    let (client, eventloop) = AsyncClient::new(o, 100);
    (Mqtt { client: Some(client), store: Default::default() }, Some(eventloop))
}

pub async fn run(app: Arc<App>, mut eventloop: rumqttc::EventLoop) {
    loop {
        match eventloop.poll().await {
            Ok(MqEvent::Incoming(Packet::ConnAck(_))) => {
                tracing::info!("MQTT connected");
                // Clean session: subscribe again after every connect. Only devices added by hand.
                match app.db.devices() {
                    Ok(devices) => {
                        for d in devices {
                            if let Err(e) = app.mqtt.listen(&d.device_id).await {
                                tracing::error!("subscribe {}: {e}", d.device_id);
                            }
                        }
                    }
                    Err(e) => tracing::error!("device list: {e}"),
                }
            }
            Ok(MqEvent::Incoming(Packet::Publish(p))) => {
                if let Err(e) = on_publish(&app, &p.topic, &p.payload).await {
                    tracing::warn!("{}: {e}", p.topic);
                }
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!("MQTT: {e}; reconnect in 1 s");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

async fn on_publish(app: &Arc<App>, topic: &str, payload: &[u8]) -> Result<()> {
    let parts: Vec<&str> = topic.split('/').collect();
    if parts.len() < 3 || parts[0] != "tabletennis" || parts[parts.len() - 1] != "update" {
        return Ok(()); // config / command echoes
    }
    let device_id = parts[1].to_string();
    // A message can still arrive just after an unsubscribe.
    if !app.db.devices()?.iter().any(|d| d.device_id == device_id) {
        return Ok(());
    }
    let mqtt_match_id = if parts.len() == 4 { parts[2].to_string() } else { String::new() };
    let key = (device_id.clone(), mqtt_match_id.clone());

    if payload.is_empty() {
        app.mqtt.store.lock().unwrap().matches.remove(&key);
        app.emit(Event::DeviceRemoved { device_id, mqtt_match_id });
        return Ok(());
    }

    let state = scoreboard::decode(payload)?;
    let bound = app.db.attribute(&device_id, &mqtt_match_id)?;

    let dm = {
        let mut st = app.mqtt.store.lock().unwrap();
        st.seq += 1;
        let dm = DeviceMatch {
            device_id: device_id.clone(),
            mqtt_match_id: mqtt_match_id.clone(),
            state: state.clone(),
            match_id: bound.as_ref().map(|b| b.0.clone()),
            position: bound.as_ref().map(|b| b.1),
            seq: st.seq,
        };
        st.matches.insert(key, dm.clone());
        dm
    };

    if let Some((match_id, position)) = bound {
        if !state.games.is_empty() && app.db.save_result_scoreboard(&match_id, position, &state.games)? {
            app.emit(Event::Result { match_id: match_id.clone(), position });
        }
        // A finished match frees the device for the next position.
        if state.finished {
            if let Some(Link { match_id: lm, position: lp, .. }) = app.db.link(&device_id)? {
                if lm == match_id && lp == position {
                    app.db.delete_link(&device_id)?;
                    app.emit(Event::Links);
                }
            }
        }
    }
    app.emit(Event::Device(Box::new(dm)));
    Ok(())
}
