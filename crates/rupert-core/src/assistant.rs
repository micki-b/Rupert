//! The assistant: a chat that searches the library and steers the app,
//! using a language model with tools.
//!
//! Backends:
//! - **OpenAI-compatible chat completions**: LM Studio on this computer
//!   (the default; nothing leaves the machine) or a cloud endpoint with an
//!   API key.
//! - **Anthropic's Messages API** over HTTP (there is no official Rust SDK),
//!   with an API key or an OAuth token from the `ant` CLI login.
//!
//! The model only gets read-only tools (search, tags, folders, a
//! document's indexed text) plus "show this in the app" requests, which
//! the UI carries out. Nothing here changes files. With a cloud backend,
//! what the tools return (titles, tags, snippets, document text the model
//! asks for) is sent to that provider; the UI says so.

use std::cell::RefCell;
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};

use crate::error::{Error, Result};
use crate::index::{Index, SearchQuery, TagMode};
use crate::listing::list_dir;
use crate::source::Source;

/// Tool rounds per message before giving up (a model stuck in a loop).
const MAX_ROUNDS: usize = 8;
const ANTHROPIC_URL: &str = "https://api.anthropic.com";
const ANTHROPIC_VERSION: &str = "2026-10-21";
pub const ANTHROPIC_DEFAULT_MODEL: &str = "claude-opus-5-5";
/// Models that take server-side refusal fallbacks (`fallbacks: "default"`).
const FALLBACK_MODELS: &[&str] = &[
    "claude-fable-5-2",
    "claude-fable-5-1",
    "claude-opus-5-5",
    "claude-opus-5",
    "claude-sonnet-5-5",
];
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
const OAUTH_BETA: &str = "oauth-2025-04-20";

fn ai_err(e: impl std::fmt::Display) -> Error {
    Error::Unsupported(format!("assistant: {e}"))
}

// MARK: - Backends

#[derive(Debug, Clone)]
pub enum Backend {
    /// `/chat/completions` below `base_url` (e.g. `http://localhost:1234/v1`).
    OpenAiCompatible {
        base_url: String,
        model: String,
        api_key: Option<String>,
    },
    /// `/v1/messages` below `base_url` (Anthropic's API unless testing).
    Anthropic {
        base_url: String,
        model: String,
        auth: AnthropicAuth,
    },
}

#[derive(Debug, Clone)]
pub enum AnthropicAuth {
    ApiKey(String),
    /// From `ant auth print-credentials --access-token`.
    OAuthToken(String),
}

impl Backend {
    pub fn anthropic(model: Option<&str>, auth: AnthropicAuth) -> Self {
        Self::Anthropic {
            base_url: ANTHROPIC_URL.into(),
            model: model
                .map(str::trim)
                .filter(|m| !m.is_empty())
                .unwrap_or(ANTHROPIC_DEFAULT_MODEL)
                .into(),
            auth,
        }
    }

    pub fn model(&self) -> &str {
        match self {
            Self::OpenAiCompatible { model, .. } | Self::Anthropic { model, .. } => model,
        }
    }

    fn format(&self) -> Format {
        match self {
            Self::OpenAiCompatible { .. } => Format::OpenAi,
            Self::Anthropic { .. } => Format::Anthropic,
        }
    }
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .http_status_as_error(false)
        .build()
        .into()
}

/// The JSON body of a response, or its error message.
fn read_json(mut resp: ureq::http::Response<ureq::Body>) -> Result<Value> {
    let status = resp.status().as_u16();
    let text = resp
        .body_mut()
        .with_config()
        .limit(32 * 1024 * 1024)
        .read_to_string()
        .map_err(ai_err)?;
    let body: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if (200..300).contains(&status) {
        return Ok(body);
    }
    let message = body
        .pointer("/error/message")
        .or_else(|| body.pointer("/error"))
        .and_then(Value::as_str)
        .map(String::from)
        .unwrap_or_else(|| text.chars().take(300).collect());
    Err(ai_err(match status {
        401 | 403 => format!("not signed in or no access ({message})"),
        429 => format!("rate limited, try again shortly ({message})"),
        _ => format!("HTTP {status}: {message}"),
    }))
}

// MARK: - Tools

/// A tool the model may call.
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub schema: Value,
}

/// What the assistant can do; implemented over the library below.
pub trait ToolBox {
    fn specs(&self) -> Vec<ToolSpec>;
    /// The result as text for the model (errors too: the model can react).
    fn call(&self, name: &str, input: &Value) -> String;
}

