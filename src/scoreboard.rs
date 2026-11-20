//! Decoding of the binary `update` payload of the ESP32 scoreboard and derived match state.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Timeout {
    pub active: bool,
    /// 0 general, 1 player 1, 2 player 2
    pub owner: u8,
    pub p1_used: bool,
    pub p2_used: bool,
    pub expired: bool,
    /// UTC epoch ms, 0 = unknown
    pub deadline: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MatchState {
    pub duration_ms: u32,
    pub best_of: u8,
    pub point_count: u8,
    /// Finished games as (player1, player2) points.
    pub games: Vec<(u8, u8)>,
    /// Score of the game in progress (0-0 after a finished match).
    pub current: (u8, u8),
    pub games_won: (u8, u8),
    /// 1 or 2
    pub server: u8,
    /// Player (1 or 2) who served first in game 1.
    pub first_server: u8,
    /// Every counted point in order: '0' = player 1 won it, '1' = player 2.
    pub points: String,
    pub finished: bool,
    /// None = timeout state unknown (no extension, or a bad one).
    pub timeout: Option<Timeout>,
}

/// Sport number of the scoreboard firmware (`SportId`): bits 1..3 of the meta byte, and the
/// `sport` field of the `new` command. This server only knows table tennis.
pub const SPORT_TABLE_TENNIS: u8 = 0;

pub fn decode(p: &[u8]) -> Result<MatchState> {
    if p.len() < 7 {
        bail!("payload too short ({} bytes)", p.len());
    }
    // A board that plays another sport (badminton = 1) sends points that the table tennis
    // rules below would split into wrong games. Firmware before the sport bits always sends 0.
    let sport = (p[5] >> 1) & 7;
    if sport != SPORT_TABLE_TENNIS {
        bail!("board plays sport {sport}, not table tennis");
    }
    let duration_ms = u32::from_be_bytes([p[0], p[1], p[2], p[3]]);
    let best_of = p[4];
    // Any best-of the board can play (1, 3, 5, 7). Whether it is the format of the linked
    // match is checked by the caller, which knows the match system.
    if best_of.is_multiple_of(2) {
        bail!("unknown match type {best_of}");
    }
    let meta = p[5];
    let point_count = p[6];
    let first_server = meta & 1; // 0 = player 1

    // The timeout extension is found from the end of the payload.
    let mut end = p.len();
    let mut timeout = None;
    if meta & 0x80 != 0 && p.len() >= 7 + 10 {
        let t = &p[p.len() - 10..];
        if t[0] == 1 {
            let f = t[1];
            timeout = Some(Timeout {
                active: f & 1 != 0,
                owner: (f >> 1) & 3,
                p1_used: f & 8 != 0,
                p2_used: f & 16 != 0,
                expired: f & 32 != 0,
                deadline: u64::from_be_bytes(t[2..10].try_into().unwrap()),
            });
        }
        end = p.len() - 10;
    }
    let point_bytes = &p[7..end.max(7)];
    if point_bytes.len() * 8 < point_count as usize {
        bail!("payload has {} point bits, needs {point_count}", point_bytes.len() * 8);
    }

    let need = best_of / 2 + 1;
    let mut games = Vec::new();
    let (mut a, mut b) = (0u8, 0u8);
    let mut won = (0u8, 0u8);
    let mut finished = false;
    let mut points = String::with_capacity(point_count as usize);
    for i in 0..point_count as usize {
        if finished {
            break;
        }
        let bit = (point_bytes[i / 8] >> (7 - (i % 8))) & 1;
        points.push(if bit == 0 { '0' } else { '1' });
        if bit == 0 { a += 1 } else { b += 1 }
        if (a >= 11 || b >= 11) && a.abs_diff(b) >= 2 {
            games.push((a, b));
            if a > b { won.0 += 1 } else { won.1 += 1 }
            a = 0;
            b = 0;
            finished = won.0 == need || won.1 == need;
        }
    }

    // Serve: first server of game g alternates; changes every 2 points, every point from 10-10.
    let game_first = first_server ^ (games.len() as u8 & 1);
    let n = a as u32 + b as u32;
    let switches = if a >= 10 && b >= 10 { 10 + (n - 20) } else { n / 2 };
    let server = (game_first ^ (switches & 1) as u8) + 1;

    Ok(MatchState {
        duration_ms,
        best_of,
        point_count,
        games,
        current: (a, b),
        games_won: won,
        server,
        first_server: first_server + 1,
        points,
        finished,
        timeout,
    })
}

