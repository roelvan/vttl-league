mod api;
mod app;
mod auth;
mod boards;
mod crypto;
mod public;
mod rules;
mod db;
mod scoreboard;
mod sheets;
mod tabt;
mod uploader;
mod xml;

use app::App;
use axum::extract::Request;
use axum::http::{HeaderValue, header};
use axum::middleware::{self, Next};
use axum::response::Response;
use std::env::var;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::compression::CompressionLayer;
use tower_http::services::{ServeDir, ServeFile};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let bind = var("BIND").unwrap_or_else(|_| "127.0.0.1:3100".into());
    let database = var("DATABASE").unwrap_or_else(|_| "data/vttl-league.sqlite".into());
    let data_dir = PathBuf::from(&database).parent().map(PathBuf::from).unwrap_or_default();
    std::fs::create_dir_all(&data_dir).ok();
    let local = bind.starts_with("127.0.0.1") || bind.starts_with("localhost");
    let web_dir = PathBuf::from(var("WEB_DIR").unwrap_or_else(|_| "web/dist".into()));

    let (boards, mqtt) = boards::Boards::connect();
    let app = Arc::new(App {
        tabt: tabt::Tabt::new(var("VTTL_ENDPOINT").unwrap_or_else(|_| "https://api.vttl.be/0.7/index.php?s=vttl".into())),
        db: db::Db::open(&database)?,
        crypto: crypto::Crypto::load(var("SECRET_KEY").ok(), &data_dir.join("secret.key"))?,
        cookie_secure: var("COOKIE_SECURE").map(|v| v != "false").unwrap_or(!local),
        events: tokio::sync::broadcast::channel(1024).0,
        upload_enabled: var("UPLOAD_ENABLED").is_ok_and(|v| v == "true"),
        boards,
        uploads: uploader::Uploads::default(),
    });
    if let Some(eventloop) = mqtt {
        tokio::spawn(boards::run(app.clone(), eventloop));
    }
    if app.upload_enabled {
        tracing::warn!("UPLOAD_ENABLED=true: uploads change the LIVE VTTL database");
    }

    // The client is a single-page app: unknown paths get index.html.
    let static_files = ServeDir::new(&web_dir).fallback(ServeFile::new(web_dir.join("index.html")));
    let router = api::router(app)
        .fallback_service(static_files)
        .layer(middleware::from_fn(cache_headers))
        .layer(CompressionLayer::new().br(true).gzip(true));

    tracing::info!("listening on http://{bind}");
    axum::serve(tokio::net::TcpListener::bind(&bind).await?, router).await?;
    Ok(())
}

/// Hashed build files never change, so the browser may keep them for a year.
/// Everything else (index.html, the service worker, the API) must be checked each time.
async fn cache_headers(req: Request, next: Next) -> Response {
    let immutable = req.uri().path().starts_with("/assets/");
    let mut res = next.run(req).await;
    // Marks answers from this server. The service worker uses it to tell our pages from a
    // captive-portal login page that the hall WiFi sends instead.
    res.headers_mut().insert("x-vl-app", HeaderValue::from_static("1"));
    if !res.headers().contains_key(header::CACHE_CONTROL) {
        let v = if immutable { "public, max-age=31536000, immutable" } else { "no-cache" };
        res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static(v));
    }
    res
}
