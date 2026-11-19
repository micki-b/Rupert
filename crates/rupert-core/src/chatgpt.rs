//! Signing in with ChatGPT so the assistant can use the person's ChatGPT
//! plan instead of an API key ("Sign in with ChatGPT", plan usage for local
//! and open-source apps; preview). See
//! developers.openai.com/siwc/token-sharing-open-source.
//!
//! - OAuth 2.0 authorization code with PKCE, as a public client with no
//!   secret. The first sign-in registers Rupert dynamically
//!   (`client_id=dynamic_agent_client`); the answer carries the client id to
//!   use from then on.
//! - The browser comes back to a listener on `127.0.0.1` (the caller runs
//!   it); only the port may change between sign-ins.
//! - Access tokens last an hour; refresh tokens 30 days and rotate.
//! - Requests then go to OpenAI's Responses API with the access token (see
//!   `assistant.rs`).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::oauth::{Pkce, base64url_decode, parse_query, query};

const AUTHORIZE_URL: &str = "https://auth.openai.com/api/accounts/authorize";
const TOKEN_URL: &str = "https://auth.openai.com/api/accounts/oauth/token";
const ISSUER: &str = "https://auth.openai.com";
pub const API_BASE: &str = "https://api.openai.com/v1";
const FIRST_CLIENT: &str = "dynamic_agent_client";
const SCOPES: &str =
    "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";
/// The permission to use the person's ChatGPT plan.
const PLAN_SCOPE: &str = "chatgpt.tokens.use.direct";
pub const CALLBACK_PATH: &str = "/auth/callback";
/// The port OpenAI's examples use; any free one works.
pub const PREFERRED_PORT: u16 = 1455;

fn err(e: impl std::fmt::Display) -> Error {
    Error::Unsupported(format!("ChatGPT: {e}"))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(60)))
        .http_status_as_error(false)
        .build()
        .into()
}

// MARK: - Credentials

/// What a sign-in leaves behind; the caller keeps it in the keychain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Credentials {
    pub email: Option<String>,
    /// The account (`sub` of the ID token).
    pub subject: String,
    /// Issued to Rupert on the first sign-in (`oaiapp_…`).
    pub client_id: String,
    pub host_id: String,
    pub id_token: String,
    pub access_token: String,
    pub refresh_token: String,
    /// Seconds since the epoch.
    pub expires_at: u64,
    pub scopes: String,
}

impl Credentials {
    /// Whether the access token is good for at least a few more minutes.
    pub fn is_fresh(&self) -> bool {
        now_secs() + 300 < self.expires_at
    }
}

/// An opaque, stable id for this computer (`urn:uuid:…`), made once.
pub fn new_host_id() -> String {
    format!("urn:uuid:{}", uuid::Uuid::new_v4())
}

// MARK: - Signing in

/// One sign-in attempt.
#[derive(Debug, Clone)]
pub struct SignIn {
    pub pkce: Pkce,
    pub redirect_uri: String,
    pub host_id: String,
    /// The client id from an earlier sign-in, if any.
    pub client_id: Option<String>,
}

impl SignIn {
    pub fn new(port: u16, host_id: &str, earlier: Option<&Credentials>) -> Self {
        Self {
            pkce: Pkce::new(),
            redirect_uri: format!("http://127.0.0.1:{port}{CALLBACK_PATH}"),
            host_id: host_id.to_string(),
            client_id: earlier.map(|c| c.client_id.clone()),
        }
    }

    /// The page to open in the browser.
    pub fn authorize_url(&self, id_token_hint: Option<&str>) -> String {
        let client_id = self.client_id.as_deref().unwrap_or(FIRST_CLIENT);
        let mut params: Vec<(&str, &str)> = vec![("client_id", client_id)];
        if self.client_id.is_none() {
            params.push(("agent_name_hint", "Rupert"));
        } else if let Some(hint) = id_token_hint {
            params.push(("id_token_hint", hint));
        }
        params.extend([
            ("ext_agent_host_id", self.host_id.as_str()),
            ("response_type", "code"),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("scope", SCOPES),
            ("resource", API_BASE),
            ("state", self.pkce.state.as_str()),
            ("nonce", self.pkce.nonce.as_str()),
            ("code_challenge_method", "S256"),
            ("code_challenge", self.pkce.challenge.as_str()),
        ]);
        format!("{AUTHORIZE_URL}?{}", query(&params))
    }

