//! HTTP API. All JSON is camelCase. Errors are `{ "error": code, "message": text }`.

use crate::app::App;
use crate::auth::{self, User};
use crate::boards;
use crate::public;
use crate::crypto;
use crate::db::{self, Account};
use crate::sheets;
use crate::tabt::{self, Creds, TeamMatch};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

// ---------------------------------------------------------------- errors

#[derive(Debug)]
pub enum ApiError {
    Unauthorized,
    /// The account has not picked "this is me" yet, so it has no club.
    NoMember,
    BadRequest(String),
    NotFound(String),
    TooManyRequests,
    /// VTTL is down or returned a fault.
    Vttl(String),
    Internal(String),
}

impl ApiError {
    fn vttl(e: anyhow::Error) -> Self {
        tracing::warn!("vttl: {e:#}");
        ApiError::Vttl(e.to_string())
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!("{e:#}");
        ApiError::Internal("interne fout".into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, msg) = match self {
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized", "Niet aangemeld".to_string()),
            ApiError::NoMember => (StatusCode::CONFLICT, "no_member", "Kies eerst wie je bent".to_string()),
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, "bad_request", m),
            ApiError::NotFound(m) => (StatusCode::NOT_FOUND, "not_found", m),
            ApiError::TooManyRequests => {
                (StatusCode::TOO_MANY_REQUESTS, "too_many_requests", "Te veel pogingen. Wacht 15 minuten.".to_string())
            }
            ApiError::Vttl(m) => (StatusCode::BAD_GATEWAY, "vttl", m),
            ApiError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, "internal", m),
        };
        (status, Json(json!({ "error": code, "message": msg }))).into_response()
    }
}

type R<T> = Result<Json<T>, ApiError>;

// ---------------------------------------------------------------- router

pub fn router(app: Arc<App>) -> Router {
    Router::new()
        .route("/api/health", get(|| async { "ok" }))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/me", get(me))
        .route("/api/me/member", post(set_member))
        .route("/api/members/search", get(search_members))
        .route("/api/player", get(player))
        .route("/api/players/warm", post(warm_players))
        .route("/api/seasons", get(seasons))
        .route("/api/club", get(club))
        .route("/api/club/matches", get(club_matches))
        .route("/api/club/members", get(club_members))
        .route("/api/match", get(match_detail))
        .route("/api/clubs", get(clubs))
        .route("/api/player-categories", get(player_categories))
        .route("/api/match-systems", get(match_systems))
        .route("/api/sheets", get(sheets::list))
        .route("/api/sheet", get(sheets::get_one).put(sheets::create))
        .route("/api/sheet/ops", post(sheets::ops))
        .route("/api/ws", get(sheets::ws))
        .route("/api/upload", post(upload))
        .route("/api/uploads", get(upload_log))
        .route("/api/devices", get(boards::list_devices).put(boards::save_device).delete(boards::delete_device))
        .route("/api/boards", get(boards::states))
        .route("/api/boards/link", post(boards::link))
        .route("/api/public/clubs", get(public::clubs))
        .route("/api/public/club", get(public::club))
        .route("/api/public/division", get(public::division))
        .route("/api/public/rankings", get(public::rankings))
        .route("/api/public/sheets", get(public::sheets))
        .route("/api/public/sheet", get(public::sheet))
        .route("/api/public/match", get(public::team_match))
        .route("/api/public/player", get(public::player))
        .route("/api/public/players/warm", post(public::warm_players))
        .route("/api/public/boards", get(public::boards))
        .route("/api/public/ws", get(public::ws))
        .with_state(app)
}

// ---------------------------------------------------------------- login

#[derive(Deserialize)]
struct LoginBody {
    account: String,
    password: String,
}

/// Failed logins per account name, to stop password guessing through this server.
static FAILURES: Mutex<Option<HashMap<String, Vec<Instant>>>> = Mutex::new(None);
const MAX_FAILURES: usize = 10;
const FAILURE_WINDOW_SECS: u64 = 15 * 60;

