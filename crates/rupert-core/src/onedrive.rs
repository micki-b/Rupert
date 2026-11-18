//! OneDrive through Microsoft Graph, for the phone (the iPhone's Files app
//! can't hand folders from OneDrive to other apps). Personal accounts only.
//!
//! - Sign-in: OAuth 2.0 authorization code with PKCE, as a public client
//!   (no secret). The app shows the Microsoft page; this module builds the
//!   URL and redeems the code. Tokens are the caller's to keep (Keychain).
//! - Reading: a folder's complete listing through `delta`, Rupert's small
//!   hidden files and, when the user opens one, a document.
//! - Writing: only new files (scans) into the inbox folder, never
//!   replacing anything (`conflictBehavior=rename`).

use std::collections::HashMap;
use std::io::Read;
use std::time::{Duration, Instant};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::remote::RemoteFile;

const AUTHORITY: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
const GRAPH: &str = "https://graph.microsoft.com/v1.0";
/// Reading the drive, and adding scans to the inbox.
pub const SCOPES: &str = "Files.ReadWrite offline_access";
/// Graph's limit for a single-request upload.
const SIMPLE_UPLOAD_MAX: usize = 4 * 1024 * 1024;
/// Upload session chunks must be multiples of 320 KiB.
const UPLOAD_CHUNK: usize = 320 * 1024 * 12;

fn net_err(e: impl std::fmt::Display) -> Error {
    Error::Unsupported(format!("OneDrive: {e}"))
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(120)))
        .http_status_as_error(false)
        .build()
        .into()
}

/// Turns an error response into a readable message.
fn check(mut resp: ureq::http::Response<ureq::Body>) -> Result<ureq::http::Response<ureq::Body>> {
    let status = resp.status().as_u16();
    if (200..300).contains(&status) {
        return Ok(resp);
    }
    #[derive(Deserialize)]
    struct Detail {
        message: Option<String>,
        code: Option<String>,
    }
    #[derive(Deserialize)]
    struct Body {
        error: Option<serde_json::Value>,
        error_description: Option<String>,
    }
    let text = resp.body_mut().read_to_string().unwrap_or_default();
    let message = serde_json::from_str::<Body>(&text)
        .ok()
        .and_then(|b| {
            b.error_description.or_else(|| {
                let d: Detail = serde_json::from_value(b.error?).ok()?;
                d.message.or(d.code)
            })
        })
        .unwrap_or_else(|| format!("HTTP {status}"));
    Err(if status == 401 {
        Error::Unsupported("OneDrive: signed out, please sign in again".into())
    } else {
        net_err(message)
    })
}

// MARK: - Sign-in

/// One sign-in attempt's secrets (RFC 7636).
#[derive(Debug, Clone)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
    pub state: String,
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
        }
    }
}

impl Default for Pkce {
    fn default() -> Self {
        Self::new()
    }
}

fn challenge_for(verifier: &str) -> String {
    base64url(&Sha256::digest(verifier.as_bytes()))
}

/// The Microsoft sign-in page to show.
pub fn authorize_url(client_id: &str, redirect_uri: &str, pkce: &Pkce) -> String {
    let params = [
        ("client_id", client_id),
        ("response_type", "code"),
        ("redirect_uri", redirect_uri),
        ("response_mode", "query"),
        ("scope", SCOPES),
        ("code_challenge", &pkce.challenge),
        ("code_challenge_method", "S256"),
        ("state", &pkce.state),
    ];
    format!("{AUTHORITY}/authorize?{}", query(&params))
}

/// The authorization code from the page's redirect, after checking it
/// answers this attempt.
pub fn code_from_redirect(url: &str, pkce: &Pkce) -> Result<String> {
    let q = url.split_once('?').map(|(_, q)| q).unwrap_or_default();
    let q = q.split('#').next().unwrap_or_default();
    let params: HashMap<String, String> = q
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (percent_decode(k), percent_decode(v)))
        .collect();
    if let Some(e) = params.get("error") {
        let detail = params.get("error_description").unwrap_or(e);
        return Err(net_err(detail));
    }
    if params.get("state") != Some(&pkce.state) {
        return Err(net_err("the sign-in answer didn't match; please try again"));
    }
    params
        .get("code")
        .cloned()
        .ok_or_else(|| net_err("the sign-in page returned no code"))
}