    /// Reads the browser's return (`/auth/callback?code=…`): the code and
    /// the client id to use. Checks it answers this attempt.
    pub fn callback(&self, target: &str) -> Result<(String, String)> {
        let q = parse_query(target);
        if q.get("state") != Some(&self.pkce.state) {
            return Err(err("the sign-in answer didn't match; please try again"));
        }
        if let Some(e) = q.get("error") {
            return Err(err(if e == "access_denied" {
                "signing in was cancelled".to_string()
            } else {
                q.get("error_description").unwrap_or(e).clone()
            }));
        }
        let code = q
            .get("code")
            .cloned()
            .ok_or_else(|| err("the sign-in page returned no code"))?;
        let client_id = match (&self.client_id, q.get("client_id")) {
            (Some(known), Some(given)) if given != known => {
                return Err(err("the sign-in returned another app registration"));
            }
            (Some(known), _) => known.clone(),
            (None, Some(given)) if given != FIRST_CLIENT && !given.is_empty() => given.clone(),
            (None, _) => return Err(err("registering Rupert with ChatGPT didn't finish")),
        };
        Ok((code, client_id))
    }

    /// Redeems the code and checks the answer.
    pub fn finish(&self, code: &str, client_id: &str) -> Result<Credentials> {
        let tokens = token_request(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("code", code),
            ("code_verifier", &self.pkce.verifier),
            ("redirect_uri", &self.redirect_uri),
            ("resource", API_BASE),
        ])?;
        credentials(
            tokens,
            client_id,
            &self.host_id,
            Some(&self.pkce.nonce),
            None,
        )
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    #[serde(default)]
    id_token: Option<String>,
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    scope: Option<String>,
}

fn token_request(form: &[(&str, &str)]) -> Result<TokenResponse> {
    let mut resp = agent()
        .post(TOKEN_URL)
        .send_form(form.iter().copied())
        .map_err(err)?;
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().map_err(err)?;
    if status != 200 {
        let body: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        let message = body
            .get("error_description")
            .or_else(|| body.get("error"))
            .and_then(Value::as_str)
            .map(String::from)
            .unwrap_or_else(|| format!("HTTP {status}"));
        return Err(err(if message == "invalid_grant" {
            "the sign-in has expired; please sign in again".to_string()
        } else {
            message
        }));
    }
    serde_json::from_str(&text).map_err(err)
}

/// Checks a token answer and turns it into credentials. `previous` is the
/// record being refreshed (an answer may leave out what didn't change).
fn credentials(
    t: TokenResponse,
    client_id: &str,
    host_id: &str,
    nonce: Option<&str>,
    previous: Option<&Credentials>,
) -> Result<Credentials> {
    let id_token = t
        .id_token
        .or_else(|| previous.map(|p| p.id_token.clone()))
        .ok_or_else(|| err("the sign-in returned no ID token"))?;
    // The ID token came straight from the token endpoint over TLS, which
    // OpenID Connect Core 3.1.3.7 accepts in place of checking its
    // signature; its claims are still checked.
    let claims = id_claims(&id_token)?;
    if claims.get("iss").and_then(Value::as_str) != Some(ISSUER) {
        return Err(err("the sign-in came from an unexpected issuer"));
    }
    let aud_ok = match claims.get("aud") {
        Some(Value::String(a)) => a == client_id,
        Some(Value::Array(a)) => a.iter().any(|v| v.as_str() == Some(client_id)),
        _ => false,
    };
    if !aud_ok {
        return Err(err("the sign-in was meant for another app"));
    }
    if previous.is_none()
        && claims.get("exp").and_then(Value::as_u64).unwrap_or(0) + 60 < now_secs()
    {
        return Err(err("the sign-in has expired; please try again"));
    }
    if let Some(nonce) = nonce
        && claims.get("nonce").and_then(Value::as_str) != Some(nonce)
    {
        return Err(err("the sign-in answer didn't match; please try again"));
    }
    let subject = claims
        .get("sub")
        .and_then(Value::as_str)
        .ok_or_else(|| err("the sign-in named no account"))?
        .to_string();
    if let Some(p) = previous
        && p.subject != subject
    {
        return Err(err("the refreshed sign-in belongs to another account"));
    }
    let scopes = t
        .scope
        .or_else(|| previous.map(|p| p.scopes.clone()))
        .unwrap_or_default();
    if !scopes.split_whitespace().any(|s| s == PLAN_SCOPE) {
        return Err(err(
            "permission to use your ChatGPT plan wasn't granted; sign in again and allow it",
        ));
    }
    let refresh_token = t
        .refresh_token
        .or_else(|| previous.map(|p| p.refresh_token.clone()))
        .ok_or_else(|| err("the sign-in returned no refresh token"))?;
    Ok(Credentials {
        email: claims
            .get("email")
            .and_then(Value::as_str)
            .map(String::from)
            .or_else(|| previous.and_then(|p| p.email.clone())),
        subject,
        client_id: client_id.to_string(),
        host_id: host_id.to_string(),
        id_token,
        access_token: t.access_token,
        refresh_token,
        expires_at: now_secs() + t.expires_in.unwrap_or(3600),
        scopes,
    })
}