fn failures(account: &str) -> usize {
    let mut g = FAILURES.lock().unwrap();
    let m = g.get_or_insert_with(HashMap::new);
    let v = m.entry(account.to_lowercase()).or_default();
    v.retain(|t| t.elapsed().as_secs() < FAILURE_WINDOW_SECS);
    v.len()
}

fn add_failure(account: &str) {
    let mut g = FAILURES.lock().unwrap();
    g.get_or_insert_with(HashMap::new).entry(account.to_lowercase()).or_default().push(Instant::now());
}

async fn login(State(app): State<Arc<App>>, Json(b): Json<LoginBody>) -> Result<Response, ApiError> {
    let account = b.account.trim().to_string();
    if account.is_empty() || b.password.is_empty() {
        return Err(ApiError::BadRequest("Geef je VTTL-login en wachtwoord".into()));
    }
    if failures(&account) >= MAX_FAILURES {
        return Err(ApiError::TooManyRequests);
    }
    let creds = Creds { account: account.clone(), password: b.password };
    let t = app.tabt.test(&creds).await.map_err(ApiError::vttl)?;
    if !t.is_valid_account {
        add_failure(&account);
        return Err(ApiError::Unauthorized);
    }
    let (token, hash) = crypto::new_token();
    let acc = app.db.create_session(&account, &hash, &app.crypto.encrypt(&creds.password))?;
    tracing::info!(account = %account, "login");
    let body = me_body(&app, &creds, &acc).await;
    Ok(([auth::set_cookie(&app, &token, db::SESSION_IDLE)], Json(body)).into_response())
}

async fn logout(State(app): State<Arc<App>>, headers: HeaderMap) -> Result<Response, ApiError> {
    if let Some(t) = auth::token_from(&headers) {
        app.db.delete_session(&crypto::token_hash(&t))?;
    }
    Ok(([auth::set_cookie(&app, "", 0)], StatusCode::NO_CONTENT).into_response())
}

async fn me_body(app: &App, creds: &Creds, acc: &Account) -> Value {
    let season = app.tabt.current_season(Some(creds)).await.ok();
    let club = match (&acc.club, season) {
        (Some(c), Some(s)) => app.tabt.club(Some(creds), s, c).await.ok().flatten(),
        _ => None,
    };
    json!({ "account": acc, "club": club, "currentSeason": season, "uploadEnabled": app.upload_enabled })
}

