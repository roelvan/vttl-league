mod api;
mod app;
mod db;
mod mqtt;
mod scoreboard;
mod upload;
mod vttl;
mod xml;

use app::{App, Config};
use std::env::var;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let account = match (var("VTTL_ACCOUNT"), var("VTTL_PASSWORD")) {
        (Ok(a), Ok(p)) if !a.is_empty() => Some((a, p)),
        _ => None,
    };
    let mqtt_cfg = var("MQTT_HOST").ok().filter(|h| !h.is_empty()).map(|host| mqtt::MqttConfig {
        host,
        port: var("MQTT_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8883),
        user: var("MQTT_USER").unwrap_or_default(),
        password: var("MQTT_PASSWORD").unwrap_or_default(),
        tls: var("MQTT_TLS").map(|v| v != "false").unwrap_or(true),
    });
    let (mqtt, eventloop) = mqtt::start(mqtt_cfg);
    let dry_run = var("DRY_RUN").map(|v| v != "false").unwrap_or(true);
    if !dry_run {
        tracing::warn!("DRY_RUN=false: uploads change the LIVE VTTL database");
    }

    let app = Arc::new(App {
        cfg: Config { club: var("CLUB").unwrap_or_else(|_| "WVL154".into()), dry_run },
        vttl: vttl::Vttl::new(
            var("VTTL_ENDPOINT").unwrap_or_else(|_| "https://api.vttl.be/0.7/index.php?s=vttl".into()),
            account,
        ),
        db: db::Db::open(&var("DATABASE").unwrap_or_else(|_| "vttl-league.sqlite".into()))?,
        mqtt,
        events: tokio::sync::broadcast::channel(256).0,
    });
    if let Some(el) = eventloop {
        tokio::spawn(mqtt::run(app.clone(), el));
    }

    let router = api::router(app).fallback_service(tower_http::services::ServeDir::new("static"));
    let bind = var("BIND").unwrap_or_else(|_| "127.0.0.1:3100".into());
    tracing::info!("listening on http://{bind}");
    axum::serve(tokio::net::TcpListener::bind(&bind).await?, router).await?;
    Ok(())
}