/// Name for the display: no accents, uppercase, printable ASCII only.
pub fn display_name(s: &str) -> String {
    deunicode::deunicode(s)
        .to_uppercase()
        .chars()
        .filter(|c| (' '..='~').contains(c))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(points: &[u8]) -> Vec<u8> {
        let mut v = vec![0u8; points.len().div_ceil(8)];
        for (i, &p) in points.iter().enumerate() {
            v[i / 8] |= p << (7 - i % 8);
        }
        v
    }
    fn payload(meta: u8, points: &[u8]) -> Vec<u8> {
        let mut v = vec![0, 0, 0xea, 0x60, 5, meta, points.len() as u8];
        v.extend(bits(points));
        v
    }

    #[test]
    fn every_odd_best_of() {
        fn with_best_of(best_of: u8, points: &[u8]) -> Vec<u8> {
            let mut v = payload(0, points);
            v[4] = best_of;
            v
        }
        // best of 1: one game decides the match
        let s = decode(&with_best_of(1, &[0; 11])).unwrap();
        assert_eq!((s.best_of, s.games.clone(), s.finished), (1, vec![(11, 0)], true));
        // best of 3: two games
        let s = decode(&with_best_of(3, &[0; 11])).unwrap();
        assert!(!s.finished);
        let s = decode(&with_best_of(3, &[0; 22])).unwrap();
        assert_eq!((s.games_won, s.finished), ((2, 0), true));
        // best of 5 is not finished after two games, best of 7 not after three
        assert!(!decode(&with_best_of(5, &[0; 22])).unwrap().finished);
        assert!(!decode(&with_best_of(7, &[0; 33])).unwrap().finished);
        assert!(decode(&with_best_of(7, &[0; 44])).unwrap().finished);
        // no deciding game exists for an even count
        for bad in [0, 2, 4, 6] {
            assert!(decode(&with_best_of(bad, &[0])).is_err(), "best of {bad}");
        }
    }

    #[test]
    fn other_sport_is_rejected() {
        // meta 0x02 = badminton, player 1 serves first; also with the timeout bit and best of 5
        for meta in [0x02, 0x03, 0x82] {
            let e = decode(&payload(meta, &[0, 1, 0])).unwrap_err();
            assert!(e.to_string().contains("sport 1"), "{e}");
        }
        // the sport bits of table tennis are 0: nothing changes for it
        assert!(decode(&payload(0x01, &[0, 1, 0])).is_ok());
    }

    #[test]
    fn doc_example() {
        let s = decode(&[0x00, 0x00, 0xea, 0x60, 0x05, 0x00, 0x03, 0x40]).unwrap();
        assert_eq!(s.duration_ms, 60_000);
        assert_eq!(s.current, (2, 1));
        assert_eq!(s.server, 2); // 3 points played: serve changed once
        assert!(s.timeout.is_none());
    }

    #[test]
    fn full_match_and_serve() {
        // three games 11-0 for player 1
        let s = decode(&payload(1, &[0; 33])).unwrap();
        assert_eq!(s.games, vec![(11, 0); 3]);
        assert!(s.finished);
        assert_eq!(s.first_server, 2);
        assert_eq!(s.points, "0".repeat(33));
        // points after the end of the match are not counted
        assert_eq!(decode(&payload(1, &[0; 40])).unwrap().points.len(), 33);
        // game 2 starts with the other server: meta says p2 first, game 2 -> p1
        let s = decode(&payload(1, &[0; 11])).unwrap();
        assert_eq!(s.server, 1);
    }

    #[test]
    fn deuce_serve() {
        // 10-10: alternate p1,p2 x10 -> serve switched 10 times -> first server again
        let pts: Vec<u8> = (0..20).map(|i| i % 2).collect();
        let s = decode(&payload(0, &pts)).unwrap();
        assert_eq!(s.current, (10, 10));
        assert_eq!(s.server, 1);
        let mut p2 = pts.clone();
        p2.push(0);
        assert_eq!(decode(&payload(0, &p2)).unwrap().server, 2);
    }

    #[test]
    fn extra_point_bytes_and_timeout() {
        let mut p = payload(0x80, &[0, 1, 0]);
        p.extend([0xff, 0xff]); // firmware may send more point bytes
        p.extend([1, 0b01001, 0, 0, 0, 0, 0, 0, 0, 42]);
        let s = decode(&p).unwrap();
        assert_eq!(s.current, (2, 1));
        let t = s.timeout.unwrap();
        assert!(t.active && t.p1_used && !t.p2_used);
        assert_eq!(t.deadline, 42);
    }

    #[test]
    fn bad_timeout_is_ignored() {
        let mut p = payload(0x80, &[0, 1, 0]);
        p.extend([9, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let s = decode(&p).unwrap();
        assert!(s.timeout.is_none());
        assert_eq!(s.current, (2, 1));
    }

    #[test]
    fn names() {
        assert_eq!(display_name("Éls De Smét"), "ELS DE SMET");
        assert_eq!(display_name("Strauß"), "STRAUSS");
    }
}