async fn me(State(app): State<Arc<App>>, u: User) -> R<Value> {
    Ok(Json(me_body(&app, &u.creds, &u.account).await))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemberBody {
    unique_index: Option<i64>,
}

async fn set_member(State(app): State<Arc<App>>, u: User, Json(b): Json<MemberBody>) -> R<Value> {
    let acc = match b.unique_index {
        None => app.db.set_member(&u.account.account, None)?,
        Some(idx) => {
            let m = app
                .tabt
                .member(&u.creds, idx)
                .await
                .map_err(ApiError::vttl)?
                .ok_or_else(|| ApiError::NotFound(format!("Lid {idx} niet gevonden")))?;
            let name = format!("{} {}", m.last_name, title_case(&m.first_name));
            app.db.set_member(&u.account.account, Some((idx, &name, &m.club)))?
        }
    };
    Ok(Json(me_body(&app, &u.creds, &acc).await))
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
}

async fn search_members(State(app): State<Arc<App>>, u: User, Query(q): Query<SearchQuery>) -> R<Vec<tabt::Member>> {
    let q = q.q.trim();
    if q.chars().count() < 3 {
        return Err(ApiError::BadRequest("Geef minstens 3 letters".into()));
    }
    let mut v = app.tabt.search_members(&u.creds, q).await.map_err(ApiError::vttl)?;
    v.truncate(100);
    Ok(Json(v))
}

// ---------------------------------------------------------------- players

#[derive(Deserialize)]
pub struct PlayerQuery {
    pub id: i64,
}

/// One player for the player widget. The answer is at most a day old (`tabt::TTL_PLAYER`);
/// an older one comes at once and a refresh runs in the background.
async fn player(State(app): State<Arc<App>>, u: User, Query(q): Query<PlayerQuery>) -> R<tabt::PlayerInfo> {
    if q.id <= 0 {
        return Err(ApiError::BadRequest("Ongeldige speler".into()));
    }
    app.tabt
        .player(Some(&u.creds), q.id)
        .await
        .map_err(ApiError::vttl)?
        .map(Json)
        .ok_or_else(|| ApiError::NotFound(format!("Speler {} niet gevonden", q.id)))
}

#[derive(Deserialize)]
struct WarmBody {
    ids: Vec<i64>,
}

/// The players of one match, with room for substitutes.
pub const WARM_MAX: usize = 16;
/// Between two player calls to VTTL.
const WARM_PAUSE: std::time::Duration = std::time::Duration::from_millis(500);
/// One warm-up at a time for the whole server: VTTL gets the player calls in sequence.
static WARMING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// One player for a spectator, with the guest session of VTTL. A player that the cache does
/// not have waits for its turn after the warm-ups, and the pause comes after it too: all
/// spectators together cannot make this server call VTTL faster than the warm-up does.
pub async fn queued_player(app: &App, id: i64) -> anyhow::Result<Option<tabt::PlayerInfo>> {
    if app.tabt.has_player(id) {
        // Fresh, or old: then the cache gives it at once and starts one refresh.
        return app.tabt.player(None, id).await;
    }
    let one = WARMING.lock().await;
    // A warm-up, or another spectator, got it while this one waited.
    if app.tabt.has_player(id) {
        return app.tabt.player(None, id).await;
    }
    let r = app.tabt.player(None, id).await;
    tokio::spawn(async move {
        tokio::time::sleep(WARM_PAUSE).await;
        drop(one);
    });
    r
}

/// A user opened a match: get its players in the background, so the player widget opens
/// at once. A player with a fresh answer costs no call.
async fn warm_players(State(app): State<Arc<App>>, u: User, Json(b): Json<WarmBody>) -> StatusCode {
    warm(app, Some(u.creds), b.ids.into_iter().filter(|id| *id > 0).take(WARM_MAX).collect());
    StatusCode::NO_CONTENT
}

/// Gets the players that have no fresh answer, in the background and in sequence.
/// `creds`: none for a spectator (the guest session of VTTL).
pub fn warm(app: Arc<App>, creds: Option<tabt::Creds>, ids: Vec<i64>) {
    let ids: Vec<i64> = ids.into_iter().take(WARM_MAX).filter(|id| !app.tabt.player_is_fresh(*id)).collect();
    if ids.is_empty() {
        return;
    }
    tokio::spawn(async move {
        let _one = WARMING.lock().await;
        for id in ids {
            // Another warm-up got it while this one waited.
            if app.tabt.player_is_fresh(id) {
                continue;
            }
            if let Err(e) = app.tabt.player(creds.as_ref(), id).await {
                tracing::warn!(id, "player warm-up: {e:#}");
            }
            tokio::time::sleep(WARM_PAUSE).await;
        }
    });
}

// ---------------------------------------------------------------- data

#[derive(Deserialize)]
struct SeasonQuery {
    season: Option<i64>,
}

async fn season_or_current(app: &App, u: &User, s: Option<i64>) -> Result<i64, ApiError> {
    match s {
        Some(s) => Ok(s),
        None => app.tabt.current_season(Some(&u.creds)).await.map_err(ApiError::vttl),
    }
}

async fn seasons(State(app): State<Arc<App>>, u: User) -> R<Vec<tabt::Season>> {
    Ok(Json(app.tabt.seasons(&u.creds).await.map_err(ApiError::vttl)?))
}

async fn club(State(app): State<Arc<App>>, u: User, Query(q): Query<SeasonQuery>) -> R<Value> {
    let season = season_or_current(&app, &u, q.season).await?;
    let club = u.club()?;
    let (c, teams) = tokio::try_join!(app.tabt.club(Some(&u.creds), season, club), app.tabt.teams(Some(&u.creds), season, club))
        .map_err(ApiError::vttl)?;
    Ok(Json(json!({ "club": c, "teams": teams, "season": season })))
}

async fn clubs(State(app): State<Arc<App>>, u: User, Query(q): Query<SeasonQuery>) -> R<Vec<tabt::Club>> {
    let season = season_or_current(&app, &u, q.season).await?;
    Ok(Json(app.tabt.clubs(Some(&u.creds), season).await.map_err(ApiError::vttl)?))
}

/// A row in the match list: a match without the details, plus what the list needs.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MatchRow<'a> {
    #[serde(flatten)]
    m: MatchHead<'a>,
    is_home: bool,
    team_letter: Option<&'a str>,
    division_name: Option<&'a str>,
    match_type: Option<i64>,
    match_system: Option<i64>,
    /// TabT division level (1 national, 6 super, 15 regional VTTL, 7-10 provinces...).
    division_level: Option<i64>,
    /// Winner per game in VTTL order: "h", "a", or "-" when not played. None without results.
    games: Option<String>,
    /// A match of the division without a team of the club (the division page only).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    neutral: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MatchHead<'a> {
    match_id: &'a str,
    unique_id: i64,
    week: &'a str,
    date: &'a str,
    time: &'a str,
    venue: &'a Option<tabt::Venue>,
    home_club: &'a str,
    home_team: &'a str,
    away_club: &'a str,
    away_team: &'a str,
    score: &'a str,
    division_id: i64,
    division_category: i64,
    is_home_forfeited: bool,
    is_away_forfeited: bool,
    is_validated: bool,
    is_locked: bool,
    is_bye: bool,
}

