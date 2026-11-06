//! Shared state.

use crate::crypto::Crypto;
use crate::db::Db;
use crate::tabt::Tabt;

pub struct App {
    pub tabt: Tabt,
    pub db: Db,
    pub crypto: Crypto,
    /// Send the session cookie with the `Secure` flag (off only for local http).
    pub cookie_secure: bool,
}