/// The claims (payload) of a JWT, without checking its signature.
fn id_claims(jwt: &str) -> Result<serde_json::Map<String, Value>> {
    let payload = jwt
        .split('.')
        .nth(1)
        .and_then(base64url_decode)
        .ok_or_else(|| err("the ID token is malformed"))?;
    match serde_json::from_slice(&payload) {
        Ok(Value::Object(map)) => Ok(map),
        _ => Err(err("the ID token is malformed")),
    }
}

/// New tokens for `c` (the refresh token rotates: keep the returned record).
pub fn refresh(c: &Credentials) -> Result<Credentials> {
    let t = token_request(&[
        ("grant_type", "refresh_token"),
        ("client_id", &c.client_id),
        ("refresh_token", &c.refresh_token),
        ("resource", API_BASE),
    ])?;
    credentials(t, &c.client_id, &c.host_id, None, Some(c))
}

// MARK: - Models

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlanModel {
    pub slug: String,
    pub display_name: String,
}

/// The models this account may use, in OpenAI's order.
pub fn models(access_token: &str) -> Result<Vec<PlanModel>> {
    let mut resp = agent()
        .get(&format!("{API_BASE}/models"))
        .header("authorization", &format!("Bearer {access_token}"))
        .call()
        .map_err(err)?;
    let status = resp.status().as_u16();
    let body: Value = resp.body_mut().read_json().map_err(err)?;
    if status != 200 {
        return Err(err(format!("listing models failed (HTTP {status})")));
    }
    Ok(listed_models(&body))
}

