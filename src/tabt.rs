//! VTTL TabT SOAP client.
//!
//! - Every call uses the credentials of the user who asks (the quota is per account).
//! - The cache key does NOT contain the credentials: read data is the same for every
//!   account (checked on 2026-11-06: ExtendedInformation gives the same fields for
//!   any club), so all users share one cache.
//! - Concurrent requests for the same key share one HTTP call.

use crate::xml::{self, Node, escape};
use anyhow::{Result, anyhow};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
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
    cache: Mutex<HashMap<String, (Instant, Slot)>>,
}

/// After its TTL, a cached answer is still served for this long while a fresh copy loads in
/// the background (stale-while-revalidate). Only a cold cache makes a user wait for VTTL.
const STALE_FOR: Duration = Duration::from_secs(24 * 3600);

// ---------------------------------------------------------------- types

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Season {
    pub id: i64,
    pub name: String,
    pub is_current: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Venue {
    pub name: String,
    pub street: String,
    pub town: String,
    pub phone: String,
    pub comment: String,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Team {
    pub team_id: String,
    pub letter: String,
    pub division_id: i64,
    pub division_name: String,
    pub division_category: i64,
    pub match_type: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Division {
    pub id: i64,
    pub name: String,
    pub category: i64,
    pub level: i64,
    pub match_type: i64,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchDef {
    pub position: i64,
    /// 1 = single, 2 = double
    pub kind: i64,
    pub home_index: i64,
    pub away_index: i64,
    pub allow_substitute: bool,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DoubleTeam {
    pub position: i64,
    /// 1 = players 1-2, 2 = 1-3, 3 = 2-3, 0 = unknown
    pub team: i64,
    pub is_forfeited: bool,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub is_valid_account: bool,
    pub current_quota: i64,
    pub allowed_quota: i64,
}

// ---------------------------------------------------------------- client

pub const TTL_NONE: Duration = Duration::ZERO;
pub const TTL_SHORT: Duration = Duration::from_secs(60);
pub const TTL_HOUR: Duration = Duration::from_secs(3600);
pub const TTL_DAY: Duration = Duration::from_secs(86400);

impl Tabt {
    pub fn new(endpoint: String) -> Self {
        Self {
            inner: Arc::new(Inner {
                http: reqwest::Client::builder()
                    .timeout(Duration::from_secs(20))
                    .pool_idle_timeout(Duration::from_secs(90))
                    .build()
                    .unwrap(),
                endpoint,
                cache: Mutex::new(HashMap::new()),
            }),
        }
    }

    async fn call(&self, creds: Option<&Creds>, element: &str, fields: &[(&str, String)], ttl: Duration) -> Result<Arc<Node>> {
        let mut inner = String::new();
        for (k, v) in fields {
            inner.push_str(&format!("<ns1:{k}>{}</ns1:{k}>", escape(v)));
        }
        if ttl.is_zero() {
            return self.inner.fetch(creds, element, &inner).await.map(Arc::new);
        }
        let key = format!("{element}|{inner}");
        enum Plan {
            Use(Slot),
            /// Serve this stale value and refresh in the background.
            Stale(Arc<Node>),
        }
        let plan = {
            let mut cache = self.inner.cache.lock().unwrap();
            match cache.get(&key) {
                Some((t, s)) if t.elapsed() < ttl => Plan::Use(s.clone()),
                Some((t, s)) if t.elapsed() < ttl + STALE_FOR && matches!(s.get(), Some(Ok(_))) => {
                    let old = s.get().unwrap().as_ref().unwrap().clone();
                    // Keep serving the old value as fresh while one background task refreshes it.
                    let kept: Slot = Arc::new(OnceCell::new_with(Some(Ok(old.clone()))));
                    cache.insert(key.clone(), (Instant::now(), kept.clone()));
                    let this = self.clone();
                    let creds = creds.cloned();
                    let (k, el, inn) = (key.clone(), element.to_string(), inner.clone());
                    tokio::spawn(async move {
                        match this.inner.fetch(creds.as_ref(), &el, &inn).await {
                            Ok(n) => {
                                let mut cache = this.inner.cache.lock().unwrap();
                                // Only replace what we put there (an invalidate may have run).
                                if cache.get(&k).is_some_and(|(_, s)| Arc::ptr_eq(s, &kept)) {
                                    let new: Slot = Arc::new(OnceCell::new_with(Some(Ok(Arc::new(n)))));
                                    cache.insert(k, (Instant::now(), new));
                                }
                            }
                            // The old value stays for another TTL.
                            Err(e) => tracing::warn!("background refresh of {el} failed: {e:#}"),
                        }
                    });
                    Plan::Stale(old)
                }
                _ => {
                    let s: Slot = Arc::new(OnceCell::new());
                    cache.insert(key.clone(), (Instant::now(), s.clone()));
                    Plan::Use(s)
                }
            }
        };
        let slot = match plan {
            Plan::Stale(old) => return Ok(old),
            Plan::Use(s) => s,
        };
        let r = slot
            .get_or_init(|| async { self.inner.fetch(creds, element, &inner).await.map(Arc::new).map_err(|e| e.to_string()) })
            .await
            .clone();
        if r.is_err() {
            // Do not keep errors: the next request tries again.
            let mut cache = self.inner.cache.lock().unwrap();
            if cache.get(&key).is_some_and(|(_, s)| Arc::ptr_eq(s, &slot)) {
                cache.remove(&key);
            }
        }
        r.map_err(|e| anyhow!(e))
    }

}

impl Inner {
    async fn fetch(&self, creds: Option<&Creds>, element: &str, inner: &str) -> Result<Node> {
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
        xml::soap_body(&resp)
    }
}

impl Tabt {

    /// Drops every cached entry whose key contains `needle` (e.g. a club id).
    #[allow(dead_code)]
    pub fn invalidate(&self, needle: &str) {
        self.inner.cache.lock().unwrap().retain(|k, _| !k.contains(needle));
    }

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

    pub async fn current_season(&self, c: &Creds) -> Result<i64> {
        let r = self.call(Some(c), "GetSeasonsRequest", &[], TTL_HOUR).await?;
        Ok(r.i("CurrentSeason"))
    }

    pub async fn club(&self, c: &Creds, season: i64, club: &str) -> Result<Option<Club>> {
        let r = self
            .call(Some(c), "GetClubs", &[("Season", season.to_string()), ("Club", club.into())], TTL_DAY)
            .await?;
        Ok(r.children("ClubEntries").next().map(club_entry))
    }

    pub async fn clubs(&self, c: &Creds, season: i64) -> Result<Vec<Club>> {
        let r = self.call(Some(c), "GetClubs", &[("Season", season.to_string())], TTL_DAY).await?;
        Ok(r.children("ClubEntries").map(club_entry).collect())
    }

    pub async fn teams(&self, c: &Creds, season: i64, club: &str) -> Result<Vec<Team>> {
        let r = self
            .call(Some(c), "GetClubTeamsRequest", &[("Club", club.into()), ("Season", season.to_string())], TTL_HOUR)
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

    pub async fn divisions(&self, c: &Creds, season: i64) -> Result<Vec<Division>> {
        let r = self.call(Some(c), "GetDivisions", &[("Season", season.to_string())], TTL_DAY).await?;
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
    pub async fn club_matches(&self, c: &Creds, season: i64, club: &str) -> Result<Vec<TeamMatch>> {
        let r = self
            .call(
                Some(c),
                "GetMatchesRequest",
                &[("Club", club.into()), ("Season", season.to_string()), ("WithDetails", "true".into())],
                TTL_SHORT,
            )
            .await?;
        Ok(r.children("TeamMatchesEntries").map(team_match).collect())
    }

    /// All matches of a division in a season (for the division-day wizard).
    #[allow(dead_code)]
    pub async fn division_matches(&self, c: &Creds, season: i64, division: i64) -> Result<Vec<TeamMatch>> {
        let r = self
            .call(
                Some(c),
                "GetMatchesRequest",
                &[("DivisionId", division.to_string()), ("Season", season.to_string()), ("WithDetails", "true".into())],
                TTL_SHORT,
            )
            .await?;
        Ok(r.children("TeamMatchesEntries").map(team_match).collect())
    }

    /// One match. `fresh` skips the cache (right before an upload, and for live tracking).
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub async fn upload(&self, c: &Creds, data: &str) -> Result<(bool, Vec<String>)> {
        let r = self.call(Some(c), "UploadRequest", &[("Data", data.into())], TTL_NONE).await?;
        Ok((r.b("Result"), r.children("ErrorLines").map(|n| n.text.clone()).collect()))
    }
}

// ---------------------------------------------------------------- parsing

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
#[allow(dead_code)]
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
        let t = Tabt::new(url);
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
