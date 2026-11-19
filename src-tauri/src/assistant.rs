//! The chat assistant's commands: which model to use, keys in the system
//! keychain, the `ant` command-line login for Claude, and the conversation
//! itself (logic in `rupert_core::assistant`).

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use rupert_core::assistant::{
    self, AnthropicAuth, Backend, Cancel, Chat, LibraryTools, Step, UiAction,
};
use rupert_core::chatgpt::{self, Credentials, PlanModel, SignIn};
use rupert_core::llm::LmStudio;
use rupert_core::settings::{AssistantProvider, Settings};
use rupert_core::{Error, Index};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::{AppState, CmdResult, err};

/// A tool call while answering. Payload: `Step`.
const ASSISTANT_STEP: &str = "assistant-step";
const KEYCHAIN_SERVICE: &str = "io.github.micki-b.rupert";
const OPENAI_URL: &str = "https://api.openai.com/v1";

pub struct AssistantState {
    chat: Mutex<Chat>,
    /// Stops the answer in progress, if any.
    cancel: Mutex<Cancel>,
}

impl AssistantState {
    pub fn new() -> Self {
        Self {
            chat: Mutex::new(new_chat()),
            cancel: Mutex::new(Cancel::new()),
        }
    }
}

fn new_chat() -> Chat {
    Chat::new(assistant::system_prompt(&today()))
}

/// Today's date (UTC), for the model's sense of "this year".
fn today() -> String {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0) as i64;
    // Civil from days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

// MARK: - Keys

fn key_name(provider: AssistantProvider) -> Option<&'static str> {
    match provider {
        AssistantProvider::Local => None,
        AssistantProvider::Anthropic => Some("anthropic-api-key"),
        AssistantProvider::Openai => Some("openai-api-key"),
        // Signs in instead (`chatgpt_sign_in`).
        AssistantProvider::Chatgpt => None,
    }
}

fn read_key(provider: AssistantProvider) -> Option<String> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, key_name(provider)?).ok()?;
    entry.get_password().ok().filter(|k| !k.trim().is_empty())
}

/// Saves (or, with `None`, removes) a provider's API key in the keychain.
#[tauri::command]
pub fn assistant_set_key(provider: AssistantProvider, key: Option<String>) -> CmdResult<()> {
    let name = key_name(provider).ok_or("This provider needs no key.")?;
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, name).map_err(err)?;
    match key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty()) {
        Some(k) => entry.set_password(&k).map_err(err),
        None => match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(err(e)),
        },
    }
}

#[tauri::command]
pub fn assistant_has_key(provider: AssistantProvider) -> bool {
    read_key(provider).is_some()
}

// MARK: - The `ant` login (Claude)

/// The `ant` CLI. Apps started from the Finder get a short PATH, so the
/// usual install places are tried too.
fn ant() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .chain([
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
        ])
        .chain(
            home.into_iter()
                .flat_map(|h| [h.join(".local/bin"), h.join("go/bin")]),
        )
        .map(|d| d.join(if cfg!(windows) { "ant.exe" } else { "ant" }))
        .find(|p| p.is_file())
}

fn ant_token() -> Result<String, String> {
    let ant = ant().ok_or("The ant command isn't installed (see Settings ▸ Assistant).")?;
    let out = Command::new(ant)
        .args(["auth", "print-credentials", "--access-token"])
        .output()
        .map_err(err)?;
    let token = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || token.is_empty() {
        return Err("Not signed in to Anthropic: use Sign in… in Settings ▸ Assistant.".into());
    }
    Ok(token)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliStatus {
    installed: bool,
    signed_in: bool,
}

#[tauri::command]
pub async fn assistant_cli_status() -> CliStatus {
    tauri::async_runtime::spawn_blocking(|| CliStatus {
        installed: ant().is_some(),
        signed_in: ant_token().is_ok(),
    })
    .await
    .unwrap_or(CliStatus {
        installed: false,
        signed_in: false,
    })
}

/// Runs `ant auth login`, which signs in through the browser.
#[tauri::command]
pub async fn assistant_cli_login() -> CmdResult<()> {
    tauri::async_runtime::spawn_blocking(|| {
        let ant = ant().ok_or("The ant command isn't installed.")?;
        let status = Command::new(ant)
            .args(["auth", "login"])
            .status()
            .map_err(err)?;
        if status.success() {
            Ok(())
        } else {
            Err("Signing in didn't finish.".to_string())
        }
    })
    .await
    .map_err(err)?
}

// MARK: - ChatGPT plan

/// The ChatGPT sign-in, as one JSON entry in the keychain.
const CHATGPT_ENTRY: &str = "chatgpt-plan";
/// How long to wait for the browser to come back.
const SIGN_IN_WAIT: std::time::Duration = std::time::Duration::from_secs(300);

fn chatgpt_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, CHATGPT_ENTRY).map_err(err)
}

