//! TabT-Upload I-line construction.

use crate::db::{Forfeit, Lineup, ResultRow};
use crate::vttl::{IndividualResult, MatchSystem, TeamMatch};

/// 1 = home, 2 = away, None = not finished. `sets_to_win` = MatchSystem.SetCount.
pub fn winner(sets: &[(u8, u8)], sets_to_win: usize) -> Option<u8> {
    let h = sets.iter().filter(|(a, b)| a > b).count();
    let a = sets.len() - h;
    if h == sets_to_win {
        Some(1)
    } else if a == sets_to_win {
        Some(2)
    } else {
        None
    }
}

/// Outcome of one position for the team score: "home" or "away" (winner), "void" (both
/// forfeit), None when not played yet. A finished local result wins over VTTL, as in the upload.
pub fn outcome(local: Option<&ResultRow>, vttl: Option<&IndividualResult>, sets_to_win: usize) -> Option<&'static str> {
    let side = |w| if w == 1 { "home" } else { "away" };
    if let Some(r) = local {
        match r.forfeit {
            Some(Forfeit::Home) => return Some("away"),
            Some(Forfeit::Away) => return Some("home"),
            Some(Forfeit::Both) => return Some("void"),
            None => {
                if let Some(w) = winner(&r.sets, sets_to_win) {
                    return Some(side(w));
                }
            }
        }
    }
    let v = vttl?;
    match v.entry.as_str() {
        "8" => Some("away"),
        "9" => Some("home"),
        "-" => Some("void"),
        _ if v.home_sets == sets_to_win as i64 => Some("home"),
        _ if v.away_sets == sets_to_win as i64 => Some("away"),
        _ => None,
    }
}

/// Checks a manual result. Returns an error message for the user.
pub fn validate(sets: &[(u8, u8)], sets_to_win: usize) -> Result<(), String> {
    for (i, &(h, a)) in sets.iter().enumerate() {
        let (hi, lo) = (h.max(a), h.min(a));
        let ok = (hi == 11 && lo <= 9) || (hi > 11 && hi - lo == 2);
        if !ok {
            return Err(format!("set {}: {h}-{a} is not a valid set score", i + 1));
        }
        if winner(&sets[..i], sets_to_win).is_some() {
            return Err(format!("set {}: the match was already won", i + 1));
        }
    }
    if winner(sets, sets_to_win).is_none() {
        return Err(format!("not finished: a player must win {sets_to_win} sets"));
    }
    Ok(())
}

/// One result entry: loser points per set, negative when away wins the set.
pub fn entry(r: &ResultRow) -> String {
    match r.forfeit {
        Some(Forfeit::Home) => "8".into(),
        Some(Forfeit::Away) => "9".into(),
        Some(Forfeit::Both) => "-".into(),
        None => r
            .sets
            .iter()
            .map(|&(h, a)| if h > a { a.to_string() } else { format!("-{h}") })
            .collect::<Vec<_>>()
            .join("/"),
    }
}

fn club(c: &str) -> String {
    if c.is_empty() || c == "-" { "bye".into() } else { c.replace('-', "").to_uppercase() }
}

fn players(l: Option<&Lineup>, m: &[crate::vttl::Player], count: i64) -> String {
    (1..=count as usize)
        .map(|pos| {
            let local = l.and_then(|l| l.players.get(pos - 1).cloned().flatten()).map(|p| p.unique_index);
            let vttl = m.iter().find(|p| p.position == pos as i64).map(|p| p.unique_index);
            local.or(vttl).map(|u| u.to_string()).unwrap_or_else(|| "?".into())
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Builds the I-line. Positions without a local finished result keep the VTTL result,
/// so an upload never erases what VTTL already has. Returns (line, warnings).
pub fn i_line(
    season: &str,
    m: &TeamMatch,
    sys: &MatchSystem,
    home: Option<&Lineup>,
    away: Option<&Lineup>,
    results: &[ResultRow],
) -> (String, Vec<String>) {
    let mut warn = vec![];
    if sys.defs.iter().any(|d| d.kind != 1) {
        warn.push("this match system has doubles: double team compositions are not sent".into());
    }
    let to_win = sys.set_count as usize;
    let mut entries = vec![];
    for pos in 1..=sys.team_match_count {
        let local = results.iter().find(|r| r.position == pos && !r.is_empty());
        let local_ok = local.filter(|r| r.forfeit.is_some() || winner(&r.sets, to_win).is_some());
        if local.is_some() && local_ok.is_none() {
            warn.push(format!("position {pos}: local result not finished, not sent"));
        }
        let vttl = m.results.iter().find(|r| r.position == pos).map(|r| r.entry.clone());
        entries.push(local_ok.map(entry).or(vttl).unwrap_or_else(|| "?".into()));
    }
    // A local captain wins over the VTTL captain.
    let home_cap = home.and_then(|l| l.captain).unwrap_or(m.home_captain);
    let away_cap = away.and_then(|l| l.captain).unwrap_or(m.away_captain);
    let cap = |c: i64| if c == 0 { "?".to_string() } else { c.to_string() };
    let hours = format!("{},?", m.time.get(..5).unwrap_or("?"));
    let line = format!(
        "I;{season};{};{};{};{};{};P{};{},{},-,-;{hours};;",
        m.match_id,
        club(&m.home_club),
        club(&m.away_club),
        players(home, &m.home_players, sys.players_per_team),
        players(away, &m.away_players, sys.players_per_team),
        entries.join(","),
        cap(home_cap),
        cap(away_cap),
    );
    if home_cap == 0 || away_cap == 0 {
        warn.push("captain not set: sent as '?'".into());
    }
    (line, warn)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(sets: Vec<(u8, u8)>) -> ResultRow {
        ResultRow { position: 1, sets, forfeit: None, source: None, rev: 1 }
    }

    #[test]
    fn doc_entry() {
        assert_eq!(entry(&row(vec![(11, 7), (4, 11), (11, 8), (11, 5)])), "7/-4/8/5");
        assert_eq!(entry(&row(vec![(0, 11)])), "-0");
    }

    #[test]
    fn validation() {
        assert!(validate(&[(11, 7), (11, 9), (11, 5)], 3).is_ok());
        assert!(validate(&[(11, 10)], 3).is_err());
        assert!(validate(&[(11, 7), (11, 9)], 3).is_err());
        assert!(validate(&[(11, 7), (11, 9), (11, 5), (11, 3)], 3).is_err());
        assert!(validate(&[(14, 12), (9, 11), (11, 2), (11, 3)], 3).is_ok());
    }

    #[test]
    fn outcomes() {
        let v = |entry: &str, h, a| IndividualResult { position: 1, entry: entry.into(), home_sets: h, away_sets: a };
        let ff = |f| ResultRow { forfeit: Some(f), ..row(vec![]) };
        assert_eq!(outcome(None, None, 3), None);
        assert_eq!(outcome(Some(&row(vec![(11, 7), (11, 9), (11, 5)])), Some(&v("8", 0, 0)), 3), Some("home"));
        assert_eq!(outcome(Some(&ff(Forfeit::Home)), None, 3), Some("away"));
        assert_eq!(outcome(Some(&ff(Forfeit::Both)), None, 3), Some("void"));
        // An unfinished local result falls back to VTTL.
        assert_eq!(outcome(Some(&row(vec![(11, 7)])), Some(&v("-7/-7/-7", 0, 3)), 3), Some("away"));
        assert_eq!(outcome(None, Some(&v("9", 0, 0)), 3), Some("home"));
        assert_eq!(outcome(None, Some(&v("7", 1, 0)), 3), None);
    }
}
