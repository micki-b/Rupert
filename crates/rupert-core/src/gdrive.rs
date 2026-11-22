//! Google Drive through the Drive API (v3), for the phone: the Google Drive
//! app doesn't hand its folders to other apps through Files. "My Drive"
//! only; shared drives aren't listed.
//!
//! - Sign-in: OAuth 2.0 authorization code with PKCE, as an iOS public
//!   client (no secret), redirecting to the client's reversed id. Tokens
//!   are the caller's to keep (Keychain).
//! - Reading: a folder's complete listing, level by level (Drive has no
//!   "everything below" query), Rupert's small hidden files and, when the
//!   user opens one, a document.
//! - Writing: new scans into the inbox (never replacing anything: Drive
//!   allows duplicate names, so a free one is picked first) and folders'
//!   `.rupert/.folder.json`.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::error::{Error, Result};
pub use crate::oauth::{Pkce, Tokens};
use crate::oauth::{encode, parse_time_ms, query};
use crate::remote::RemoteFile;

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const API: &str = "https://www.googleapis.com/drive/v3";
const UPLOAD_API: &str = "https://www.googleapis.com/upload/drive/v3";
/// Reading the drive, adding scans, and starring folders.
pub const SCOPES: &str = "https://www.googleapis.com/auth/drive";
const FOLDER_MIME: &str = "application/vnd.google-apps.folder";
/// Google Docs, Sheets, shortcuts, …: no file behind them to read.
const GOOGLE_APPS: &str = "application/vnd.google-apps.";
/// Folders whose children are asked for in one listing request.
const PARENTS_PER_QUERY: usize = 40;
const ITEM_FIELDS: &str = "id,name,mimeType,size,modifiedTime,parents";

fn net_err(e: impl std::fmt::Display) -> Error {
    Error::Unsupported(format!("Google Drive: {e}"))
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
    let text = resp.body_mut().read_to_string().unwrap_or_default();
    Err(match status {
        401 => net_err(crate::i18n::tr("signed out, please sign in again")),
        _ => net_err(error_message(&text).unwrap_or_else(|| format!("HTTP {status}"))),
    })
}

/// The message in an error answer: the token endpoint's
/// `{"error": "...", "error_description": "..."}` or the API's
/// `{"error": {"message": "..."}}`.
fn error_message(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    if let Some(d) = v.get("error_description").and_then(|d| d.as_str()) {
        return Some(d.to_string());
    }
    match v.get("error")? {
        serde_json::Value::String(s) => Some(s.clone()),
        e => e.get("message")?.as_str().map(String::from),
    }
}

// MARK: - Sign-in

/// The redirect address and its URL scheme for an iOS client id
/// (`123-abc.apps.googleusercontent.com` → `com.googleusercontent.apps.123-abc`).
pub fn redirect_for(client_id: &str) -> Option<(String, String)> {
    let prefix = client_id
        .trim()
        .strip_suffix(".apps.googleusercontent.com")?;
    if prefix.is_empty() {
        return None;
    }
    let scheme = format!("com.googleusercontent.apps.{prefix}");
    Some((format!("{scheme}:/oauthredirect"), scheme))
}

/// Google's sign-in page to show.
pub fn authorize_url(client_id: &str, redirect_uri: &str, pkce: &Pkce) -> String {
    let params = [
        ("client_id", client_id),
        ("response_type", "code"),
        ("redirect_uri", redirect_uri),
        ("scope", SCOPES),
        ("code_challenge", &pkce.challenge),
        ("code_challenge_method", "S256"),
        ("state", &pkce.state),
        // A refresh token, every time (also after signing out and in).
        ("access_type", "offline"),
        ("prompt", "consent"),
    ];
    format!("{AUTH_URL}?{}", query(&params))
}

/// The authorization code from the page's redirect.
pub fn code_from_redirect(url: &str, pkce: &Pkce) -> Result<String> {
    crate::oauth::code_from_redirect(url, pkce).map_err(net_err)
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
}

