//! SQLite storage. The schema is versioned with `PRAGMA user_version`; each entry of
//! `MIGRATIONS` runs once, in order.

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const MIGRATIONS: &[&str] = &[
    // 1: accounts and sessions
    "CREATE TABLE account(
        account      TEXT PRIMARY KEY,
        member       INTEGER,
        member_name  TEXT,
        club         TEXT,
        created_at   INTEGER NOT NULL
     );
     CREATE TABLE session(
        token_hash   TEXT PRIMARY KEY,
        account      TEXT NOT NULL REFERENCES account(account),
        secret       TEXT NOT NULL,
        created_at   INTEGER NOT NULL,
        last_seen    INTEGER NOT NULL
     );
     CREATE INDEX session_account ON session(account);",
];

/// Sessions end after this many seconds without use.
pub const SESSION_IDLE: i64 = 120 * 86400;

pub struct Db {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub account: String,
    /// VTTL unique index of the member the user picked as "this is me".
    pub member: Option<i64>,
    pub member_name: Option<String>,
    pub club: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub account: Account,
    pub secret: String,
    pub last_seen: i64,
}

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
}

impl Db {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;")?;
        let version: usize = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        for (i, m) in MIGRATIONS.iter().enumerate().skip(version) {
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(m)?;
            tx.execute_batch(&format!("PRAGMA user_version = {}", i + 1))?;
            tx.commit()?;
            tracing::info!("database migrated to version {}", i + 1);
        }
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn create_session(&self, account: &str, token_hash: &str, secret: &str) -> Result<Account> {
        let c = self.conn.lock().unwrap();
        let t = now();
        c.execute(
            "INSERT INTO account(account, created_at) VALUES (?1, ?2) ON CONFLICT(account) DO NOTHING",
            params![account, t],
        )?;
        c.execute(
            "INSERT INTO session(token_hash, account, secret, created_at, last_seen) VALUES (?1, ?2, ?3, ?4, ?4)",
            params![token_hash, account, secret, t],
        )?;
        // A new login also carries the newest password: update the other sessions.
        c.execute("UPDATE session SET secret = ?2 WHERE account = ?1", params![account, secret])?;
        c.execute("DELETE FROM session WHERE last_seen < ?1", params![t - SESSION_IDLE])?;
        account_row(&c, account).map(|a| a.expect("account exists"))
    }

    pub fn session(&self, token_hash: &str) -> Result<Option<Session>> {
        let c = self.conn.lock().unwrap();
        let row = c
            .query_row(
                "SELECT s.secret, s.last_seen, a.account, a.member, a.member_name, a.club
                 FROM session s JOIN account a ON a.account = s.account WHERE s.token_hash = ?1",
                params![token_hash],
                |r| {
                    Ok(Session {
                        secret: r.get(0)?,
                        last_seen: r.get(1)?,
                        account: Account { account: r.get(2)?, member: r.get(3)?, member_name: r.get(4)?, club: r.get(5)? },
                    })
                },
            )
            .optional()?;
        Ok(row.filter(|s| s.last_seen >= now() - SESSION_IDLE))
    }

    pub fn touch_session(&self, token_hash: &str) -> Result<()> {
        let c = self.conn.lock().unwrap();
        c.execute("UPDATE session SET last_seen = ?2 WHERE token_hash = ?1", params![token_hash, now()])?;
        Ok(())
    }

    pub fn delete_session(&self, token_hash: &str) -> Result<()> {
        let c = self.conn.lock().unwrap();
        c.execute("DELETE FROM session WHERE token_hash = ?1", params![token_hash])?;
        Ok(())
    }

    pub fn set_member(&self, account: &str, member: Option<(i64, &str, &str)>) -> Result<Account> {
        let c = self.conn.lock().unwrap();
        let (m, name, club) = match member {
            Some((m, n, cl)) => (Some(m), Some(n), Some(cl)),
            None => (None, None, None),
        };
        c.execute(
            "UPDATE account SET member = ?2, member_name = ?3, club = ?4 WHERE account = ?1",
            params![account, m, name, club],
        )?;
        account_row(&c, account).map(|a| a.expect("account exists"))
    }
}

fn account_row(c: &Connection, account: &str) -> Result<Option<Account>> {
    Ok(c.query_row(
        "SELECT account, member, member_name, club FROM account WHERE account = ?1",
        params![account],
        |r| Ok(Account { account: r.get(0)?, member: r.get(1)?, member_name: r.get(2)?, club: r.get(3)? }),
    )
    .optional()?)
}

#[cfg(test)]
pub fn memory() -> Db {
    let path = std::env::temp_dir().join(format!("vl-test-{}.sqlite", rand::random::<u64>()));
    Db::open(path.to_str().unwrap()).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_lifecycle() {
        let db = memory();
        let a = db.create_session("jan", "h1", "s1").unwrap();
        assert_eq!(a.member, None);
        db.create_session("jan", "h2", "s2").unwrap();
        // The older session gets the new password too.
        assert_eq!(db.session("h1").unwrap().unwrap().secret, "s2");
        db.set_member("jan", Some((530077, "VANEYGHEN Roel", "WVL154"))).unwrap();
        let s = db.session("h2").unwrap().unwrap();
        assert_eq!(s.account.club.as_deref(), Some("WVL154"));
        db.delete_session("h2").unwrap();
        assert!(db.session("h2").unwrap().is_none());
        assert!(db.session("h1").unwrap().is_some());
    }
}
