//! SQLite storage: the local match sheet (lineups, results) and scoreboard links.
//!
//! Every lineup side and every result row has a `rev`. A write from a browser must
//! carry the rev it saw; a mismatch is a `Conflict` (somebody else changed it first).
//! Rows are never deleted, only cleared, so revs never go back to an old value.

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Debug)]
pub struct Conflict {
    pub current_rev: i64,
}
impl std::fmt::Display for Conflict {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "changed by another user (rev {})", self.current_rev)
    }
}
impl std::error::Error for Conflict {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LineupPlayer {
    pub unique_index: i64,
    pub first_name: String,
    pub last_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Lineup {
    /// Index 0 = team position 1. None = not assigned.
    pub players: Vec<Option<LineupPlayer>>,
    /// Unique index of the captain. None = not set.
    pub captain: Option<i64>,
    pub rev: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Forfeit {
    Home,
    Away,
    Both,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResultRow {
    pub position: i64,
    /// (home, away) points per set
    pub sets: Vec<(u8, u8)>,
    pub forfeit: Option<Forfeit>,
    /// "manual" | "scoreboard" | None when cleared
    pub source: Option<String>,
    pub rev: i64,
}

impl ResultRow {
    pub fn is_empty(&self) -> bool {
        self.sets.is_empty() && self.forfeit.is_none()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Link {
    pub device_id: String,
    pub match_id: String,
    pub position: i64,
}

/// A scoreboard added by hand, with a display name.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Device {
    pub device_id: String,
    pub alias: String,
}

pub struct Db(Mutex<Connection>);

impl Db {
    pub fn open(path: &str) -> Result<Self> {
        let c = Connection::open(path)?;
        c.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS lineup(
               match_id TEXT NOT NULL, side TEXT NOT NULL, players TEXT NOT NULL,
               rev INTEGER NOT NULL, PRIMARY KEY(match_id, side));
             CREATE TABLE IF NOT EXISTS result(
               match_id TEXT NOT NULL, position INTEGER NOT NULL, sets TEXT NOT NULL,
               forfeit TEXT, source TEXT, rev INTEGER NOT NULL,
               updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
               PRIMARY KEY(match_id, position));
             CREATE TABLE IF NOT EXISTS link(
               device_id TEXT PRIMARY KEY, match_id TEXT NOT NULL, position INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS device_match(
               device_id TEXT NOT NULL, mqtt_match_id TEXT NOT NULL,
               match_id TEXT, position INTEGER,
               PRIMARY KEY(device_id, mqtt_match_id));
             CREATE TABLE IF NOT EXISTS device(
               device_id TEXT PRIMARY KEY, alias TEXT NOT NULL DEFAULT '');",
        )?;
        // Added later: databases from before have no captain column.
        let has_captain = c
            .prepare("SELECT 1 FROM pragma_table_info('lineup') WHERE name='captain'")?
            .exists([])?;
        if !has_captain {
            c.execute_batch("ALTER TABLE lineup ADD COLUMN captain INTEGER;")?;
        }
        Ok(Self(Mutex::new(c)))
    }

    pub fn lineup(&self, match_id: &str, side: &str) -> Result<Option<Lineup>> {
        let c = self.0.lock().unwrap();
        let row: Option<(String, Option<i64>, i64)> = c
            .query_row(
                "SELECT players, captain, rev FROM lineup WHERE match_id=?1 AND side=?2",
                params![match_id, side],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        Ok(match row {
            Some((p, captain, rev)) => Some(Lineup { players: serde_json::from_str(&p)?, captain, rev }),
            None => None,
        })
    }

    pub fn save_lineup(
        &self,
        match_id: &str,
        side: &str,
        players: &[Option<LineupPlayer>],
        captain: Option<i64>,
        expected_rev: i64,
    ) -> Result<i64> {
        let mut c = self.0.lock().unwrap();
        let tx = c.transaction()?;
        let cur: i64 = tx
            .query_row(
                "SELECT rev FROM lineup WHERE match_id=?1 AND side=?2",
                params![match_id, side],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0);
        if cur != expected_rev {
            return Err(Conflict { current_rev: cur }.into());
        }
        tx.execute(
            "INSERT INTO lineup(match_id, side, players, captain, rev) VALUES(?1,?2,?3,?4,?5)
             ON CONFLICT(match_id, side) DO UPDATE SET players=excluded.players, captain=excluded.captain,
               rev=excluded.rev",
            params![match_id, side, serde_json::to_string(players)?, captain, cur + 1],
        )?;
        tx.commit()?;
        Ok(cur + 1)
    }

    pub fn results(&self, match_id: &str) -> Result<Vec<ResultRow>> {
        let c = self.0.lock().unwrap();
        let mut st = c.prepare(
            "SELECT position, sets, forfeit, source, rev FROM result WHERE match_id=?1 ORDER BY position",
        )?;
        let rows = st
            .query_map(params![match_id], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(position, sets, forfeit, source, rev)| {
                Ok(ResultRow {
                    position,
                    sets: serde_json::from_str(&sets)?,
                    forfeit: forfeit.map(|f| serde_json::from_str(&format!("\"{f}\""))).transpose()?,
                    source,
                    rev,
                })
            })
            .collect()
    }

    /// Manual write from a browser, checked against `expected_rev`.
    /// Empty sets and no forfeit clears the row.
    pub fn save_result_manual(
        &self,
        match_id: &str,
        position: i64,
        sets: &[(u8, u8)],
        forfeit: Option<&Forfeit>,
        expected_rev: i64,
    ) -> Result<i64> {
        let mut c = self.0.lock().unwrap();
        let tx = c.transaction()?;
        let cur = result_rev(&tx, match_id, position)?;
        if cur != expected_rev {
            return Err(Conflict { current_rev: cur }.into());
        }
        let empty = sets.is_empty() && forfeit.is_none();
        upsert_result(&tx, match_id, position, sets, forfeit, (!empty).then_some("manual"), cur + 1)?;
        tx.commit()?;
        Ok(cur + 1)
    }

    /// Write from a scoreboard. Never overwrites a manual result.
    /// Returns true when the row changed.
    pub fn save_result_scoreboard(&self, match_id: &str, position: i64, sets: &[(u8, u8)]) -> Result<bool> {
        let mut c = self.0.lock().unwrap();
        let tx = c.transaction()?;
        let existing: Option<(String, Option<String>)> = tx
            .query_row(
                "SELECT sets, source FROM result WHERE match_id=?1 AND position=?2",
                params![match_id, position],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let new_sets = serde_json::to_string(sets)?;
        if let Some((s, src)) = &existing {
            if src.as_deref() == Some("manual") || (src.as_deref() == Some("scoreboard") && *s == new_sets) {
                return Ok(false);
            }
        }
        let cur = result_rev(&tx, match_id, position)?;
        upsert_result(&tx, match_id, position, sets, None, Some("scoreboard"), cur + 1)?;
        tx.commit()?;
        Ok(true)
    }

    pub fn devices(&self) -> Result<Vec<Device>> {
        let c = self.0.lock().unwrap();
        let mut st = c.prepare("SELECT device_id, alias FROM device ORDER BY device_id")?;
        let v = st
            .query_map([], |r| Ok(Device { device_id: r.get(0)?, alias: r.get(1)? }))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(v)
    }

    /// Adds a device or changes its alias.
    pub fn save_device(&self, d: &Device) -> Result<()> {
        self.0.lock().unwrap().execute(
            "INSERT INTO device(device_id, alias) VALUES(?1,?2)
             ON CONFLICT(device_id) DO UPDATE SET alias=excluded.alias",
            params![d.device_id, d.alias],
        )?;
        Ok(())
    }

    pub fn delete_device(&self, device_id: &str) -> Result<bool> {
        Ok(self.0.lock().unwrap().execute("DELETE FROM device WHERE device_id=?1", params![device_id])? > 0)
    }

    pub fn links(&self) -> Result<Vec<Link>> {
        let c = self.0.lock().unwrap();
        let mut st = c.prepare("SELECT device_id, match_id, position FROM link ORDER BY device_id")?;
        let v = st
            .query_map([], |r| Ok(Link { device_id: r.get(0)?, match_id: r.get(1)?, position: r.get(2)? }))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(v)
    }

    pub fn link(&self, device_id: &str) -> Result<Option<Link>> {
        Ok(self.links()?.into_iter().find(|l| l.device_id == device_id))
    }

    pub fn set_link(&self, l: &Link) -> Result<()> {
        self.0.lock().unwrap().execute(
            "INSERT INTO link(device_id, match_id, position) VALUES(?1,?2,?3)
             ON CONFLICT(device_id) DO UPDATE SET match_id=excluded.match_id, position=excluded.position",
            params![l.device_id, l.match_id, l.position],
        )?;
        Ok(())
    }

    pub fn delete_link(&self, device_id: &str) -> Result<bool> {
        Ok(self.0.lock().unwrap().execute("DELETE FROM link WHERE device_id=?1", params![device_id])? > 0)
    }

    /// Which team match position a device match belongs to. The first time a device
    /// match is seen it is bound to the device's link at that moment (or to nothing).
    pub fn attribute(&self, device_id: &str, mqtt_match_id: &str) -> Result<Option<(String, i64)>> {
        let c = self.0.lock().unwrap();
        let row: Option<(Option<String>, Option<i64>)> = c
            .query_row(
                "SELECT match_id, position FROM device_match WHERE device_id=?1 AND mqtt_match_id=?2",
                params![device_id, mqtt_match_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((m, p)) = row {
            return Ok(m.zip(p));
        }
        let link: Option<(String, i64)> = c
            .query_row(
                "SELECT match_id, position FROM link WHERE device_id=?1",
                params![device_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        c.execute(
            "INSERT INTO device_match(device_id, mqtt_match_id, match_id, position) VALUES(?1,?2,?3,?4)",
            params![device_id, mqtt_match_id, link.as_ref().map(|l| &l.0), link.as_ref().map(|l| l.1)],
        )?;
        Ok(link)
    }

    /// Binds a device match to a team match position (overwrites).
    pub fn bind(&self, device_id: &str, mqtt_match_id: &str, match_id: &str, position: i64) -> Result<()> {
        self.0.lock().unwrap().execute(
            "INSERT INTO device_match(device_id, mqtt_match_id, match_id, position) VALUES(?1,?2,?3,?4)
             ON CONFLICT(device_id, mqtt_match_id) DO UPDATE SET match_id=excluded.match_id, position=excluded.position",
            params![device_id, mqtt_match_id, match_id, position],
        )?;
        Ok(())
    }

    /// Marks device matches that already exist as "not part of a link", so a new link
    /// only picks up a match that starts after it.
    pub fn mark_seen(&self, device_id: &str, mqtt_match_ids: &[String]) -> Result<()> {
        let c = self.0.lock().unwrap();
        for m in mqtt_match_ids {
            c.execute(
                "INSERT OR IGNORE INTO device_match(device_id, mqtt_match_id) VALUES(?1,?2)",
                params![device_id, m],
            )?;
        }
        Ok(())
    }
}

fn result_rev(c: &Connection, match_id: &str, position: i64) -> Result<i64> {
    Ok(c
        .query_row(
            "SELECT rev FROM result WHERE match_id=?1 AND position=?2",
            params![match_id, position],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(0))
}

fn upsert_result(
    c: &Connection,
    match_id: &str,
    position: i64,
    sets: &[(u8, u8)],
    forfeit: Option<&Forfeit>,
    source: Option<&str>,
    rev: i64,
) -> Result<()> {
    let forfeit = forfeit.map(|f| serde_json::to_value(f).unwrap().as_str().unwrap().to_string());
    c.execute(
        "INSERT INTO result(match_id, position, sets, forfeit, source, rev) VALUES(?1,?2,?3,?4,?5,?6)
         ON CONFLICT(match_id, position) DO UPDATE SET sets=excluded.sets, forfeit=excluded.forfeit,
           source=excluded.source, rev=excluded.rev, updated_at=CURRENT_TIMESTAMP",
        params![match_id, position, serde_json::to_string(sets)?, forfeit, source, rev],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revisions_and_manual_wins() {
        let db = Db::open(":memory:").unwrap();
        assert_eq!(db.save_result_manual("M", 1, &[(11, 7)], None, 0).unwrap(), 1);
        // stale writer
        let e = db.save_result_manual("M", 1, &[(11, 8)], None, 0).unwrap_err();
        assert!(e.downcast_ref::<Conflict>().is_some());
        // scoreboard does not overwrite manual
        assert!(!db.save_result_scoreboard("M", 1, &[(11, 9)]).unwrap());
        // clear keeps the rev moving forward
        assert_eq!(db.save_result_manual("M", 1, &[], None, 1).unwrap(), 2);
        assert!(db.save_result_scoreboard("M", 1, &[(11, 9)]).unwrap());
        assert_eq!(db.results("M").unwrap()[0].rev, 3);
    }

    #[test]
    fn lineup_captain() {
        let db = Db::open(":memory:").unwrap();
        let p = LineupPlayer { unique_index: 7, first_name: "A".into(), last_name: "B".into() };
        assert_eq!(db.save_lineup("M", "H", &[Some(p), None], Some(7), 0).unwrap(), 1);
        let l = db.lineup("M", "H").unwrap().unwrap();
        assert_eq!((l.captain, l.players.len(), l.rev), (Some(7), 2, 1));
        db.save_lineup("M", "H", &[None, None], None, 1).unwrap();
        assert_eq!(db.lineup("M", "H").unwrap().unwrap().captain, None);
    }

    #[test]
    fn devices() {
        let db = Db::open(":memory:").unwrap();
        db.save_device(&Device { device_id: "B".into(), alias: "Table 2".into() }).unwrap();
        db.save_device(&Device { device_id: "A".into(), alias: "".into() }).unwrap();
        db.save_device(&Device { device_id: "A".into(), alias: "Table 1".into() }).unwrap();
        let v = db.devices().unwrap();
        assert_eq!(v.iter().map(|d| (&*d.device_id, &*d.alias)).collect::<Vec<_>>(), [("A", "Table 1"), ("B", "Table 2")]);
        assert!(db.delete_device("A").unwrap());
        assert!(!db.delete_device("A").unwrap());
    }

    #[test]
    fn attribution() {
        let db = Db::open(":memory:").unwrap();
        db.mark_seen("D", &["old".into()]).unwrap();
        db.set_link(&Link { device_id: "D".into(), match_id: "M".into(), position: 3 }).unwrap();
        assert_eq!(db.attribute("D", "old").unwrap(), None);
        assert_eq!(db.attribute("D", "new").unwrap(), Some(("M".into(), 3)));
        db.delete_link("D").unwrap();
        assert_eq!(db.attribute("D", "new").unwrap(), Some(("M".into(), 3)));
    }
}