fn token_request(form: &[(&str, &str)]) -> Result<Tokens> {
    let resp = agent()
        .post(TOKEN_URL)
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
    ])
}

/// A new access token. Google keeps the refresh token the same.
pub fn refresh(client_id: &str, refresh_token: &str) -> Result<Tokens> {
    token_request(&[
        ("client_id", client_id),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
    ])
}

// MARK: - Items and listings

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub mime_type: String,
    /// A decimal string in the API; absent for folders and Google Docs.
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub modified_time: Option<String>,
    #[serde(default)]
    pub parents: Vec<String>,
}

impl Item {
    pub fn is_folder(&self) -> bool {
        self.mime_type == FOLDER_MIME
    }

    /// A real file Rupert can read (not a Google Doc or a shortcut).
    fn is_file(&self) -> bool {
        !self.mime_type.starts_with(GOOGLE_APPS)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Page {
    #[serde(default)]
    files: Vec<Item>,
    next_page_token: Option<String>,
}

/// Everything below a folder: the files as `remote::sync` takes them, and
/// the ids behind their paths (Drive works by id only).
#[derive(Debug, Default, Clone)]
pub struct Listing {
    pub files: Vec<RemoteFile>,
    /// File path (relative to the folder) → id.
    pub file_ids: HashMap<String, String>,
    /// Folder path → id; `""` is the folder itself.
    pub dir_ids: HashMap<String, String>,
}

/// Walks a folder tree level by level. `children(parents)` returns the
/// children of the given folder ids (in any order); each item names its
/// parent. Folders reached twice (Drive allows several parents) count once.
pub fn walk(
    root_id: &str,
    mut children: impl FnMut(&[String]) -> Result<Vec<Item>>,
) -> Result<Listing> {
    let mut listing = Listing::default();
    listing.dir_ids.insert(String::new(), root_id.to_string());
    let mut path_of: HashMap<String, String> =
        HashMap::from([(root_id.to_string(), String::new())]);
    let mut seen: HashSet<String> = HashSet::from([root_id.to_string()]);
    let mut level = vec![root_id.to_string()];
    while !level.is_empty() {
        let mut next = Vec::new();
        for batch in level.chunks(PARENTS_PER_QUERY) {
            for item in children(batch)? {
                let Some(parent_path) = item
                    .parents
                    .iter()
                    .find_map(|p| batch.contains(p).then(|| path_of.get(p)).flatten())
                    .cloned()
                else {
                    continue;
                };
                let rel = if parent_path.is_empty() {
                    item.name.clone()
                } else {
                    format!("{parent_path}/{}", item.name)
                };
                if item.is_folder() {
                    if seen.insert(item.id.clone()) {
                        path_of.insert(item.id.clone(), rel.clone());
                        listing.dir_ids.insert(rel, item.id.clone());
                        next.push(item.id);
                    }
                } else if item.is_file() && !listing.file_ids.contains_key(&rel) {
                    listing.files.push(RemoteFile {
                        rel_path: rel.clone(),
                        size: item
                            .size
                            .as_deref()
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0),
                        modified_ms: item.modified_time.as_deref().and_then(parse_time_ms),
                    });
                    listing.file_ids.insert(rel, item.id);
                }
            }
        }
        level = next;
    }
    Ok(listing)
}

/// `name`, or with " (2)", " (3)", … before the extension if `taken`.
pub fn free_name(name: &str, taken: &HashSet<String>) -> String {
    if !taken.contains(name) {
        return name.to_string();
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, format!(".{e}")),
        _ => (name, String::new()),
    };
    (2..)
        .map(|n| format!("{stem} ({n}){ext}"))
        .find(|candidate| !taken.contains(candidate))
        .expect("an unused name")
}

/// A thumbnail link asking for `side` pixels: Drive's links end in a size
/// (`=s220`), which can be changed.
fn sized_thumbnail(link: &str, side: u32) -> String {
    match link.rsplit_once("=s") {
        Some((base, size)) if !size.is_empty() && size.bytes().all(|b| b.is_ascii_digit()) => {
            format!("{base}=s{side}")
        }
        _ => link.to_string(),
    }
}