fn head(m: &TeamMatch) -> MatchHead<'_> {
    MatchHead {
        match_id: &m.match_id,
        unique_id: m.unique_id,
        week: &m.week,
        date: &m.date,
        time: &m.time,
        venue: &m.venue,
        home_club: &m.home_club,
        home_team: &m.home_team,
        away_club: &m.away_club,
        away_team: &m.away_team,
        score: &m.score,
        division_id: m.division_id,
        division_category: m.division_category,
        is_home_forfeited: m.is_home_forfeited,
        is_away_forfeited: m.is_away_forfeited,
        is_validated: m.is_validated,
        is_locked: m.is_locked,
        is_bye: m.is_bye,
    }
}

/// "haah-…": the winner of each individual game, ordered by position.
fn game_winners(d: &tabt::MatchDetails) -> Option<String> {
    if d.results.is_empty() {
        return None;
    }
    let mut r: Vec<&tabt::IndividualResult> = d.results.iter().collect();
    r.sort_by_key(|g| g.position);
    Some(
        r.iter()
            .map(|g| match (g.is_home_forfeited, g.is_away_forfeited) {
                (false, true) => 'h',
                (true, false) => 'a',
                _ if g.home_sets > g.away_sets => 'h',
                _ if g.away_sets > g.home_sets => 'a',
                _ => '-',
            })
            .collect(),
    )
}

/// "Vl-B234" and "VLB234" are the same club.
pub fn same_club(a: &str, b: &str) -> bool {
    let n = |s: &str| s.chars().filter(|c| *c != '-').collect::<String>().to_uppercase();
    n(a) == n(b)
}

/// The team letter of our club in a match: "Zonnebeke A" -> "A".
fn team_letter(team_name: &str) -> &str {
    team_name.rsplit(' ').next().unwrap_or("")
}

async fn club_matches(State(app): State<Arc<App>>, u: User, Query(q): Query<SeasonQuery>) -> Result<Response, ApiError> {
    let season = season_or_current(&app, &u, q.season).await?;
    Ok(Json(match_list(&app, Some(&u.creds), season, u.club()?).await?).into_response())
}