fn chatgpt_load() -> Option<Credentials> {
    let json = chatgpt_entry().ok()?.get_password().ok()?;
    serde_json::from_str(&json).ok()
}

fn chatgpt_save(c: &Credentials) -> Result<(), String> {
    chatgpt_entry()?
        .set_password(&serde_json::to_string(c).map_err(err)?)
        .map_err(err)
}

/// The signed-in credentials with an access token good for a while yet
/// (refreshed and saved if needed: refresh tokens rotate).
fn chatgpt_credentials() -> Result<Credentials, String> {
    let c = chatgpt_load().ok_or("Sign in with ChatGPT in Settings ▸ Assistant.")?;
    if c.is_fresh() {
        return Ok(c);
    }
    let fresh = chatgpt::refresh(&c).map_err(err)?;
    chatgpt_save(&fresh)?;
    Ok(fresh)
}

/// This computer's id for OpenAI, made once (not a secret: it only tells
/// one computer from another).
fn chatgpt_host_id(app: &AppHandle) -> Result<String, String> {
    let path = app
        .path()
        .app_config_dir()
        .map_err(err)?
        .join("chatgpt-host-id");
    if let Ok(id) = std::fs::read_to_string(&path)
        && id.trim().starts_with("urn:")
    {
        return Ok(id.trim().to_string());
    }
    let id = chatgpt::new_host_id();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(err)?;
    }
    std::fs::write(&path, &id).map_err(err)?;
    Ok(id)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatGptStatus {
    signed_in: bool,
    email: Option<String>,
    models: Vec<PlanModel>,
    /// Why the models couldn't be listed, if they couldn't.
    problem: Option<String>,
}

fn chatgpt_status_now() -> ChatGptStatus {
    let Some(saved) = chatgpt_load() else {
        return ChatGptStatus {
            signed_in: false,
            email: None,
            models: Vec::new(),
            problem: None,
        };
    };
    let listed = chatgpt_credentials().and_then(|c| chatgpt::models(&c.access_token).map_err(err));
    let (models, problem) = match listed {
        Ok(m) => (m, None),
        Err(e) => (Vec::new(), Some(e)),
    };
    ChatGptStatus {
        signed_in: true,
        email: saved.email,
        models,
        problem,
    }
}

#[tauri::command]
pub async fn chatgpt_status() -> ChatGptStatus {
    tauri::async_runtime::spawn_blocking(chatgpt_status_now)
        .await
        .unwrap_or(ChatGptStatus {
            signed_in: false,
            email: None,
            models: Vec::new(),
            problem: None,
        })
}

/// Signs in with ChatGPT in the browser: OpenAI's page comes back to a
/// one-time listener on 127.0.0.1, then the code is redeemed and the
/// tokens go into the keychain.
#[tauri::command]
pub async fn chatgpt_sign_in(app: AppHandle) -> CmdResult<ChatGptStatus> {
    tauri::async_runtime::spawn_blocking(move || {
        let listener = TcpListener::bind(("127.0.0.1", chatgpt::PREFERRED_PORT))
            .or_else(|_| TcpListener::bind(("127.0.0.1", 0)))
            .map_err(err)?;
        let port = listener.local_addr().map_err(err)?.port();
        let earlier = chatgpt_load();
        let sign_in = SignIn::new(port, &chatgpt_host_id(&app)?, earlier.as_ref());
        let url = sign_in.authorize_url(earlier.as_ref().map(|c| c.id_token.as_str()));
        app.opener().open_url(url, None::<&str>).map_err(err)?;

        let (code, client_id) = wait_for_callback(&listener, &sign_in)?;
        let credentials = sign_in.finish(&code, &client_id).map_err(err)?;
        chatgpt_save(&credentials)?;
        Ok(chatgpt_status_now())
    })
    .await
    .map_err(err)?
}

