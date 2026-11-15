//! Reads VTTL in the background, so the results are there before somebody asks.
//!
//! A club is on the list while one of its members has a session, or while somebody watches
//! it (an open WebSocket, also a spectator). For each club:
//!
//! | Situation                                    | Call                                  |
//! |----------------------------------------------|---------------------------------------|
//! | Always                                       | The full season, once a day           |
//! | Match evening, somebody watches              | The matches of the day, each minute   |
//! | Match evening, nobody watches                | The matches of the day, each 5 min    |
//! | All matches complete, or the evening is over | One check an hour later, one at 08:00 |
//! | All matches validated by VTTL                | Nothing more                          |
//!
//! The match evening starts 15 minutes before the first match and ends 5 hours after the
//! last start. A home match with a stored record is live in our own data: it does not need
//! the fast rate.
//!
//! The day answer is small (the season is about 800 kB). `Tabt::club_matches` puts it on top
//! of the season, so every reader gets the new scores without a call to VTTL.
//!
//! Each call uses the login of a club member with a session, a different one each time.
//! Without a login, or when VTTL refuses it, the call goes without credentials.

use crate::api::same_club;
use crate::app::App;
use crate::db::{self, Db};
use crate::tabt::{Creds, Team, TeamMatch};
use crate::when::{local_day, local_unix};
use crate::{rules, xml};
use rusqlite::params;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const TICK: Duration = Duration::from_secs(20);
const OPEN_BEFORE: i64 = 15 * 60;
const CLOSE_AFTER: i64 = 5 * 3600;
const FAST: i64 = 60;
const SLOW: i64 = 300;
const SEASON_EVERY: i64 = 86400;
const CHECK_LATER: i64 = 3600;
const MORNING: &str = "08:00";
/// After a failed call: wait 1, 2, 4 ... minutes, at most 15.
fn backoff(failures: u32) -> i64 {
    (60 * 2i64.pow(failures.saturating_sub(1).min(4))).min(900)
}

// ---------------------------------------------------------------- watchers

/// How many WebSockets are open for each club.
#[derive(Default, Clone)]
pub struct Watchers(Arc<Mutex<HashMap<String, usize>>>);

/// One open WebSocket. Dropping it ends the watch.
pub struct Watching {
    all: Watchers,
    club: String,
}

impl Watchers {
    pub fn enter(&self, club: &str) -> Watching {
        *self.0.lock().unwrap().entry(club.to_string()).or_default() += 1;
        Watching { all: self.clone(), club: club.to_string() }
    }

    fn clubs(&self) -> Vec<String> {
        self.0.lock().unwrap().keys().cloned().collect()
    }

    fn any(&self, club: &str) -> bool {
        self.0.lock().unwrap().contains_key(club)
    }
}

impl Drop for Watching {
    fn drop(&mut self) {
        let mut m = self.all.0.lock().unwrap();
        if let Some(n) = m.get_mut(&self.club) {
            *n -= 1;
            if *n == 0 {
                m.remove(&self.club);
            }
        }
    }
}

// ---------------------------------------------------------------- logins

