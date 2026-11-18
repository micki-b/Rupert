//! The chat assistant's commands: which model to use, keys in the system
//! keychain, the `ant` command-line login for Claude, and the conversation
//! itself (logic in `rupert_core::assistant`).

use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use rupert_core::assistant::{
    self, AnthropicAuth, Backend, Cancel, Chat, LibraryTools, Step, UiAction,
};
use rupert_core::llm::LmStudio;
use rupert_core::settings::{AssistantProvider, Settings};
use rupert_core::{Error, Index};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

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
        .chain(home.map(|h| h.join(".local/bin")))
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
