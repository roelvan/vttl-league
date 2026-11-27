//! VTTL TabT SOAP client.
//!
//! - Every call uses the credentials of the user who asks (the quota is per account).
//! - The cache key does NOT contain the credentials: read data is the same for every
//!   account (checked on 2026-11-06: ExtendedInformation gives the same fields for
//!   any club), so all users share one cache.
//! - Concurrent requests for the same key share one HTTP call.
//! - Every cached answer also goes to a `Store` (the database), so a restart of the server
//!   keeps the cache: the first request after a deploy does not wait for VTTL.

use crate::xml::{self, Node, escape};
use anyhow::{Result, anyhow};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::OnceCell;

const NS: &str = "http://api.frenoy.net/TabTAPI";

#[derive(Clone)]
pub struct Creds {
    pub account: String,
    pub password: String,
}

type Slot = Arc<OnceCell<std::result::Result<Arc<Node>, String>>>;

/// Cheap to clone: all clones share the HTTP client and the cache.
#[derive(Clone)]
pub struct Tabt {
    inner: Arc<Inner>,
}

struct Inner {
    http: reqwest::Client,
    endpoint: String,
    /// Key -> (time of the fetch in Unix ms, answer). Wall-clock time, so that an answer from
    /// the store keeps its age over a restart.
    cache: Mutex<HashMap<String, Entry>>,
    /// (club, season, match id) -> (time in Unix ms, the match as read back after an upload).
    held: Mutex<HashMap<(String, i64, String), (i64, TeamMatch)>>,
    store: Option<Arc<dyn Store>>,
}

struct Entry {
    /// When this entry was last found good enough (Unix ms): the TTL counts from here.
    checked: i64,
    /// When VTTL gave the answer (Unix ms).
    fetched: i64,
    slot: Slot,
}

impl Entry {
    fn fresh(slot: Slot) -> Self {
        let t = now_ms();
        Self { checked: t, fetched: t, slot }
    }
}

fn ready(n: Arc<Node>) -> Slot {
    Arc::new(OnceCell::new_with(Some(Ok(n))))
}

fn fields_xml(fields: &[(&str, String)]) -> String {
    fields.iter().map(|(k, v)| format!("<ns1:{k}>{}</ns1:{k}>", escape(v))).collect()
}

/// Where the cached answers live over a restart (the database). The key has the request
/// fields only, never the credentials.
pub trait Store: Send + Sync {
    /// (time of the fetch in Unix ms, SOAP response)
    fn load(&self, key: &str) -> Option<(i64, String)>;
    fn save(&self, key: &str, at: i64, body: &str);
}

/// After its TTL, a cached answer is still served for this long while a fresh copy loads in
/// the background (stale-while-revalidate). Only a cold cache makes a user wait for VTTL.
pub const STALE_FOR: Duration = Duration::from_secs(30 * 86400);

pub(crate) fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64
}

fn age(at: i64) -> Duration {
    Duration::from_millis((now_ms() - at).max(0) as u64)
}