impl Db {
    /// The clubs that have a member with a session.
    pub fn session_clubs(&self) -> anyhow::Result<Vec<String>> {
        let c = self.conn();
        let mut st = c.prepare(
            "SELECT DISTINCT a.club FROM account a JOIN session s ON s.account = a.account
             WHERE a.club IS NOT NULL AND s.last_seen >= ?1",
        )?;
        let rows = st.query_map(params![db::now() - db::SESSION_IDLE], |r| r.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Accounts of the club with a session (so the server has their password), newest use first.
    pub fn poll_accounts(&self, club: &str) -> anyhow::Result<Vec<String>> {
        let c = self.conn();
        let mut st = c.prepare(
            "SELECT a.account FROM account a JOIN session s ON s.account = a.account
             WHERE a.club = ?1 AND s.last_seen >= ?2 GROUP BY a.account ORDER BY MAX(s.last_seen) DESC LIMIT 5",
        )?;
        let rows = st.query_map(params![club, db::now() - db::SESSION_IDLE], |r| r.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

/// The logins to try for a call, in order: two accounts of the club (`turn` moves the start,
/// so the calls are spread over the accounts), then no login.
fn logins(app: &App, club: &str, turn: usize) -> Vec<Option<Creds>> {
    let names = app.db.poll_accounts(club).unwrap_or_default();
    let mut out: Vec<Option<Creds>> = (0..names.len().min(2))
        .filter_map(|i| {
            let account = &names[(turn + i) % names.len()];
            let sealed = app.db.account_secret(account).ok()??;
            let password = app.crypto.decrypt(&sealed)?;
            Some(Some(Creds { account: account.clone(), password }))
        })
        .collect();
    out.push(None);
    out
}

#[derive(Clone, Copy)]
enum What<'a> {
    Season,
    Day(&'a str),
}

/// Gets the season or one day of a club from VTTL, with the first login that VTTL accepts.
/// A refusal by VTTL (a SOAP fault, for example a changed password) goes to the next login;
/// no answer at all stops at once.
async fn fetch(app: &App, club: &str, season: i64, turn: usize, what: What<'_>) -> anyhow::Result<()> {
    let mut last = None;
    for c in logins(app, club, turn) {
        let r = match what {
            What::Season => app.tabt.refresh_club_matches(c.as_ref(), season, club).await,
            What::Day(day) => app.tabt.refresh_day(c.as_ref(), season, club, day).await,
        };
        match r {
            Ok(()) => return Ok(()),
            Err(e) if e.downcast_ref::<xml::Fault>().is_some() => {
                tracing::debug!(club, account = c.as_ref().map(|c| c.account.as_str()).unwrap_or("-"), "vttl refused: {e:#}");
                last = Some(e);
            }
            Err(e) => return Err(e),
        }
    }
    Err(last.unwrap_or_else(|| anyhow::anyhow!("no login")))
}

// ---------------------------------------------------------------- when to ask

/// The matches of a club on one day, as the schedule sees them.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Evening {
    /// First and last start (Unix seconds).
    first: i64,
    last: i64,
    /// Every match has its full result.
    complete: bool,
    /// A match without a result is not live in our own data.
    fast: bool,
}

fn total_games(m: &TeamMatch, teams: &[Team]) -> usize {
    let system = m
        .details
        .as_ref()
        .map(|d| d.match_system)
        .filter(|s| *s != 0)
        .or_else(|| teams.iter().find(|t| t.division_id == m.division_id).map(|t| t.match_type));
    system.and_then(rules::system).map(|s| s.games.len()).unwrap_or(0)
}

fn is_complete(m: &TeamMatch, total: usize) -> bool {
    if m.is_validated || m.is_locked || m.is_home_forfeited || m.is_away_forfeited || m.is_home_withdrawn || m.is_away_withdrawn {
        return true;
    }
    let played: Option<usize> = m.score.split_once('-').and_then(|(h, a)| Some(h.trim().parse::<usize>().ok()? + a.trim().parse::<usize>().ok()?));
    total > 0 && played.is_some_and(|p| p >= total)
}

/// `recorded`: the ids of the matches of the club with a stored record.
fn evening(day: &str, club: &str, matches: &[&TeamMatch], teams: &[Team], recorded: &HashSet<String>) -> Evening {
    let starts: Vec<i64> = matches.iter().filter_map(|m| local_unix(day, &m.time)).collect();
    // No hour known: the whole day.
    let first = starts.iter().copied().min().or_else(|| local_unix(day, "09:00")).unwrap_or(0);
    let last = starts.iter().copied().max().or_else(|| local_unix(day, "19:00")).unwrap_or(0);
    let open: Vec<&&TeamMatch> = matches.iter().filter(|m| !is_complete(m, total_games(m, teams))).collect();
    let fast = open.iter().any(|m| !(same_club(&m.home_club, club) && recorded.contains(&m.match_id)));
    Evening { first, last, complete: open.is_empty(), fast }
}

/// Is a call for this day due? `last` = the newest answer we have for the day, `complete_since`
/// = when we first saw every result, `morning` = 08:00 of the next day (all Unix seconds).
fn due(now: i64, last: Option<i64>, e: &Evening, watched: bool, complete_since: Option<i64>, morning: i64) -> bool {
    let close = e.last + CLOSE_AFTER;
    if now < e.first - OPEN_BEFORE {
        return false;
    }
    let Some(last) = last else { return true };
    if !e.complete && now <= close {
        return now - last >= if watched && e.fast { FAST } else { SLOW };
    }
    let later = complete_since.unwrap_or(close) + CHECK_LATER;
    [later, morning].iter().any(|t| last < *t && *t <= now)
}

// ---------------------------------------------------------------- the loop

#[derive(Default)]
struct ClubState {
    /// Counts the calls: picks the login.
    turn: usize,
    failures: u32,
    retry_at: i64,
    /// Day -> when we first saw that every match of the day was complete.
    complete_since: HashMap<String, i64>,
}

pub async fn run(app: Arc<App>) {
    tokio::time::sleep(Duration::from_secs(15)).await;
    let mut states: HashMap<String, ClubState> = HashMap::new();
    loop {
        if let Err(e) = tick(&app, &mut states).await {
            tracing::warn!("vttl poll: {e:#}");
        }
        tokio::time::sleep(TICK).await;
    }
}

async fn tick(app: &Arc<App>, states: &mut HashMap<String, ClubState>) -> anyhow::Result<()> {
    let mut clubs = app.db.session_clubs()?;
    for c in app.watchers.clubs() {
        if !clubs.contains(&c) {
            clubs.push(c);
        }
    }
    states.retain(|c, _| clubs.contains(c));
    if clubs.is_empty() {
        return Ok(());
    }
    let season = app.tabt.current_season(None).await?;
    for club in clubs {
        let st = states.entry(club.clone()).or_default();
        let now = db::now();
        if now < st.retry_at {
            continue;
        }
        match poll_club(app, &club, season, now, st).await {
            Ok(()) => st.failures = 0,
            Err(e) => {
                st.failures += 1;
                st.retry_at = now + backoff(st.failures);
                tracing::warn!(club, failures = st.failures, "vttl poll: {e:#}");
            }
        }
    }
    Ok(())
}

async fn poll_club(app: &Arc<App>, club: &str, season: i64, now: i64, st: &mut ClubState) -> anyhow::Result<()> {
    // The calendar: once a day, and the first time for a new club.
    let mut cached = app.tabt.club_matches_cached(season, club);
    if cached.as_ref().is_none_or(|(at, _)| now - at / 1000 >= SEASON_EVERY) {
        st.turn += 1;
        fetch(app, club, season, st.turn, What::Season).await?;
        tracing::debug!(club, "vttl poll: season");
        cached = app.tabt.club_matches_cached(season, club);
    }
    let Some((season_at, matches)) = cached else { return Ok(()) };

    let today = local_day(now);
    st.complete_since.retain(|d, _| *d >= local_day(now - 86400));
    for day in [local_day(now - 86400), today] {
        let of_day: Vec<&TeamMatch> = matches.iter().filter(|m| m.date == day && !m.is_bye).collect();
        if of_day.is_empty() || of_day.iter().all(|m| m.is_validated) {
            continue;
        }
        let teams = app.tabt.teams(None, season, club).await.unwrap_or_default();
        let recorded: HashSet<String> = app.db.sheets_on(club, &day)?.into_iter().map(|s| s.id).collect();
        let e = evening(&day, club, &of_day, &teams, &recorded);
        let last = app.tabt.day_fetched_at(season, club, &day).unwrap_or(0).max(season_at) / 1000;
        if e.complete {
            st.complete_since.entry(day.clone()).or_insert(last);
        } else {
            st.complete_since.remove(&day);
        }
        let morning = local_unix(&day, MORNING).unwrap_or(0) + 86400;
        if !due(now, Some(last), &e, app.watchers.any(club), st.complete_since.get(&day).copied(), morning) {
            continue;
        }
        st.turn += 1;
        fetch(app, club, season, st.turn, What::Day(&day)).await?;
        tracing::debug!(club, day, complete = e.complete, "vttl poll: day");
    }
    Ok(())
}

/// Gets one day of a club again in the background (after an upload: VTTL has new data).
pub fn refresh_day_soon(app: &Arc<App>, club: &str, season: i64, day: &str) {
    let (app, club, day) = (app.clone(), club.to_string(), day.to_string());
    tokio::spawn(async move {
        if let Err(e) = fetch(&app, &club, season, 0, What::Day(&day)).await {
            tracing::warn!(club, day, "vttl refresh after upload: {e:#}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: &str, time: &str, score: &str, home: &str) -> TeamMatch {
        let x = format!(
            "<Envelope><Body><R><TeamMatchesEntries><MatchId>{id}</MatchId><Date>2026-11-14</Date><Time>{time}</Time>
             <HomeClub>{home}</HomeClub><AwayClub>X9</AwayClub><Score>{score}</Score>
             <MatchDetails><MatchSystem>2</MatchSystem></MatchDetails></TeamMatchesEntries></R></Body></Envelope>"
        );
        crate::tabt::parse_match(&x)
    }

    #[test]
    fn evening_of_a_club() {
        let total = rules::system(2).unwrap().games.len();
        assert_eq!(total, 16);
        let (a, b) = (m("H/1", "19:00:00", "12-4", "WVL154"), m("H/2", "14:00:00", "3-2", "X1"));
        let start = |t| local_unix("2026-11-14", t).unwrap();
        let none = HashSet::new();
        // The away match has no full result yet.
        let e = evening("2026-11-14", "WVL154", &[&a, &b], &[], &none);
        assert_eq!(e, Evening { first: start("14:00"), last: start("19:00"), complete: false, fast: true });
        // A home match without a result: fast, unless we have the record ourselves.
        let c = m("H/3", "19:00:00", "", "WVL154");
        assert!(evening("2026-11-14", "WVL154", &[&a, &c], &[], &none).fast);
        let ours: HashSet<String> = ["H/3".to_string()].into();
        let e = evening("2026-11-14", "WVL154", &[&a, &c], &[], &ours);
        assert!(!e.fast && !e.complete);
        assert!(evening("2026-11-14", "WVL154", &[&a], &[], &none).complete);
        // No hour: the whole day.
        let d = m("H/4", "", "", "X1");
        let e = evening("2026-11-14", "WVL154", &[&d], &[], &none);
        assert_eq!((e.first, e.last), (start("09:00"), start("19:00")));
    }

    #[test]
    fn when_a_call_is_due() {
        let t = |h: &str| local_unix("2026-11-14", h).unwrap();
        let morning = t("08:00") + 86400;
        let busy = Evening { first: t("19:00"), last: t("19:00"), complete: false, fast: true };
        // Before the evening: nothing.
        assert!(!due(t("18:44"), Some(t("03:00")), &busy, true, None, morning));
        assert!(due(t("18:45"), Some(t("03:00")), &busy, true, None, morning));
        // During: each minute with a watcher, each 5 minutes without.
        assert!(!due(t("20:00") + 59, Some(t("20:00")), &busy, true, None, morning));
        assert!(due(t("20:00") + 60, Some(t("20:00")), &busy, true, None, morning));
        assert!(!due(t("20:00") + 299, Some(t("20:00")), &busy, false, None, morning));
        assert!(due(t("20:00") + 300, Some(t("20:00")), &busy, false, None, morning));
        // Only our own live matches are open: the slow rate, also with a watcher.
        let own = Evening { fast: false, ..busy };
        assert!(!due(t("20:00") + 60, Some(t("20:00")), &own, true, None, morning));

        // All complete at 22:30: one check at 23:30, one at 08:00, then nothing.
        let done = Evening { complete: true, fast: false, ..busy };
        let since = Some(t("22:30"));
        assert!(!due(t("22:45"), Some(t("22:30")), &done, true, since, morning));
        assert!(due(t("23:30"), Some(t("22:30")), &done, true, since, morning));
        assert!(!due(t("23:59"), Some(t("23:30")), &done, true, since, morning));
        assert!(due(morning, Some(t("23:30")), &done, false, since, morning));
        assert!(!due(morning + 7200, Some(morning), &done, true, since, morning));

        // Not complete at the end of the evening (00:00): one check at 01:00, one at 08:00.
        let close = t("19:00") + CLOSE_AFTER;
        assert!(due(close, Some(close - 300), &busy, false, None, morning));
        assert!(!due(close + 600, Some(close), &busy, true, None, morning));
        assert!(due(close + 3600, Some(close), &busy, true, None, morning));
        assert!(!due(close + 7200, Some(close + 3600), &busy, true, None, morning));
        assert!(due(morning + 60, Some(close + 3600), &busy, false, None, morning));
    }

    #[test]
    fn watchers_count() {
        let w = Watchers::default();
        let a = w.enter("WVL154");
        let b = w.enter("WVL154");
        assert!(w.any("WVL154"));
        drop(a);
        assert!(w.any("WVL154"));
        drop(b);
        assert!(!w.any("WVL154") && w.clubs().is_empty());
    }

    #[test]
    fn waits_after_failures() {
        assert_eq!((backoff(1), backoff(2), backoff(3), backoff(9)), (60, 120, 240, 900));
    }
}
