//! VTTL TabT SOAP client with a short in-memory cache (protects the request quota
//! when many browsers reload the same data).

use crate::xml::{self, Node, escape};
use anyhow::{Result, bail};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const NS: &str = "http://api.frenoy.net/TabTAPI";

pub struct Vttl {
    http: reqwest::Client,
    endpoint: String,
    account: Option<(String, String)>,
    cache: Mutex<HashMap<String, (Instant, Node)>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Player {
    pub position: i64,
    pub unique_index: i64,
    pub first_name: String,
    pub last_name: String,
    pub ranking: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndividualResult {
    pub position: i64,
    /// TabT upload entry, e.g. "7/-7/-7/-6", "8", "9", "-"
    pub entry: String,
    pub home_sets: i64,
    pub away_sets: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TeamMatch {
    pub match_id: String,
    pub unique_id: String,
    pub week: String,
    pub date: String,
    pub time: String,
    pub home_club: String,
    pub home_team: String,
    pub away_club: String,
    pub away_team: String,
    pub score: String,
    pub is_validated: bool,
    pub is_locked: bool,
    pub details_created: bool,
    pub match_system: i64,
    pub home_captain: i64,
    pub away_captain: i64,
    pub home_players: Vec<Player>,
    pub away_players: Vec<Player>,
    pub results: Vec<IndividualResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Member {
    pub unique_index: i64,
    pub first_name: String,
    pub last_name: String,
    pub ranking: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchDef {
    pub position: i64,
    /// 1 = single, other = double
    pub kind: i64,
    pub home_index: i64,
    pub away_index: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchSystem {
    pub id: i64,
    pub name: String,
    pub set_count: i64,
    pub team_match_count: i64,
    pub players_per_team: i64,
    pub defs: Vec<MatchDef>,
}

impl Vttl {
    pub fn new(endpoint: String, account: Option<(String, String)>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .build()
                .unwrap(),
            endpoint,
            account,
            cache: Mutex::new(HashMap::new()),
        }
    }

    fn creds(&self) -> String {
        match &self.account {
            Some((a, p)) => format!(
                "<ns1:Credentials><ns1:Account>{}</ns1:Account><ns1:Password>{}</ns1:Password></ns1:Credentials>",
                escape(a),
                escape(p)
            ),
            None => String::new(),
        }
    }

    async fn call(&self, element: &str, fields: &[(&str, String)], ttl: Duration) -> Result<Node> {
        let mut inner = self.creds();
        for (k, v) in fields {
            inner.push_str(&format!("<ns1:{k}>{}</ns1:{k}>", escape(v)));
        }
        let key = format!("{element}|{inner}");
        if !ttl.is_zero() {
            if let Some((t, n)) = self.cache.lock().unwrap().get(&key) {
                if t.elapsed() < ttl {
                    return Ok(n.clone());
                }
            }
        }
        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><SOAP-ENV:Envelope xmlns:SOAP-ENV="http://schemas.xmlsoap.org/soap/envelope/" xmlns:ns1="{NS}"><SOAP-ENV:Body><ns1:{element}>{inner}</ns1:{element}></SOAP-ENV:Body></SOAP-ENV:Envelope>"#
        );
        let resp = self
            .http
            .post(&self.endpoint)
            .header("Content-Type", "application/soap+xml; charset=utf-8")
            .body(body)
            .send()
            .await?
            .text()
            .await?;
        let node = xml::soap_body(&resp)?;
        if !ttl.is_zero() {
            self.cache.lock().unwrap().insert(key, (Instant::now(), node.clone()));
        }
        Ok(node)
    }

    /// Matches of a club on one day (home and away).
    pub async fn matches_on(&self, club: &str, date: &str) -> Result<Vec<TeamMatch>> {
        let r = self
            .call(
                "GetMatchesRequest",
                &[
                    ("Club", club.into()),
                    ("YearDateFrom", date.into()),
                    ("YearDateTo", date.into()),
                    ("WithDetails", "true".into()),
                ],
                Duration::from_secs(60),
            )
            .await?;
        Ok(r.children("TeamMatchesEntries").map(team_match).collect())
    }

    /// One team match. `fresh` skips the cache (used right before an upload).
    pub async fn team_match(&self, home_club: &str, match_id: &str, fresh: bool) -> Result<TeamMatch> {
        let ttl = if fresh { Duration::ZERO } else { Duration::from_secs(60) };
        let r = self
            .call(
                "GetMatchesRequest",
                &[
                    ("Club", home_club.into()),
                    ("MatchId", match_id.into()),
                    ("WithDetails", "true".into()),
                ],
                ttl,
            )
            .await?;
        match r.children("TeamMatchesEntries").next() {
            Some(n) => Ok(team_match(n)),
            None => bail!("match {match_id} not found on VTTL"),
        }
    }

    pub async fn members(&self, club: &str) -> Result<Vec<Member>> {
        let r = self
            .call("GetMembersRequest", &[("Club", club.into())], Duration::from_secs(3600))
            .await?;
        let mut v: Vec<Member> = r
            .children("MemberEntries")
            .map(|m| Member {
                unique_index: m.i("UniqueIndex"),
                first_name: m.s("FirstName"),
                last_name: m.s("LastName"),
                ranking: m.s("Ranking"),
            })
            .collect();
        v.sort_by(|a, b| {
            (rank_key(&a.ranking), &a.last_name, &a.first_name).cmp(&(rank_key(&b.ranking), &b.last_name, &b.first_name))
        });
        Ok(v)
    }

    pub async fn match_system(&self, id: i64) -> Result<MatchSystem> {
        let r = self
            .call("GetMatchSystems", &[("UniqueIndex", id.to_string())], Duration::from_secs(86400))
            .await?;
        let Some(e) = r.children("MatchSystemEntries").next() else {
            bail!("match system {id} not found");
        };
        let defs: Vec<MatchDef> = e
            .children("TeamMatchDefinitionEntries")
            .map(|d| MatchDef {
                position: d.i("Position"),
                kind: d.i("Type"),
                home_index: d.i("HomePlayerIndex"),
                away_index: d.i("AwayPlayerIndex"),
            })
            .collect();
        let players_per_team = defs
            .iter()
            .filter(|d| d.kind == 1)
            .map(|d| d.home_index.max(d.away_index))
            .max()
            .unwrap_or(0);
        Ok(MatchSystem {
            id,
            name: e.s("Name"),
            set_count: e.i("SetCount"),
            team_match_count: e.i("TeamMatchCount"),
            players_per_team,
            defs,
        })
    }

    pub async fn current_season(&self) -> Result<String> {
        let r = self.call("GetSeasonsRequest", &[], Duration::from_secs(3600)).await?;
        Ok(r.s("CurrentSeason"))
    }

    /// Returns (Result, ErrorLines).
    pub async fn upload(&self, data: &str) -> Result<(bool, Vec<String>)> {
        if self.account.is_none() {
            bail!("VTTL_ACCOUNT / VTTL_PASSWORD are required for an upload");
        }
        let r = self
            .call("UploadRequest", &[("Data", data.into())], Duration::ZERO)
            .await?;
        Ok((r.b("Result"), r.children("ErrorLines").map(|n| n.text.clone()).collect()))
    }
}

/// Sort key for a Belgian ranking, best first: A (A1 before A20), B0 … E6, NG, then unknown.
pub fn rank_key(ranking: &str) -> (u8, u32) {
    let r = ranking.trim().to_uppercase();
    if r == "NG" {
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

fn players(n: Option<&Node>) -> Vec<Player> {
    let Some(n) = n else { return vec![] };
    let mut v: Vec<Player> = n
        .children("Players")
        .map(|p| Player {
            position: p.i("Position"),
            unique_index: p.i("UniqueIndex"),
            first_name: p.s("FirstName"),
            last_name: p.s("LastName"),
            ranking: p.s("Ranking"),
        })
        .filter(|p| p.unique_index != 0) // empty lineup placeholder
        .collect();
    v.sort_by_key(|p| p.position);
    v
}

fn team_match(n: &Node) -> TeamMatch {
    let d = n.child("MatchDetails");
    let ds = |k: &str| d.map(|d| d.s(k)).unwrap_or_default();
    let results = d
        .map(|d| {
            d.children("IndividualMatchResults")
                .map(|r| {
                    let entry = if r.b("IsHomeForfeited") && r.b("IsAwayForfeited") {
                        "-".to_string()
                    } else if r.b("IsHomeForfeited") {
                        "8".into()
                    } else if r.b("IsAwayForfeited") {
                        "9".into()
                    } else {
                        r.s("Scores")
                            .split(',')
                            .filter_map(|s| s.split('|').nth(1))
                            .collect::<Vec<_>>()
                            .join("/")
                    };
                    IndividualResult {
                        position: r.i("Position"),
                        entry,
                        home_sets: r.i("HomeSetCount"),
                        away_sets: r.i("AwaySetCount"),
                    }
                })
                .filter(|r| !r.entry.is_empty())
                .collect()
        })
        .unwrap_or_default();
    TeamMatch {
        match_id: n.s("MatchId"),
        unique_id: n.s("MatchUniqueId"),
        week: n.s("WeekName"),
        date: n.s("Date"),
        time: n.s("Time"),
        home_club: n.s("HomeClub"),
        home_team: n.s("HomeTeam"),
        away_club: n.s("AwayClub"),
        away_team: n.s("AwayTeam"),
        score: n.s("Score"),
        is_validated: n.b("IsValidated"),
        is_locked: n.b("IsLocked"),
        details_created: ds("DetailsCreated") == "true",
        match_system: ds("MatchSystem").parse().unwrap_or(0),
        home_captain: ds("HomeCaptain").parse().unwrap_or(0),
        away_captain: ds("AwayCaptain").parse().unwrap_or(0),
        home_players: players(d.and_then(|d| d.child("HomePlayers"))),
        away_players: players(d.and_then(|d| d.child("AwayPlayers"))),
        results,
    }
}

#[cfg(test)]
mod tests {
    use super::rank_key;

    #[test]
    fn ranking_order() {
        let mut v = vec!["NG", "C2", "A12", "B0", "E6", "A", "A3", "D4", "x", "B6"];
        v.sort_by_key(|r| rank_key(r));
        assert_eq!(v, ["A", "A3", "A12", "B0", "B6", "C2", "D4", "E6", "NG", "x"]);
    }
}