#[derive(Debug, Clone)]
pub struct Tokens {
    pub access_token: String,
    /// Replaces the stored one (refresh tokens rotate).
    pub refresh_token: Option<String>,
    pub expires_at: Instant,
}

impl Tokens {
    pub fn is_fresh(&self) -> bool {
        Instant::now() + Duration::from_secs(120) < self.expires_at
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
}

fn token_request(form: &[(&str, &str)]) -> Result<Tokens> {
    let resp = agent()
        .post(&format!("{AUTHORITY}/token"))
        .send_form(form.iter().copied())
        .map_err(net_err)?;
    let t: TokenResponse = check(resp)?.body_mut().read_json().map_err(net_err)?;
    Ok(Tokens {
        access_token: t.access_token,
        refresh_token: t.refresh_token,
        expires_at: Instant::now() + Duration::from_secs(t.expires_in),
    })
}

pub fn redeem_code(client_id: &str, redirect_uri: &str, code: &str, pkce: &Pkce) -> Result<Tokens> {
    token_request(&[
        ("client_id", client_id),
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("code_verifier", &pkce.verifier),
        ("scope", SCOPES),
    ])
}

pub fn refresh(client_id: &str, refresh_token: &str) -> Result<Tokens> {
    token_request(&[
        ("client_id", client_id),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("scope", SCOPES),
    ])
}

// MARK: - Drive items

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveItem {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub folder: Option<serde_json::Value>,
    #[serde(default)]
    pub deleted: Option<serde_json::Value>,
    #[serde(default)]
    pub parent_reference: Option<ParentReference>,
    #[serde(default)]
    pub file_system_info: Option<FileSystemInfo>,
}

impl DriveItem {
    pub fn is_folder(&self) -> bool {
        self.folder.is_some()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParentReference {
    pub id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSystemInfo {
    pub last_modified_date_time: Option<String>,
}

#[derive(Deserialize)]
struct Page {
    value: Vec<DriveItem>,
    #[serde(rename = "@odata.nextLink")]
    next: Option<String>,
}

/// Every file below `root_id`, with paths relative to it, from a complete
/// delta listing (which gives parents by id, not by path).
pub fn files_below(root_id: &str, items: &[DriveItem]) -> Vec<RemoteFile> {
    let by_id: HashMap<&str, &DriveItem> = items
        .iter()
        .filter(|i| i.deleted.is_none())
        .map(|i| (i.id.as_str(), i))
        .collect();
    let path_of = |item: &DriveItem| -> Option<String> {
        let mut parts = vec![item.name.as_str()];
        let mut parent = item.parent_reference.as_ref()?.id.as_deref()?;
        for _ in 0..256 {
            if parent == root_id {
                parts.reverse();
                return Some(parts.join("/"));
            }
            let p = by_id.get(parent)?;
            parts.push(&p.name);
            parent = p.parent_reference.as_ref()?.id.as_deref()?;
        }
        None
    };
    items
        .iter()
        .filter(|i| i.deleted.is_none() && !i.is_folder() && i.id != root_id)
        .filter_map(|i| {
            Some(RemoteFile {
                rel_path: path_of(i)?,
                size: i.size,
                modified_ms: i
                    .file_system_info
                    .as_ref()
                    .and_then(|f| f.last_modified_date_time.as_deref())
                    .and_then(parse_time_ms),
            })
        })
        .collect()
}

// MARK: - Graph

/// A signed-in connection (holds a current access token).
pub struct Graph {
    http: ureq::Agent,
    token: String,
}

impl Graph {
    pub fn new(access_token: &str) -> Self {
        Self {
            http: agent(),
            token: access_token.to_string(),
        }
    }

    fn auth(&self) -> String {
        format!("Bearer {}", self.token)
    }

    fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let resp = self
            .http
            .get(url)
            .header("Authorization", &self.auth())
            .call()
            .map_err(net_err)?;
        check(resp)?.body_mut().read_json().map_err(net_err)
    }

    fn pages(&self, first: String) -> Result<Vec<DriveItem>> {
        let mut out = Vec::new();
        let mut next = Some(first);
        while let Some(url) = next {
            let page: Page = self.get_json(&url)?;
            out.extend(page.value);
            next = page.next;
        }
        Ok(out)
    }

    /// The signed-in account's name, to show in Settings.
    pub fn account_name(&self) -> Result<String> {
        #[derive(Deserialize)]
        struct Owner {
            user: Option<User>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct User {
            display_name: Option<String>,
        }
        #[derive(Deserialize)]
        struct Drive {
            owner: Option<Owner>,
        }
        let d: Drive = self.get_json(&format!("{GRAPH}/me/drive?$select=owner"))?;
        Ok(d.owner
            .and_then(|o| o.user)
            .and_then(|u| u.display_name)
            .unwrap_or_else(|| "OneDrive".into()))
    }

    /// Subfolders of a folder (`None`: the drive's top level).
    pub fn folders(&self, folder_id: Option<&str>) -> Result<Vec<DriveItem>> {
        let base = match folder_id {
            Some(id) => format!("{GRAPH}/me/drive/items/{}", encode(id)),
            None => format!("{GRAPH}/me/drive/root"),
        };
        let mut items = self.pages(format!("{base}/children?$select=id,name,folder&$top=200"))?;
        items.retain(|i| i.is_folder() && !i.name.starts_with('.'));
        items.sort_by_key(|i| i.name.to_lowercase());
        Ok(items)
    }

    /// Every file below a folder, however deep.
    pub fn list_all(&self, folder_id: &str) -> Result<Vec<RemoteFile>> {
        let items = self.pages(format!(
            "{GRAPH}/me/drive/items/{}/delta?$select=id,name,size,folder,file,deleted,parentReference,fileSystemInfo",
            encode(folder_id)
        ))?;
        Ok(files_below(folder_id, &items))
    }

    fn content_url(folder_id: &str, rel_path: &str) -> String {
        let path: Vec<String> = rel_path.split('/').map(encode).collect();
        format!(
            "{GRAPH}/me/drive/items/{}:/{}:/content",
            encode(folder_id),
            path.join("/")
        )
    }

    /// A file below a folder, in memory (for Rupert's small files).
    pub fn read(&self, folder_id: &str, rel_path: &str) -> Result<Vec<u8>> {
        let resp = self
            .http
            .get(&Self::content_url(folder_id, rel_path))
            .header("Authorization", &self.auth())
            .call()
            .map_err(net_err)?;
        let mut bytes = Vec::new();
        check(resp)?
            .body_mut()
            .as_reader()
            .read_to_end(&mut bytes)
            .map_err(net_err)?;
        Ok(bytes)
    }

    /// Downloads a document the user opened into `dest`.
    pub fn download(&self, folder_id: &str, rel_path: &str, dest: &std::path::Path) -> Result<()> {
        let resp = self
            .http
            .get(&Self::content_url(folder_id, rel_path))
            .header("Authorization", &self.auth())
            .call()
            .map_err(net_err)?;
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = dest.with_extension("part");
        let mut file = std::fs::File::create(&tmp)?;
        std::io::copy(&mut check(resp)?.body_mut().as_reader(), &mut file)?;
        std::fs::rename(&tmp, dest)?;
        Ok(())
    }

    /// Adds a new file to a folder; if the name is taken OneDrive picks a
    /// free one. Returns the name it got.
    pub fn upload(&self, folder_id: &str, name: &str, bytes: &[u8]) -> Result<String> {
        let target = format!(
            "{GRAPH}/me/drive/items/{}:/{}:",
            encode(folder_id),
            encode(name)
        );
        let item: DriveItem = if bytes.len() <= SIMPLE_UPLOAD_MAX {
            let resp = self
                .http
                .put(&format!(
                    "{target}/content?@microsoft.graph.conflictBehavior=rename"
                ))
                .header("Authorization", &self.auth())
                .header("Content-Type", "application/octet-stream")
                .send(bytes)
                .map_err(net_err)?;
            check(resp)?.body_mut().read_json().map_err(net_err)?
        } else {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Session {
                upload_url: String,
            }
            let resp = self
                .http
                .post(&format!("{target}/createUploadSession"))
                .header("Authorization", &self.auth())
                .send_json(serde_json::json!({
                    "item": { "@microsoft.graph.conflictBehavior": "rename" }
                }))
                .map_err(net_err)?;
            let session: Session = check(resp)?.body_mut().read_json().map_err(net_err)?;
            let total = bytes.len();
            let mut last = None;
            for (i, chunk) in bytes.chunks(UPLOAD_CHUNK).enumerate() {
                let start = i * UPLOAD_CHUNK;
                let end = start + chunk.len() - 1;
                // The upload URL is pre-authorized: no bearer token.
                let resp = self
                    .http
                    .put(&session.upload_url)
                    .header("Content-Range", &format!("bytes {start}-{end}/{total}"))
                    .send(chunk)
                    .map_err(net_err)?;
                last = Some(check(resp)?);
            }
            last.ok_or_else(|| net_err("empty upload"))?
                .body_mut()
                .read_json()
                .map_err(net_err)?
        };
        Ok(item.name)
    }
}

// MARK: - Encoding helpers

fn base64url(bytes: &[u8]) -> String {
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

/// Percent-encodes everything but RFC 3986 unreserved characters.
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn query(params: &[(&str, &str)]) -> String {
    params
        .iter()
        .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

fn percent_decode(s: &str) -> String {
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
    fn sign_in_url_and_redirect() {
        let p = Pkce::new();
        let url = authorize_url("abc-123", "msauth.x.y://auth", &p);
        assert!(
            url.starts_with("https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize?")
        );
        assert!(url.contains("redirect_uri=msauth.x.y%3A%2F%2Fauth"));
        assert!(url.contains("scope=Files.ReadWrite%20offline_access"));
        assert!(url.contains(&format!("code_challenge={}", p.challenge)));

        let back = format!("msauth.x.y://auth?code=M.C5_a%2Fb&state={}", p.state);
        assert_eq!(code_from_redirect(&back, &p).unwrap(), "M.C5_a/b");
        assert!(code_from_redirect("msauth.x.y://auth?code=c&state=other", &p).is_err());
        let denied = "msauth.x.y://auth?error=access_denied&error_description=The+user+said+no";
        let e = code_from_redirect(denied, &p).unwrap_err().to_string();
        assert!(e.contains("The user said no"), "{e}");
    }

    #[test]
    fn parses_graph_times() {
        assert_eq!(parse_time_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_time_ms("2025-03-14T18:22:05Z"),
            Some(1_741_976_525_000)
        );
        assert_eq!(
            parse_time_ms("2025-03-14T18:22:05.12Z"),
            Some(1_741_976_525_120)
        );
        assert_eq!(
            parse_time_ms("2024-02-29T00:00:00.999Z"),
            Some(1_709_164_800_999)
        );
        assert_eq!(parse_time_ms("2025-03-14 18:22:05"), None);
    }

    #[test]
    fn rebuilds_paths_from_a_delta_listing() {
        let json = r#"{"value":[
            {"id":"ROOT","name":"Boating","folder":{},"parentReference":{"id":"TOP"}},
            {"id":"E","name":"Engine","folder":{},"parentReference":{"id":"ROOT"}},
            {"id":"f1","name":"impeller.pdf","size":10,"file":{},"parentReference":{"id":"E"},
             "fileSystemInfo":{"lastModifiedDateTime":"2025-03-14T18:22:05Z"}},
            {"id":"R","name":".rupert","folder":{},"parentReference":{"id":"E"}},
            {"id":"f2","name":"impeller.pdf.json","size":5,"file":{},"parentReference":{"id":"R"}},
            {"id":"f3","name":"gone.pdf","deleted":{},"parentReference":{"id":"ROOT"}},
            {"id":"f4","name":"lease.pdf","size":7,"file":{},"parentReference":{"id":"ROOT"}}
        ]}"#;
        let page: Page = serde_json::from_str(json).unwrap();
        let mut files = files_below("ROOT", &page.value);
        files.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
        let paths: Vec<&str> = files.iter().map(|f| f.rel_path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "Engine/.rupert/impeller.pdf.json",
                "Engine/impeller.pdf",
                "lease.pdf"
            ]
        );
        assert_eq!(files[1].modified_ms, Some(1_741_976_525_000));
        assert_eq!(files[1].size, 10);
    }

    #[test]
    fn content_urls_encode_each_segment() {
        assert_eq!(
            Graph::content_url("A!1", "Taxes 2025/W-2 #1.pdf"),
            "https://graph.microsoft.com/v1.0/me/drive/items/A%211:/Taxes%202025/W-2%20%231.pdf:/content"
        );
    }
}