/// The type Drive is told a new file has, from its name.
fn mime_for(name: &str) -> &'static str {
    let path = std::path::Path::new(name);
    if crate::file_kind::FileKind::from_path(path) == crate::file_kind::FileKind::Pdf {
        return "application/pdf";
    }
    crate::file_kind::image_mime(path).unwrap_or("application/octet-stream")
}

/// A Drive query string literal.
fn quoted(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

// MARK: - Drive

/// A signed-in connection (holds a current access token).
pub struct Drive {
    http: ureq::Agent,
    token: String,
}

impl Drive {
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

    /// Every item matching a query, across pages.
    fn search(&self, q: &str, fields: &str) -> Result<Vec<Item>> {
        let mut out = Vec::new();
        let mut token: Option<String> = None;
        loop {
            let mut params = vec![
                ("q", q),
                ("fields", fields),
                ("pageSize", "1000"),
                ("spaces", "drive"),
            ];
            if let Some(t) = &token {
                params.push(("pageToken", t));
            }
            let page: Page = self.get_json(&format!("{API}/files?{}", query(&params)))?;
            out.extend(page.files);
            match page.next_page_token {
                Some(t) => token = Some(t),
                None => return Ok(out),
            }
        }
    }

    /// The signed-in account's name, to show in Settings.
    pub fn account_name(&self) -> Result<String> {
        #[derive(Deserialize)]
        struct About {
            user: Option<User>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct User {
            display_name: Option<String>,
            email_address: Option<String>,
        }
        let a: About = self.get_json(&format!(
            "{API}/about?fields=user(displayName,emailAddress)"
        ))?;
        Ok(a.user
            .and_then(|u| u.email_address.or(u.display_name))
            .unwrap_or_else(|| "Google Drive".into()))
    }

    /// Subfolders of a folder (`None`: the top of My Drive), for choosing.
    pub fn folders(&self, folder_id: Option<&str>) -> Result<Vec<Item>> {
        let parent = folder_id.unwrap_or("root");
        let q = format!(
            "{} in parents and mimeType = '{FOLDER_MIME}' and trashed = false",
            quoted(parent)
        );
        let mut items = self.search(&q, &format!("nextPageToken,files({ITEM_FIELDS})"))?;
        items.retain(|i| !i.name.starts_with('.'));
        items.sort_by_key(|i| i.name.to_lowercase());
        Ok(items)
    }

    /// Everything below a folder, however deep (Rupert's hidden files too).
    pub fn list_all(&self, folder_id: &str) -> Result<Listing> {
        walk(folder_id, |parents| {
            let any_parent: Vec<String> = parents
                .iter()
                .map(|p| format!("{} in parents", quoted(p)))
                .collect();
            let q = format!("({}) and trashed = false", any_parent.join(" or "));
            self.search(&q, &format!("nextPageToken,files({ITEM_FIELDS})"))
        })
    }

    /// A file's contents, in memory (for Rupert's small files).
    pub fn read(&self, file_id: &str) -> Result<Vec<u8>> {
        let resp = self
            .http
            .get(&format!("{API}/files/{}?alt=media", encode(file_id)))
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
    pub fn download(&self, file_id: &str, dest: &std::path::Path) -> Result<()> {
        let resp = self
            .http
            .get(&format!("{API}/files/{}?alt=media", encode(file_id)))
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

    /// A document's picture made by Google Drive (fitting in `side` pixels),
    /// or `None` if it has none. The document itself isn't downloaded.
    pub fn thumbnail(&self, file_id: &str, side: u32) -> Result<Option<Vec<u8>>> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Link {
            thumbnail_link: Option<String>,
        }
        let link: Link = self.get_json(&format!(
            "{API}/files/{}?fields=thumbnailLink",
            encode(file_id)
        ))?;
        let Some(link) = link.thumbnail_link else {
            return Ok(None);
        };
        let resp = self
            .http
            .get(&sized_thumbnail(&link, side))
            .header("Authorization", &self.auth())
            .call()
            .map_err(net_err)?;
        if resp.status().as_u16() == 404 {
            return Ok(None);
        }
        let bytes = check(resp)?
            .body_mut()
            .with_config()
            .limit(8 * 1024 * 1024)
            .read_to_vec()
            .map_err(net_err)?;
        Ok((!bytes.is_empty()).then_some(bytes))
    }

    /// The child of a folder with this name, if any (the first, if Drive
    /// holds several).
    fn child(&self, parent_id: &str, name: &str) -> Result<Option<Item>> {
        let q = format!(
            "{} in parents and name = {} and trashed = false",
            quoted(parent_id),
            quoted(name)
        );
        Ok(self
            .search(&q, &format!("nextPageToken,files({ITEM_FIELDS})"))?
            .into_iter()
            .next())
    }

    fn create_folder(&self, parent_id: &str, name: &str) -> Result<String> {
        let resp = self
            .http
            .post(&format!("{API}/files?fields=id"))
            .header("Authorization", &self.auth())
            .send_json(serde_json::json!({
                "name": name,
                "mimeType": FOLDER_MIME,
                "parents": [parent_id],
            }))
            .map_err(net_err)?;
        let item: Item = check(resp)?.body_mut().read_json().map_err(net_err)?;
        Ok(item.id)
    }

    /// Creates a file (resumable upload: one request for the details, one
    /// for the bytes, at any size). Returns its id and name.
    fn create_file(&self, parent_id: &str, name: &str, mime: &str, bytes: &[u8]) -> Result<Item> {
        let resp = self
            .http
            .post(&format!(
                "{UPLOAD_API}/files?uploadType=resumable&fields=id,name"
            ))
            .header("Authorization", &self.auth())
            .header("X-Upload-Content-Type", mime)
            .header("X-Upload-Content-Length", &bytes.len().to_string())
            .send_json(serde_json::json!({ "name": name, "parents": [parent_id] }))
            .map_err(net_err)?;
        let resp = check(resp)?;
        let session = resp
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| net_err("no upload address"))?
            .to_string();
        let resp = self
            .http
            .put(&session)
            .header("Content-Type", mime)
            .send(bytes)
            .map_err(net_err)?;
        check(resp)?.body_mut().read_json().map_err(net_err)
    }

    /// Replaces a file's contents.
    fn update_file(&self, file_id: &str, bytes: &[u8]) -> Result<()> {
        let resp = self
            .http
            .patch(&format!(
                "{UPLOAD_API}/files/{}?uploadType=media",
                encode(file_id)
            ))
            .header("Authorization", &self.auth())
            .header("Content-Type", "application/octet-stream")
            .send(bytes)
            .map_err(net_err)?;
        check(resp)?;
        Ok(())
    }

    /// Writes one of Rupert's small files (e.g. `Boat/.rupert/.folder.json`)
    /// below a folder: replaced if it exists, created otherwise, along with
    /// its parent folders. `listing` is the folder's last listing (ids by
    /// path); anything missing from it is looked up.
    pub fn write_small(
        &self,
        folder_id: &str,
        listing: &Listing,
        rel_path: &str,
        bytes: &[u8],
    ) -> Result<()> {
        if let Some(id) = listing.file_ids.get(rel_path) {
            return self.update_file(id, bytes);
        }
        let (dir, name) = rel_path.rsplit_once('/').unwrap_or(("", rel_path));
        let mut parent = folder_id.to_string();
        let mut path = String::new();
        for part in dir.split('/').filter(|p| !p.is_empty()) {
            path = if path.is_empty() {
                part.to_string()
            } else {
                format!("{path}/{part}")
            };
            parent = match listing.dir_ids.get(&path) {
                Some(id) => id.clone(),
                None => match self.child(&parent, part)? {
                    Some(item) if item.is_folder() => item.id,
                    _ => self.create_folder(&parent, part)?,
                },
            };
        }
        match self.child(&parent, name)? {
            Some(existing) => self.update_file(&existing.id, bytes),
            None => self
                .create_file(&parent, name, "application/json", bytes)
                .map(|_| ()),
        }
    }

    /// The id of the file at `rel_path` below a folder: from the last
    /// listing, or looked up by name (it may be newer than the listing).
    pub fn find(
        &self,
        folder_id: &str,
        listing: &Listing,
        rel_path: &str,
    ) -> Result<Option<String>> {
        if let Some(id) = listing.file_ids.get(rel_path) {
            return Ok(Some(id.clone()));
        }
        let mut id = folder_id.to_string();
        for part in rel_path.split('/').filter(|p| !p.is_empty()) {
            match self.child(&id, part)? {
                Some(item) => id = item.id,
                None => return Ok(None),
            }
        }
        Ok(Some(id))
    }

    /// Moves one of Rupert's small files to Drive's trash. Already gone
    /// counts as done.
    pub fn delete_small(&self, file_id: &str) -> Result<()> {
        let resp = self
            .http
            .patch(&format!("{API}/files/{}", encode(file_id)))
            .header("Authorization", &self.auth())
            .send_json(serde_json::json!({ "trashed": true }))
            .map_err(net_err)?;
        if resp.status().as_u16() != 404 {
            check(resp)?;
        }
        Ok(())
    }

    /// Adds a new file to a folder under a name not yet used there (Drive
    /// would allow a duplicate). Returns the name it got.
    pub fn upload(&self, folder_id: &str, name: &str, bytes: &[u8]) -> Result<String> {
        let q = format!("{} in parents and trashed = false", quoted(folder_id));
        let taken: HashSet<String> = self
            // Every listed item needs its id (`Item` requires it).
            .search(&q, "nextPageToken,files(id,name)")?
            .into_iter()
            .map(|i| i.name)
            .collect();
        let item = self.create_file(folder_id, &free_name(name, &taken), mime_for(name), bytes)?;
        Ok(item.name)
    }
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, name: &str, mime: &str, parent: &str) -> Item {
        Item {
            id: id.into(),
            name: name.into(),
            mime_type: mime.into(),
            size: (mime != FOLDER_MIME).then(|| "10".into()),
            modified_time: Some("2026-01-02T03:04:05.000Z".into()),
            parents: vec![parent.into()],
        }
    }

    #[test]
    fn thumbnail_links_get_the_asked_size() {
        assert_eq!(
            sized_thumbnail("https://lh3.googleusercontent.com/abc=s220", 480),
            "https://lh3.googleusercontent.com/abc=s480"
        );
        assert_eq!(
            sized_thumbnail("https://drive.google.com/thumb?id=x", 480),
            "https://drive.google.com/thumb?id=x"
        );
    }

    #[test]
    fn redirect_comes_from_the_client_id() {
        assert_eq!(
            redirect_for("123-abc.apps.googleusercontent.com"),
            Some((
                "com.googleusercontent.apps.123-abc:/oauthredirect".into(),
                "com.googleusercontent.apps.123-abc".into()
            ))
        );
        assert_eq!(redirect_for("not-a-google-id"), None);
        assert_eq!(redirect_for(".apps.googleusercontent.com"), None);
    }

    #[test]
    fn sign_in_url_asks_for_a_refresh_token() {
        let p = Pkce::new();
        let url = authorize_url("cid", "com.googleusercontent.apps.cid:/oauthredirect", &p);
        let q = crate::oauth::parse_query(&url);
        assert!(url.starts_with(AUTH_URL));
        assert_eq!(q["client_id"], "cid");
        assert_eq!(q["scope"], SCOPES);
        assert_eq!(q["access_type"], "offline");
        assert_eq!(q["code_challenge"], p.challenge);
        assert_eq!(q["state"], p.state);
    }

    #[test]
    fn walks_a_tree_level_by_level() {
        // ROOT/lease.pdf, ROOT/Boat/engine.pdf, ROOT/Boat/.rupert/engine.pdf.json,
        // plus a Google Doc and a shortcut (skipped) and a trashed-free tree.
        let all = [
            item("f1", "lease.pdf", "application/pdf", "ROOT"),
            item("B", "Boat", FOLDER_MIME, "ROOT"),
            item(
                "doc",
                "Notes",
                "application/vnd.google-apps.document",
                "ROOT",
            ),
            item("sc", "Link", "application/vnd.google-apps.shortcut", "ROOT"),
            item("f2", "engine.pdf", "application/pdf", "B"),
            item("R", ".rupert", FOLDER_MIME, "B"),
            item("f3", "engine.pdf.json", "application/json", "R"),
            item("other", "elsewhere.pdf", "application/pdf", "OUTSIDE"),
        ];
        let mut asked: Vec<Vec<String>> = Vec::new();
        let listing = walk("ROOT", |parents| {
            asked.push(parents.to_vec());
            Ok(all
                .iter()
                .filter(|i| i.parents.iter().any(|p| parents.contains(p)))
                .cloned()
                .collect())
        })
        .unwrap();
        let mut paths: Vec<&str> = listing.files.iter().map(|f| f.rel_path.as_str()).collect();
        paths.sort();
        assert_eq!(
            paths,
            [
                "Boat/.rupert/engine.pdf.json",
                "Boat/engine.pdf",
                "lease.pdf"
            ]
        );
        assert_eq!(listing.file_ids["Boat/engine.pdf"], "f2");
        assert_eq!(listing.dir_ids["Boat/.rupert"], "R");
        assert_eq!(listing.dir_ids[""], "ROOT");
        let f = &listing.files[0];
        assert_eq!(f.size, 10);
        assert_eq!(f.modified_ms, parse_time_ms("2026-01-02T03:04:05.000Z"));
        // One request per level: the root, Boat, then .rupert.
        assert_eq!(asked, [vec!["ROOT"], vec!["B"], vec!["R"]]);
    }

    #[test]
    fn many_folders_share_requests() {
        let mut all = Vec::new();
        for n in 0..100 {
            all.push(item(
                &format!("D{n}"),
                &format!("d{n}"),
                FOLDER_MIME,
                "ROOT",
            ));
        }
        let mut requests = 0;
        let listing = walk("ROOT", |parents| {
            requests += 1;
            Ok(all
                .iter()
                .filter(|i| i.parents.iter().any(|p| parents.contains(p)))
                .cloned()
                .collect())
        })
        .unwrap();
        assert_eq!(listing.dir_ids.len(), 101);
        // The root, then 100 folders in batches of 40.
        assert_eq!(requests, 1 + 3);
    }

    #[test]
    fn picks_a_free_name() {
        let taken: HashSet<String> = ["Scan.pdf", "Scan (2).pdf", "Notes"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(free_name("Bill.pdf", &taken), "Bill.pdf");
        assert_eq!(free_name("Scan.pdf", &taken), "Scan (3).pdf");
        assert_eq!(free_name("Notes", &taken), "Notes (2)");
    }

    #[test]
    fn quotes_query_strings() {
        assert_eq!(quoted("Bob's"), r"'Bob\'s'");
        assert_eq!(quoted(r"a\b"), r"'a\\b'");
    }

    #[test]
    fn reads_error_answers() {
        assert_eq!(
            error_message(r#"{"error":"invalid_grant","error_description":"Token has been expired or revoked."}"#)
                .as_deref(),
            Some("Token has been expired or revoked.")
        );
        assert_eq!(
            error_message(
                r#"{"error":{"code":403,"message":"The user has not granted the app access."}}"#
            )
            .as_deref(),
            Some("The user has not granted the app access.")
        );
        assert_eq!(error_message("<html>"), None);
    }

    #[test]
    fn parses_a_page() {
        let page: Page = serde_json::from_str(
            r#"{"nextPageToken":"t2","files":[{"id":"a","name":"x.pdf","mimeType":"application/pdf",
                "size":"123","modifiedTime":"2026-01-02T03:04:05.000Z","parents":["P"]}]}"#,
        )
        .unwrap();
        assert_eq!(page.next_page_token.as_deref(), Some("t2"));
        assert_eq!(page.files[0].size.as_deref(), Some("123"));
        assert!(!page.files[0].is_folder());
    }
}
