//! Login with a VTTL account.
//!
//! The VTTL `Test` call checks the account. The server keeps the password encrypted in
//! the session row, because uploads need it later. The browser only gets a random
//! token in an HttpOnly cookie; the database keeps a hash of that token.

use crate::api::ApiError;
use crate::app::App;
use crate::crypto;
use crate::db::{self, Account};
use crate::tabt::Creds;
use axum::extract::FromRequestParts;
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue};
use std::sync::Arc;

pub const COOKIE_NAME: &str = "vl_sid";

/// The logged-in user of a request.
#[allow(dead_code)]
pub struct User {
    pub token_hash: String,
    pub account: Account,
    pub creds: Creds,
}

impl User {
    /// The club of the member this account picked, or an error the client shows as
    /// "choose who you are first".
    pub fn club(&self) -> Result<&str, ApiError> {
        self.account.club.as_deref().ok_or(ApiError::NoMember)
    }
}

pub fn token_from(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == COOKIE_NAME)
        .map(|(_, v)| v.to_string())
}

pub fn set_cookie(app: &App, token: &str, max_age: i64) -> (axum::http::HeaderName, HeaderValue) {
    let secure = if app.cookie_secure { "; Secure" } else { "" };
    let v = format!("{COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}");
    (SET_COOKIE, HeaderValue::from_str(&v).unwrap())
}

impl FromRequestParts<Arc<App>> for User {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, app: &Arc<App>) -> Result<Self, ApiError> {
        let token = token_from(&parts.headers).ok_or(ApiError::Unauthorized)?;
        let hash = crypto::token_hash(&token);
        let s = app.db.session(&hash)?.ok_or(ApiError::Unauthorized)?;
        let password = app.crypto.decrypt(&s.secret).ok_or(ApiError::Unauthorized)?;
        // Sliding expiry, written at most once per hour.
        if s.last_seen < db::now() - 3600 {
            app.db.touch_session(&hash)?;
        }
        Ok(User {
            token_hash: hash,
            creds: Creds { account: s.account.account.clone(), password },
            account: s.account,
        })
    }
}