/// The season of a club as the client's `MatchList`. Also used without login (public pages).
pub async fn match_list(app: &App, creds: Option<&Creds>, season: i64, club: &str) -> Result<Value, ApiError> {
    let (matches, teams, divisions) = tokio::try_join!(
        app.tabt.club_matches(creds, season, club),
        app.tabt.teams(creds, season, club),
        app.tabt.divisions(creds, season)
    )
    .map_err(ApiError::vttl)?;
    let rows = match_rows(&matches.iter().collect::<Vec<_>>(), &teams, &divisions, club);
    Ok(json!({ "season": season, "teams": teams, "matches": rows }))
}

/// The rows of the match list (the client's `MatchRow`) for matches of a club. Also the
/// content of the "vttl" push, when the poller finds new data.
pub fn match_rows(matches: &[&TeamMatch], teams: &[tabt::Team], divisions: &[tabt::Division], club: &str) -> Value {
    let levels: HashMap<i64, i64> = divisions.iter().map(|d| (d.id, d.level)).collect();
    // VTTL lists a match twice when two teams of the club play each other.
    let mut seen = std::collections::HashSet::new();
    let rows: Vec<MatchRow> = matches
        .iter()
        .filter(|m| seen.insert(m.match_id.clone()))
        .map(|m| {
            let is_home = same_club(&m.home_club, club);
            let letter = team_letter(if is_home { &m.home_team } else { &m.away_team });
            let team = teams.iter().find(|t| t.division_id == m.division_id && t.letter == letter);
            MatchRow {
                m: head(m),
                is_home,
                team_letter: team.map(|t| t.letter.as_str()),
                division_name: team.map(|t| t.division_name.as_str()),
                match_type: team.map(|t| t.match_type),
                match_system: m.details.as_ref().map(|d| d.match_system).filter(|s| *s != 0),
                division_level: levels.get(&m.division_id).copied(),
                games: m.details.as_ref().and_then(game_winners),
                neutral: false,
            }
        })
        .collect();
    json!(rows)
}

/// The rows of the division page: all matches of a division, seen from `club`. A match
/// without a team of the club is `neutral`.
pub fn division_rows(matches: &[TeamMatch], division: &tabt::Division, club: &str) -> Value {
    let rows: Vec<MatchRow> = matches
        .iter()
        .map(|m| {
            let is_home = same_club(&m.home_club, club);
            let neutral = !is_home && !same_club(&m.away_club, club);
            MatchRow {
                m: head(m),
                is_home,
                team_letter: (!neutral).then(|| team_letter(if is_home { &m.home_team } else { &m.away_team })),
                division_name: Some(&division.name),
                match_type: Some(division.match_type),
                match_system: m.details.as_ref().map(|d| d.match_system).filter(|s| *s != 0),
                division_level: Some(division.level),
                games: m.details.as_ref().and_then(game_winners),
                neutral,
            }
        })
        .collect();
    json!(rows)
}

#[derive(Deserialize)]
struct MatchQuery {
    season: Option<i64>,
    /// The VTTL MatchId, e.g. "PWVLH07/012". Played and future matches both have one;
    /// MatchUniqueId only exists once a match is played.
    id: String,
}

/// A match of VTTL with the age of its data. The server reads VTTL through a cache, so the
/// answer can be older than the last upload of the match: the browser must not take such an
/// answer for a change at VTTL (web/src/lib/sheet/vttl.ts `olderThanUpload`).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MatchDetail {
    #[serde(flatten)]
    m: TeamMatch,
    /// When this data came from VTTL (Unix ms). None when the cache does not tell.
    fetched_at: Option<i64>,
}

async fn match_detail(State(app): State<Arc<App>>, u: User, Query(q): Query<MatchQuery>) -> R<MatchDetail> {
    let season = season_or_current(&app, &u, q.season).await?;
    let club = u.club()?;
    let matches = app.tabt.club_matches(Some(&u.creds), season, club).await.map_err(ApiError::vttl)?;
    let m = matches
        .into_iter()
        .find(|m| m.match_id == q.id)
        .ok_or_else(|| ApiError::NotFound(format!("Wedstrijd {} niet gevonden", q.id)))?;
    let fetched_at = app.tabt.match_fetched_at(season, club, &m);
    Ok(Json(MatchDetail { m, fetched_at }))
}

