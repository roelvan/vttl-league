//! The match rules on the server, so the server can build and send the VTTL upload itself.
//!
//! A port of web/src/lib/sheet/{systems,score,export}.ts and `canonical` from vttl.ts (which
//! are ports of PieterP ScoreSheet). Keep both sides equal: `tests/upload-fixtures.json` is
//! written by the web tests (export.fixtures.test.ts) and checked here.

use serde::{Deserialize, Deserializer};
use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;

// ---------------------------------------------------------------- the stored document

/// null in the JSON (arrays grow with nulls) reads as the default value.
fn nd<'de, D: Deserializer<'de>, T: Deserialize<'de> + Default>(d: D) -> Result<T, D::Error> {
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SetScore {
    #[serde(default, deserialize_with = "nd")]
    pub l: String,
    #[serde(default, deserialize_with = "nd")]
    pub r: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    #[serde(default, deserialize_with = "nd")]
    pub computer_number: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    #[serde(default, deserialize_with = "nd")]
    pub computer_number: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Team {
    #[serde(default, deserialize_with = "nd")]
    pub club_id: String,
    #[serde(default, deserialize_with = "nd")]
    pub forfeit: bool,
    #[serde(default, deserialize_with = "nd")]
    pub players: Vec<Option<Player>>,
    #[serde(default, deserialize_with = "nd")]
    pub doubles: Vec<Option<String>>,
    #[serde(default, deserialize_with = "nd")]
    pub substitutes: Vec<Option<i64>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sheet {
    #[serde(default, deserialize_with = "nd")]
    pub season: i64,
    #[serde(default, deserialize_with = "nd")]
    pub match_id: String,
    #[serde(default, deserialize_with = "nd")]
    pub is_official: bool,
    #[serde(default, deserialize_with = "nd")]
    pub match_system_id: i64,
    #[serde(default, deserialize_with = "nd")]
    pub start_hour: String,
    #[serde(default, deserialize_with = "nd")]
    pub end_hour: String,
    #[serde(default, deserialize_with = "nd")]
    pub comments: String,
    #[serde(default, deserialize_with = "nd")]
    pub home: Team,
    #[serde(default, deserialize_with = "nd")]
    pub away: Team,
    #[serde(default, deserialize_with = "nd")]
    pub results: Vec<Option<Vec<Option<SetScore>>>>,
    #[serde(default, deserialize_with = "nd")]
    pub chief_referee: Person,
    #[serde(default, deserialize_with = "nd")]
    pub home_captain: Person,
    #[serde(default, deserialize_with = "nd")]
    pub away_captain: Person,
    #[serde(default, deserialize_with = "nd")]
    pub room_commissioner: Person,
    #[serde(default, deserialize_with = "nd")]
    pub vttl_locked: bool,
    #[serde(default, deserialize_with = "nd")]
    pub disable_partial_upload: bool,
    #[serde(default, deserialize_with = "nd")]
    pub vttl_seen: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Home,
    Away,
}

impl Sheet {
    fn team(&self, s: Side) -> &Team {
        if s == Side::Home { &self.home } else { &self.away }
    }
    fn sets(&self, gi: usize) -> Vec<SetScore> {
        self.results.get(gi).cloned().flatten().unwrap_or_default().into_iter().map(Option::unwrap_or_default).collect()
    }
}

impl Team {
    fn player_cn(&self, i: usize) -> &str {
        self.players.get(i).and_then(|p| p.as_ref()).map(|p| p.computer_number.as_str()).unwrap_or("")
    }
    fn double(&self, i: usize) -> Option<&str> {
        self.doubles.get(i).and_then(|d| d.as_deref())
    }
    fn substitute(&self, i: usize) -> Option<i64> {
        self.substitutes.get(i).copied().flatten()
    }
}

// ---------------------------------------------------------------- match systems

#[derive(Debug, Clone)]
pub enum Slot {
    Single { optional: bool, player: usize },
    Double { options: Vec<&'static str>, double: usize },
    Sub { sub: usize, transferable: usize },
}

#[derive(Debug, Clone)]
pub struct Game {
    pub pos: usize,
    pub home: Vec<usize>,
    pub away: Vec<usize>,
    pub allow_sub: bool,
}

#[derive(Debug, Clone)]
pub struct MatchSystem {
    pub id: i64,
    pub set_count: u32,
    pub point_count: u32,
    pub boxes: usize,
    pub competitive: bool,
    pub slots: Vec<Slot>,
    pub games: Vec<Game>,
}

type G = (&'static [usize], &'static [usize], bool);

fn build(id: i64, set_count: u32, point_count: u32, competitive: bool, slots: Vec<Slot>, games: &[G]) -> MatchSystem {
    MatchSystem {
        id,
        set_count,
        point_count,
        boxes: (set_count * 2 - 1) as usize,
        competitive,
        slots,
        games: games.iter().enumerate().map(|(i, (h, a, s))| Game { pos: i + 1, home: h.to_vec(), away: a.to_vec(), allow_sub: *s }).collect(),
    }
}

fn singles(n: usize, optional_from: usize) -> Vec<Slot> {
    (0..n).map(|i| Slot::Single { optional: i >= optional_from, player: i }).collect()
}
fn dbl(options: &[&'static str]) -> Slot {
    Slot::Double { options: options.to_vec(), double: 0 }
}

const fn gs(h: &'static [usize], a: &'static [usize], sub: bool) -> G {
    (h, a, sub)
}

macro_rules! g {
    ([$($h:expr),*], [$($a:expr),*]) => { gs(&[$($h),*], &[$($a),*], false) };
    ($h:expr, $a:expr) => { gs(&[$h], &[$a], false) };
    ($h:expr, $a:expr, sub) => { gs(&[$h], &[$a], true) };
}

const P123: &[&str] = &["1/2", "1/3", "2/3"];
const P12: &[&str] = &["1/2"];
const P1234: &[&str] = &["1/2", "1/3", "1/4", "2/3", "2/4", "3/4"];

static MEN: [G; 16] = [
    g!(3, 1), g!(2, 0), g!(1, 3), g!(0, 2), g!(3, 0), g!(2, 1), g!(1, 2), g!(0, 3),
    g!(3, 2), g!(2, 3), g!(1, 0), g!(0, 1), g!(3, 3), g!(2, 2), g!(1, 1), g!(0, 0),
];
static WOMEN: [G; 10] = [g!(2, 1), g!(1, 0), g!(0, 2), g!(2, 0), g!(1, 2), g!(0, 1), g!(3, 3), g!(2, 2), g!(1, 1), g!(0, 0)];

static SYSTEMS: LazyLock<HashMap<i64, MatchSystem>> = LazyLock::new(|| {
    let n = usize::MAX;
    [
        build(1, 2, 21, true, singles(4, n), &MEN),
        build(2, 3, 11, true, singles(4, n), &MEN),
        build(3, 2, 21, true, [singles(3, n), vec![dbl(P123)]].concat(), &WOMEN),
        build(4, 3, 11, true, [singles(3, n), vec![dbl(P123)]].concat(), &WOMEN),
        build(5, 3, 11, true, [singles(3, n), vec![dbl(P123)]].concat(), &[g!(0, 1), g!(1, 0), g!(2, 2), g!(3, 3), g!(0, 0), g!(2, 1), g!(1, 2)]),
        build(6, 3, 11, true, [singles(2, n), vec![dbl(P12)]].concat(), &[g!(1, 0), g!(0, 1), g!(2, 2), g!(1, 1), g!(0, 0)]),
        build(
            7,
            3,
            11,
            false,
            singles(3, n),
            &[g!([0, 1], [0, 1]), g!(2, 2), g!(0, 1), g!(1, 0), g!([0, 2], [0, 2]), g!(1, 1), g!(2, 0), g!(0, 2), g!([1, 2], [1, 2]), g!(0, 0), g!(2, 1), g!(1, 2)],
        ),
        build(
            8,
            3,
            11,
            true,
            [singles(4, 3), vec![Slot::Sub { sub: 0, transferable: 3 }]].concat(),
            &[g!(0, 1), g!(1, 0), g!(2, 2), g!(0, 0, sub), g!(2, 1, sub), g!(1, 2, sub)],
        ),
        build(9, 3, 11, true, singles(3, n), &[g!(2, 1), g!(1, 0), g!(0, 2), g!(2, 0), g!(1, 2), g!(0, 1), g!(2, 2), g!(1, 1), g!(0, 0)]),
        build(-2, 4, 11, false, [singles(4, n), vec![dbl(P1234)]].concat(), &[g!(0, 0), g!(1, 1), g!(4, 4), g!(0, 1), g!(1, 0)]),
    ]
    .into_iter()
    .map(|s| (s.id, s))
    .collect()
});

pub fn system(id: i64) -> Option<&'static MatchSystem> {
    SYSTEMS.get(&id)
}

// ---------------------------------------------------------------- score

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Won {
    Empty,
    Partial,
    Home,
    HomeFF,
    HomeWO,
    Away,
    AwayFF,
    AwayWO,
    Neither,
    Error,
}

impl Won {
    fn home(self) -> bool {
        matches!(self, Won::Home | Won::HomeFF | Won::HomeWO)
    }
    fn away(self) -> bool {
        matches!(self, Won::Away | Won::AwayFF | Won::AwayWO)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Winner {
    Home,
    Away,
    Draw,
    Error,
    Incomplete,
}

fn is_empty_set(s: Option<&SetScore>) -> bool {
    s.is_none_or(|s| s.l.is_empty() && s.r.is_empty())
}

fn digits(s: &str) -> Option<u32> {
    (!s.is_empty() && s.chars().all(|c| c.is_ascii_digit())).then(|| s.parse().unwrap_or(u32::MAX))
}

pub fn set_winner(index: usize, s: &SetScore, sys: &MatchSystem) -> Won {
    let left = s.l.trim().to_uppercase();
    let right = s.r.trim().to_uppercase();
    if index > 0 && (left == "WO" || right == "WO") {
        return Won::Error;
    }
    if left == "WO" && right == "WO" {
        return Won::Neither;
    }
    if left == "FF" || left == "WO" {
        if !right.is_empty() {
            return Won::Error;
        }
        return if left == "FF" { Won::AwayFF } else { Won::AwayWO };
    }
    if right == "FF" || right == "WO" {
        if !left.is_empty() {
            return Won::Error;
        }
        return if right == "FF" { Won::HomeFF } else { Won::HomeWO };
    }
    let (Some(home), Some(away)) = (digits(&left), digits(&right)) else { return Won::Error };
    let pc = sys.point_count;
    if home > away {
        if home < pc || (home > pc && home - away != 2) || home - away < 2 {
            return Won::Error;
        }
        return Won::Home;
    }
    if away < pc || (away > pc && away - home != 2) || away.saturating_sub(home) < 2 {
        return Won::Error;
    }
    Won::Away
}

/// (result, home sets, away sets)
pub fn game_result(sets: &[SetScore], sys: &MatchSystem, home_forfeit: bool, away_forfeit: bool) -> (Won, u32, u32) {
    if home_forfeit {
        return (Won::AwayWO, 0, 0);
    }
    if away_forfeit {
        return (Won::HomeWO, 0, 0);
    }
    let boxes: Vec<Option<&SetScore>> = (0..sys.boxes).map(|i| sets.get(i)).collect();
    if boxes.iter().all(|b| is_empty_set(*b)) {
        return (Won::Empty, 0, 0);
    }
    let mut result = Won::Empty;
    let (mut hs, mut aws) = (0u32, 0u32);
    for i in 0..boxes.len() {
        let s = boxes[i];
        if result != Won::Empty {
            if !is_empty_set(s) {
                result = Won::Error;
            }
            continue;
        }
        if is_empty_set(s) && boxes[i..].iter().all(|b| is_empty_set(*b)) {
            result = Won::Partial;
            break;
        }
        let empty = SetScore::default();
        let w = set_winner(i, s.unwrap_or(&empty), sys);
        if w == Won::Error {
            result = Won::Error;
        } else if w.home() {
            if hs < sys.set_count {
                hs += 1;
                if hs == sys.set_count || w == Won::HomeFF || w == Won::HomeWO {
                    result = w;
                }
            } else {
                result = Won::Error;
            }
        } else if w.away() {
            if aws < sys.set_count {
                aws += 1;
                if aws == sys.set_count || w == Won::AwayFF || w == Won::AwayWO {
                    result = w;
                }
            } else {
                result = Won::Error;
            }
        } else if w == Won::Neither {
            result = Won::Neither;
        }
    }
    match result {
        Won::HomeWO => (result, sys.set_count, 0),
        Won::AwayWO => (result, 0, sys.set_count),
        Won::HomeFF => (result, sys.set_count, aws),
        Won::AwayFF => (result, hs, sys.set_count),
        _ => (result, hs, aws),
    }
}

/// Slot indexes that play a game on one side, with the Super substitute applied.
pub fn game_slots(sheet: &Sheet, sys: &MatchSystem, gi: usize, side: Side) -> Vec<usize> {
    let g = &sys.games[gi];
    let team = sheet.team(side);
    for slot in &sys.slots {
        if let Slot::Sub { sub, transferable } = slot
            && team.substitute(*sub) == Some(g.pos as i64)
            && g.allow_sub
        {
            return vec![*transferable];
        }
    }
    if side == Side::Home { g.home.clone() } else { g.away.clone() }
}

pub fn is_bye_club(c: &str) -> bool {
    matches!(c.trim().to_lowercase().as_str(), "-" | "bye" | "vrij")
}

pub fn is_player_wo(sheet: &Sheet, sys: &MatchSystem, side: Side, slot: usize) -> bool {
    if sheet.team(side).forfeit {
        return true;
    }
    let mut played = 0;
    for gi in 0..sys.games.len() {
        if !game_slots(sheet, sys, gi, side).contains(&slot) {
            continue;
        }
        played += 1;
        let first = sheet.sets(gi).into_iter().next().unwrap_or_default();
        let w = if sheet.home.forfeit {
            Won::AwayWO
        } else if sheet.away.forfeit {
            Won::HomeWO
        } else {
            set_winner(0, &first, sys)
        };
        let lost_by_wo = if side == Side::Home { w == Won::AwayWO } else { w == Won::HomeWO };
        if !lost_by_wo && w != Won::Neither {
            return false;
        }
    }
    played != 0
}

#[derive(Debug, Clone, PartialEq)]
pub struct Score {
    pub home_wins: usize,
    pub away_wins: usize,
    pub winner: Winner,
}

impl Score {
    pub fn complete(&self) -> bool {
        matches!(self.winner, Winner::Home | Winner::Away | Winner::Draw)
    }
}

pub fn compute_score(sheet: &Sheet, sys: &MatchSystem) -> Score {
    let (hf, af) = (sheet.home.forfeit, sheet.away.forfeit);
    let bye = is_bye_club(&sheet.home.club_id) || is_bye_club(&sheet.away.club_id);
    let n = sys.games.len();
    if bye || hf || af {
        let (h, a) = if bye { (0, 0) } else if hf { (0, n) } else { (n, 0) };
        let winner = if h == a { Winner::Draw } else if h > a { Winner::Home } else { Winner::Away };
        return Score { home_wins: h, away_wins: a, winner };
    }
    let (mut h, mut a, mut errors, mut incomplete) = (0, 0, false, false);
    for gi in 0..n {
        let (w, _, _) = game_result(&sheet.sets(gi), sys, false, false);
        match w {
            Won::Empty | Won::Partial => incomplete = true,
            Won::Error => errors = true,
            w if w.home() => h += 1,
            w if w.away() => a += 1,
            _ => {}
        }
    }
    let winner = if errors {
        Winner::Error
    } else if incomplete {
        Winner::Incomplete
    } else if h == a {
        Winner::Draw
    } else if h > a {
        Winner::Home
    } else {
        Winner::Away
    };
    Score { home_wins: h, away_wins: a, winner }
}

// ---------------------------------------------------------------- upload lines

pub const HEADER: &str = "C;Match result exported by Scoresheet, Copyright (c) PieterP.be\r\n";

fn is_number(s: &str) -> bool {
    let t = s.trim();
    let t = t.strip_prefix(['+', '-']).unwrap_or(t);
    !t.is_empty() && t.chars().all(|c| c.is_ascii_digit())
}

/// "Vl-B234" -> "VLB234"; byes become "bye".
pub fn club_index(c: &str) -> String {
    if is_bye_club(c) {
        return "bye".into();
    }
    match c.strip_prefix("Vl-B") {
        Some(rest) => format!("VLB{rest}"),
        None => c.to_string(),
    }
}

/// One game in TabT notation: "4/-7/9/0", "9" (home WO), "8" (away WO), "-" (nobody).
pub fn game_notation(won: Won, sets: &[SetScore], sys: &MatchSystem) -> String {
    match won {
        Won::Neither => "-".into(),
        Won::HomeWO => "9".into(),
        Won::AwayWO => "8".into(),
        Won::Home | Won::Away | Won::HomeFF | Won::AwayFF => {
            let mut parts: Vec<String> = Vec::new();
            let (mut won_sets, mut lost_sets) = (0, 0);
            for (i, s) in sets.iter().enumerate() {
                if is_empty_set(Some(s)) {
                    break;
                }
                match set_winner(i, s, sys) {
                    Won::Home => {
                        parts.push(s.r.trim().to_string());
                        won_sets += 1;
                    }
                    Won::Away => {
                        parts.push(format!("-{}", s.l.trim()));
                        lost_sets += 1;
                    }
                    Won::HomeFF => {
                        parts.push("0".into());
                        won_sets += 1;
                        while won_sets < sys.set_count {
                            parts.push("0".into());
                            won_sets += 1;
                        }
                    }
                    Won::AwayFF => {
                        parts.push("-0".into());
                        lost_sets += 1;
                        while lost_sets < sys.set_count {
                            parts.push("-0".into());
                            lost_sets += 1;
                        }
                    }
                    _ => {}
                }
            }
            parts.join("/")
        }
        _ => String::new(),
    }
}

fn team_column(s: &Sheet, sys: &MatchSystem, side: Side) -> String {
    let t = s.team(side);
    let mut out: Vec<String> = Vec::new();
    for (si, slot) in sys.slots.iter().enumerate() {
        match slot {
            Slot::Single { optional, player } => {
                let cn = t.player_cn(*player).trim();
                if cn.is_empty() && *optional {
                    continue;
                }
                if cn != "?" && cn != "-" && !is_number(cn) {
                    out.push(String::new());
                    continue;
                }
                let mut v = format!("{}{}", if is_player_wo(s, sys, side, si) { "-" } else { "" }, if cn == "-" { "?" } else { cn });
                for sub in &sys.slots {
                    if let Slot::Sub { sub, transferable } = sub
                        && *transferable == si
                        && let Some(x) = t.substitute(*sub)
                    {
                        v.push_str(&format!("+{x}"));
                    }
                }
                out.push(v);
            }
            Slot::Double { options, double } => {
                let d = t.double(*double).unwrap_or(options[0]);
                out.push(d.chars().map(|c| if matches!(c, '/' | '\\' | '+') { '-' } else { c }).collect());
            }
            Slot::Sub { .. } => {}
        }
    }
    out.join(",")
}

/// "18:30" / "18u30" / "18.30" -> "18:30".
fn hhmm(s: &str) -> Option<String> {
    let t = s.trim();
    let sep = t.find([':', '.', 'h', 'u'])?;
    let (h, m) = (&t[..sep], &t[sep + 1..]);
    let ok_h = (1..=2).contains(&h.len()) && h.chars().all(|c| c.is_ascii_digit()) && h.parse::<u32>().ok()? <= 23;
    let ok_m = m.len() == 2 && m.chars().all(|c| c.is_ascii_digit()) && m.parse::<u32>().ok()? <= 59;
    // [01]?\d|2[0-3]: a two-digit hour from 20 must be 20-23; "24".."29" fail via <= 23.
    (ok_h && ok_m).then(|| format!("{h:0>2}:{m}"))
}

/// The I line of one match (with \r\n). `partial`: unfinished games become "?" and the
/// people/hours/comments are left out, unless every game is known.
pub fn i_line(s: &Sheet, sys: &MatchSystem, mut partial: bool) -> String {
    let score = compute_score(s, sys);
    let club = |c: &str| if c.is_empty() { String::new() } else { club_index(c) };
    let mut cols: Vec<String> = vec![
        "I".into(),
        s.season.to_string(),
        s.match_id.clone(),
        club(&s.home.club_id),
        club(&s.away.club_id),
        team_column(s, sys, Side::Home),
        team_column(s, sys, Side::Away),
    ];
    let mut complete = true;
    let bye = is_bye_club(&s.home.club_id) || is_bye_club(&s.away.club_id);
    if bye {
        cols.push(format!("U{}-{}", score.home_wins, score.away_wins));
    } else {
        let games: Vec<String> = (0..sys.games.len())
            .map(|gi| {
                let sets = s.sets(gi);
                let (w, _, _) = game_result(&sets, sys, s.home.forfeit, s.away.forfeit);
                if partial && matches!(w, Won::Empty | Won::Partial | Won::Error) {
                    complete = false;
                    return "?".into();
                }
                game_notation(w, &sets, sys)
            })
            .collect();
        cols.push(format!("P{}", games.join(",")));
    }
    if complete {
        partial = false;
    }
    if !partial {
        let empty = s.home.forfeit || s.away.forfeit || bye;
        let person = |cn: &str, optional: bool| {
            if empty || (optional && cn.is_empty()) {
                "-".to_string()
            } else if is_number(cn) {
                cn.trim().to_string()
            } else {
                "-".to_string()
            }
        };
        cols.push(
            [
                person(&s.home_captain.computer_number, false),
                person(&s.away_captain.computer_number, false),
                person(&s.chief_referee.computer_number, false),
                person(&s.room_commissioner.computer_number, true),
            ]
            .join(","),
        );
        cols.push(match (hhmm(&s.start_hour), hhmm(&s.end_hour)) {
            (Some(a), Some(b)) => format!("{a},{b}"),
            _ => String::new(),
        });
        if !s.comments.is_empty() {
            let c = s.comments.replace('\\', "\\\\").replace("\r\n", "\\n").replace('\n', "\\n").replace('"', "'");
            cols.push(format!("\"{c}\""));
        }
    }
    cols.join(";") + "\r\n"
}

pub fn export_csv(s: &Sheet, sys: &MatchSystem, partial: bool) -> String {
    format!("{HEADER}{}", i_line(s, sys, partial))
}

/// What an upload sends, as a key -> text map (web/src/lib/sheet/vttl.ts `canonical`).
pub fn canonical(s: &Sheet, sys: &MatchSystem) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (side, name) in [(Side::Home, "home"), (Side::Away, "away")] {
        let t = s.team(side);
        for slot in &sys.slots {
            match slot {
                Slot::Single { player, .. } => {
                    out.insert(format!("{name}.p.{player}"), t.player_cn(*player).trim().to_string());
                }
                Slot::Double { double, .. } => {
                    out.insert(format!("{name}.d.{double}"), t.double(*double).unwrap_or("").to_string());
                }
                Slot::Sub { sub, .. } => {
                    out.insert(format!("{name}.sub.{sub}"), t.substitute(*sub).map(|x| x.to_string()).unwrap_or_default());
                }
            }
        }
        out.insert(format!("{name}.ff"), if t.forfeit { "1".into() } else { String::new() });
        let cap = if side == Side::Home { &s.home_captain } else { &s.away_captain };
        out.insert(format!("{name}.cap"), cap.computer_number.trim().to_string());
    }
    out.insert("ref".into(), s.chief_referee.computer_number.trim().to_string());
    for gi in 0..sys.games.len() {
        let sets = s.sets(gi);
        let (w, _, _) = game_result(&sets, sys, s.home.forfeit, s.away.forfeit);
        out.insert(format!("g.{gi}"), game_notation(w, &sets, sys));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    /// The one-game system of ScoreSheet's CsvExporterTests.
    fn test_sys(slots: Vec<Slot>) -> MatchSystem {
        build(0, 3, 11, true, slots, &[g!(0, 0)])
    }

    fn sets(enc: &str) -> Value {
        let mut out = vec![json!({ "l": "", "r": "" }); 5];
        if !enc.is_empty() {
            for (i, s) in enc.split('|').enumerate() {
                let (l, r) = s.split_once(':').unwrap();
                out[i] = json!({ "l": l, "r": r });
            }
        }
        Value::Array(out)
    }

    fn complete() -> Value {
        json!({
            "season": 42, "matchId": "M-1", "isOfficial": true, "matchSystemId": 0, "startHour": "18:30", "endHour": "21:45", "comments": "",
            "home": { "clubId": "Vl-B123", "forfeit": false, "players": [{ "computerNumber": "1001" }], "doubles": [], "substitutes": [] },
            "away": { "clubId": "AFTT456", "forfeit": false, "players": [{ "computerNumber": "2001" }], "doubles": [], "substitutes": [] },
            "results": [sets("11:4|7:11|11:9|11:0")],
            "chiefReferee": { "computerNumber": "3003" }, "homeCaptain": { "computerNumber": "3001" },
            "awayCaptain": { "computerNumber": "3002" }, "roomCommissioner": { "computerNumber": "" },
        })
    }
    fn sheet(v: Value) -> Sheet {
        serde_json::from_value(v).unwrap()
    }
    fn line(result: &str, officials: &str, times: &str, comment: Option<&str>) -> String {
        format!("I;42;M-1;VLB123;AFTT456;1001;2001;P{result};{officials};{times}{}\r\n", comment.map(|c| format!(";{c}")).unwrap_or_default())
    }
    fn one() -> MatchSystem {
        test_sys(vec![Slot::Single { optional: false, player: 0 }])
    }

    #[test]
    fn exporter_cases() {
        let sys = one();
        assert_eq!(export_csv(&sheet(complete()), &sys, false), format!("{HEADER}{}", line("4/-7/9/0", "3001,3002,3003,-", "18:30,21:45", None)));
        for (enc, exp) in [
            ("11:4|7:11|11:9|11:0", "4/-7/9/0"),
            ("4:11|11:7|9:11|0:11", "-4/7/-9/-0"),
            (":WO", "9"),
            ("WO:", "8"),
            (":FF", "0/0/0"),
            ("FF:", "-0/-0/-0"),
            ("WO:WO", "-"),
        ] {
            let mut v = complete();
            v["results"] = json!([sets(enc)]);
            assert!(i_line(&sheet(v), &sys, false).contains(&format!(";P{exp};")), "{enc}");
        }
        for enc in ["", "1:1"] {
            let mut v = complete();
            v["results"] = json!([sets(enc)]);
            assert_eq!(i_line(&sheet(v.clone()), &sys, true), "I;42;M-1;VLB123;AFTT456;1001;2001;P?\r\n");
            assert_eq!(i_line(&sheet(v), &sys, false), line("", "3001,3002,3003,-", "18:30,21:45", None));
        }
        assert_eq!(i_line(&sheet(complete()), &sys, true), line("4/-7/9/0", "3001,3002,3003,-", "18:30,21:45", None));
        let mut v = complete();
        v["home"]["clubId"] = json!("Vrij");
        assert_eq!(i_line(&sheet(v), &sys, true), "I;42;M-1;bye;AFTT456;1001;2001;U0-0;-,-,-,-;18:30,21:45\r\n");
        let mut v = complete();
        v["home"]["forfeit"] = json!(true);
        assert!(i_line(&sheet(v), &sys, false).contains(";-1001;2001;P8;-,-,-,-;"));
        let mut v = complete();
        v["homeCaptain"]["computerNumber"] = json!("-1");
        v["awayCaptain"]["computerNumber"] = json!("+2");
        v["chiefReferee"]["computerNumber"] = json!("003");
        v["roomCommissioner"]["computerNumber"] = json!("4");
        assert!(i_line(&sheet(v), &sys, false).contains(";P4/-7/9/0;-1,+2,003,4;"));
        let mut v = complete();
        v["endHour"] = json!("invalid");
        assert_eq!(i_line(&sheet(v), &sys, false), line("4/-7/9/0", "3001,3002,3003,-", "", None));
        let mut v = complete();
        v["comments"] = json!("Folder\\file\r\n\"quoted\"");
        assert_eq!(i_line(&sheet(v), &sys, false), line("4/-7/9/0", "3001,3002,3003,-", "18:30,21:45", Some("\"Folder\\\\file\\n'quoted'\"")));
    }

    #[test]
    fn slots_doubles_and_substitutes() {
        let sys = test_sys(vec![
            Slot::Single { optional: false, player: 0 },
            Slot::Single { optional: true, player: 1 },
            Slot::Single { optional: false, player: 2 },
            Slot::Single { optional: false, player: 3 },
            Slot::Double { options: vec!["1/2+3\\4"], double: 0 },
            Slot::Sub { sub: 0, transferable: 0 },
        ]);
        let mut v = complete();
        v["home"]["players"] = json!([{ "computerNumber": "1001" }, { "computerNumber": "" }, { "computerNumber": "-" }, { "computerNumber": "not-a-number" }]);
        v["home"]["doubles"] = json!(["1/2+3\\4"]);
        v["home"]["substitutes"] = json!([1]);
        assert!(i_line(&sheet(v), &sys, false).contains(";1001+1,?,,1-2-3-4;"));
    }

    #[test]
    fn hours() {
        assert_eq!(hhmm("18u30").as_deref(), Some("18:30"));
        assert_eq!(hhmm("9.05").as_deref(), Some("09:05"));
        assert_eq!(hhmm("24:00"), None);
        assert_eq!(hhmm("18:3"), None);
    }

    #[test]
    fn nulls_in_the_document() {
        let s: Sheet = serde_json::from_value(json!({ "results": [null, [null, { "l": "11", "r": null }]], "home": { "players": [null] } })).unwrap();
        assert_eq!(s.sets(1)[1].l, "11");
        assert!(s.sets(0).is_empty());
    }

    /// Same output as the TypeScript rules on the matches in tests/upload-fixtures.json.
    #[test]
    fn same_as_the_web_client() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/upload-fixtures.json");
        let fixtures: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(path).expect("run `pnpm test` in web/ first")).unwrap();
        assert!(fixtures.len() > 20);
        for f in fixtures {
            let s: Sheet = serde_json::from_value(f["sheet"].clone()).unwrap();
            let sys = system(s.match_system_id).unwrap();
            let name = f["name"].as_str().unwrap();
            assert_eq!(i_line(&s, sys, false), f["full"].as_str().unwrap(), "full {name}");
            assert_eq!(i_line(&s, sys, true), f["partial"].as_str().unwrap(), "partial {name}");
            let score = compute_score(&s, sys);
            assert_eq!(json!([score.home_wins, score.away_wins]), f["score"], "score {name}");
            let canon: Value = serde_json::to_value(canonical(&s, sys)).unwrap();
            assert_eq!(canon, f["canonical"], "canonical {name}");
        }
    }
}
