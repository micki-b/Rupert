//! App settings that aren't about a single source: the inbox, OCR and
//! suggestion preferences, and how filed documents are named. Saved as
//! JSON in the app's config folder.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::llm;

/// The inbox: a folder inside one of the sources where new scans arrive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxLocation {
    pub source_id: String,
    pub rel_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub inbox: Option<InboxLocation>,
    /// Add a text layer to scans in the inbox.
    pub ocr: bool,
    /// Ask Local AI for a title, date, tags and folder.
    pub suggestions: bool,
    /// Which Local AI: a server (the address below) or Apple Intelligence.
    pub local_engine: LocalEngine,
    /// Local AI: an OpenAI-compatible server on this computer or the local
    /// network (LM Studio, Ollama, …). Saved as `lmStudioUrl` up to 0.3.
    #[serde(alias = "lmStudioUrl")]
    pub local_url: String,
    /// `None` = whichever chat model the server lists first.
    #[serde(alias = "lmStudioModel")]
    pub local_model: Option<String>,
    /// How filed documents are named: `{date}` and `{title}` are replaced.
    pub name_pattern: String,
    /// The chat assistant. (API keys are kept in the system's keychain,
    /// never here.)
    pub assistant: AssistantSettings,
    /// Desktop: look for a new version on GitHub now and then.
    pub check_updates: bool,
    /// Rupert's language ("en", "es"); `None` follows the system.
    pub language: Option<String>,
    /// Sending a PDF (Share…, Email…): include the notes Rupert keeps
    /// inside it. Off: a copy without them is sent.
    pub send_notes: bool,
}

/// Where Local AI runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LocalEngine {
    /// An OpenAI-compatible server (LM Studio, Ollama, …) at `local_url`.
    #[default]
    Server,
    /// Apple Intelligence, Apple's on-device model (macOS 26 or later).
    Apple,
}

/// Which model the assistant talks to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AssistantProvider {
    /// Local AI (the server and model above).
    #[default]
    Local,
    /// Claude, through Anthropic's API.
    Anthropic,
    /// Any OpenAI-compatible service (OpenAI, OpenRouter, …).
    Openai,
    /// OpenAI's models paid by the person's ChatGPT plan ("Sign in with
    /// ChatGPT"; credentials in the keychain, see `chatgpt.rs`).
    Chatgpt,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AssistantSettings {
    pub provider: AssistantProvider,
    /// Model for a cloud provider (`None`: Anthropic's default; required
    /// for OpenAI-compatible services). Local AI uses `local_model`.
    pub model: Option<String>,
    /// OpenAI-compatible service address (`None`: OpenAI's).
    pub base_url: Option<String>,
    /// Anthropic: use the `ant` command-line login instead of an API key.
    pub anthropic_cli_login: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            inbox: None,
            ocr: true,
            suggestions: true,
            local_engine: LocalEngine::Server,
            local_url: llm::DEFAULT_URL.into(),
            local_model: None,
            name_pattern: "{date} {title}".into(),
            assistant: AssistantSettings::default(),
            check_updates: true,
            language: None,
            send_notes: false,
        }
    }
}

impl Settings {
    /// Settings for a first start: on a Mac that can run Apple Intelligence,
    /// Local AI is Apple Intelligence, so inbox suggestions and Ask Rupert
    /// work without installing anything. (Settings saved before, without
    /// `localEngine`, stay on the server they were set up with.)
    pub fn first_start() -> Self {
        let mut settings = Self::default();
        if crate::apple_ai::status().capable() {
            settings.local_engine = LocalEngine::Apple;
        }
        settings
    }

    /// The file name (without extension) for a document, from the pattern.
    pub fn file_name(&self, date: Option<&str>, title: &str) -> String {
        self.name_pattern
            .replace("{date}", date.unwrap_or(""))
            .replace("{title}", title)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .trim_matches(|c: char| c == '-' || c == '_' || c.is_whitespace())
            .to_string()
    }
}

/// The saved settings. Every change is written to disk immediately.
#[derive(Debug)]
pub struct SettingsStore {
    path: PathBuf,
    settings: Settings,
}

impl SettingsStore {
    pub fn load(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let settings = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Settings::first_start(),
            Err(e) => return Err(e.into()),
        };
        Ok(Self { path, settings })
    }

    pub fn get(&self) -> &Settings {
        &self.settings
    }

    pub fn set(&mut self, settings: Settings) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(&settings)?)?;
        fs::rename(&tmp, &self.path)?;
        self.settings = settings;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_from_pattern() {
        let s = Settings::default();
        assert_eq!(
            s.file_name(Some("2026-04-29"), "Acme auto policy"),
            "2026-04-29 Acme auto policy"
        );
        assert_eq!(s.file_name(None, "Acme auto policy"), "Acme auto policy");
        let dashed = Settings {
            name_pattern: "{title} - {date}".into(),
            ..Default::default()
        };
        assert_eq!(dashed.file_name(None, "Lease"), "Lease");
    }

    #[test]
    fn round_trip_and_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut store = SettingsStore::load(&path).unwrap();
        assert!(store.get().ocr && store.get().suggestions);
        // A first start picks Apple Intelligence where the Mac can run it.
        let apple = crate::apple_ai::status().capable();
        assert_eq!(store.get().local_engine == LocalEngine::Apple, apple);
        let mut s = store.get().clone();
        s.inbox = Some(InboxLocation {
            source_id: "a".into(),
            rel_path: "Scans".into(),
        });
        store.set(s.clone()).unwrap();
        assert_eq!(SettingsStore::load(&path).unwrap().get(), &s);
        // Older or partial files get defaults for missing fields.
        fs::write(&path, r#"{"ocr": false}"#).unwrap();
        let loaded = SettingsStore::load(&path).unwrap();
        assert!(!loaded.get().ocr);
        assert_eq!(loaded.get().name_pattern, "{date} {title}");
        // Files from before Local AI keep their server and model.
        fs::write(
            &path,
            r#"{"lmStudioUrl": "http://localhost:11434", "lmStudioModel": "qwen3:4b"}"#,
        )
        .unwrap();
        let loaded = SettingsStore::load(&path).unwrap();
        assert_eq!(loaded.get().local_engine, LocalEngine::Server);
        assert_eq!(loaded.get().local_url, "http://localhost:11434");
        assert_eq!(loaded.get().local_model.as_deref(), Some("qwen3:4b"));
    }
}