#[derive(Deserialize)]
struct MembersQuery {
    season: Option<i64>,
    club: Option<String>,
    category: i64,
}

async fn club_members(State(app): State<Arc<App>>, u: User, Query(q): Query<MembersQuery>) -> R<Vec<tabt::Member>> {
    let season = season_or_current(&app, &u, q.season).await?;
    let club = match &q.club {
        Some(c) => c.as_str(),
        None => u.club()?,
    };
    let members = app.tabt.members(&u.creds, season, club, q.category).await.map_err(ApiError::vttl)?;
    Ok(Json(active_members(members)))
}

/// Same as ScoreSheet (TabTUpdater.FixMembers): keep the statuses that may play and renumber
/// the strength-list positions after the filter. If nobody is left, keep the list as it is.
fn active_members(members: Vec<tabt::Member>) -> Vec<tabt::Member> {
    let mut active: Vec<tabt::Member> = members
        .iter()
        .filter(|m| matches!(m.status.as_str(), "A" | "V" | "S" | "T" | "O" | "P" | "D"))
        .cloned()
        .collect();
    if active.is_empty() {
        return members;
    }
    for (i, m) in active.iter_mut().enumerate() {
        m.position = i as i64 + 1;
    }
    active
}

async fn player_categories(State(app): State<Arc<App>>, u: User, Query(q): Query<SeasonQuery>) -> R<Vec<tabt::PlayerCategory>> {
    let season = season_or_current(&app, &u, q.season).await?;
    Ok(Json(app.tabt.player_categories(&u.creds, season).await.map_err(ApiError::vttl)?))
}

async fn match_systems(State(app): State<Arc<App>>, u: User) -> R<Vec<tabt::MatchSystem>> {
    Ok(Json(app.tabt.match_systems(&u.creds).await.map_err(ApiError::vttl)?))
}

#[derive(Deserialize)]
struct UploadBody {
    /// The match (sheet) id. The server builds the upload lines from the stored match.
    id: String,
}

/// A manual upload (retry or correction). The server also uploads by itself after every change
/// (src/uploader.rs); this one tries the login of the user first and sends even when nothing changed.
async fn upload(State(app): State<Arc<App>>, u: User, Json(b): Json<UploadBody>) -> R<Value> {
    let club = u.club()?.to_string();
    let o = crate::uploader::upload(&app, &club, &b.id, crate::uploader::Source::Manual, Some(u.creds.clone())).await?;
    Ok(Json(match o {
        Some(o) => json!(o),
        None => json!({ "ok": false, "dryRun": !app.upload_enabled, "partial": false, "data": "",
            "errors": ["Er is niets te uploaden: geen officiële competitiewedstrijd, al gevalideerd, of er loopt al een upload."] }),
    }))
}

#[derive(Deserialize)]
struct LogQuery {
    id: String,
}

/// The VTTL log of one match: every upload attempt, newest first.
async fn upload_log(State(app): State<Arc<App>>, u: User, Query(q): Query<LogQuery>) -> R<Vec<crate::uploader::LogEntry>> {
    Ok(Json(app.db.upload_log(u.club()?, &q.id)?))
}

/// "ROEL" -> "Roel", "JEAN-PIERRE" -> "Jean-Pierre".
pub fn title_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut up = true;
    for c in s.chars() {
        if up {
            out.extend(c.to_uppercase());
        } else {
            out.extend(c.to_lowercase());
        }
        up = !c.is_alphabetic();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_clubs() {
        assert_eq!(title_case("JEAN-PIERRE"), "Jean-Pierre");
        assert_eq!(title_case("roel"), "Roel");
        assert!(same_club("Vl-B234", "VLB234"));
        assert!(!same_club("WVL154", "WVL129"));
        assert_eq!(team_letter("Zonnebeke A"), "A");
    }
}