/// Waits for the browser's return and answers it with a short page.
fn wait_for_callback(listener: &TcpListener, sign_in: &SignIn) -> Result<(String, String), String> {
    listener.set_nonblocking(true).map_err(err)?;
    let deadline = std::time::Instant::now() + SIGN_IN_WAIT;
    loop {
        if std::time::Instant::now() > deadline {
            return Err("Signing in took too long; please try again.".into());
        }
        let mut stream = match listener.accept() {
            Ok((s, _)) => s,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(200));
                continue;
            }
            Err(e) => return Err(err(e)),
        };
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(10)));
        let mut request_line = String::new();
        let _ = BufReader::new(&stream).read_line(&mut request_line);
        // "GET /auth/callback?code=… HTTP/1.1"
        let target = request_line
            .split_whitespace()
            .nth(1)
            .unwrap_or_default()
            .to_string();
        if !target.starts_with(chatgpt::CALLBACK_PATH) {
            let _ = stream.write_all(
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
            continue; // e.g. the browser asking for a favicon
        }
        let result = sign_in.callback(&target).map_err(err);
        let (title, line) = match &result {
            Ok(_) => (
                "Signed in",
                "Rupert can now use your ChatGPT plan. You can close this tab.",
            ),
            Err(e) => ("Not signed in", e.as_str()),
        };
        let page = format!(
            "<!doctype html><meta charset=utf-8><title>Rupert</title>\
             <body style=\"font:16px -apple-system,system-ui,sans-serif;margin:15vh auto;max-width:28em;text-align:center\">\
             <h1 style=\"font-size:22px\">{title}</h1><p>{}</p></body>",
            line.replace('<', "&lt;")
        );
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
            page.len()
        );
        return result;
    }
}

/// Forgets the ChatGPT sign-in on this computer.
#[tauri::command]
pub fn chatgpt_sign_out() -> CmdResult<()> {
    match chatgpt_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(err(e)),
    }
}

// MARK: - Backend

fn is_cloud(settings: &Settings) -> bool {
    settings.assistant.provider != AssistantProvider::Local
}