// ---------------------------------------------------------------- types

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Season {
    pub id: i64,
    pub name: String,
    pub is_current: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Venue {
    pub name: String,
    pub street: String,
    pub town: String,
    pub phone: String,
    pub comment: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Club {
    pub id: String,
    pub name: String,
    pub long_name: String,
    /// Province id (TabT ClubCategory).
    pub category: i64,
    pub category_name: String,
    pub venues: Vec<Venue>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Team {
    pub team_id: String,
    pub letter: String,
    pub division_id: i64,
    pub division_name: String,
    pub division_category: i64,
    pub match_type: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Division {
    pub id: i64,
    pub name: String,
    pub category: i64,
    pub level: i64,
    pub match_type: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub position: i64,
    pub unique_index: i64,
    pub ranking_index: i64,
    pub first_name: String,
    pub last_name: String,
    pub ranking: String,
    pub status: String,
    pub club: String,
    pub gender: String,
    pub category: String,
}

/// One singles game of a player, as VTTL lists it for the ranking.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerResult {
    /// YYYY-MM-DD
    pub date: String,
    pub first_name: String,
    pub last_name: String,
    pub ranking: String,
    pub club: String,
    pub won: bool,
    pub sets_for: i64,
    pub sets_against: i64,
    /// A tournament game (not interclub).
    pub tournament: bool,
}

/// Wins and losses against the opponents of one ranking.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RankCount {
    pub ranking: String,
    pub wins: i64,
    pub losses: i64,
}

/// One player with the results of the current season (the player widget).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerInfo {
    pub unique_index: i64,
    pub first_name: String,
    pub last_name: String,
    pub ranking: String,
    pub club: String,
    pub elo: Option<i64>,
    pub wins: i64,
    pub losses: i64,
    /// Best ranking first.
    pub by_rank: Vec<RankCount>,
    /// The newest games first.
    pub last: Vec<PlayerResult>,
    /// When VTTL gave this answer (Unix ms).
    pub fetched_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerCategory {
    pub id: i64,
    pub name: String,
    pub short_name: String,
    pub ranking_category: i64,
    pub sex: String,
    pub min_age: Option<i64>,
    pub max_age: Option<i64>,
    pub is_group: bool,
    /// For a group: the ids of its member categories.
    pub group_members: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchDef {
    pub position: i64,
    /// 1 = single, 2 = double
    pub kind: i64,
    pub home_index: i64,
    pub away_index: i64,
    pub allow_substitute: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchSystem {
    pub id: i64,
    pub name: String,
    /// Sets needed to win a game (3 = best of 5).
    pub set_count: i64,
    pub point_count: i64,
    pub single_match_count: i64,
    pub double_match_count: i64,
    pub forced_double_teams: bool,
    pub substitute_count: i64,
    pub defs: Vec<MatchDef>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    pub position: i64,
    pub unique_index: i64,
    pub first_name: String,
    pub last_name: String,
    pub ranking: String,
    pub victory_count: i64,
    pub is_forfeited: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoubleTeam {
    pub position: i64,
    /// 1 = players 1-2, 2 = 1-3, 3 = 2-3, 0 = unknown
    pub team: i64,
    pub is_forfeited: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndividualResult {
    pub position: i64,
    pub home_indexes: Vec<i64>,
    pub away_indexes: Vec<i64>,
    pub home_sets: i64,
    pub away_sets: i64,
    pub is_home_forfeited: bool,
    pub is_away_forfeited: bool,
    /// As TabT gives it: "1|-3,2|-4,3|-7" (set|signed loser points, + = home won).
    pub scores: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchDetails {
    pub details_created: bool,
    pub match_system: i64,
    pub home_captain: i64,
    pub away_captain: i64,
    pub referee: i64,
    pub home_players: Vec<Player>,
    pub away_players: Vec<Player>,
    pub home_doubles: Vec<DoubleTeam>,
    pub away_doubles: Vec<DoubleTeam>,
    pub results: Vec<IndividualResult>,
    pub home_score: i64,
    pub away_score: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamMatch {
    pub match_id: String,
    pub unique_id: i64,
    pub week: String,
    /// YYYY-MM-DD, "" for a bye
    pub date: String,
    /// HH:MM, "" when unknown
    pub time: String,
    pub venue: Option<Venue>,
    pub venue_club: String,
    pub home_club: String,
    pub home_team: String,
    pub away_club: String,
    pub away_team: String,
    pub score: String,
    pub division_id: i64,
    pub division_category: i64,
    pub is_home_forfeited: bool,
    pub is_away_forfeited: bool,
    pub is_home_withdrawn: bool,
    pub is_away_withdrawn: bool,
    pub is_validated: bool,
    pub is_locked: bool,
    pub is_bye: bool,
    pub details: Option<MatchDetails>,
}

/// One team in the standings of a division.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RankingEntry {
    pub position: i64,
    pub team: String,
    pub team_club: String,
    /// Team matches.
    pub played: i64,
    pub won: i64,
    pub lost: i64,
    pub draw: i64,
    /// Individual games.
    pub games_won: i64,
    pub games_lost: i64,
    pub sets_won: i64,
    pub sets_lost: i64,
    pub points: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub is_valid_account: bool,
    pub current_quota: i64,
    pub allowed_quota: i64,
}

// ---------------------------------------------------------------- client

pub const TTL_NONE: Duration = Duration::ZERO;
pub const TTL_SHORT: Duration = Duration::from_secs(60);
/// The season of a club (about 800 kB). The poller keeps the match days fresh.
pub const TTL_SEASON: Duration = Duration::from_secs(600);
/// A day answer of the poller stays on top of the season for today and this many days back,
/// minus one. The poller refreshes the season each day, so older day answers are not needed.
const OVERLAY_DAYS: i64 = 3;
pub const TTL_HOUR: Duration = Duration::from_secs(3600);
pub const TTL_DAY: Duration = Duration::from_secs(86400);
/// One player with results. Long, to keep the load on VTTL low: a player costs one call per
/// day at most. The results change after a match only, and then `expire_player` asks sooner.
pub const TTL_PLAYER: Duration = TTL_DAY;
/// How many games `PlayerInfo::last` has.
const LAST_RESULTS: usize = 5;

impl Tabt {
    pub fn new(endpoint: String, store: Option<Arc<dyn Store>>) -> Self {
        Self {
            inner: Arc::new(Inner {
                http: reqwest::Client::builder()
                    .timeout(Duration::from_secs(20))
                    .pool_idle_timeout(Duration::from_secs(90))
                    .build()
                    .unwrap(),
                endpoint,
                cache: Mutex::new(HashMap::new()),
                held: Mutex::new(HashMap::new()),
                store,
            }),
        }
    }

    async fn call(&self, creds: Option<&Creds>, element: &str, fields: &[(&str, String)], ttl: Duration) -> Result<Arc<Node>> {
        self.call_at(creds, element, fields, ttl).await.map(|(_, n)| n)
    }

    /// As `call`, with the time (Unix ms) at which VTTL gave the answer.
    async fn call_at(&self, creds: Option<&Creds>, element: &str, fields: &[(&str, String)], ttl: Duration) -> Result<(i64, Arc<Node>)> {
        let inner = fields_xml(fields);
        if ttl.is_zero() {
            return self.inner.fetch(creds, element, &inner).await.map(|n| (now_ms(), Arc::new(n)));
        }
        let key = format!("{element}|{inner}");
        self.warm(&key);
        enum Plan {
            Use(Slot, i64),
            /// Serve this stale value and refresh in the background.
            Stale(Arc<Node>, i64),
        }
        let plan = {
            let mut cache = self.inner.cache.lock().unwrap();
            match cache.get(&key) {
                Some(e) if age(e.checked) < ttl => Plan::Use(e.slot.clone(), e.fetched),
                Some(e) if age(e.fetched) < STALE_FOR && matches!(e.slot.get(), Some(Ok(_))) => {
                    let old = e.slot.get().unwrap().as_ref().unwrap().clone();
                    let fetched = e.fetched;
                    // Keep serving the old value as fresh while one background task refreshes it.
                    let kept = ready(old.clone());
                    cache.insert(key.clone(), Entry { checked: now_ms(), fetched, slot: kept.clone() });
                    let this = self.clone();
                    let creds = creds.cloned();
                    let (k, el, inn) = (key.clone(), element.to_string(), inner.clone());
                    tokio::spawn(async move {
                        match this.inner.fetch_keep(creds.as_ref(), &el, &inn, &k).await {
                            Ok(n) => {
                                let mut cache = this.inner.cache.lock().unwrap();
                                // Only replace what we put there (a newer answer may be there).
                                if cache.get(&k).is_some_and(|e| Arc::ptr_eq(&e.slot, &kept)) {
                                    cache.insert(k, Entry::fresh(ready(Arc::new(n))));
                                }
                            }
                            // The old value stays for another TTL.
                            Err(e) => tracing::warn!("background refresh of {el} failed: {e:#}"),
                        }
                    });
                    Plan::Stale(old, fetched)
                }
                _ => {
                    let s: Slot = Arc::new(OnceCell::new());
                    cache.insert(key.clone(), Entry::fresh(s.clone()));
                    Plan::Use(s, now_ms())
                }
            }
        };
        let (slot, fetched) = match plan {
            Plan::Stale(old, fetched) => return Ok((fetched, old)),
            Plan::Use(s, fetched) => (s, fetched),
        };
        let r = slot
            .get_or_init(|| async { self.inner.fetch_keep(creds, element, &inner, &key).await.map(Arc::new).map_err(|e| e.to_string()) })
            .await
            .clone();
        if r.is_err() {
            // Do not keep errors: the next request tries again.
            let mut cache = self.inner.cache.lock().unwrap();
            if cache.get(&key).is_some_and(|e| Arc::ptr_eq(&e.slot, &slot)) {
                cache.remove(&key);
            }
        }
        r.map(|n| (fetched, n)).map_err(|e| anyhow!(e))
    }

    /// Asks VTTL now, whatever the cache has, and keeps the answer (the poller).
    async fn call_fresh(&self, creds: Option<&Creds>, element: &str, fields: &[(&str, String)]) -> Result<Arc<Node>> {
        let inner = fields_xml(fields);
        let key = format!("{element}|{inner}");
        let n = Arc::new(self.inner.fetch_keep(creds, element, &inner, &key).await?);
        self.inner.cache.lock().unwrap().insert(key, Entry::fresh(ready(n.clone())));
        Ok(n)
    }

    /// What the cache or the store has, with the time of its fetch. Never calls VTTL.
    fn peek(&self, element: &str, fields: &[(&str, String)]) -> Option<(i64, Arc<Node>)> {
        let key = format!("{element}|{}", fields_xml(fields));
        self.warm(&key);
        let cache = self.inner.cache.lock().unwrap();
        let e = cache.get(&key)?;
        match e.slot.get() {
            Some(Ok(n)) => Some((e.fetched, n.clone())),
            _ => None,
        }
    }

    /// Brings the stored answer for `key` into memory, with its real age, when memory has none.
    fn warm(&self, key: &str) {
        let Some(store) = &self.inner.store else { return };
        if self.inner.cache.lock().unwrap().contains_key(key) {
            return;
        }
        let Some((at, body)) = store.load(key) else { return };
        let Ok(node) = xml::soap_body(&body) else { return };
        let e = Entry { checked: at, fetched: at, slot: ready(Arc::new(node)) };
        self.inner.cache.lock().unwrap().entry(key.to_string()).or_insert(e);
    }

    /// Does the cache have an answer that is younger than `ttl` (or a fetch that runs now)?
    fn is_fresh(&self, element: &str, fields: &[(&str, String)], ttl: Duration) -> bool {
        let key = format!("{element}|{}", fields_xml(fields));
        self.warm(&key);
        self.inner.cache.lock().unwrap().get(&key).is_some_and(|e| age(e.checked) < ttl)
    }

    /// Makes a cached answer stale: the next request still gets it at once, and starts a
    /// refresh. In memory only: after a restart the answer has its normal TTL again.
    fn expire(&self, element: &str, fields: &[(&str, String)]) {
        let key = format!("{element}|{}", fields_xml(fields));
        self.warm(&key);
        if let Some(e) = self.inner.cache.lock().unwrap().get_mut(&key)
            && matches!(e.slot.get(), Some(Ok(_)))
        {
            e.checked = 0;
        }
    }
}

impl Inner {
    async fn fetch(&self, creds: Option<&Creds>, element: &str, inner: &str) -> Result<Node> {
        xml::soap_body(&self.post(creds, element, inner).await?)
    }

    /// A fetch for the cache: a good answer also goes to the store.
    async fn fetch_keep(&self, creds: Option<&Creds>, element: &str, inner: &str, key: &str) -> Result<Node> {
        let resp = self.post(creds, element, inner).await?;
        let node = xml::soap_body(&resp)?;
        if let Some(store) = &self.store {
            store.save(key, now_ms(), &resp);
        }
        Ok(node)
    }

    async fn post(&self, creds: Option<&Creds>, element: &str, inner: &str) -> Result<String> {
        let creds = creds
            .map(|c| {
                format!(
                    "<ns1:Credentials><ns1:Account>{}</ns1:Account><ns1:Password>{}</ns1:Password></ns1:Credentials>",
                    escape(&c.account),
                    escape(&c.password)
                )
            })
            .unwrap_or_default();
        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/" xmlns:ns1="{NS}"><SOAP-ENV:Body><ns1:{element}>{creds}{inner}</ns1:{element}></SOAP-ENV:Body></SOAP-ENV:Envelope>"#
        );
        let started = Instant::now();
        let resp = self
            .http
            .post(&self.endpoint)
            .header("Content-Type", "text/xml; charset=utf-8")
            .body(body)
            .send()
            .await?
            .text()
            .await?;
        tracing::debug!(element, ms = started.elapsed().as_millis() as u64, bytes = resp.len(), "tabt");
        Ok(resp)
    }
}

impl Tabt {

    pub async fn test(&self, creds: &Creds) -> Result<TestResult> {
        let r = self.call(Some(creds), "TestRequest", &[], TTL_NONE).await?;
        Ok(TestResult {
            is_valid_account: r.b("IsValidAccount"),
            current_quota: r.i("CurrentQuota"),
            allowed_quota: r.i("AllowedQuota"),
        })
    }

    pub async fn seasons(&self, c: &Creds) -> Result<Vec<Season>> {
        let r = self.call(Some(c), "GetSeasonsRequest", &[], TTL_HOUR).await?;
        let current = r.i("CurrentSeason");
        Ok(r.children("SeasonEntries")
            .map(|s| Season { id: s.i("Season"), name: s.s("Name"), is_current: s.i("Season") == current })
            .collect())
    }

    pub async fn current_season(&self, c: Option<&Creds>) -> Result<i64> {
        let r = self.call(c, "GetSeasonsRequest", &[], TTL_HOUR).await?;
        Ok(r.i("CurrentSeason"))
    }

    pub async fn club(&self, c: Option<&Creds>, season: i64, club: &str) -> Result<Option<Club>> {
        let r = self
            .call(c, "GetClubs", &[("Season", season.to_string()), ("Club", club.into())], TTL_DAY)
            .await?;
        Ok(r.children("ClubEntries").next().map(club_entry))
    }

    pub async fn clubs(&self, c: Option<&Creds>, season: i64) -> Result<Vec<Club>> {
        let r = self.call(c, "GetClubs", &[("Season", season.to_string())], TTL_DAY).await?;
        Ok(r.children("ClubEntries").map(club_entry).collect())
    }

    pub async fn teams(&self, c: Option<&Creds>, season: i64, club: &str) -> Result<Vec<Team>> {
        let r = self
            .call(c, "GetClubTeamsRequest", &[("Club", club.into()), ("Season", season.to_string())], TTL_HOUR)
            .await?;
        Ok(r.children("TeamEntries")
            .map(|t| Team {
                team_id: t.s("TeamId"),
                letter: t.s("Team"),
                division_id: t.i("DivisionId"),
                division_name: t.s("DivisionName"),
                division_category: t.i("DivisionCategory"),
                match_type: t.i("MatchType"),
            })
            .collect())
    }

    pub async fn divisions(&self, c: Option<&Creds>, season: i64) -> Result<Vec<Division>> {
        let r = self.call(c, "GetDivisions", &[("Season", season.to_string())], TTL_DAY).await?;
        Ok(r.children("DivisionEntries")
            .map(|d| Division {
                id: d.i("DivisionId"),
                name: d.s("DivisionName"),
                category: d.i("DivisionCategory"),
                level: d.i("Level"),
                match_type: d.i("MatchType"),
            })
            .collect())
    }

    /// All matches of a club in a season (home and away), with details.
    ///
    /// The season answer is large, so it is refreshed seldom. On a match day the poller asks
    /// for the matches of that day only (`refresh_day`): those newer answers go on top here.
    pub async fn club_matches(&self, c: Option<&Creds>, season: i64, club: &str) -> Result<Vec<TeamMatch>> {
        let (at, r) = self.call_at(c, "GetMatchesRequest", &season_fields(club, season), TTL_SEASON).await?;
        Ok(self.with_days(at, &r, season, club))
    }

    /// As `club_matches`, from the cache only: (time of the newest fetch in Unix ms, matches).
    pub fn club_matches_cached(&self, season: i64, club: &str) -> Option<(i64, Vec<TeamMatch>)> {
        let (at, r) = self.peek("GetMatchesRequest", &season_fields(club, season))?;
        Some((at, self.with_days(at, &r, season, club)))
    }

    /// Keeps a match as VTTL gave it right after an upload (the read-back). It goes on top of
    /// every older season or day answer: a match of an earlier week has no day answer, and the
    /// season answer can be minutes old. Without this, the old data looks like a change on VTTL
    /// and comes back into the stored match.
    pub fn note_match(&self, season: i64, club: &str, m: &TeamMatch) {
        self.inner.held.lock().unwrap().insert((club.to_string(), season, m.match_id.clone()), (now_ms(), m.clone()));
    }

    fn with_days(&self, at: i64, r: &Node, season: i64, club: &str) -> Vec<TeamMatch> {
        // Each match with the time of the answer it comes from.
        let mut list: Vec<(i64, TeamMatch)> = r.children("TeamMatchesEntries").map(|n| (at, team_match(n))).collect();
        let now = now_ms() / 1000;
        for back in 0..OVERLAY_DAYS {
            let day = crate::when::local_day(now - back * 86400);
            let Some((day_at, d)) = self.peek("GetMatchesRequest", &day_fields(club, season, &day)) else { continue };
            if day_at <= at {
                continue;
            }
            for m in d.children("TeamMatchesEntries").map(team_match) {
                // A match between two teams of the club is in the list twice.
                for old in list.iter_mut().filter(|x| x.1.match_id == m.match_id) {
                    *old = (day_at, m.clone());
                }
            }
        }
        let mut held = self.inner.held.lock().unwrap();
        // A newer season answer has everything a read-back has.
        held.retain(|(c, s, _), (t, _)| c != club || *s != season || *t > at);
        for ((c, s, id), (t, m)) in held.iter() {
            if c != club || *s != season {
                continue;
            }
            for old in list.iter_mut().filter(|x| x.1.match_id == *id && x.0 < *t) {
                *old = (*t, m.clone());
            }
        }
        list.into_iter().map(|x| x.1).collect()
    }

    /// When the data of match `m` in `club_matches` came from VTTL (Unix ms): the answer of its
    /// day when that one is on top, else the season answer. None without a season in the cache.
    pub fn match_fetched_at(&self, season: i64, club: &str, m: &TeamMatch) -> Option<i64> {
        let (at, _) = self.peek("GetMatchesRequest", &season_fields(club, season))?;
        let now = now_ms() / 1000;
        let on_top = (0..OVERLAY_DAYS).any(|back| crate::when::local_day(now - back * 86400) == m.date);
        let day_at = if on_top { self.day_fetched_at(season, club, &m.date).filter(|d| *d > at) } else { None };
        Some(day_at.unwrap_or(at))
    }

    /// Gets the season of a club from VTTL now.
    pub async fn refresh_club_matches(&self, c: Option<&Creds>, season: i64, club: &str) -> Result<()> {
        self.call_fresh(c, "GetMatchesRequest", &season_fields(club, season)).await.map(|_| ())
    }

    /// Gets the matches of a club on one day ("2026-11-14") from VTTL now.
    pub async fn refresh_day(&self, c: Option<&Creds>, season: i64, club: &str, day: &str) -> Result<()> {
        self.call_fresh(c, "GetMatchesRequest", &day_fields(club, season, day)).await.map(|_| ())
    }

    /// When `refresh_day` last had an answer for this day (Unix ms).
    pub fn day_fetched_at(&self, season: i64, club: &str, day: &str) -> Option<i64> {
        self.peek("GetMatchesRequest", &day_fields(club, season, day)).map(|(at, _)| at)
    }

    /// The calendar of a division: all its matches, without details.
    pub async fn division_calendar(&self, c: Option<&Creds>, season: i64, division: i64) -> Result<Vec<TeamMatch>> {
        let r = self
            .call(c, "GetMatchesRequest", &[("DivisionId", division.to_string()), ("Season", season.to_string())], TTL_SEASON)
            .await?;
        Ok(r.children("TeamMatchesEntries").map(team_match).collect())
    }

    /// The matches of a division in one week, with details.
    pub async fn division_week(&self, c: Option<&Creds>, season: i64, division: i64, week: &str) -> Result<Vec<TeamMatch>> {
        let r = self
            .call(
                c,
                "GetMatchesRequest",
                &[("DivisionId", division.to_string()), ("Season", season.to_string()), ("WeekName", week.into()), ("WithDetails", "true".into())],
                TTL_SHORT,
            )
            .await?;
        Ok(r.children("TeamMatchesEntries").map(team_match).collect())
    }

    /// The standings of a division, not older than `ttl`. A division id is of one season, so
    /// there is no season here.
    pub async fn division_ranking(&self, c: Option<&Creds>, division: i64, ttl: Duration) -> Result<Vec<RankingEntry>> {
        let r = self.call(c, "GetDivisionRankingRequest", &[("DivisionId", division.to_string())], ttl).await?;
        Ok(r.children("RankingEntries")
            .map(|e| RankingEntry {
                position: e.i("Position"),
                team: e.s("Team").trim().to_string(),
                team_club: e.s("TeamClub"),
                played: e.i("GamesPlayed"),
                won: e.i("GamesWon"),
                lost: e.i("GamesLost"),
                draw: e.i("GamesDraw"),
                games_won: e.i("IndividualMatchesWon"),
                games_lost: e.i("IndividualMatchesLost"),
                sets_won: e.i("IndividualSetsWon"),
                sets_lost: e.i("IndividualSetsLost"),
                points: e.i("Points"),
            })
            .collect())
    }

    /// One match. `fresh` skips the cache (the read-back after an upload).
    pub async fn team_match(&self, c: &Creds, season: i64, club: &str, match_id: &str, fresh: bool) -> Result<Option<TeamMatch>> {
        let r = self
            .call(
                Some(c),
                "GetMatchesRequest",
                &[
                    ("Club", club.into()),
                    ("Season", season.to_string()),
                    ("MatchId", match_id.into()),
                    ("WithDetails", "true".into()),
                ],
                if fresh { TTL_NONE } else { TTL_SHORT },
            )
            .await?;
        Ok(r.children("TeamMatchesEntries").next().map(team_match))
    }

    /// Members of a club for one player category (the strength list).
    pub async fn members(&self, c: &Creds, season: i64, club: &str, category: i64) -> Result<Vec<Member>> {
        let r = self
            .call(
                Some(c),
                "GetMembersRequest",
                &[("Club", club.into()), ("Season", season.to_string()), ("PlayerCategory", category.to_string())],
                TTL_HOUR,
            )
            .await?;
        Ok(r.children("MemberEntries").map(member).collect())
    }

    /// Name search over all VTTL members (current season).
    pub async fn search_members(&self, c: &Creds, name: &str) -> Result<Vec<Member>> {
        let r = self
            .call(Some(c), "GetMembersRequest", &[("NameSearch", name.into())], TTL_HOUR)
            .await?;
        Ok(r.children("MemberEntries").map(member).collect())
    }

    pub async fn member(&self, c: &Creds, unique_index: i64) -> Result<Option<Member>> {
        let r = self
            .call(Some(c), "GetMembersRequest", &[("UniqueIndex", unique_index.to_string())], TTL_HOUR)
            .await?;
        Ok(r.children("MemberEntries").next().map(member))
    }

    /// One player with the ELO points and the results of the current season. VTTL gives
    /// this without a login too (checked on 2026-11-16).
    pub async fn player(&self, c: Option<&Creds>, unique_index: i64) -> Result<Option<PlayerInfo>> {
        let (at, r) = self.call_at(c, "GetMembersRequest", &player_fields(unique_index), TTL_PLAYER).await?;
        Ok(r.children("MemberEntries").next().map(|m| player_info(m, at)))
    }

    /// Is there an answer for this player, of any age? Never calls VTTL.
    pub fn has_player(&self, unique_index: i64) -> bool {
        self.peek("GetMembersRequest", &player_fields(unique_index)).is_some()
    }

    /// Would `player` answer without a call to VTTL?
    pub fn player_is_fresh(&self, unique_index: i64) -> bool {
        self.is_fresh("GetMembersRequest", &player_fields(unique_index), TTL_PLAYER)
    }

    /// The player has new results at VTTL: the next `player` call starts a refresh.
    pub fn expire_player(&self, unique_index: i64) {
        self.expire("GetMembersRequest", &player_fields(unique_index));
    }

    pub async fn player_categories(&self, c: &Creds, season: i64) -> Result<Vec<PlayerCategory>> {
        let r = self
            .call(Some(c), "GetPlayerCategories", &[("Season", season.to_string())], TTL_DAY)
            .await?;
        let opt = |n: &Node, k: &str| n.s(k).parse().ok();
        Ok(r.children("PlayerCategoryEntries")
            .map(|p| PlayerCategory {
                id: p.i("UniqueIndex"),
                name: p.s("Name"),
                short_name: p.s("ShortName"),
                ranking_category: p.i("RankingCategory"),
                sex: p.s("Sex"),
                min_age: opt(p, "MinimumAge"),
                max_age: opt(p, "MaximumAge"),
                is_group: p.b("IsGroup"),
                group_members: p.s("GroupMembers").split(',').filter_map(|x| x.trim().parse().ok()).collect(),
            })
            .collect())
    }

    pub async fn match_systems(&self, c: &Creds) -> Result<Vec<MatchSystem>> {
        let r = self.call(Some(c), "GetMatchSystems", &[], TTL_DAY).await?;
        Ok(r.children("MatchSystemEntries")
            .map(|e| MatchSystem {
                id: e.i("UniqueIndex"),
                name: e.s("Name"),
                set_count: e.i("SetCount"),
                point_count: e.i("PointCount"),
                single_match_count: e.i("SingleMatchCount"),
                double_match_count: e.i("DoubleMatchCount"),
                forced_double_teams: e.b("ForcedDoubleTeams"),
                substitute_count: e.i("SubstituteCount"),
                defs: e
                    .children("TeamMatchDefinitionEntries")
                    .map(|d| MatchDef {
                        position: d.i("Position"),
                        kind: d.i("Type"),
                        home_index: d.i("HomePlayerIndex"),
                        away_index: d.i("AwayPlayerIndex"),
                        allow_substitute: d.b("AllowSubstitute"),
                    })
                    .collect(),
            })
            .collect())
    }

    /// Returns (Result, ErrorLines). Never cached, never retried.
    pub async fn upload(&self, c: &Creds, data: &str) -> Result<(bool, Vec<String>)> {
        let r = self.call(Some(c), "UploadRequest", &[("Data", data.into())], TTL_NONE).await?;
        Ok((r.b("Result"), r.children("ErrorLines").map(|n| n.text.clone()).collect()))
    }
}

fn season_fields(club: &str, season: i64) -> [(&'static str, String); 3] {
    [("Club", club.into()), ("Season", season.to_string()), ("WithDetails", "true".into())]
}

fn day_fields(club: &str, season: i64, day: &str) -> [(&'static str, String); 5] {
    [
        ("Club", club.into()),
        ("Season", season.to_string()),
        ("YearDateFrom", day.into()),
        ("YearDateTo", day.into()),
        ("WithDetails", "true".into()),
    ]
}

fn player_fields(unique_index: i64) -> [(&'static str, String); 3] {
    [
        ("UniqueIndex", unique_index.to_string()),
        ("RankingPointsInformation", "true".into()),
        ("WithResults", "true".into()),
    ]
}

// ---------------------------------------------------------------- parsing

fn player_info(m: &Node, fetched_at: i64) -> PlayerInfo {
    // "V" (victory) or "D" (defeat). Anything else is not a played game.
    let mut results: Vec<PlayerResult> = m
        .children("ResultEntries")
        .filter(|r| matches!(r.s("Result").as_str(), "V" | "D"))
        .map(|r| PlayerResult {
            date: r.s("Date"),
            first_name: r.s("FirstName"),
            last_name: r.s("LastName"),
            ranking: r.s("Ranking"),
            club: r.s("Club"),
            won: r.s("Result") == "V",
            sets_for: r.i("SetFor"),
            sets_against: r.i("SetAgainst"),
            tournament: r.s("CompetitionType") == "T",
        })
        .collect();
    // Stable: the games of one day keep the order of VTTL.
    results.sort_by(|a, b| a.date.cmp(&b.date));
    let mut by_rank: Vec<RankCount> = Vec::new();
    for r in &results {
        let i = by_rank.iter().position(|x| x.ranking == r.ranking).unwrap_or_else(|| {
            by_rank.push(RankCount { ranking: r.ranking.clone(), wins: 0, losses: 0 });
            by_rank.len() - 1
        });
        if r.won { by_rank[i].wins += 1 } else { by_rank[i].losses += 1 }
    }
    by_rank.sort_by_key(|x| rank_key(&x.ranking));
    let wins = results.iter().filter(|r| r.won).count() as i64;
    PlayerInfo {
        unique_index: m.i("UniqueIndex"),
        first_name: m.s("FirstName"),
        last_name: m.s("LastName"),
        ranking: m.s("Ranking"),
        club: m.s("Club"),
        elo: m.children("RankingPointsEntries").find(|e| e.s("MethodName") == "ELO").and_then(|e| e.s("Value").parse().ok()),
        wins,
        losses: results.len() as i64 - wins,
        by_rank,
        last: results.into_iter().rev().take(LAST_RESULTS).collect(),
        fetched_at,
    }
}

fn venue(n: &Node) -> Venue {
    Venue {
        name: n.s("Name"),
        street: n.s("Street"),
        town: n.s("Town"),
        phone: n.s("Phone"),
        comment: n.s("Comment"),
    }
}

fn club_entry(n: &Node) -> Club {
    Club {
        id: n.s("UniqueIndex"),
        name: n.s("Name"),
        long_name: n.s("LongName"),
        category: n.i("Category"),
        category_name: n.s("CategoryName"),
        venues: n.children("VenueEntries").map(venue).collect(),
    }
}

fn member(m: &Node) -> Member {
    Member {
        position: m.i("Position"),
        unique_index: m.i("UniqueIndex"),
        ranking_index: m.i("RankingIndex"),
        first_name: m.s("FirstName"),
        last_name: m.s("LastName"),
        ranking: m.s("Ranking"),
        status: m.s("Status"),
        club: m.s("Club"),
        gender: m.s("Gender"),
        category: m.s("Category"),
    }
}

fn players(n: Option<&Node>) -> (Vec<Player>, Vec<DoubleTeam>) {
    let Some(n) = n else { return (vec![], vec![]) };
    let mut p: Vec<Player> = n
        .children("Players")
        .map(|p| Player {
            position: p.i("Position"),
            unique_index: p.i("UniqueIndex"),
            first_name: p.s("FirstName"),
            last_name: p.s("LastName"),
            ranking: p.s("Ranking"),
            victory_count: p.i("VictoryCount"),
            is_forfeited: p.b("IsForfeited"),
        })
        .filter(|p| p.unique_index != 0) // empty lineup placeholder
        .collect();
    p.sort_by_key(|p| p.position);
    let d = n
        .children("DoubleTeams")
        .map(|d| DoubleTeam { position: d.i("Position"), team: d.i("Team"), is_forfeited: d.b("IsForfeited") })
        .collect();
    (p, d)
}

fn details(d: &Node) -> MatchDetails {
    let (home_players, home_doubles) = players(d.child("HomePlayers"));
    let (away_players, away_doubles) = players(d.child("AwayPlayers"));
    let idx = |r: &Node, k: &str| r.children(k).map(|n| n.text.trim().parse().unwrap_or(0)).collect();
    MatchDetails {
        details_created: d.b("DetailsCreated"),
        match_system: d.i("MatchSystem"),
        home_captain: d.i("HomeCaptain"),
        away_captain: d.i("AwayCaptain"),
        referee: d.i("Referee"),
        home_players,
        away_players,
        home_doubles,
        away_doubles,
        results: d
            .children("IndividualMatchResults")
            .map(|r| IndividualResult {
                position: r.i("Position"),
                home_indexes: idx(r, "HomePlayerMatchIndex"),
                away_indexes: idx(r, "AwayPlayerMatchIndex"),
                home_sets: r.i("HomeSetCount"),
                away_sets: r.i("AwaySetCount"),
                is_home_forfeited: r.b("IsHomeForfeited"),
                is_away_forfeited: r.b("IsAwayForfeited"),
                scores: r.s("Scores"),
            })
            .collect(),
        home_score: d.i("HomeScore"),
        away_score: d.i("AwayScore"),
    }
}

/// A match from a SOAP answer with one `TeamMatchesEntries` (tests).
#[cfg(test)]
pub fn parse_match(soap: &str) -> TeamMatch {
    team_match(xml::soap_body(soap).unwrap().child("TeamMatchesEntries").unwrap())
}

pub fn is_bye_club(c: &str) -> bool {
    matches!(c.trim().to_lowercase().as_str(), "-" | "" | "bye" | "vrij")
}

fn team_match(n: &Node) -> TeamMatch {
    let time = n.s("Time");
    TeamMatch {
        match_id: n.s("MatchId"),
        unique_id: n.i("MatchUniqueId"),
        week: n.s("WeekName"),
        date: n.s("Date"),
        time: time.get(..5).unwrap_or(&time).to_string(),
        venue: n.child("VenueEntry").map(venue),
        venue_club: n.s("VenueClub"),
        home_club: n.s("HomeClub"),
        home_team: n.s("HomeTeam"),
        away_club: n.s("AwayClub"),
        away_team: n.s("AwayTeam"),
        score: n.s("Score"),
        division_id: n.i("DivisionId"),
        division_category: n.i("DivisionCategory"),
        is_home_forfeited: n.b("IsHomeForfeited"),
        is_away_forfeited: n.b("IsAwayForfeited"),
        is_home_withdrawn: n.s("IsHomeWithdrawn") == "1" || n.b("IsHomeWithdrawn"),
        is_away_withdrawn: n.s("IsAwayWithdrawn") == "1" || n.b("IsAwayWithdrawn"),
        is_validated: n.b("IsValidated"),
        is_locked: n.b("IsLocked"),
        is_bye: is_bye_club(&n.s("HomeClub")) || is_bye_club(&n.s("AwayClub")),
        details: n.child("MatchDetails").map(details),
    }
}

/// Sort key for a Belgian ranking, best first: A (A1 before A20), B0 … E6, NG/NC, then unknown.
pub fn rank_key(ranking: &str) -> (u8, u32) {
    let r = ranking.trim().to_uppercase();
    if r == "NG" || r == "NC" {
        return (5, 0);
    }
    let mut chars = r.chars();
    let letter = match chars.next() {
        Some(c @ 'A'..='E') => c as u8 - b'A',
        _ => return (6, 0),
    };
    match chars.as_str() {
        "" => (letter, 0),
        n => n.parse().map(|n| (letter, n)).unwrap_or((6, 0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake TabT endpoint that answers <R><V>n</V></R> with n = number of calls so far.
    async fn fake_vttl() -> (String, Arc<std::sync::atomic::AtomicUsize>) {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(move || {
                let n = h.fetch_add(1, Ordering::SeqCst) + 1;
                async move {
                    tokio::time::sleep(Duration::from_millis(30)).await;
                    format!("<Envelope><Body><R><V>{n}</V></R></Body></Envelope>")
                }
            }),
        );
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", l.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
        (url, hits)
    }

    #[tokio::test]
    async fn cache_coalesces_and_serves_stale() {
        use std::sync::atomic::Ordering;
        let (url, hits) = fake_vttl().await;
        let t = Tabt::new(url, None);
        let ttl = Duration::from_millis(100);
        // Ten parallel requests share one HTTP call.
        let r = futures_util::future::join_all((0..10).map(|_| t.call(None, "X", &[], ttl))).await;
        assert!(r.iter().all(|n| n.as_ref().unwrap().i("V") == 1));
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        // After the TTL: the old value comes back at once, a refresh runs in the background.
        tokio::time::sleep(Duration::from_millis(120)).await;
        let started = Instant::now();
        assert_eq!(t.call(None, "X", &[], ttl).await.unwrap().i("V"), 1);
        assert!(started.elapsed() < Duration::from_millis(20), "stale answer must not wait");
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert_eq!(hits.load(Ordering::SeqCst), 2);
        assert_eq!(t.call(None, "X", &[], ttl).await.unwrap().i("V"), 2);
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn cache_survives_a_restart() {
        use std::sync::atomic::Ordering;
        let (url, hits) = fake_vttl().await;
        let db: Arc<dyn Store> = Arc::new(crate::db::memory());
        let ttl = Duration::from_millis(100);
        let t = Tabt::new(url.clone(), Some(db.clone()));
        assert_eq!(t.call(None, "X", &[("Club", "WVL154".into())], ttl).await.unwrap().i("V"), 1);
        tokio::time::sleep(Duration::from_millis(120)).await;

        // A new server: the stored answer comes back at once (it is stale: a refresh follows).
        let t = Tabt::new(url.clone(), Some(db.clone()));
        let started = Instant::now();
        assert_eq!(t.call(None, "X", &[("Club", "WVL154".into())], ttl).await.unwrap().i("V"), 1);
        assert!(started.elapsed() < Duration::from_millis(20), "stored answer must not wait");
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert_eq!(hits.load(Ordering::SeqCst), 2);

        // The refresh went to the store too: the next server starts with it, still fresh.
        let t = Tabt::new(url.clone(), Some(db.clone()));
        assert_eq!(t.call(None, "X", &[("Club", "WVL154".into())], ttl).await.unwrap().i("V"), 2);
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }

    /// A fake TabT endpoint for matches: the day request has the score, the season request not.
    async fn fake_matches() -> String {
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(|body: String| async move {
                let today = crate::when::local_day(now_ms() / 1000);
                let entry = |id: &str, score: &str| {
                    format!("<TeamMatchesEntries><MatchId>{id}</MatchId><Date>{today}</Date><HomeClub>A1</HomeClub><AwayClub>B2</AwayClub><Score>{score}</Score></TeamMatchesEntries>")
                };
                let list = if body.contains("YearDateFrom") { entry("M/1", "12-4") } else { entry("M/1", "") + &entry("M/2", "8-8") };
                format!("<Envelope><Body><R>{list}</R></Body></Envelope>")
            }),
        );
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", l.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
        url
    }

    #[tokio::test]
    async fn day_answer_goes_on_top_of_the_season() {
        let db: Arc<dyn Store> = Arc::new(crate::db::memory());
        let t = Tabt::new(fake_matches().await, Some(db.clone()));
        let today = crate::when::local_day(now_ms() / 1000);
        let scores = |l: Vec<TeamMatch>| l.into_iter().map(|m| format!("{}={}", m.match_id, m.score)).collect::<Vec<_>>().join(" ");
        assert!(t.club_matches_cached(27, "A1").is_none());
        assert_eq!(scores(t.club_matches(None, 27, "A1").await.unwrap()), "M/1= M/2=8-8");
        assert!(t.day_fetched_at(27, "A1", &today).is_none());

        tokio::time::sleep(Duration::from_millis(5)).await;
        t.refresh_day(None, 27, "A1", &today).await.unwrap();
        assert!(t.day_fetched_at(27, "A1", &today).is_some());
        assert_eq!(scores(t.club_matches(None, 27, "A1").await.unwrap()), "M/1=12-4 M/2=8-8");
        // Also after a restart, and without a call to VTTL.
        let t = Tabt::new("http://127.0.0.1:9/".into(), Some(db));
        assert_eq!(scores(t.club_matches_cached(27, "A1").unwrap().1), "M/1=12-4 M/2=8-8");

        // A newer season answer wins from the day answer.
        let t2 = Tabt::new(fake_matches().await, None);
        t2.refresh_day(None, 27, "A1", &today).await.unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;
        t2.refresh_club_matches(None, 27, "A1").await.unwrap();
        assert_eq!(scores(t2.club_matches(None, 27, "A1").await.unwrap()), "M/1= M/2=8-8");

        // A read-back after an upload wins from the older season and day answers...
        tokio::time::sleep(Duration::from_millis(5)).await;
        let mut held = t2.club_matches(None, 27, "A1").await.unwrap().remove(1);
        held.score = "9-7".into();
        t2.note_match(27, "A1", &held);
        assert_eq!(scores(t2.club_matches(None, 27, "A1").await.unwrap()), "M/1= M/2=9-7");
        assert_eq!(scores(t2.club_matches(None, 27, "B2").await.unwrap()), "M/1= M/2=8-8");
        // ...until a newer answer from VTTL comes.
        tokio::time::sleep(Duration::from_millis(5)).await;
        t2.refresh_club_matches(None, 27, "A1").await.unwrap();
        assert_eq!(scores(t2.club_matches(None, 27, "A1").await.unwrap()), "M/1= M/2=8-8");
    }

    #[test]
    fn player_summary() {
        let game = |date: &str, rank: &str, result: &str, f: i64, a: i64, kind: &str| {
            format!("<ResultEntries><Date>{date}</Date><UniqueIndex>1</UniqueIndex><FirstName>AN</FirstName><LastName>{date}{rank}</LastName><Ranking>{rank}</Ranking><Result>{result}</Result><SetFor>{f}</SetFor><SetAgainst>{a}</SetAgainst><CompetitionType>{kind}</CompetitionType><Club>WVL100</Club></ResultEntries>")
        };
        let games = [
            game("2026-09-19", "C2", "V", 3, 1, "C"),
            game("2026-09-19", "C0", "D", 0, 3, "C"),
            game("2026-08-30", "D0", "V", 3, 0, "T"),
            game("2026-09-26", "C2", "D", 2, 3, "C"),
            game("2026-09-26", "B6", "D", 1, 3, "C"),
            game("2026-10-03", "C2", "V", 3, 2, "C"),
            game("2026-10-03", "C0", "V", 3, 0, "C"),
            // Not a played game.
            game("2026-10-10", "C0", "", 0, 0, "C"),
        ]
        .concat();
        let x = format!(
            "<Envelope><Body><R><MemberEntries><UniqueIndex>509740</UniqueIndex><FirstName>JAN</FirstName><LastName>PEETERS</LastName><Ranking>C4</Ranking><Club>WVL129</Club>\
             <RankingPointsEntries><MethodName>ELO</MethodName><Value>1463</Value></RankingPointsEntries>{games}</MemberEntries></R></Body></Envelope>"
        );
        let r = xml::soap_body(&x).unwrap();
        let p = player_info(r.child("MemberEntries").unwrap(), 42);
        assert_eq!((p.unique_index, p.ranking.as_str(), p.elo, p.fetched_at), (509740, "C4", Some(1463), 42));
        assert_eq!((p.wins, p.losses), (4, 3));
        let ranks: Vec<_> = p.by_rank.iter().map(|x| format!("{} {}-{}", x.ranking, x.wins, x.losses)).collect();
        assert_eq!(ranks, ["B6 0-1", "C0 1-1", "C2 2-1", "D0 1-0"]);
        // The newest first; the games of one day keep the order of VTTL, reversed.
        let last: Vec<_> = p.last.iter().map(|x| x.last_name.as_str()).collect();
        assert_eq!(last, ["2026-10-03C0", "2026-10-03C2", "2026-09-26B6", "2026-09-26C2", "2026-09-19C0"]);
        assert!(p.last[0].won && !p.last[0].tournament);
        assert_eq!((p.last[3].sets_for, p.last[3].sets_against), (2, 3));

        // No ELO entry, no results.
        let r = xml::soap_body("<Envelope><Body><R><MemberEntries><UniqueIndex>7</UniqueIndex></MemberEntries></R></Body></Envelope>").unwrap();
        let p = player_info(r.child("MemberEntries").unwrap(), 0);
        assert_eq!((p.elo, p.wins, p.losses, p.last.len()), (None, 0, 0, 0));
    }

    #[tokio::test]
    async fn expired_answer_is_served_and_refreshed() {
        use std::sync::atomic::Ordering;
        let (url, hits) = fake_vttl().await;
        let t = Tabt::new(url, None);
        let ttl = Duration::from_secs(3600);
        let f = [("UniqueIndex", "1".to_string())];
        assert!(!t.is_fresh("X", &f, ttl));
        // Nothing to expire yet: no call, no entry.
        t.expire("X", &f);
        assert_eq!(t.call(None, "X", &f, ttl).await.unwrap().i("V"), 1);
        assert!(t.is_fresh("X", &f, ttl));

        t.expire("X", &f);
        assert!(!t.is_fresh("X", &f, ttl));
        // The old answer comes at once; one refresh runs in the background.
        assert_eq!(t.call(None, "X", &f, ttl).await.unwrap().i("V"), 1);
        assert!(t.is_fresh("X", &f, ttl));
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert_eq!(t.call(None, "X", &f, ttl).await.unwrap().i("V"), 2);
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn ranking_order() {
        let mut v = vec!["NG", "C2", "A12", "B0", "E6", "A", "A3", "D4", "x", "B6"];
        v.sort_by_key(|r| rank_key(r));
        assert_eq!(v, ["A", "A3", "A12", "B0", "B6", "C2", "D4", "E6", "NG", "x"]);
    }

    #[test]
    fn parses_match_with_double_indexes() {
        let x = r#"<Envelope><Body><R><TeamMatchesEntries><MatchId>PWVLH01/012</MatchId><Date>2026-09-19</Date><Time>19:00:00</Time>
            <HomeClub>WVL129</HomeClub><AwayClub>-</AwayClub><MatchDetails><MatchSystem>4</MatchSystem>
            <IndividualMatchResults><Position>7</Position><HomePlayerMatchIndex>1</HomePlayerMatchIndex><HomePlayerMatchIndex>3</HomePlayerMatchIndex>
            <AwayPlayerMatchIndex>2</AwayPlayerMatchIndex><AwayPlayerMatchIndex>3</AwayPlayerMatchIndex><Scores>1|-3,2|-4,3|-7</Scores></IndividualMatchResults>
            </MatchDetails></TeamMatchesEntries></R></Body></Envelope>"#;
        let r = xml::soap_body(x).unwrap();
        let m = team_match(r.child("TeamMatchesEntries").unwrap());
        assert_eq!(m.time, "19:00");
        assert!(m.is_bye);
        let d = m.details.unwrap();
        assert_eq!(d.match_system, 4);
        assert_eq!(d.results[0].home_indexes, vec![1, 3]);
        assert_eq!(d.results[0].away_indexes, vec![2, 3]);
    }
}