/// One tool call, for showing progress in the chat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub tool: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reply {
    pub text: String,
    pub steps: Vec<Step>,
    /// The model that answered (after any fallback).
    pub model: String,
}

// MARK: - Chat

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    OpenAi,
    Anthropic,
}

/// A conversation. Its history is kept in the backend's own format, so
/// switching backends starts over.
pub struct Chat {
    system: String,
    format: Option<Format>,
    history: Vec<Value>,
}

impl Chat {
    pub fn new(system: impl Into<String>) -> Self {
        Self {
            system: system.into(),
            format: None,
            history: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }

    /// Sends a message and runs tool calls until the model answers.
    /// `on_step` is told about each tool call as it happens. On an error
    /// the conversation is left as it was before the message.
    pub fn send(
        &mut self,
        backend: &Backend,
        tools: &dyn ToolBox,
        text: &str,
        on_step: &mut dyn FnMut(&Step),
    ) -> Result<Reply> {
        if self.format != Some(backend.format()) {
            self.history.clear();
            self.format = Some(backend.format());
        }
        let mark = self.history.len();
        let result = match backend {
            Backend::OpenAiCompatible {
                base_url,
                model,
                api_key,
            } => self.send_openai(base_url, model, api_key.as_deref(), tools, text, on_step),
            Backend::Anthropic {
                base_url,
                model,
                auth,
            } => self.send_anthropic(base_url, model, auth, tools, text, on_step),
        };
        if result.is_err() {
            self.history.truncate(mark);
        }
        result
    }

    fn run_tools(
        tools: &dyn ToolBox,
        calls: &[(String, String, Value)],
        steps: &mut Vec<Step>,
        on_step: &mut dyn FnMut(&Step),
    ) -> Vec<(String, String)> {
        calls
            .iter()
            .map(|(id, name, input)| {
                let step = Step {
                    tool: name.clone(),
                    summary: describe_call(name, input),
                };
                on_step(&step);
                steps.push(step);
                (id.clone(), tools.call(name, input))
            })
            .collect()
    }

    // MARK: Anthropic

    fn send_anthropic(
        &mut self,
        base_url: &str,
        model: &str,
        auth: &AnthropicAuth,
        tools: &dyn ToolBox,
        text: &str,
        on_step: &mut dyn FnMut(&Step),
    ) -> Result<Reply> {
        let http = agent(Duration::from_secs(300));
        let tool_defs: Vec<Value> = tools
            .specs()
            .into_iter()
            .map(
                |t| json!({"name": t.name, "description": t.description, "input_schema": t.schema}),
            )
            .collect();
        let fallback = FALLBACK_MODELS.contains(&model);
        let mut betas: Vec<&str> = Vec::new();
        if fallback {
            betas.push(FALLBACK_BETA);
        }
        if matches!(auth, AnthropicAuth::OAuthToken(_)) {
            betas.push(OAUTH_BETA);
        }

        self.history.push(json!({"role": "user", "content": text}));
        let mut steps = Vec::new();
        let mut served_by = model.to_string();
        for _ in 0..MAX_ROUNDS {
            let mut body = json!({
                "model": model,
                "max_tokens": 16000,
                "system": self.system,
                "tools": tool_defs,
                "messages": self.history,
            });
            if fallback {
                body["fallbacks"] = json!("default");
            }
            let mut req = http
                .post(&format!("{}/v1/messages", base_url.trim_end_matches('/')))
                .header("anthropic-version", ANTHROPIC_VERSION)
                .header("content-type", "application/json");
            req = match auth {
                AnthropicAuth::ApiKey(k) => req.header("x-api-key", k),
                AnthropicAuth::OAuthToken(t) => req.header("authorization", &format!("Bearer {t}")),
            };
            if !betas.is_empty() {
                req = req.header("anthropic-beta", &betas.join(","));
            }
            let resp = read_json(req.send_json(&body).map_err(ai_err)?)?;
            if let Some(m) = resp.get("model").and_then(Value::as_str) {
                served_by = m.to_string();
            }
            let stop = resp
                .get("stop_reason")
                .and_then(Value::as_str)
                .unwrap_or("");
            let content = echo_content(
                resp.get("content")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default(),
            );
            self.history
                .push(json!({"role": "assistant", "content": content}));

            let said = text_of(&content);
            if stop == "refusal" {
                let reply = if said.is_empty() {
                    "The model declined to answer that.".to_string()
                } else {
                    said
                };
                return Ok(Reply {
                    text: reply,
                    steps,
                    model: served_by,
                });
            }
            let calls: Vec<(String, String, Value)> = content
                .iter()
                .filter(|b| b.get("type").and_then(Value::as_str) == Some("tool_use"))
                .map(|b| {
                    (
                        b.get("id")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        b.get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        b.get("input").cloned().unwrap_or(json!({})),
                    )
                })
                .collect();
            if stop == "tool_use" && !calls.is_empty() {
                // All results in one user message (keeps parallel calls working).
                let results: Vec<Value> = Self::run_tools(tools, &calls, &mut steps, on_step)
                    .into_iter()
                    .map(|(id, out)| json!({"type": "tool_result", "tool_use_id": id, "content": out}))
                    .collect();
                self.history
                    .push(json!({"role": "user", "content": results}));
                continue;
            }
            if stop == "pause_turn" {
                continue;
            }
            let mut reply = said;
            if stop == "max_tokens" {
                reply.push_str("\n\n(The answer was cut off.)");
            }
            return Ok(Reply {
                text: reply,
                steps,
                model: served_by,
            });
        }
        Ok(Reply {
            text: "I couldn't finish that in a reasonable number of steps.".into(),
            steps,
            model: served_by,
        })
    }

    // MARK: OpenAI-compatible

    fn send_openai(
        &mut self,
        base_url: &str,
        model: &str,
        api_key: Option<&str>,
        tools: &dyn ToolBox,
        text: &str,
        on_step: &mut dyn FnMut(&Step),
    ) -> Result<Reply> {
        let http = agent(Duration::from_secs(300));
        let tool_defs: Vec<Value> = tools
            .specs()
            .into_iter()
            .map(|t| {
                json!({"type": "function", "function": {
                    "name": t.name, "description": t.description, "parameters": t.schema
                }})
            })
            .collect();
        self.history.push(json!({"role": "user", "content": text}));
        let mut steps = Vec::new();
        for _ in 0..MAX_ROUNDS {
            let mut messages = vec![json!({"role": "system", "content": self.system})];
            messages.extend(self.history.iter().cloned());
            let body = json!({"model": model, "messages": messages, "tools": tool_defs});
            let mut req = http
                .post(&format!(
                    "{}/chat/completions",
                    base_url.trim_end_matches('/')
                ))
                .header("content-type", "application/json");
            if let Some(k) = api_key {
                req = req.header("authorization", &format!("Bearer {k}"));
            }
            let resp = read_json(req.send_json(&body).map_err(ai_err)?)?;
            let msg = resp
                .pointer("/choices/0/message")
                .cloned()
                .ok_or_else(|| ai_err("the model sent no answer"))?;
            let calls: Vec<(String, String, Value)> = msg
                .get("tool_calls")
                .and_then(Value::as_array)
                .map(|calls| {
                    calls
                        .iter()
                        .map(|c| {
                            let args = c.pointer("/function/arguments");
                            let input = match args {
                                Some(Value::String(s)) => {
                                    serde_json::from_str(s).unwrap_or(json!({}))
                                }
                                Some(v) => v.clone(),
                                None => json!({}),
                            };
                            (
                                c.get("id")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default()
                                    .to_string(),
                                c.pointer("/function/name")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default()
                                    .to_string(),
                                input,
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            let content = msg
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let mut kept = json!({"role": "assistant", "content": content});
            if let Some(tc) = msg.get("tool_calls").filter(|_| !calls.is_empty()) {
                kept["tool_calls"] = tc.clone();
            }
            self.history.push(kept);
            if calls.is_empty() {
                let mut reply = strip_think(content);
                if reply.is_empty() {
                    reply = strip_think(
                        msg.get("reasoning_content")
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                    );
                }
                return Ok(Reply {
                    text: reply,
                    steps,
                    model: model.to_string(),
                });
            }
            for (id, out) in Self::run_tools(tools, &calls, &mut steps, on_step) {
                self.history
                    .push(json!({"role": "tool", "tool_call_id": id, "content": out}));
            }
        }
        Ok(Reply {
            text: "I couldn't finish that in a reasonable number of steps.".into(),
            steps,
            model: model.to_string(),
        })
    }
}

/// The assistant turn as it goes back into the history. After a refusal
/// fallback mid-answer, thinking and tool calls before the last `fallback`
/// block are left out (they belong to the declined attempt); the
/// `fallback` block itself stays where it was.
fn echo_content(content: Vec<Value>) -> Vec<Value> {
    let last_fallback = content
        .iter()
        .rposition(|b| b.get("type").and_then(Value::as_str) == Some("fallback"));
    let Some(boundary) = last_fallback else {
        return content;
    };
    content
        .into_iter()
        .enumerate()
        .filter(|(i, b)| {
            let kind = b.get("type").and_then(Value::as_str).unwrap_or("");
            *i >= boundary
                || !matches!(
                    kind,
                    "thinking" | "redacted_thinking" | "tool_use" | "server_tool_use"
                )
        })
        .map(|(_, b)| b)
        .collect()
}

fn text_of(content: &[Value]) -> String {
    content
        .iter()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|b| b.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("")
        .trim()
        .to_string()
}

/// Local reasoning models sometimes put their thinking inline.
fn strip_think(s: &str) -> String {
    let mut out = s.to_string();
    while let (Some(a), Some(b)) = (out.find("<think>"), out.find("</think>")) {
        if b < a {
            break;
        }
        out.replace_range(a..b + "</think>".len(), "");
    }
    out.trim().to_string()
}

/// "Searching for “impeller”", for the chat's progress lines.
fn describe_call(name: &str, input: &Value) -> String {
    let s = |k: &str| input.get(k).and_then(Value::as_str).unwrap_or_default();
    let tags = input
        .get("tags")
        .and_then(Value::as_array)
        .map(|t| {
            t.iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    match name {
        "search_documents" => match (s("query"), tags.as_str()) {
            ("", "") => "Searching documents".into(),
            (q, "") => format!("Searching for “{q}”"),
            ("", t) => format!("Looking up tags {t}"),
            (q, t) => format!("Searching for “{q}” tagged {t}"),
        },
        "list_tags" => "Looking at tags".into(),
        "list_sources" => "Looking at the library".into(),
        "list_folder" => format!("Opening {}", s("path").trim_matches('/')),
        "read_document" => format!("Reading {}", s("path")),
        "open_folder" => format!("Showing folder {}", s("path")),
        "show_document" => format!("Showing {}", s("path")),
        "show_results" => "Showing results".into(),
        other => other.to_string(),
    }
}

// MARK: - The library's tools

/// Something the app should show; the UI carries these out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum UiAction {
    OpenFolder {
        source_id: String,
        rel_path: String,
    },
    ShowDocument {
        source_id: String,
        rel_path: String,
    },
    Search {
        text: String,
        tags: Vec<String>,
        tag_mode: TagMode,
        favorites_only: bool,
    },
}

/// Read-only tools over the index and the sources, plus UI requests.
pub struct LibraryTools<'a> {
    pub index: &'a Index,
    pub sources: &'a [Source],
    actions: RefCell<Vec<UiAction>>,
}

impl<'a> LibraryTools<'a> {
    pub fn new(index: &'a Index, sources: &'a [Source]) -> Self {
        Self {
            index,
            sources,
            actions: RefCell::new(Vec::new()),
        }
    }

    /// The UI requests made during the conversation turn.
    pub fn take_actions(&self) -> Vec<UiAction> {
        std::mem::take(&mut self.actions.borrow_mut())
    }

    fn source(&self, input: &Value) -> std::result::Result<&Source, String> {
        let key = input
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or_default();
        self.sources
            .iter()
            .find(|s| s.id == key || s.name.eq_ignore_ascii_case(key))
            .or_else(|| (self.sources.len() == 1 && key.is_empty()).then(|| &self.sources[0]))
            .ok_or_else(|| {
                let names: Vec<&str> = self.sources.iter().map(|s| s.name.as_str()).collect();
                format!("Unknown source “{key}”. Sources: {}", names.join(", "))
            })
    }

    fn source_name(&self, id: &str) -> String {
        self.sources
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| id.to_string())
    }

    fn query(input: &Value) -> SearchQuery {
        SearchQuery {
            text: input
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into(),
            source_id: None,
            tags: input
                .get("tags")
                .and_then(Value::as_array)
                .map(|t| {
                    t.iter()
                        .filter_map(Value::as_str)
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default(),
            tag_mode: match input.get("tag_mode").and_then(Value::as_str) {
                Some("all") => TagMode::All,
                _ => TagMode::Any,
            },
            favorites_only: input
                .get("favorites_only")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            limit: input
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(15)
                .clamp(1, 50) as usize,
        }
    }

    fn rel(input: &Value) -> String {
        input
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim_matches('/')
            .to_string()
    }

    fn run(&self, name: &str, input: &Value) -> std::result::Result<Value, String> {
        match name {
            "search_documents" => {
                let q = Self::query(input);
                let hits = self.index.search(&q).map_err(|e| e.to_string())?;
                Ok(json!(hits.iter().map(|h| json!({
                    "source": self.source_name(&h.source_id),
                    "path": h.rel_path,
                    "title": h.title,
                    "date": h.document_date,
                    "tags": h.tags,
                    "favorite": h.favorite,
                    "snippet": h.snippet.as_deref().map(|s| s.replace(['\u{E000}', '\u{E001}'], "")),
                })).collect::<Vec<_>>()))
            }
            "list_tags" => {
                let filter = input
                    .get("filter")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let limit = input
                    .get("limit")
                    .and_then(Value::as_u64)
                    .unwrap_or(40)
                    .clamp(1, 300);
                let list = self
                    .index
                    .tag_suggestions(filter, &[], TagMode::Any, limit as usize)
                    .map_err(|e| e.to_string())?;
                Ok(json!({"tags": list.tags, "matching": list.matching}))
            }
            "list_sources" => {
                let favs = self.index.favorite_folders().map_err(|e| e.to_string())?;
                Ok(json!({
                    "sources": self.sources.iter().map(|s| json!({"name": s.name, "provider": s.provider})).collect::<Vec<_>>(),
                    "favorite_folders": favs.iter().map(|(id, p)| json!({"source": self.source_name(id), "path": p})).collect::<Vec<_>>(),
                }))
            }
            "list_folder" => {
                let source = self.source(input)?;
                let rel = Self::rel(input);
                let entries = list_dir(&source.root, &rel, source.show_all_types)
                    .map_err(|e| e.to_string())?;
                let meta = self.index.folder_meta(&source.id, &rel).unwrap_or_default();
                let mut folders = Vec::new();
                let mut docs = Vec::new();
                for e in entries.iter().take(200) {
                    if e.is_dir {
                        folders.push(json!(e.name));
                    } else {
                        let m = meta.get(&e.rel_path);
                        docs.push(json!({
                            "name": e.name,
                            "title": m.and_then(|m| m.title.clone()),
                            "date": m.and_then(|m| m.document_date.clone()),
                            "tags": m.map(|m| m.tags.clone()).unwrap_or_default(),
                        }));
                    }
                }
                Ok(
                    json!({"source": source.name, "path": rel, "folders": folders, "documents": docs}),
                )
            }
            "read_document" => {
                let source = self.source(input)?;
                let rel = Self::rel(input);
                let max = input
                    .get("max_chars")
                    .and_then(Value::as_u64)
                    .unwrap_or(6000)
                    .clamp(200, 30000) as usize;
                let info = self
                    .index
                    .document(&source.id, &rel)
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| format!("No document “{rel}” in {}", source.name))?;
                let text = self
                    .index
                    .document_text(&source.id, &rel)
                    .map_err(|e| e.to_string())?;
                let total = text.as_ref().map(|t| t.chars().count()).unwrap_or(0);
                Ok(json!({
                    "title": info.hit.title,
                    "date": info.hit.document_date,
                    "tags": info.hit.tags,
                    "notes": info.notes,
                    "text": text.map(|t| t.chars().take(max).collect::<String>()),
                    "text_chars_total": total,
                    "text_note": if total == 0 { Some("No text is known for this document (e.g. an unrecognized scan, or online-only and not in the catalog).") } else { None },
                }))
            }
            "open_folder" => {
                let source = self.source(input)?;
                let rel = Self::rel(input);
                if !rupert_dir_exists(&source.root, &rel) {
                    return Err(format!("No folder “{rel}” in {}", source.name));
                }
                self.actions.borrow_mut().push(UiAction::OpenFolder {
                    source_id: source.id.clone(),
                    rel_path: rel,
                });
                Ok(json!("Shown."))
            }
            "show_document" => {
                let source = self.source(input)?;
                let rel = Self::rel(input);
                if self
                    .index
                    .document(&source.id, &rel)
                    .map_err(|e| e.to_string())?
                    .is_none()
                {
                    return Err(format!("No document “{rel}” in {}", source.name));
                }
                self.actions.borrow_mut().push(UiAction::ShowDocument {
                    source_id: source.id.clone(),
                    rel_path: rel,
                });
                Ok(json!("Shown."))
            }
            "show_results" => {
                let q = Self::query(input);
                self.actions.borrow_mut().push(UiAction::Search {
                    text: q.text,
                    tags: q.tags,
                    tag_mode: q.tag_mode,
                    favorites_only: q.favorites_only,
                });
                Ok(json!("Shown."))
            }
            other => Err(format!("Unknown tool {other}")),
        }
    }
}

fn rupert_dir_exists(root: &std::path::Path, rel: &str) -> bool {
    crate::listing::resolve(root, rel).is_ok_and(|p| p.is_dir())
}

fn schema(props: Value, required: &[&str]) -> Value {
    json!({"type": "object", "properties": props, "required": required, "additionalProperties": false})
}

impl ToolBox for LibraryTools<'_> {
    fn specs(&self) -> Vec<ToolSpec> {
        let search_props = json!({
            "query": {"type": "string", "description": "Words to find in titles, names, tags, notes and text. Every word must match; the last may be a prefix. Empty to list by tags or favorites only."},
            "tags": {"type": "array", "items": {"type": "string"}, "description": "Only documents with these tags (ignoring case)."},
            "tag_mode": {"type": "string", "enum": ["any", "all"], "description": "With several tags: documents with any of them, or all of them. Default any."},
            "favorites_only": {"type": "boolean"},
            "limit": {"type": "integer", "description": "Most results to return (default 15, at most 50)."}
        });
        let doc_props = json!({
            "source": {"type": "string", "description": "The source's name (see list_sources)."},
            "path": {"type": "string", "description": "Path within the source, as search results give it."}
        });
        vec![
            ToolSpec {
                name: "search_documents",
                description: "Full-text search of the user's documents. Returns source, path, title, document date, tags and a text snippet for each match, best first.",
                schema: schema(search_props.clone(), &[]),
            },
            ToolSpec {
                name: "list_tags",
                description: "Tags in use with how many documents have each, most used first. Optionally only tags matching a filter text.",
                schema: schema(
                    json!({
                        "filter": {"type": "string"},
                        "limit": {"type": "integer"}
                    }),
                    &[],
                ),
            },
            ToolSpec {
                name: "list_sources",
                description: "The library's sources (top-level folders) and the user's favorite folders.",
                schema: schema(json!({}), &[]),
            },
            ToolSpec {
                name: "list_folder",
                description: "Subfolders and documents in a folder (path empty for the source's top level), with titles, dates and tags.",
                schema: schema(
                    json!({
                        "source": {"type": "string"},
                        "path": {"type": "string"}
                    }),
                    &["source"],
                ),
            },
            ToolSpec {
                name: "read_document",
                description: "A document's details and its recognized text (from the index; nothing is downloaded). Use to answer questions about a document's contents.",
                schema: schema(
                    json!({
                        "source": {"type": "string"},
                        "path": {"type": "string"},
                        "max_chars": {"type": "integer", "description": "How much text to return (default 6000)."}
                    }),
                    &["source", "path"],
                ),
            },
            ToolSpec {
                name: "open_folder",
                description: "Show a folder in the app's main window.",
                schema: schema(
                    json!({
                        "source": {"type": "string"},
                        "path": {"type": "string"}
                    }),
                    &["source"],
                ),
            },
            ToolSpec {
                name: "show_document",
                description: "Select a document in the app's main window (its folder opens and its details show).",
                schema: schema(doc_props, &["source", "path"]),
            },
            ToolSpec {
                name: "show_results",
                description: "Show a search in the app's main window, with the same options as search_documents, so the user can browse the results.",
                schema: schema(search_props, &[]),
            },
        ]
    }

    fn call(&self, name: &str, input: &Value) -> String {
        match self.run(name, input) {
            Ok(v) => v.to_string(),
            Err(e) => format!("Error: {e}"),
        }
    }
}

/// The assistant's instructions.
pub fn system_prompt(today: &str) -> String {
    format!(
        "You are Rupert, the librarian in a document manager for the user's scanned paperwork \
         (PDFs and other documents in folders on their computer, synced with iCloud Drive or OneDrive). \
         Today is {today}.\n\n\
         Use the tools to find documents and answer questions about them; never guess at documents, \
         dates or amounts you haven't seen in a tool result. Search with a few distinctive words \
         first, then refine with tags or other words. Read a document's text before answering a \
         question about its contents.\n\n\
         When the user asks to open, show or go to something, do it with open_folder, show_document \
         or show_results. You can't change, move, rename, tag or delete files; say so if asked.\n\n\
         Answer briefly in plain text. Name documents by title (or file name) and folder."
    )
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::DocRow;
    use crate::listing::CloudState;
    use crate::meta::{DocMeta, Storage};
    use crate::source::Provider;
    use crate::{FileKind, index::TextState};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    fn library() -> (tempfile::TempDir, Index, Vec<Source>) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("Boat/Engine")).unwrap();
        let idx = Index::open_in_memory().unwrap();
        idx.upsert(
            &DocRow {
                source_id: "s1".into(),
                rel_path: "Boat/Engine/impeller.pdf".into(),
                name: "impeller.pdf".into(),
                kind: FileKind::Pdf,
                size: 10,
                modified_ms: Some(1),
                cloud: CloudState::Local,
                meta: DocMeta {
                    title: Some("Impeller invoice".into()),
                    tags: vec!["Boat".into()],
                    document_date: Some("2025-03-14".into()),
                    ..Default::default()
                },
                storage: Storage::Embedded,
                text_state: TextState::Extracted,
                sidecar_ms: None,
            },
            Some("Harbor Marine Supply. Raw water impeller. Total due $412.50"),
        )
        .unwrap();
        let sources = vec![Source {
            id: "s1".into(),
            name: "Documents".into(),
            root: dir.path().to_path_buf(),
            provider: Provider::Local,
            show_all_types: false,
            download_online_only: false,
        }];
        (dir, idx, sources)
    }

    #[test]
    fn tools_search_read_and_request_ui() {
        let (_dir, idx, sources) = library();
        let tools = LibraryTools::new(&idx, &sources);
        let found: Value =
            serde_json::from_str(&tools.call("search_documents", &json!({"query": "impeller"})))
                .unwrap();
        assert_eq!(found[0]["title"], "Impeller invoice");
        assert_eq!(found[0]["source"], "Documents");

        let doc: Value = serde_json::from_str(&tools.call(
            "read_document",
            &json!({"source": "documents", "path": "Boat/Engine/impeller.pdf"}),
        ))
        .unwrap();
        assert!(doc["text"].as_str().unwrap().contains("$412.50"));

        assert!(
            tools
                .call("read_document", &json!({"source": "Nope", "path": "x"}))
                .starts_with("Error: Unknown source")
        );
        assert!(
            tools
                .call(
                    "open_folder",
                    &json!({"source": "Documents", "path": "Missing"})
                )
                .starts_with("Error")
        );
        tools.call(
            "open_folder",
            &json!({"source": "Documents", "path": "Boat/Engine"}),
        );
        tools.call(
            "show_results",
            &json!({"tags": ["Boat"], "tag_mode": "all"}),
        );
        assert_eq!(
            tools.take_actions(),
            [
                UiAction::OpenFolder {
                    source_id: "s1".into(),
                    rel_path: "Boat/Engine".into()
                },
                UiAction::Search {
                    text: "".into(),
                    tags: vec!["Boat".into()],
                    tag_mode: TagMode::All,
                    favorites_only: false
                },
            ]
        );
        assert!(tools.take_actions().is_empty());
        let listed: Value = serde_json::from_str(&tools.call(
            "list_folder",
            &json!({"source": "Documents", "path": "Boat"}),
        ))
        .unwrap();
        assert_eq!(listed["folders"][0], "Engine");
    }

    #[test]
    fn ui_actions_serialize_for_the_window() {
        let a = UiAction::ShowDocument {
            source_id: "s1".into(),
            rel_path: "Boat/a.pdf".into(),
        };
        assert_eq!(
            serde_json::to_value(&a).unwrap(),
            json!({"kind": "showDocument", "sourceId": "s1", "relPath": "Boat/a.pdf"})
        );
        let s = UiAction::Search {
            text: "x".into(),
            tags: vec![],
            tag_mode: TagMode::All,
            favorites_only: true,
        };
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["tagMode"], "all");
        assert_eq!(v["favoritesOnly"], true);
    }

    #[test]
    fn echo_drops_declined_reasoning_before_a_fallback() {
        let content = vec![
            json!({"type": "thinking", "thinking": "", "signature": "a"}),
            json!({"type": "text", "text": "Partial "}),
            json!({"type": "tool_use", "id": "t0", "name": "x", "input": {}}),
            json!({"type": "fallback", "from": {"model": "m1"}, "to": {"model": "m2"}}),
            json!({"type": "thinking", "thinking": "", "signature": "b"}),
            json!({"type": "tool_use", "id": "t1", "name": "x", "input": {}}),
        ];
        let kept: Vec<String> = echo_content(content)
            .iter()
            .map(|b| b["type"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(kept, ["text", "fallback", "thinking", "tool_use"]);
        let plain = vec![
            json!({"type": "thinking"}),
            json!({"type": "text", "text": "hi"}),
        ];
        assert_eq!(echo_content(plain.clone()), plain);
        assert_eq!(strip_think("<think>hmm</think> Answer"), "Answer");
    }

    /// A tiny HTTP server that answers each POST with the next canned body
    /// and records the request bodies and headers it got.
    /// Requests the mock received: header text and JSON body.
    type Seen = Arc<Mutex<Vec<(String, Value)>>>;

    fn mock(answers: Vec<Value>) -> (String, Seen) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for (answer, stream) in answers.into_iter().zip(listener.incoming()) {
                let mut stream = stream.unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut head = String::new();
                let mut len = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line.to_ascii_lowercase().starts_with("content-length:") {
                        len = line[15..].trim().parse().unwrap();
                    }
                    if line == "\r\n" {
                        break;
                    }
                    head.push_str(&line);
                }
                let mut body = vec![0; len];
                reader.read_exact(&mut body).unwrap();
                log.lock()
                    .unwrap()
                    .push((head, serde_json::from_slice(&body).unwrap()));
                let out = answer.to_string();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len()).unwrap();
            }
        });
        (url, seen)
    }

    #[test]
    fn anthropic_tool_loop() {
        let (_dir, idx, sources) = library();
        let tools = LibraryTools::new(&idx, &sources);
        let (url, seen) = mock(vec![
            json!({"model": "claude-opus-5-5", "stop_reason": "tool_use", "content": [
                {"type": "thinking", "thinking": "", "signature": "sig"},
                {"type": "tool_use", "id": "toolu_1", "name": "search_documents", "input": {"query": "impeller"}}
            ]}),
            json!({"model": "claude-opus-5-5", "stop_reason": "end_turn", "content": [
                {"type": "text", "text": "The impeller invoice is in Boat/Engine."}
            ]}),
        ]);
        let backend = Backend::Anthropic {
            base_url: url,
            model: "claude-opus-5-5".into(),
            auth: AnthropicAuth::OAuthToken("tok".into()),
        };
        let mut chat = Chat::new("sys");
        let mut progress = Vec::new();
        let reply = chat
            .send(
                &backend,
                &tools,
                "where is the impeller invoice?",
                &mut |s| progress.push(s.summary.clone()),
            )
            .unwrap();
        assert_eq!(reply.text, "The impeller invoice is in Boat/Engine.");
        assert_eq!(progress, ["Searching for “impeller”"]);

        let seen = seen.lock().unwrap();
        let (head, first) = &seen[0];
        let head = head.to_ascii_lowercase();
        assert!(head.contains("authorization: bearer tok"));
        assert!(head.contains("anthropic-version: 2026-10-21"));
        assert!(
            head.contains("server-side-fallback-2026-07-01") && head.contains("oauth-2025-04-20")
        );
        assert_eq!(first["fallbacks"], "default");
        assert_eq!(first["tools"].as_array().unwrap().len(), 8);
        // Second request: the assistant turn echoed unchanged (thinking
        // included) and all tool results in one user message.
        let second = &seen[1].1["messages"];
        assert_eq!(second[1]["content"][0]["type"], "thinking");
        assert_eq!(second[2]["content"][0]["type"], "tool_result");
        assert!(
            second[2]["content"][0]["content"]
                .as_str()
                .unwrap()
                .contains("Impeller invoice")
        );
    }

    #[test]
    fn openai_compatible_tool_loop_and_rollback() {
        let (_dir, idx, sources) = library();
        let tools = LibraryTools::new(&idx, &sources);
        let (url, seen) = mock(vec![
            json!({"choices": [{"message": {"role": "assistant", "content": "", "tool_calls": [
                {"id": "c1", "type": "function", "function": {"name": "show_results", "arguments": "{\"tags\":[\"Boat\"]}"}}
            ]}}]}),
            json!({"choices": [{"message": {"role": "assistant", "content": "<think>ok</think>Here are your boat documents."}}]}),
        ]);
        let backend = Backend::OpenAiCompatible {
            base_url: format!("{url}/v1"),
            model: "qwen".into(),
            api_key: None,
        };
        let mut chat = Chat::new("sys");
        let reply = chat
            .send(&backend, &tools, "show boat stuff", &mut |_| {})
            .unwrap();
        assert_eq!(reply.text, "Here are your boat documents.");
        assert_eq!(tools.take_actions().len(), 1);
        let seen = seen.lock().unwrap();
        assert!(seen[0].0.starts_with("POST /v1/chat/completions"));
        let msgs = &seen[1].1["messages"];
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[3]["role"], "tool");
        drop(seen);

        // A failing request leaves the conversation as it was.
        let before = chat.history.len();
        let dead = Backend::OpenAiCompatible {
            base_url: "http://127.0.0.1:9/v1".into(),
            model: "qwen".into(),
            api_key: None,
        };
        assert!(chat.send(&dead, &tools, "again", &mut |_| {}).is_err());
        assert_eq!(chat.history.len(), before);
    }
}
