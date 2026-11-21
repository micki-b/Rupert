//! Ask Rupert on the iPhone, with Apple Intelligence (iOS 26 or later, on
//! an iPhone that can run it, with Apple Intelligence turned on). The same
//! engine as on the computer (`rupert_core::assistant`), on Apple's
//! on-device model only: nothing leaves the phone, and there are no
//! servers or cloud models to set up here.
//!
//! Read-only, like the rest of the phone app: the tools search and read the
//! index and ask the screen to show a folder, a document or search results.

use std::sync::Mutex;

use rupert_core::Index;
use rupert_core::apple_ai;
use rupert_core::assistant::{self, Backend, Cancel, Chat, LibraryTools, Step, UiAction};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{AppState, CmdResult, err};

const ASSISTANT_STEP: &str = "assistant-step";

// MARK: - State

pub struct AssistantState {
    chat: Mutex<Chat>,
    /// Stops the answer in progress, if any.
    cancel: Mutex<Cancel>,
}

impl Default for AssistantState {
    fn default() -> Self {
        Self {
            chat: Mutex::new(new_chat()),
            cancel: Mutex::new(Cancel::new()),
        }
    }
}

fn new_chat() -> Chat {
    Chat::new(assistant::system_prompt(&rupert_core::pdf_meta::utc_today()))
}

// MARK: - Commands

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantStatus {
    /// Ask Rupert can answer now.
    available: bool,
    /// This iPhone can run Apple Intelligence (perhaps once it's turned on
    /// or has downloaded its model): offer Ask Rupert, with `message`.
    capable: bool,
    /// Why it can't answer now.
    message: Option<String>,
}

/// Whether Ask Rupert can be offered on this iPhone.
#[tauri::command]
pub fn assistant_status() -> AssistantStatus {
    let s = apple_ai::status();
    AssistantStatus {
        available: s.available,
        capable: s.capable(),
        message: s.reason.map(|r| r.message().to_string()),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantReply {
    text: String,
    steps: Vec<Step>,
    /// Things to show on the screen.
    actions: Vec<UiAction>,
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
        let sources = state.sources();
        // Its own connection, so browsing isn't blocked while it thinks.
        let index = Index::open(&state.index_path).map_err(err)?;
        let tools = LibraryTools::new(&index, &sources);
        let assistant = app.state::<AssistantState>();
        let mut chat = assistant.chat.lock().unwrap();
        let result = chat.send(&Backend::Apple, &tools, &text, &cancel, &mut |step| {
            let _ = app.emit(ASSISTANT_STEP, step);
        });
        match result {
            Ok(reply) => {
                // Buttons under the answer: what Rupert asked to show, or
                // else what he read or found (Apple's small model often
                // answers without asking to show anything).
                let mut actions = tools.take_actions();
                if actions.is_empty() {
                    actions = tools.take_suggestions();
                }
                Ok(AssistantReply {
                    text: reply.text,
                    steps: reply.steps,
                    actions,
                    stopped: false,
                })
            }
            Err(rupert_core::Error::Cancelled) => Ok(AssistantReply {
                text: String::new(),
                steps: Vec::new(),
                actions: Vec::new(),
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