fn listed_models(body: &Value) -> Vec<PlanModel> {
    body.get("models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|m| m.get("visibility").and_then(Value::as_str) == Some("list"))
        .filter_map(|m| {
            let slug = m.get("slug")?.as_str()?.to_string();
            let display_name = m
                .get("display_name")
                .and_then(Value::as_str)
                .unwrap_or(&slug)
                .to_string();
            Some(PlanModel { slug, display_name })
        })
        .collect()
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oauth::base64url;
    use serde_json::json;

    fn jwt(claims: Value) -> String {
        format!(
            "{}.{}.sig",
            base64url(br#"{"alg":"RS256"}"#),
            base64url(claims.to_string().as_bytes())
        )
    }

    fn answer(claims: Value, scope: &str) -> TokenResponse {
        TokenResponse {
            id_token: Some(jwt(claims)),
            access_token: "at".into(),
            refresh_token: Some("rt".into()),
            expires_in: Some(3600),
            scope: Some(scope.into()),
        }
    }

    #[test]
    fn first_sign_in_url() {
        let s = SignIn::new(1455, "urn:uuid:h", None);
        let url = s.authorize_url(None);
        assert!(url.starts_with("https://auth.openai.com/api/accounts/authorize?"));
        let q = parse_query(&url);
        assert_eq!(q["client_id"], "dynamic_agent_client");
        assert_eq!(q["agent_name_hint"], "Rupert");
        assert_eq!(q["redirect_uri"], "http://127.0.0.1:1455/auth/callback");
        assert_eq!(q["scope"], SCOPES);
        assert_eq!(q["resource"], "https://api.openai.com/v1");
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["ext_agent_host_id"], "urn:uuid:h");
        assert_eq!(q["nonce"], s.pkce.nonce);
        assert!(!q.contains_key("id_token_hint"));
    }

    #[test]
    fn later_sign_in_reuses_the_client() {
        let earlier = Credentials {
            email: None,
            subject: "u".into(),
            client_id: "oaiapp_1".into(),
            host_id: "h".into(),
            id_token: "old".into(),
            access_token: "a".into(),
            refresh_token: "r".into(),
            expires_at: 0,
            scopes: PLAN_SCOPE.into(),
        };
        let s = SignIn::new(5000, "h", Some(&earlier));
        let q = parse_query(&s.authorize_url(Some("old")));
        assert_eq!(q["client_id"], "oaiapp_1");
        assert_eq!(q["id_token_hint"], "old");
        assert!(!q.contains_key("agent_name_hint"));
        // The callback may leave the client id out, but not change it.
        let state = &s.pkce.state;
        assert_eq!(
            s.callback(&format!("/auth/callback?code=c&state={state}"))
                .unwrap(),
            ("c".into(), "oaiapp_1".into())
        );
        assert!(
            s.callback(&format!(
                "/auth/callback?code=c&state={state}&client_id=oaiapp_2"
            ))
            .is_err()
        );
    }

    #[test]
    fn callbacks_are_checked() {
        let s = SignIn::new(1455, "h", None);
        let state = s.pkce.state.clone();
        assert_eq!(
            s.callback(&format!(
                "/auth/callback?code=c1&state={state}&client_id=oaiapp_9"
            ))
            .unwrap(),
            ("c1".into(), "oaiapp_9".into())
        );
        assert!(
            s.callback("/auth/callback?code=c1&state=other&client_id=oaiapp_9")
                .is_err()
        );
        let denied = s
            .callback(&format!("/auth/callback?error=access_denied&state={state}"))
            .unwrap_err();
        assert!(denied.to_string().contains("cancelled"));
        // A first sign-in must come back with an issued client id.
        assert!(
            s.callback(&format!("/auth/callback?code=c1&state={state}"))
                .is_err()
        );
    }

    #[test]
    fn token_answers_are_checked() {
        let good = json!({"iss": ISSUER, "aud": "oaiapp_1", "exp": now_secs() + 600,
            "nonce": "n", "sub": "user-1", "email": "a@example.com"});
        let c = credentials(
            answer(good.clone(), SCOPES),
            "oaiapp_1",
            "h",
            Some("n"),
            None,
        )
        .unwrap();
        assert_eq!(c.subject, "user-1");
        assert_eq!(c.email.as_deref(), Some("a@example.com"));
        assert!(c.is_fresh());

        let mut wrong_aud = good.clone();
        wrong_aud["aud"] = json!("someone-else");
        assert!(credentials(answer(wrong_aud, SCOPES), "oaiapp_1", "h", Some("n"), None).is_err());
        let mut wrong_iss = good.clone();
        wrong_iss["iss"] = json!("https://evil.example");
        assert!(credentials(answer(wrong_iss, SCOPES), "oaiapp_1", "h", Some("n"), None).is_err());
        assert!(
            credentials(
                answer(good.clone(), SCOPES),
                "oaiapp_1",
                "h",
                Some("x"),
                None
            )
            .is_err()
        );
        let no_plan = credentials(
            answer(good.clone(), "openid email"),
            "oaiapp_1",
            "h",
            Some("n"),
            None,
        )
        .unwrap_err();
        assert!(no_plan.to_string().contains("plan"));

        // A refresh may leave out the ID token and scopes; the account must not change.
        let refreshed = TokenResponse {
            id_token: None,
            access_token: "at2".into(),
            refresh_token: Some("rt2".into()),
            expires_in: Some(3600),
            scope: None,
        };
        let c2 = credentials(refreshed, "oaiapp_1", "h", None, Some(&c)).unwrap();
        assert_eq!(
            (c2.access_token.as_str(), c2.refresh_token.as_str()),
            ("at2", "rt2")
        );
        let mut other = good;
        other["sub"] = json!("user-2");
        assert!(credentials(answer(other, SCOPES), "oaiapp_1", "h", None, Some(&c)).is_err());
    }

    #[test]
    fn listed_models_only() {
        let body = json!({"models": [
            {"slug": "gpt-a", "display_name": "GPT A", "visibility": "list"},
            {"slug": "gpt-hidden", "display_name": "Hidden", "visibility": "hide"},
            {"slug": "gpt-b", "visibility": "list"}
        ]});
        assert_eq!(
            listed_models(&body),
            vec![
                PlanModel {
                    slug: "gpt-a".into(),
                    display_name: "GPT A".into()
                },
                PlanModel {
                    slug: "gpt-b".into(),
                    display_name: "gpt-b".into()
                },
            ]
        );
    }
}
