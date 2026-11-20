//! Pieces shared by the OAuth sign-ins (OneDrive and Google Drive on the
//! phone, the ChatGPT plan on the desktop): PKCE (RFC 7636), the redirect's
//! code, tokens, URL encoding, base64url, and service timestamps.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

// MARK: - PKCE

/// One sign-in attempt's secrets (RFC 7636), plus `state` and `nonce`.
#[derive(Debug, Clone)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
    pub state: String,
    /// For OpenID Connect: comes back inside the ID token.
    pub nonce: String,
}

impl Pkce {
    pub fn new() -> Self {
        let verifier = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        Self {
            challenge: challenge_for(&verifier),
            verifier,
            state: uuid::Uuid::new_v4().simple().to_string(),
            nonce: uuid::Uuid::new_v4().simple().to_string(),
        }
    }
}

impl Default for Pkce {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn challenge_for(verifier: &str) -> String {
    base64url(&Sha256::digest(verifier.as_bytes()))
}

// MARK: - Sign-in answers

/// The authorization code from a sign-in page's redirect
/// (`scheme://…?code=…&state=…`), after checking it answers this attempt.
pub fn code_from_redirect(url: &str, pkce: &Pkce) -> std::result::Result<String, String> {
    let q = url.split_once('?').map(|(_, q)| q).unwrap_or_default();
    let q = q.split('#').next().unwrap_or_default();
    let params: HashMap<String, String> = q
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (percent_decode(k), percent_decode(v)))
        .collect();
    if let Some(e) = params.get("error") {
        return Err(params.get("error_description").unwrap_or(e).clone());
    }
    if params.get("state") != Some(&pkce.state) {
        return Err("the sign-in answer didn't match; please try again".into());
    }
    params
        .get("code")
        .cloned()
        .ok_or_else(|| "the sign-in page returned no code".into())
}

/// A signed-in session's tokens. The refresh token is the caller's to keep
/// (Keychain); the access token lives only in memory.
#[derive(Debug, Clone)]
pub struct Tokens {
    pub access_token: String,
    /// Replaces the stored one when given (OneDrive's rotate; Google's don't).
    pub refresh_token: Option<String>,
    pub expires_at: Instant,
}

impl Tokens {
    pub fn is_fresh(&self) -> bool {
        Instant::now() + Duration::from_secs(120) < self.expires_at
    }
}

// MARK: - Times

/// "2025-03-14T18:22:05Z" / "…05.123Z" → milliseconds since the epoch.
pub fn parse_time_ms(s: &str) -> Option<i64> {
    let s = s.strip_suffix('Z')?;
    let (date, time) = s.split_once('T')?;
    let mut d = date.split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let (hms, frac) = time.split_once('.').unwrap_or((time, ""));
    let mut t = hms.split(':').map(|p| p.parse::<i64>().ok());
    let (hh, mm, ss) = (t.next()??, t.next()??, t.next()??);
    let ms = if frac.is_empty() {
        0
    } else {
        format!("{frac:0<3}")[..3].parse::<i64>().ok()?
    };
    // Days from civil (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 24 + hh) * 60 + mm) * 60_000 + ss * 1000 + ms)
}

// MARK: - Encoding

pub(crate) fn base64url(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        for i in 0..=chunk.len() {
            out.push(ABC[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

/// Decodes base64url (padding optional); `None` if it isn't.
pub(crate) fn base64url_decode(s: &str) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            _ => return None,
        } as u32)
    };
    let s = s.trim_end_matches('=').as_bytes();
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    for chunk in s.chunks(4) {
        if chunk.len() == 1 {
            return None;
        }
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= value(c)? << (18 - 6 * i);
        }
        for i in 0..chunk.len() - 1 {
            out.push((n >> (16 - 8 * i)) as u8);
        }
    }
    Some(out)
}

/// Percent-encodes everything but RFC 3986 unreserved characters.
pub(crate) fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub(crate) fn query(params: &[(&str, &str)]) -> String {
    params
        .iter()
        .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

pub(crate) fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => match (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                (Some(h), Some(l)) => {
                    out.push(h << 4 | l);
                    i += 3;
                    continue;
                }
                _ => out.push(b'%'),
            },
            b'+' => out.push(b' '),
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The parameters of a query string (`a=1&b=2`, or a whole URL with one).
pub(crate) fn parse_query(s: &str) -> HashMap<String, String> {
    let q = s.split_once('?').map_or(s, |(_, q)| q);
    let q = q.split('#').next().unwrap_or_default();
    q.split('&')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (percent_decode(k), percent_decode(v)))
        .collect()
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_matches_the_rfc_example() {
        assert_eq!(
            challenge_for("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        let p = Pkce::new();
        assert_eq!(p.verifier.len(), 64);
        assert_ne!(p.verifier, Pkce::new().verifier);
    }

    #[test]
    fn base64url_pads_nothing() {
        assert_eq!(base64url(b"f"), "Zg");
        assert_eq!(base64url(b"fo"), "Zm8");
        assert_eq!(base64url(b"foo"), "Zm9v");
        assert_eq!(base64url(&[0xfb, 0xff]), "-_8");
    }

    #[test]
    fn base64url_round_trips() {
        for data in [&b""[..], b"f", b"fo", b"foo", b"foob", &[0xfb, 0xff, 0x00]] {
            assert_eq!(base64url_decode(&base64url(data)).unwrap(), data);
        }
        assert_eq!(base64url_decode("Zm9vYg==").unwrap(), b"foob");
        assert!(base64url_decode("a").is_none());
        assert!(base64url_decode("a*b=").is_none());
    }

    #[test]
    fn queries_parse() {
        let q = parse_query("http://127.0.0.1:1455/auth/callback?code=a%2Bb&state=s1&scope=x+y");
        assert_eq!(q["code"], "a+b");
        assert_eq!(q["state"], "s1");
        assert_eq!(q["scope"], "x y");
    }
}