/// The backend the settings describe (may ask LM Studio for its model, or
/// `ant` for a token).
fn backend(settings: &Settings) -> Result<Backend, String> {
    let a = &settings.assistant;
    let model = a.model.as_deref().map(str::trim).filter(|m| !m.is_empty());
    match a.provider {
        AssistantProvider::Local => {
            let model = match &settings.lm_studio_model {
                Some(m) => m.clone(),
                None => LmStudio {
                    url: settings.lm_studio_url.clone(),
                    model: None,
                }
                .models()
                .map_err(|_| "LM Studio isn't running, or has no model loaded.".to_string())?
                .into_iter()
                .next()
                .ok_or("LM Studio has no model loaded.")?,
            };
            Ok(Backend::OpenAiCompatible {
                base_url: format!("{}/v1", settings.lm_studio_url.trim_end_matches('/')),
                model,
                api_key: None,
            })
        }
        AssistantProvider::Anthropic => {
            let auth = if a.anthropic_cli_login {
                AnthropicAuth::OAuthToken(ant_token()?)
            } else {
                AnthropicAuth::ApiKey(
                    read_key(AssistantProvider::Anthropic)
                        .ok_or("Add an Anthropic API key in Settings ▸ Assistant.")?,
                )
            };
            Ok(Backend::anthropic(model, auth))
        }
        AssistantProvider::Chatgpt => {
            let c = chatgpt_credentials()?;
            let model = match model {
                Some(m) => m.to_string(),
                None => chatgpt::models(&c.access_token)
                    .map_err(err)?
                    .into_iter()
                    .next()
                    .map(|m| m.slug)
                    .ok_or("Your ChatGPT plan lists no models for other apps.")?,
            };
            Ok(Backend::ChatGptPlan {
                base_url: chatgpt::API_BASE.into(),
                model,
                access_token: c.access_token,
            })
        }
        AssistantProvider::Openai => Ok(Backend::OpenAiCompatible {
            base_url: a
                .base_url
                .clone()
                .filter(|u| !u.trim().is_empty())
                .unwrap_or_else(|| OPENAI_URL.into()),
            model: model
                .ok_or("Choose a model in Settings ▸ Assistant.")?
                .to_string(),
            api_key: Some(
                read_key(AssistantProvider::Openai)
                    .ok_or("Add an API key in Settings ▸ Assistant.")?,
            ),
        }),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantInfo {
    /// Documents' details and text go to a cloud service.
    cloud: bool,
    /// "LM Studio", "Claude (claude-opus-5-5)", …
    label: String,
}

/// What the assistant is set up to use (without contacting anything).
#[tauri::command]
pub fn assistant_info(state: State<AppState>) -> AssistantInfo {
    let settings = state.settings.lock().unwrap().get().clone();
    let a = &settings.assistant;
    let model = a.model.clone().filter(|m| !m.trim().is_empty());
    let label = match a.provider {
        AssistantProvider::Local => match &settings.lm_studio_model {
            Some(m) => format!("LM Studio · {m}"),
            None => "LM Studio".into(),
        },
        AssistantProvider::Anthropic => format!(
            "Claude · {}",
            model.unwrap_or_else(|| assistant::ANTHROPIC_DEFAULT_MODEL.into())
        ),
        AssistantProvider::Chatgpt => match model {
            Some(m) => format!("ChatGPT plan · {m}"),
            None => "ChatGPT plan".into(),
        },
        AssistantProvider::Openai => match model {
            Some(m) => format!("Cloud · {m}"),
            None => "Cloud (no model chosen)".into(),
        },
    };
    AssistantInfo {
        cloud: is_cloud(&settings),
        label,
    }
}

// MARK: - Conversation

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantReply {
    text: String,
    steps: Vec<Step>,
    /// Things to show in the main window.
    actions: Vec<UiAction>,
    model: String,
    cloud: bool,
    /// The user stopped it; nothing was answered or shown.
    stopped: bool,
}

#[tauri::command]
pub async fn assistant_send(text: String, app: AppHandle) -> CmdResult<AssistantReply> {
    // Set up before anything else, so an early Stop isn't lost.
    let cancel = Cancel::new();
    *app.state::<AssistantState>().cancel.lock().unwrap() = cancel.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let settings = state.settings.lock().unwrap().get().clone();
        let backend = backend(&settings)?;
        let sources = state.sources();
        // Its own connection, so searches in the window aren't blocked
        // while the model thinks.
        let index = Index::open(&state.index_path).map_err(err)?;
        let tools = LibraryTools::new(&index, &sources);
        let assistant = app.state::<AssistantState>();
        let mut chat = assistant.chat.lock().unwrap();
        let result = chat.send(&backend, &tools, &text, &cancel, &mut |step| {
            let _ = app.emit(ASSISTANT_STEP, step);
        });
        let cloud = is_cloud(&settings);
        match result {
            Ok(reply) => Ok(AssistantReply {
                text: reply.text,
                steps: reply.steps,
                actions: tools.take_actions(),
                model: reply.model,
                cloud,
                stopped: false,
            }),
            Err(Error::Cancelled) => Ok(AssistantReply {
                text: String::new(),
                steps: Vec::new(),
                actions: Vec::new(),
                model: backend.model().to_string(),
                cloud,
                stopped: true,
            }),
            Err(e) => Err(err(e)),
        }
    })
    .await
    .map_err(err)?
}

/// Stops the answer in progress (it returns `stopped` at once).
#[tauri::command]
pub fn assistant_cancel(assistant: State<AssistantState>) {
    assistant.cancel.lock().unwrap().cancel();
}

/// Starts a new conversation, stopping any answer in progress.
#[tauri::command]
pub fn assistant_reset(assistant: State<AssistantState>) {
    assistant.cancel.lock().unwrap().cancel();
    *assistant.chat.lock().unwrap() = new_chat();
}

#[cfg(test)]
mod tests {
    #[test]
    fn today_is_a_date() {
        let t = super::today();
        assert_eq!(t.len(), 10);
        assert!(t.starts_with("20"));
    }
}
