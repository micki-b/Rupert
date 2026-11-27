//! The assistant: a chat that searches the library and steers the app,
//! using a language model with tools.
//!
//! Backends:
//! - **OpenAI-compatible chat completions**: Local AI (LM Studio,
//!   Ollama, … on this computer or the local network; the default) or a
//!   cloud endpoint with an API key.
//! - **Anthropic's Messages API** over HTTP (there is no official Rust SDK),
//!   with an API key or an OAuth token from the `ant` CLI login.
//!
//! The model only gets read-only tools (search, tags, folders, a
//! document's indexed text) plus "show this in the app" requests, which
//! the UI carries out. Nothing here changes files. With a cloud backend,
//! what the tools return (titles, tags, snippets, document text the model
//! asks for) is sent to that provider; the UI says so.

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};

use crate::apple_ai;
use crate::error::{Error, Result};
use crate::i18n::{Lang, tr, trf};
use crate::index::{Index, SearchQuery, TagMode};
use crate::listing::list_dir;
use crate::source::Source;
use crate::text::PAGE_BREAK;

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
    Error::Unsupported(trf("assistant: {0}", &[&e]))
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
    /// OpenAI's Responses API (`/responses` below `base_url`), paid by the
    /// person's ChatGPT plan through a "Sign in with ChatGPT" access token
    /// (see `chatgpt.rs`). Streaming only, nothing stored at OpenAI.
    ChatGptPlan {
        base_url: String,
        model: String,
        access_token: String,
    },
    /// Apple Intelligence, on this Mac (see `apple_ai`): the model runs
    /// the tools itself, through the Swift bridge.
    Apple,
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
            Self::OpenAiCompatible { model, .. }
            | Self::Anthropic { model, .. }
            | Self::ChatGptPlan { model, .. } => model,
            Self::Apple => crate::llm::APPLE_MODEL,
        }
    }

    fn format(&self) -> Format {
        match self {
            Self::OpenAiCompatible { .. } => Format::OpenAi,
            Self::Anthropic { .. } => Format::Anthropic,
            Self::ChatGptPlan { .. } => Format::Responses,
            Self::Apple => Format::Plain,
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
        401 | 403 => trf("not signed in or no access ({0})", &[&message]),
        429 => trf("rate limited, try again shortly ({0})", &[&message]),
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

// MARK: - Stopping

/// Stops an answer in progress: hand a clone to whoever may cancel. The
/// answer ends within a moment, before the next tool call or as soon as
/// it is cancelled while waiting for the model.
#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// Sends a request on its own thread, so waiting for the model can be
/// cancelled. A cancelled request is left to finish (or time out) in the
/// background and its answer is dropped.
fn post_json(
    cancel: &Cancel,
    req: ureq::RequestBuilder<ureq::typestate::WithBody>,
    body: Value,
) -> Result<Value> {
    cancel.check()?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(req.send_json(&body).map_err(ai_err).and_then(read_json));
    });
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(answer) => {
                cancel.check()?;
                return answer;
            }
            Err(RecvTimeoutError::Timeout) => cancel.check()?,
            Err(RecvTimeoutError::Disconnected) => {
                return Err(ai_err(tr("the request ended unexpectedly")));
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    OpenAi,
    Anthropic,
    Responses,
    /// `{"role", "content"}` text turns (Apple Intelligence).
    Plain,
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
    /// `on_step` is told about each tool call as it happens. On an error,
    /// or when stopped with `cancel` (`Error::Cancelled`), the
    /// conversation is left as it was before the message.
    pub fn send(
        &mut self,
        backend: &Backend,
        tools: &dyn ToolBox,
        text: &str,
        cancel: &Cancel,
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
            } => self.send_openai(
                base_url,
                model,
                api_key.as_deref(),
                tools,
                text,
                cancel,
                on_step,
            ),
            Backend::Anthropic {
                base_url,
                model,
                auth,
            } => self.send_anthropic(base_url, model, auth, tools, text, cancel, on_step),
            Backend::ChatGptPlan {
                base_url,
                model,
                access_token,
            } => self.send_responses(base_url, model, access_token, tools, text, cancel, on_step),
            Backend::Apple => self.send_apple(tools, text, cancel, on_step),
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
        cancel: &Cancel,
        on_step: &mut dyn FnMut(&Step),
    ) -> Result<Vec<(String, String)>> {
        calls
            .iter()
            .map(|(id, name, input)| {
                cancel.check()?;
                let step = Step {
                    tool: name.clone(),
                    summary: describe_call(name, input),
                };
                on_step(&step);
                steps.push(step);
                Ok((id.clone(), tools.call(name, input)))
            })
            .collect()
    }

    // MARK: Apple Intelligence

    /// Each turn is a new session on Apple's side, so earlier turns go into
    /// the instructions. The context is short (4,096 or 8,192 tokens), so
    /// earlier turns and tool results are kept short.
    fn send_apple(
        &mut self,
        tools: &dyn ToolBox,
        text: &str,
        cancel: &Cancel,
        on_step: &mut dyn FnMut(&Step),
    ) -> Result<Reply> {
        let roomy = apple_ai::status().context_size >= 8192;
        let max_result = if roomy { 3000 } else { 1200 };
        let specs: Vec<Value> = tools
            .specs()
            .into_iter()
            .map(|t| json!({"name": t.name, "description": t.description, "schema": t.schema}))
            .collect();
        let mut instructions = self.system.clone();
        let earlier = recent_turns(&self.history, if roomy { 2500 } else { 1000 });
        if !earlier.is_empty() {
            instructions.push_str("\n\nThe conversation so far:\n");
            instructions.push_str(&earlier);
        }
        // The small model answers in the documents' language rather than the
        // question's, and gives amounts it never read (#57): say which
        // language, and check the answer.
        let asked_in = Lang::guess(text);
        if let Some(lang) = asked_in {
            let name = lang.english_name();
            instructions.push_str(&format!("\n\nThe user writes in {name}: answer in {name}."));
        }
        let mut steps = Vec::new();
        let mut seen = format!("{instructions}\n{text}");
        let mut retry_note = String::new();
        let mut answer = String::new();
        for attempt in 0..2 {
            let (mut searched, mut found) = (Vec::new(), false);
            let turn_instructions = format!("{instructions}{retry_note}");
            answer = apple_ai::chat(
                &apple_ai::Turn {
                    instructions: &turn_instructions,
                    prompt: text,
                    tools: &specs,
                    temperature: 0.2,
                    max_tokens: 1000,
                },
                &mut |name, input| {
                    if cancel.is_cancelled() {
                        return "The person stopped this. Don't call more tools.".into();
                    }
                    let step = Step {
                        tool: name.to_string(),
                        summary: describe_call(name, input),
                    };
                    on_step(&step);
                    steps.push(step);
                    let out = tools.call(name, input);
                    let out = if out.chars().count() > max_result {
                        out.chars().take(max_result).collect::<String>() + " …(cut short)"
                    } else {
                        out
                    };
                    // Found: a document read, or one with all the words
                    // (not just "April" of "Stromrechnung April").
                    match name {
                        "search_documents" => {
                            searched.push(format!(
                                "«{}»",
                                input
                                    .get("query")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default()
                            ));
                            found |= !out.contains("\"results\":[]")
                                && !out.contains("the closest to «");
                        }
                        "read_document" => found |= !out.starts_with("Error"),
                        _ => {}
                    }
                    seen.push('\n');
                    seen.push_str(&out);
                    out
                },
            )?
            .trim()
            .to_string();
            cancel.check()?;
            let mut wrong = Vec::new();
            let unseen = unseen_numbers(&answer, &seen);
            if !unseen.is_empty() {
                wrong.push(format!(
                    "it gives numbers ({}) that no tool result showed. Read the document with \
                     read_document and give only what it says, or say you didn't find it",
                    unseen.join(", ")
                ));
            }
            if let (Some(asked), Some(answered)) = (asked_in, Lang::guess(&answer))
                && asked != answered
            {
                wrong.push(format!(
                    "it is in {}, but the user wrote in {}. Answer in {}",
                    answered.english_name(),
                    asked.english_name(),
                    asked.english_name()
                ));
            }
            if !searched.is_empty() && !found && asked_in != Some(Lang::En) {
                searched.dedup();
                wrong.push(format!(
                    "its searches ({}) found nothing, and the documents may be in English. Call \
                     search_documents with those words translated into English, then answer",
                    searched.join(", ")
                ));
            }
            if wrong.is_empty() || attempt == 1 {
                break;
            }
            retry_note = format!(
                "\n\nA first answer to this question was: «{answer}». It can't be given: {}. \
                 Answer again.",
                wrong.join("; ")
            );
        }
        self.history.push(json!({"role": "user", "content": text}));
        self.history
            .push(json!({"role": "assistant", "content": answer}));
        Ok(Reply {
            text: answer,
            steps,
            model: crate::llm::APPLE_MODEL.into(),
        })
    }

    // MARK: Anthropic

    #[allow(clippy::too_many_arguments)]
    fn send_anthropic(
        &mut self,
        base_url: &str,
        model: &str,
        auth: &AnthropicAuth,
        tools: &dyn ToolBox,
        text: &str,
        cancel: &Cancel,
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
            let resp = post_json(cancel, req, body)?;
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
                    tr("The model declined to answer that.").to_string()
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
                let results: Vec<Value> = Self::run_tools(
                    tools, &calls, &mut steps, cancel, on_step,
                )?
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
                reply.push_str(tr("\n\n(The answer was cut off.)"));
            }
            return Ok(Reply {
                text: reply,
                steps,
                model: served_by,
            });
        }
        Ok(Reply {
            text: tr("I couldn't finish that in a reasonable number of steps.").into(),
            steps,
            model: served_by,
        })
    }

    // MARK: OpenAI-compatible

    #[allow(clippy::too_many_arguments)]
    fn send_openai(
        &mut self,
        base_url: &str,
        model: &str,
        api_key: Option<&str>,
        tools: &dyn ToolBox,
        text: &str,
        cancel: &Cancel,
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
            let resp = post_json(cancel, req, body)?;
            let msg = resp
                .pointer("/choices/0/message")
                .cloned()
                .ok_or_else(|| ai_err(tr("the model sent no answer")))?;
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
            for (id, out) in Self::run_tools(tools, &calls, &mut steps, cancel, on_step)? {
                self.history
                    .push(json!({"role": "tool", "tool_call_id": id, "content": out}));
            }
        }
        Ok(Reply {
            text: tr("I couldn't finish that in a reasonable number of steps.").into(),
            steps,
            model: model.to_string(),
        })
    }
}

// MARK: - Responses API (ChatGPT plan)

impl Chat {
    #[allow(clippy::too_many_arguments)]
    fn send_responses(
        &mut self,
        base_url: &str,
        model: &str,
        access_token: &str,
        tools: &dyn ToolBox,
        text: &str,
        cancel: &Cancel,
        on_step: &mut dyn FnMut(&Step),
    ) -> Result<Reply> {
        let http = agent(Duration::from_secs(300));
        let tool_defs: Vec<Value> = tools
            .specs()
            .into_iter()
            .map(|t| {
                json!({"type": "function", "name": t.name, "description": t.description,
                       "parameters": t.schema})
            })
            .collect();
        self.history.push(json!({"role": "user", "content": text}));
        let mut steps = Vec::new();
        let mut served_by = model.to_string();
        for _ in 0..MAX_ROUNDS {
            // Nothing is stored at OpenAI (`store: false`), so every request
            // carries the conversation, reasoning included (encrypted).
            let body = json!({
                "model": model,
                "instructions": self.system,
                "input": self.history,
                "tools": tool_defs,
                "store": false,
                "stream": true,
                "include": ["reasoning.encrypted_content"],
            });
            let req = http
                .post(&format!("{}/responses", base_url.trim_end_matches('/')))
                .header("authorization", &format!("Bearer {access_token}"))
                .header("content-type", "application/json")
                .header("accept", "text/event-stream");
            let response = post_stream(cancel, req, body)?;
            if let Some(m) = response.get("model").and_then(Value::as_str) {
                served_by = m.to_string();
            }
            let output = response
                .get("output")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut calls = Vec::new();
            let mut said = String::new();
            for item in output {
                match item.get("type").and_then(Value::as_str) {
                    Some("function_call") => calls.push((
                        item.get("call_id")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        item.get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        item.get("arguments")
                            .and_then(Value::as_str)
                            .and_then(|a| serde_json::from_str(a).ok())
                            .unwrap_or(json!({})),
                    )),
                    Some("message") => said.push_str(&output_text(&item)),
                    _ => {}
                }
                self.history.push(echo_item(item));
            }
            if calls.is_empty() {
                return Ok(Reply {
                    text: said.trim().to_string(),
                    steps,
                    model: served_by,
                });
            }
            for (id, out) in Self::run_tools(tools, &calls, &mut steps, cancel, on_step)? {
                self.history
                    .push(json!({"type": "function_call_output", "call_id": id, "output": out}));
            }
        }
        Ok(Reply {
            text: tr("I couldn't finish that in a reasonable number of steps.").into(),
            steps,
            model: served_by,
        })
    }
}

/// An output item as it goes back into the next request. With nothing
/// stored at OpenAI, item ids would point at nothing, so they're left out
/// (reasoning items keep theirs, with their encrypted content).
fn echo_item(mut item: Value) -> Value {
    if item.get("type").and_then(Value::as_str) != Some("reasoning")
        && let Some(map) = item.as_object_mut()
    {
        map.remove("id");
        map.remove("status");
    }
    item
}

/// The text of a `message` output item.
fn output_text(item: &Value) -> String {
    item.get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|c| c.get("type").and_then(Value::as_str) == Some("output_text"))
        .filter_map(|c| c.get("text").and_then(Value::as_str))
        .collect()
}

/// Sends a streaming Responses request on its own thread (so `cancel` can
/// stop it, closing the connection) and returns the finished response.
fn post_stream(
    cancel: &Cancel,
    req: ureq::RequestBuilder<ureq::typestate::WithBody>,
    body: Value,
) -> Result<Value> {
    cancel.check()?;
    let (tx, rx) = mpsc::channel();
    let stop = cancel.clone();
    std::thread::spawn(move || {
        let result = req.send_json(&body).map_err(ai_err).and_then(|mut resp| {
            let status = resp.status().as_u16();
            if !(200..300).contains(&status) {
                let text = resp.body_mut().read_to_string().unwrap_or_default();
                return Err(plan_error(status, &text));
            }
            let reader = std::io::BufReader::new(resp.into_body().into_reader());
            read_events(reader, &stop)
        });
        let _ = tx.send(result);
    });
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(answer) => {
                cancel.check()?;
                return answer;
            }
            Err(RecvTimeoutError::Timeout) => cancel.check()?,
            Err(RecvTimeoutError::Disconnected) => {
                return Err(ai_err(tr("the request ended unexpectedly")));
            }
        }
    }
}

/// Reads a Responses event stream (server-sent events) up to
/// `response.completed` and returns that response. Anything else that ends
/// the stream is an error.
fn read_events(reader: impl std::io::BufRead, cancel: &Cancel) -> Result<Value> {
    let mut data = String::new();
    for line in reader.lines() {
        cancel.check()?;
        let line = line.map_err(ai_err)?;
        if let Some(rest) = line.strip_prefix("data:") {
            data.push_str(rest.trim_start());
            continue;
        }
        if !line.is_empty() || data.is_empty() {
            continue; // `event:` lines, comments; or an empty block
        }
        let event: Value = serde_json::from_str(&std::mem::take(&mut data)).unwrap_or(Value::Null);
        match event.get("type").and_then(Value::as_str) {
            Some("response.completed") => {
                return Ok(event.get("response").cloned().unwrap_or(Value::Null));
            }
            Some("response.failed") => {
                let e = event
                    .pointer("/response/error")
                    .cloned()
                    .unwrap_or(Value::Null);
                return Err(plan_error_code(
                    e.get("code").and_then(Value::as_str).unwrap_or(""),
                    e.get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("the request failed"),
                ));
            }
            Some("response.incomplete") => {
                let why = event
                    .pointer("/response/incomplete_details/reason")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                return Err(ai_err(trf("the answer stopped early ({0})", &[&why])));
            }
            Some("error") => {
                return Err(plan_error_code(
                    event.get("code").and_then(Value::as_str).unwrap_or(""),
                    event
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("the request failed"),
                ));
            }
            _ => {}
        }
    }
    Err(ai_err(
        "the answer was cut off (the connection ended early)",
    ))
}

/// An HTTP error from the Responses API, in words.
fn plan_error(status: u16, body: &str) -> Error {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let code = v
        .pointer("/error/code")
        .and_then(Value::as_str)
        .unwrap_or("");
    let message = v
        .pointer("/error/message")
        .and_then(Value::as_str)
        .map(String::from)
        .unwrap_or_else(|| body.chars().take(300).collect());
    match (status, code) {
        (_, c) if c.starts_with("subscription_sharing") => plan_error_code(c, &message),
        (401, _) => ai_err(tr(
            "the ChatGPT sign-in has expired; sign in again in Settings ▸ Assistant",
        )),
        (429, _) => ai_err(trf("rate limited, try again shortly ({0})", &[&message])),
        _ => ai_err(format!("HTTP {status}: {message}")),
    }
}

fn plan_error_code(code: &str, message: &str) -> Error {
    ai_err(match code {
        "subscription_sharing_usage_limit_exceeded" => tr(
            "your ChatGPT plan's limit for Rupert is used up for now; try again later, or raise \
                Rupert's limit in ChatGPT's settings",
        )
        .to_string(),
        "subscription_sharing_usage_unavailable" => {
            tr("ChatGPT plan usage isn't available right now; try again later").to_string()
        }
        "subscription_sharing_user_not_eligible" => tr(
            "this ChatGPT account or plan can't be used by other apps (signing in again won't \
                change that)",
        )
        .to_string(),
        _ => message.to_string(),
    })
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

/// Numbers in an answer that aren't in what the model saw (its
/// instructions, the question and the tool results): amounts it made up.
/// Numbers are compared by their digits, so "$1,429.00", "1.429,00" and
/// "1429" are the same; whole cents ("…00") don't count. Short numbers (a
/// day, "2 hours") aren't checked unless they follow a currency sign.
fn unseen_numbers(answer: &str, seen: &str) -> Vec<String> {
    fn numbers(text: &str) -> Vec<(String, String, bool)> {
        let chars: Vec<char> = text.chars().collect();
        let mut out = Vec::new();
        let mut i = 0;
        while i < chars.len() {
            if !chars[i].is_ascii_digit() {
                i += 1;
                continue;
            }
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_digit()
                    || (matches!(chars[i], '.' | ',')
                        && chars.get(i + 1).is_some_and(char::is_ascii_digit)))
            {
                i += 1;
            }
            let raw: String = chars[start..i].iter().collect();
            let mut digits: String = raw.chars().filter(char::is_ascii_digit).collect();
            let cents = raw.len() > 3 && matches!(raw.as_bytes()[raw.len() - 3], b'.' | b',');
            if cents && digits.ends_with("00") {
                digits.truncate(digits.len() - 2);
            }
            let money = start > 0
                && chars[..start]
                    .iter()
                    .rev()
                    .take(2)
                    .any(|c| matches!(c, '$' | '€' | '£'));
            out.push((raw, digits, money));
        }
        out
    }
    let known: std::collections::HashSet<String> =
        numbers(seen).into_iter().map(|(_, d, _)| d).collect();
    let mut unseen: Vec<String> = numbers(answer)
        .into_iter()
        .filter(|(_, d, money)| (d.len() >= 3 || *money) && !known.contains(d))
        .map(|(raw, _, _)| raw)
        .collect();
    unseen.dedup();
    unseen
}

/// The latest text turns, oldest first, within `budget` characters, as
/// "Person: …" / "Rupert: …" lines.
fn recent_turns(history: &[Value], budget: usize) -> String {
    let mut lines = Vec::new();
    let mut used = 0;
    for turn in history.iter().rev() {
        let who = if turn["role"] == "user" {
            "Person"
        } else {
            "Rupert"
        };
        let line = format!("{who}: {}", turn["content"].as_str().unwrap_or_default());
        used += line.chars().count();
        if used > budget {
            break;
        }
        lines.push(line);
    }
    lines.reverse();
    lines.join("\n")
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
            ("", "") => tr("Searching documents").into(),
            (q, "") => trf("Searching for “{0}”", &[&q]),
            ("", t) => trf("Looking up tags {0}", &[&t]),
            (q, t) => trf("Searching for “{0}” tagged {1}", &[&q, &t]),
        },
        "list_tags" => tr("Looking at tags").into(),
        "total_amounts" => tr("Adding up amounts").into(),
        "list_senders" => tr("Looking at who documents are from").into(),
        "list_sources" => tr("Looking at the library").into(),
        "list_folder" => trf("Opening {0}", &[&s("path").trim_matches('/')]),
        "read_document" => trf("Reading {0}", &[&s("path")]),
        "open_folder" => trf("Showing folder {0}", &[&s("path")]),
        "show_document" => trf("Showing {0}", &[&s("path")]),
        "show_results" => tr("Showing results").into(),
        "show_saved_search" => trf("Showing saved search {0}", &[&s("name")]),
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
    /// A saved search (#110), by its id in its source; the words and tags
    /// come along for a window that only runs plain searches (the phone).
    SavedSearch {
        source_id: String,
        id: String,
        name: String,
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
    /// Documents read this turn, and the last search that found something
    /// (for [`LibraryTools::take_suggestions`]).
    read: RefCell<Vec<(String, String)>>,
    searched: RefCell<Option<UiAction>>,
}

impl<'a> LibraryTools<'a> {
    pub fn new(index: &'a Index, sources: &'a [Source]) -> Self {
        Self {
            index,
            sources,
            actions: RefCell::new(Vec::new()),
            read: RefCell::new(Vec::new()),
            searched: RefCell::new(None),
        }
    }

    /// The UI requests made during the conversation turn.
    pub fn take_actions(&self) -> Vec<UiAction> {
        std::mem::take(&mut self.actions.borrow_mut())
    }

    /// What the turn looked at, as things the person could choose to show:
    /// the documents read (up to three), or else the last search that found
    /// something. For the phone, which offers these as buttons when the
    /// model didn't ask to show anything itself (small models often don't).
    pub fn take_suggestions(&self) -> Vec<UiAction> {
        let read = std::mem::take(&mut *self.read.borrow_mut());
        let searched = self.searched.borrow_mut().take();
        let mut out: Vec<UiAction> = Vec::new();
        for (source_id, rel_path) in read {
            let a = UiAction::ShowDocument {
                source_id,
                rel_path,
            };
            if !out.contains(&a) && out.len() < 3 {
                out.push(a);
            }
        }
        if out.is_empty() {
            out.extend(searched);
        }
        out
    }

    /// The source and document a tool means, forgiving the slips small
    /// models make: a folder (or nothing) given as the source, the source's
    /// name repeated in the path, another extension or letter case.
    fn find_document(&self, input: &Value) -> std::result::Result<(&Source, String), String> {
        let rel = Self::rel(input);
        let sources = self.candidates(input)?;
        for source in &sources {
            let rel = strip_source(&source.name, &rel);
            if let Some(found) = self.match_document(source, &rel) {
                return Ok((source, found));
            }
        }
        let names: Vec<&str> = sources.iter().map(|s| s.name.as_str()).collect();
        Err(format!("No document “{rel}” in {}", names.join(", ")))
    }

    fn match_document(&self, source: &Source, rel: &str) -> Option<String> {
        if self
            .index
            .document(&source.id, rel)
            .ok()
            .flatten()
            .is_some()
        {
            return Some(rel.to_string());
        }
        let (dir, name) = rel.rsplit_once('/').unwrap_or(("", rel));
        let stem = |n: &str| n.rsplit_once('.').map_or(n, |(s, _)| s).to_lowercase();
        let dir = self
            .match_folder(source, dir)
            .unwrap_or_else(|| dir.to_string());
        let (_, hits) = self.index.list_folder(&source.id, &dir).ok()?;
        hits.into_iter()
            .find(|h| h.name.eq_ignore_ascii_case(name) || stem(&h.name) == stem(name))
            .map(|h| h.rel_path)
    }

    /// The source and folder a tool means, forgiving the same slips; a bare
    /// folder name is looked up anywhere in the source.
    fn find_folder(&self, input: &Value) -> std::result::Result<(&Source, String), String> {
        let mut rel = Self::rel(input);
        let key = input
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or_default();
        // A folder named as the source, with no path.
        if rel.is_empty()
            && !self
                .sources
                .iter()
                .any(|s| s.id == key || s.name.eq_ignore_ascii_case(key))
        {
            rel = key.trim_matches('/').to_string();
        }
        let sources = self.candidates(input)?;
        for source in &sources {
            let rel = strip_source(&source.name, &rel);
            if let Some(found) = self.match_folder(source, &rel) {
                return Ok((source, found));
            }
            // A document given: the folder it's in.
            if let Some(doc) = self.match_document(source, &rel) {
                let dir = doc.rsplit_once('/').map_or("", |(d, _)| d).to_string();
                return Ok((source, dir));
            }
        }
        let names: Vec<&str> = sources.iter().map(|s| s.name.as_str()).collect();
        Err(format!("No folder “{rel}” in {}", names.join(", ")))
    }

    /// The folder's path as the index spells it (disks may ignore case,
    /// the index doesn't), else as given if it exists on disk.
    fn match_folder(&self, source: &Source, rel: &str) -> Option<String> {
        if rel.is_empty() {
            return Some(String::new());
        }
        let want = rel.to_lowercase();
        let folders = self.index.folders().ok()?;
        let mut matches: Vec<&String> = folders
            .iter()
            .filter(|(s, d)| s == &source.id && !d.is_empty())
            .map(|(_, d)| d)
            .filter(|d| {
                let d = d.to_lowercase();
                d == want || d.ends_with(&format!("/{want}"))
            })
            .collect();
        matches.sort_by_key(|d| d.len());
        matches
            .first()
            .map(|d| (*d).clone())
            .or_else(|| rupert_dir_exists(&source.root, rel).then(|| rel.to_string()))
    }

    /// The source named in the input first, else every source (a model may
    /// put a folder's name there).
    fn candidates(&self, input: &Value) -> std::result::Result<Vec<&Source>, String> {
        match self.source(input) {
            Ok(s) => Ok(vec![s]),
            Err(_) if !self.sources.is_empty() => Ok(self.sources.iter().collect()),
            Err(e) => Err(e),
        }
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
        // A day the model gave, if it's one (YYYY-MM-DD).
        let date = |input: &Value, key: &str| {
            input
                .get(key)
                .and_then(Value::as_str)
                .filter(|d| crate::meta::is_iso_date(d))
                .map(String::from)
        };
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
            date_from: date(input, "date_from"),
            date_to: date(input, "date_to"),
            kinds: input
                .get("kinds")
                .and_then(Value::as_array)
                .map(|k| {
                    k.iter()
                        .filter_map(Value::as_str)
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default(),
            // #108: who it's from (one name or several) and when it's due.
            senders: match input.get("from") {
                Some(Value::String(s)) if !s.trim().is_empty() => vec![s.trim().to_string()],
                Some(Value::Array(a)) => a
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect(),
                _ => Vec::new(),
            },
            due_from: date(input, "due_from"),
            due_to: date(input, "due_to"),
            ..Default::default()
        }
    }

    /// An amount with its currency, as the tools show it ("1240.50 EUR").
    fn money(amount: &Option<String>, currency: &Option<String>) -> Option<String> {
        amount.as_ref().map(|a| match currency {
            Some(c) => format!("{a} {c}"),
            None => a.clone(),
        })
    }

    /// Searches; when nothing has every word, searches again leaving words
    /// out (the last ones first, as in "Pipeline Plumbing April"), and
    /// returns the query that found something.
    fn search_with_fewer_words(
        &self,
        q: SearchQuery,
    ) -> std::result::Result<(SearchQuery, Vec<crate::index::Hit>), String> {
        let hits = self.index.search(&q).map_err(|e| e.to_string())?;
        let words: Vec<&str> = q
            .text
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .take(6)
            .collect();
        if !hits.is_empty() || words.len() < 2 {
            return Ok((q, hits));
        }
        let n = words.len();
        for keep in (1..n).rev() {
            for leave_out in fewer(n, keep) {
                let kept: Vec<&str> = (0..n)
                    .filter(|i| !leave_out.contains(i))
                    .map(|i| words[i])
                    .collect();
                if kept.iter().all(|w| filler(w)) {
                    continue;
                }
                let text = kept.join(" ");
                let fewer_q = SearchQuery { text, ..q.clone() };
                let hits = self.index.search(&fewer_q).map_err(|e| e.to_string())?;
                if !hits.is_empty() {
                    return Ok((fewer_q, hits));
                }
            }
        }
        Ok((q, hits))
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
                let asked = Self::query(input);
                let (q, hits) = self.search_with_fewer_words(asked.clone())?;
                if !hits.is_empty() {
                    *self.searched.borrow_mut() = Some(UiAction::Search {
                        text: q.text.clone(),
                        tags: q.tags.clone(),
                        tag_mode: q.tag_mode,
                        favorites_only: q.favorites_only,
                    });
                }
                let results = json!(hits.iter().map(|h| json!({
                    "source": self.source_name(&h.source_id),
                    "path": h.rel_path,
                    "title": h.title,
                    "date": h.document_date,
                    "tags": h.tags,
                    "favorite": h.favorite,
                    "from": h.sender,
                    "amount": Self::money(&h.amount, &h.currency),
                    "due": h.due_date,
                    "snippet": h.snippet.as_deref().map(|s| s.replace(['\u{E000}', '\u{E001}'], "")),
                })).collect::<Vec<_>>());
                // Small models search once with every word of the question
                // (a month, words in another language) and give up, or
                // answer from the snippets (#57): the result says what
                // matched, and what to do next.
                let next = if hits.is_empty() {
                    if asked.text.trim().is_empty() {
                        return Ok(results);
                    }
                    format!(
                        "Nothing has the words «{}». Documents are often in another language \
                         than the question: call search_documents again with these words in \
                         English (or the documents' language), or with fewer words.",
                        asked.text.trim()
                    )
                } else if q.text != asked.text {
                    format!(
                        "These are the documents with the words «{}», the closest to «{}». \
                         Read the one that fits with read_document, then answer from it.",
                        q.text,
                        asked.text.trim()
                    )
                } else {
                    "To say what a document says (an amount, a date, what it's for), read it \
                     with read_document first."
                        .into()
                };
                Ok(json!({"results": results, "note": next}))
            }
            "total_amounts" => {
                // Every match, added up per currency (#108): no reading
                // each document, no limit of 50.
                let q = Self::query(input);
                let totals = self.index.totals(&q).map_err(|e| e.to_string())?;
                let matching = self
                    .index
                    .search(&SearchQuery {
                        limit: 1_000_000,
                        skip_excerpts: true,
                        ..q
                    })
                    .map_err(|e| e.to_string())?
                    .len();
                let with_amount: usize = totals.iter().map(|t| t.count).sum();
                Ok(json!({
                    "totals": totals.iter().map(|t| json!({
                        "currency": t.currency,
                        "total": t.amount,
                        "documents": t.count,
                    })).collect::<Vec<_>>(),
                    "documents_matching": matching,
                    "documents_without_amount": matching - with_amount,
                    "note": "Totals of the amounts recorded with each document. Documents without one aren't counted; their amounts, if any, are only in their text.",
                }))
            }
            "list_senders" => {
                let filter = input
                    .get("filter")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let list = self.index.senders(filter, 100).map_err(|e| e.to_string())?;
                Ok(
                    json!({"senders": list.iter().map(|s| json!({"from": s.tag, "documents": s.count})).collect::<Vec<_>>()}),
                )
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
                    "saved_searches": self.saved_searches().iter().map(|(source, s)| json!({
                        "name": s.name,
                        "words": s.text,
                        "tags": s.tags,
                        "tag_mode": s.tag_mode,
                        "favorites_only": s.favorites,
                        "date": s.date,
                        "kind": s.kind,
                        "from": s.from,
                        "due": s.due,
                        "in": s.in_one_source().then(|| match &s.folder {
                            Some(f) if s.scope.as_deref() == Some("folder") => format!("{}/{f}", source.name),
                            _ => source.name.clone(),
                        }),
                    })).collect::<Vec<_>>(),
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
                            "from": m.and_then(|m| m.sender.clone()),
                            "amount": m.and_then(|m| Self::money(&m.amount, &m.currency)),
                            "due": m.and_then(|m| m.due_date.clone()),
                        }));
                    }
                }
                Ok(
                    json!({"source": source.name, "path": rel, "folders": folders, "documents": docs}),
                )
            }
            "read_document" => {
                let (source, rel) = self.find_document(input)?;
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
                self.read
                    .borrow_mut()
                    .push((source.id.clone(), rel.clone()));
                let text = self
                    .index
                    .document_text(&source.id, &rel)
                    .map_err(|e| e.to_string())?
                    .map(|t| with_page_markers(&t));
                let total = text.as_ref().map(|t| t.chars().count()).unwrap_or(0);
                Ok(json!({
                    "title": info.hit.title,
                    "file": info.hit.name,
                    "date": info.hit.document_date,
                    "tags": info.hit.tags,
                    "from": info.hit.sender,
                    "amount": Self::money(&info.hit.amount, &info.hit.currency),
                    "due": info.hit.due_date,
                    "notes": info.notes,
                    "text": text.map(|t| t.chars().take(max).collect::<String>()),
                    "text_chars_total": total,
                    "text_note": if total == 0 { Some("No text is known for this document (e.g. an unrecognized scan, or online-only and not in the catalog).") } else { None },
                }))
            }
            "open_folder" => {
                let (source, rel) = self.find_folder(input)?;
                self.actions.borrow_mut().push(UiAction::OpenFolder {
                    source_id: source.id.clone(),
                    rel_path: rel,
                });
                Ok(json!("Shown."))
            }
            "show_document" => {
                let (source, rel) = self.find_document(input)?;
                self.actions.borrow_mut().push(UiAction::ShowDocument {
                    source_id: source.id.clone(),
                    rel_path: rel,
                });
                Ok(json!("Shown."))
            }
            "show_saved_search" => {
                let name = input
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_lowercase();
                let all = self.saved_searches();
                let found = all
                    .iter()
                    .find(|(_, s)| s.name.to_lowercase() == name)
                    .or_else(|| {
                        all.iter().find(|(_, s)| {
                            !name.is_empty() && s.name.to_lowercase().contains(&name)
                        })
                    });
                let Some((source, s)) = found else {
                    let names: Vec<&str> = all.iter().map(|(_, s)| s.name.as_str()).collect();
                    return Err(format!(
                        "No saved search is called \"{name}\". Saved searches: {}",
                        if names.is_empty() {
                            "none".into()
                        } else {
                            names.join(", ")
                        }
                    ));
                };
                self.actions.borrow_mut().push(UiAction::SavedSearch {
                    source_id: source.id.clone(),
                    id: s.id.clone(),
                    name: s.name.clone(),
                    text: s.text.clone(),
                    tags: s.tags.clone(),
                    tag_mode: s.tag_mode,
                    favorites_only: s.favorites,
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

/// `rel` without a leading copy of the source's name (`Home Documents/Tax`).
fn strip_source(name: &str, rel: &str) -> String {
    let n = name.len();
    match rel.get(..n) {
        Some(head) if head.eq_ignore_ascii_case(name) && rel.len() == n => String::new(),
        Some(head) if head.eq_ignore_ascii_case(name) && rel[n..].starts_with('/') => {
            rel[n + 1..].to_string()
        }
        _ => rel.to_string(),
    }
}

/// A month or a small word ("de", "im", "the"): a search for only such
/// words finds documents at random.
fn filler(word: &str) -> bool {
    const FILLER: &[&str] = &[
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
        "enero",
        "febrero",
        "marzo",
        "abril",
        "mayo",
        "junio",
        "julio",
        "agosto",
        "septiembre",
        "setiembre",
        "octubre",
        "noviembre",
        "diciembre",
        "januar",
        "februar",
        "märz",
        "juni",
        "juli",
        "oktober",
        "dezember",
        "the",
        "of",
        "a",
        "an",
        "in",
        "on",
        "for",
        "my",
        "and",
        "de",
        "del",
        "la",
        "el",
        "los",
        "las",
        "en",
        "y",
        "mi",
        "por",
        "para",
        "der",
        "die",
        "das",
        "im",
        "am",
        "von",
        "für",
        "und",
        "mein",
        "meine",
    ];
    word.chars().count() < 3 || FILLER.contains(&word.to_lowercase().as_str())
}

/// Which of `n` words to leave out to keep `keep` of them: each set of
/// indexes, those leaving out the last words first.
fn fewer(n: usize, keep: usize) -> Vec<Vec<usize>> {
    let mut masks: Vec<u32> = (0..1u32 << n)
        .filter(|m| m.count_ones() as usize == n - keep)
        .collect();
    masks.sort_unstable_by(|a, b| b.cmp(a));
    masks
        .into_iter()
        .map(|m| (0..n).filter(|i| m & (1 << i) != 0).collect())
        .collect()
}

impl LibraryTools<'_> {
    /// The saved searches (#110) kept with each source, by name.
    fn saved_searches(&self) -> Vec<(&Source, crate::saved_search::SavedSearch)> {
        let mut all: Vec<_> = self
            .sources
            .iter()
            .flat_map(|source| {
                crate::saved_search::list(&source.root)
                    .into_iter()
                    .map(move |s| (source, s))
            })
            .collect();
        all.sort_by_key(|(_, s)| s.name.to_lowercase());
        all
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
            "date_from": {"type": "string", "description": "Only documents dated on or after this day, YYYY-MM-DD (the date on the document, else the file's date)."},
            "date_to": {"type": "string", "description": "Only documents dated on or before this day, YYYY-MM-DD."},
            "kinds": {"type": "array", "items": {"type": "string", "enum": ["pdf", "image", "office", "text", "email"]}, "description": "Only these kinds of file."},
            "from": {"type": "string", "description": "Only documents from this sender (as list_senders names them, ignoring case)."},
            "due_from": {"type": "string", "description": "Only documents due on or after this day, YYYY-MM-DD (bills to pay, things expiring)."},
            "due_to": {"type": "string", "description": "Only documents due on or before this day, YYYY-MM-DD."},
            "limit": {"type": "integer", "description": "Most results to return (default 15, at most 50)."}
        });
        let doc_props = json!({
            "source": {"type": "string", "description": "The source's name (see list_sources)."},
            "path": {"type": "string", "description": "Path within the source, as search results give it."}
        });
        vec![
            ToolSpec {
                name: "search_documents",
                description: "Full-text search of the user's documents. Returns source, path, title, document date, tags, who it's from, its amount, its due date and a text snippet for each match, best first.",
                schema: schema(search_props.clone(), &[]),
            },
            ToolSpec {
                name: "total_amounts",
                description: "Adds up the amounts recorded with every document a search finds (same options as search_documents, no limit), per currency. Use for \"how much did I pay…\" or \"what do I still owe…\" instead of reading documents one by one.",
                schema: schema(search_props.clone(), &[]),
            },
            ToolSpec {
                name: "list_senders",
                description: "Who documents are from (companies, offices, people), with how many documents each, most first. Optionally only those matching a filter text.",
                schema: schema(json!({"filter": {"type": "string"}}), &[]),
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
                description: "A document's details and its recognized text (from the index; nothing is downloaded), with [page N] where each page starts. Use to answer questions about a document's contents.",
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
                name: "show_saved_search",
                description: "Show one of the user's saved searches (list_sources names them) in the app's main window, by its name.",
                schema: schema(json!({"name": {"type": "string"}}), &["name"]),
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

// MARK: - Page citations (#106)

/// A document's text with `[page N]` where each page starts, when it has
/// page breaks (text read before #106 has none and comes as it is).
fn with_page_markers(text: &str) -> String {
    if !text.contains(PAGE_BREAK) {
        return text.to_string();
    }
    let mut out = String::new();
    for (i, page) in text.split(PAGE_BREAK).enumerate() {
        let page = page.trim();
        if page.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&format!("[page {}]\n{page}", i + 1));
    }
    out
}

/// A page cited in an answer ("[Lease 2024, p. 3]"), and the document and
/// passage it points to, so the window can open the page viewer there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Citation {
    /// The citation as written in the answer, brackets included.
    pub text: String,
    pub source_id: String,
    pub rel_path: String,
    /// 0-based.
    pub page: usize,
    /// The line on that page that best matches what the answer says just
    /// before the citation, to highlight it.
    pub passage: Option<String>,
}

/// The citations in `answer`: where each starts and ends, the name as
/// written, and the page (1-based). "p.", "page" and the forms other
/// languages use (S., Seite, pág., página) all count.
fn cited(answer: &str) -> Vec<(usize, usize, String, usize)> {
    const PAGE_WORDS: [&str; 9] = [
        "p.", "pp.", "page", "pág.", "pag.", "página", "s.", "seite", "pg.",
    ];
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(open) = answer[from..].find('[').map(|i| i + from) {
        let Some(close) = answer[open..].find(']').map(|i| i + open) else {
            break;
        };
        let inner = &answer[open + 1..close];
        from = open + 1;
        if inner.contains('[') || inner.len() > 160 {
            continue;
        }
        let Some((name, page)) = inner.rsplit_once(',') else {
            continue;
        };
        let page = page.trim().to_lowercase();
        let Some(word) = PAGE_WORDS.iter().find(|w| page.starts_with(**w)) else {
            continue;
        };
        let number: String = page[word.len()..]
            .trim()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        let (name, number) = (name.trim(), number.parse::<usize>().unwrap_or(0));
        if name.is_empty() || number == 0 {
            continue;
        }
        out.push((open, close + 1, name.to_string(), number));
        from = close + 1;
    }
    out
}

/// The line of `page_text` sharing the most words with `said` (numbers
/// count double), if any shares at least two (or one number).
fn best_passage(page_text: &str, said: &str) -> Option<String> {
    let words = |t: &str| -> Vec<String> {
        t.split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.chars().count() > 2 || w.chars().any(|c| c.is_ascii_digit()))
            .map(str::to_lowercase)
            .collect()
    };
    let wanted = words(said);
    let mut best: Option<(usize, &str)> = None;
    for line in page_text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let have = words(line);
        let score: usize = wanted
            .iter()
            .filter(|w| have.contains(w))
            .map(|w| {
                if w.chars().any(|c| c.is_ascii_digit()) {
                    2
                } else {
                    1
                }
            })
            .sum();
        if score >= 2 && best.is_none_or(|(b, _)| score > b) {
            best = Some((score, line));
        }
    }
    best.map(|(_, line)| line.chars().take(200).collect())
}

impl LibraryTools<'_> {
    /// The pages `answer` cites, each with the document it names: one read
    /// this turn whose title or file name fits, or else the best match by
    /// name in the library. Citations that fit nothing are left out.
    pub fn citations(&self, answer: &str) -> Vec<Citation> {
        let read = self.read.borrow().clone();
        let fold = |s: &str| s.to_lowercase();
        let mut out: Vec<Citation> = Vec::new();
        for (start, end, name, page) in cited(answer) {
            let want = fold(&name);
            let fits = |title: Option<&str>, file: &str| {
                let stem = file.rsplit_once('.').map_or(file, |(s, _)| s);
                [title.unwrap_or_default(), file, stem]
                    .iter()
                    .filter(|t| !t.is_empty())
                    .any(|t| fold(t) == want || fold(t).contains(&want) || want.contains(&fold(t)))
            };
            let from_read = read.iter().find_map(|(sid, rel)| {
                let doc = self.index.document(sid, rel).ok().flatten()?;
                fits(doc.hit.title.as_deref(), &doc.hit.name).then(|| (sid.clone(), rel.clone()))
            });
            let found = from_read.or_else(|| {
                self.index
                    .quick_find(&name, 1)
                    .ok()?
                    .into_iter()
                    .next()
                    .map(|h| (h.source_id, h.rel_path))
            });
            let Some((source_id, rel_path)) = found else {
                continue;
            };
            // What the answer says just before the citation: its sentence.
            let before = &answer[..start];
            let said = before
                .rfind(['.', '\n', '!', '?', ']'])
                .map_or(before, |i| &before[i + 1..]);
            let said = if said.trim().len() < 8 { before } else { said };
            let passage = self
                .index
                .document_text(&source_id, &rel_path)
                .ok()
                .flatten()
                .and_then(|t| t.split(PAGE_BREAK).nth(page - 1).map(str::to_string))
                .and_then(|t| best_passage(&t, said));
            out.push(Citation {
                text: answer[start..end].to_string(),
                source_id,
                rel_path,
                page: page - 1,
                passage,
            });
        }
        out
    }
}

/// The assistant's instructions.
pub fn system_prompt(today: &str) -> String {
    format!(
        "You are Rupert, the librarian in a document manager for the user's scanned paperwork \
         (PDFs and other documents in folders on their computer, synced with iCloud Drive or OneDrive). \
         Today is {today}.\n\n\
         Use the tools to find documents and answer questions about them. Search before answering \
         or asking the user anything; don't ask them which document they mean. Never give \
         documents, dates or amounts you haven't seen in a tool result: to answer about what a \
         document says (an amount, a warranty, what it was for), read its text with read_document \
         first. Search with a few distinctive words (a name rather than a month), then refine with \
         tags or other words. If nothing is found, search again with fewer words, or with the key \
         words in English or the documents' language: documents are often in another language \
         than the question.\n\n\
         Documents may record who they're from, an amount and a due date. For questions about \
         money (\"how much did I pay the dentist this year\", \"what do I still have to pay\"), \
         use total_amounts and the from and due filters (due_from today for what's still to pay) \
         rather than reading each document; say how many documents had no amount.\n\n\
         When the user asks to open, show or go to something, do it with open_folder, show_document \
         or show_results, and only say something is shown or open after the tool answered \
         \"Shown.\". You can't change, move, rename, tag or delete files; say so if asked.\n\n\
         Answer briefly in plain text. Name documents by title (or file name) and folder. When \
         something comes from a document's text, cite the page right after it in square \
         brackets: the document's title (or file name) and the page from its [page N] marks, \
         like [Lease 2024, p. 3].\n\n\
         Answer in the language the user writes in, even when the documents are in another; if \
         that's unclear, in {language}.",
        language = crate::i18n::current().english_name()
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
                    sender: Some("Harbor Marine Supply".into()),
                    amount: Some("412.50".into()),
                    currency: Some("USD".into()),
                    due_date: Some("2025-04-13".into()),
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
            bookmark: None,
        }];
        (dir, idx, sources)
    }

    #[test]
    fn read_document_marks_pages_and_answers_cite_them() {
        let (_dir, idx, sources) = library();
        let lease = DocRow {
            source_id: "s1".into(),
            rel_path: "Home/lease.pdf".into(),
            name: "lease.pdf".into(),
            kind: FileKind::Pdf,
            size: 10,
            modified_ms: Some(1),
            cloud: CloudState::Local,
            meta: DocMeta {
                title: Some("Lease 2024".into()),
                ..Default::default()
            },
            storage: Storage::Embedded,
            text_state: TextState::Extracted,
            sidecar_ms: None,
        };
        let body = "Residential lease\n\u{c}\n\u{c}\nThe tenant pays\nA monthly rent of $1,200 on the first\nNo pets";
        idx.upsert(&lease, Some(body)).unwrap();
        let tools = LibraryTools::new(&idx, &sources);
        let read: Value = serde_json::from_str(&tools.call(
            "read_document",
            &json!({"source": "Documents", "path": "Home/lease.pdf"}),
        ))
        .unwrap();
        assert_eq!(
            read["text"],
            "[page 1]\nResidential lease\n\n[page 3]\nThe tenant pays\nA monthly rent of $1,200 on the first\nNo pets"
        );

        let answer = "The rent is $1,200 a month [Lease 2024, p. 3]. The impeller cost \
                      $412.50 [impeller, S. 1]; see also [a note] and [Nothing here, p. 2].";
        let found = tools.citations(answer);
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0].text, "[Lease 2024, p. 3]");
        assert_eq!(
            (found[0].rel_path.as_str(), found[0].page),
            ("Home/lease.pdf", 2)
        );
        assert_eq!(
            found[0].passage.as_deref(),
            Some("A monthly rent of $1,200 on the first")
        );
        // Not read this turn: found by its name in the library.
        assert_eq!(found[1].rel_path, "Boat/Engine/impeller.pdf");
        assert_eq!(found[1].page, 0);
        assert!(found[1].passage.as_deref().unwrap().contains("412.50"));
    }

    /// Small models get names slightly wrong; the tools still find what
    /// they meant, and the turn offers what it looked at.
    #[test]
    fn tools_forgive_slips_and_suggest_what_was_seen() {
        let (_dir, idx, sources) = library();
        let tools = LibraryTools::new(&idx, &sources);
        let shown = |input: Value, tool: &str| tools.call(tool, &input);
        // A folder's name as the source, and a bare folder name.
        assert_eq!(
            shown(json!({"source": "Engine", "path": ""}), "open_folder"),
            "\"Shown.\""
        );
        assert_eq!(
            shown(json!({"source": "Engine", "path": "engine"}), "open_folder"),
            "\"Shown.\""
        );
        // The source's name in the path, and a document for a folder.
        assert_eq!(
            shown(
                json!({"source": "Documents", "path": "Documents/Boat/Engine/impeller.pdf"}),
                "open_folder"
            ),
            "\"Shown.\""
        );
        // Another extension and letter case.
        assert_eq!(
            shown(
                json!({"source": "Documents", "path": "boat/engine/Impeller.html"}),
                "show_document"
            ),
            "\"Shown.\""
        );
        let doc = UiAction::ShowDocument {
            source_id: "s1".into(),
            rel_path: "Boat/Engine/impeller.pdf".into(),
        };
        let folder = UiAction::OpenFolder {
            source_id: "s1".into(),
            rel_path: "Boat/Engine".into(),
        };
        assert_eq!(
            tools.take_actions(),
            [folder.clone(), folder.clone(), folder, doc.clone()]
        );
        assert!(
            shown(
                json!({"source": "Documents", "path": "Boat/nothing.pdf"}),
                "show_document"
            )
            .starts_with("Error")
        );

        // What the turn looked at: the documents read, else the last search.
        tools.call("search_documents", &json!({"query": "impeller"}));
        assert!(matches!(
            tools.take_suggestions()[..],
            [UiAction::Search { .. }]
        ));
        tools.call("search_documents", &json!({"query": "impeller"}));
        tools.call(
            "read_document",
            &json!({"path": "Boat/Engine/impeller.PDF"}),
        );
        assert_eq!(tools.take_suggestions(), [doc]);
        assert!(tools.take_suggestions().is_empty());
    }

    #[test]
    fn tools_list_and_show_saved_searches() {
        let (_dir, idx, sources) = library();
        let saved = crate::saved_search::save(
            &sources[0].root,
            serde_json::from_value(json!({"name": "Boat costs", "tags": ["Boat"]})).unwrap(),
        )
        .unwrap();
        let tools = LibraryTools::new(&idx, &sources);
        let call = |name: &str, input: Value| -> Value {
            serde_json::from_str(&tools.call(name, &input)).unwrap_or(Value::Null)
        };
        let listed = call("list_sources", json!({}));
        assert_eq!(listed["saved_searches"][0]["name"], "Boat costs");
        assert_eq!(listed["saved_searches"][0]["tags"][0], "Boat");
        call("show_saved_search", json!({"name": "boat"}));
        assert_eq!(
            tools.take_actions(),
            vec![UiAction::SavedSearch {
                source_id: sources[0].id.clone(),
                id: saved.id,
                name: "Boat costs".into(),
                text: String::new(),
                tags: vec!["Boat".into()],
                tag_mode: TagMode::Any,
                favorites_only: false,
            }]
        );
        assert!(
            tools
                .call("show_saved_search", &json!({"name": "taxes"}))
                .contains("Boat costs")
        );
    }

    #[test]
    fn tools_find_and_add_up_by_sender_amount_and_due_date() {
        let (_dir, idx, sources) = library();
        let tools = LibraryTools::new(&idx, &sources);
        let call = |name: &str, input: Value| -> Value {
            serde_json::from_str(&tools.call(name, &input)).unwrap()
        };
        let found = call("search_documents", json!({"from": "harbor marine supply"}));
        assert_eq!(found["results"][0]["amount"], "412.50 USD");
        assert_eq!(found["results"][0]["due"], "2025-04-13");
        let none = call("search_documents", json!({"due_from": "2025-05-01"}));
        assert!(none.as_array().is_some_and(Vec::is_empty), "{none}");
        let totals = call("total_amounts", json!({"tags": ["Boat"]}));
        assert_eq!(totals["totals"][0]["total"], "412.50");
        assert_eq!(totals["totals"][0]["currency"], "USD");
        assert_eq!(totals["documents_without_amount"], 0);
        let senders = call("list_senders", json!({"filter": "harb"}));
        assert_eq!(senders["senders"][0]["from"], "Harbor Marine Supply");
    }

    #[test]
    fn tools_search_read_and_request_ui() {
        let (_dir, idx, sources) = library();
        let tools = LibraryTools::new(&idx, &sources);
        let found: Value =
            serde_json::from_str(&tools.call("search_documents", &json!({"query": "impeller"})))
                .unwrap();
        assert_eq!(found["results"][0]["title"], "Impeller invoice");
        assert_eq!(found["results"][0]["source"], "Documents");

        let doc: Value = serde_json::from_str(&tools.call(
            "read_document",
            &json!({"source": "documents", "path": "Boat/Engine/impeller.pdf"}),
        ))
        .unwrap();
        assert!(doc["text"].as_str().unwrap().contains("$412.50"));

        assert!(
            tools
                .call("read_document", &json!({"source": "Nope", "path": "x"}))
                .starts_with("Error: No document")
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

    /// A search with every word of the question (a month, another
    /// language) finds what has fewer of them, or says what to try (#57).
    #[test]
    fn searches_again_with_fewer_words() {
        let (_dir, idx, sources) = library();
        let tools = LibraryTools::new(&idx, &sources);
        let search = |q: &str| -> Value {
            serde_json::from_str(&tools.call("search_documents", &json!({"query": q}))).unwrap()
        };
        let found = search("Harbor impeller abril");
        assert_eq!(found["results"][0]["title"], "Impeller invoice");
        assert!(
            found["note"]
                .as_str()
                .unwrap()
                .contains("«Harbor impeller»")
        );
        assert!(matches!(
            &tools.take_suggestions()[..],
            [UiAction::Search { text, .. }] if text == "Harbor impeller"
        ));
        // Not down to a month or a small word alone.
        for q in ["bomba abril", "bomba de agua"] {
            let none = search(q);
            assert_eq!(none["results"], json!([]));
            assert!(none["note"].as_str().unwrap().contains("in English"));
        }
        let all = search("impeller");
        assert_eq!(all["results"][0]["title"], "Impeller invoice");
        assert!(all["note"].as_str().unwrap().contains("read_document"));

        assert_eq!(fewer(3, 2), [vec![2], vec![1], vec![0]]);
        assert_eq!(fewer(3, 1), [vec![1, 2], vec![0, 2], vec![0, 1]]);
    }

    /// Amounts the model gave without reading them (#57).
    #[test]
    fn finds_numbers_the_model_never_saw() {
        let seen = "{\"text\":\"Laptop. Reference 324682. Date 2026-03-19. Total $1,429.00\"}";
        assert_eq!(unseen_numbers("La laptop costó $1,200.", seen), ["1,200"]);
        assert_eq!(unseen_numbers("It was $800, or $5.", seen), ["800", "5"]);
        for fine in [
            "La laptop costó 1,429.00 USD.",
            "Der Laptop kostete 1.429,00 USD.",
            "It cost $1,429, bought on 19/03/2026 (3 years of warranty).",
            "Comprada el 19 de marzo de 2026.",
        ] {
            assert!(unseen_numbers(fine, seen).is_empty(), "{fine}");
        }
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
                &Cancel::new(),
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
        assert_eq!(first["tools"].as_array().unwrap().len(), 11);
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
            .send(
                &backend,
                &tools,
                "show boat stuff",
                &Cancel::new(),
                &mut |_| {},
            )
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
        assert!(
            chat.send(&dead, &tools, "again", &Cancel::new(), &mut |_| {})
                .is_err()
        );
        assert_eq!(chat.history.len(), before);
    }

    #[test]
    fn stop_while_waiting_for_the_model() {
        let (_dir, idx, sources) = library();
        let tools = LibraryTools::new(&idx, &sources);
        // A model that never answers.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            let _held: Vec<_> = listener.incoming().collect();
        });
        let backend = Backend::OpenAiCompatible {
            base_url: url,
            model: "qwen".into(),
            api_key: None,
        };
        let cancel = Cancel::new();
        let stopper = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            stopper.cancel();
        });
        let started = std::time::Instant::now();
        let mut chat = Chat::new("sys");
        let result = chat.send(&backend, &tools, "hello?", &cancel, &mut |_| {});
        assert!(matches!(result, Err(Error::Cancelled)));
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(chat.is_empty());
    }

    #[test]
    fn stop_between_tool_calls() {
        let (_dir, idx, sources) = library();
        let tools = LibraryTools::new(&idx, &sources);
        let (url, seen) = mock(vec![
            json!({"choices": [{"message": {"role": "assistant", "content": "", "tool_calls": [
                {"id": "c1", "type": "function", "function": {"name": "search_documents", "arguments": "{\"query\":\"boat\"}"}},
                {"id": "c2", "type": "function", "function": {"name": "show_results", "arguments": "{\"tags\":[\"Boat\"]}"}}
            ]}}]}),
            json!({"choices": [{"message": {"role": "assistant", "content": "never sent"}}]}),
        ]);
        let backend = Backend::OpenAiCompatible {
            base_url: format!("{url}/v1"),
            model: "qwen".into(),
            api_key: None,
        };
        let cancel = Cancel::new();
        let mut chat = Chat::new("sys");
        let result = chat.send(&backend, &tools, "boat things", &cancel, &mut |_| {
            cancel.cancel()
        });
        assert!(matches!(result, Err(Error::Cancelled)));
        // The first tool ran; the second and the next round didn't.
        assert!(tools.take_actions().is_empty());
        assert_eq!(seen.lock().unwrap().len(), 1);
        assert!(chat.is_empty());
    }

    // MARK: Responses API (ChatGPT plan)

    /// A server-sent event as the Responses API streams it.
    fn sse(event: &Value) -> String {
        format!(
            "event: {}\ndata: {event}\n\n",
            event["type"].as_str().unwrap()
        )
    }

    /// Like `mock`, but each answer is a raw event-stream body. With
    /// `hang`, the last connection gets its headers and then nothing more.
    fn mock_sse(bodies: Vec<String>, hang: bool) -> (String, Seen) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            let count = bodies.len() + usize::from(hang);
            let mut bodies = bodies.into_iter();
            for stream in listener.incoming().take(count) {
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
                match bodies.next() {
                    Some(out) => {
                        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len()).unwrap();
                    }
                    None => {
                        let _ = write!(
                            stream,
                            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\n"
                        );
                        let _ = stream.flush();
                        std::thread::sleep(Duration::from_secs(30));
                    }
                }
            }
        });
        (url, seen)
    }

    #[test]
    fn reads_event_streams() {
        let done =
            json!({"type": "response.completed", "response": {"model": "gpt-x", "output": []}});
        let stream = format!(
            ": comment\n\n{}{}{}",
            sse(&json!({"type": "response.created", "response": {}})),
            sse(&json!({"type": "response.output_text.delta", "delta": "Hi"})),
            sse(&done)
        );
        let r = read_events(stream.as_bytes(), &Cancel::new()).unwrap();
        assert_eq!(r["model"], "gpt-x");

        let failed = sse(&json!({"type": "response.failed", "response": {"error":
            {"code": "subscription_sharing_usage_limit_exceeded", "message": "limit"}}}));
        let e = read_events(failed.as_bytes(), &Cancel::new()).unwrap_err();
        assert!(e.to_string().contains("limit for Rupert"), "{e}");

        let cut = sse(&json!({"type": "response.output_text.delta", "delta": "Hi"}));
        assert!(read_events(cut.as_bytes(), &Cancel::new()).is_err());
        assert!(
            plan_error(401, "{}").to_string().contains("sign in again"),
            "expired sign-in"
        );
    }

    #[test]
    fn chatgpt_plan_tool_loop() {
        let (_dir, idx, sources) = library();
        let tools = LibraryTools::new(&idx, &sources);
        let first = sse(
            &json!({"type": "response.completed", "response": {"model": "gpt-x",
            "output": [
                {"type": "reasoning", "id": "rs_1", "summary": [], "encrypted_content": "enc"},
                {"type": "function_call", "id": "fc_1", "status": "completed", "call_id": "call_1",
                 "name": "search_documents", "arguments": "{\"query\":\"impeller\"}"}
            ]}}),
        );
        let second = format!(
            "{}{}",
            sse(&json!({"type": "response.output_text.delta", "delta": "In Boat"})),
            sse(
                &json!({"type": "response.completed", "response": {"model": "gpt-x", "output": [
                    {"type": "message", "id": "msg_1", "role": "assistant", "content":
                        [{"type": "output_text", "text": "In Boat/Engine."}]}
                ]}})
            )
        );
        let (url, seen) = mock_sse(vec![first, second], false);
        let backend = Backend::ChatGptPlan {
            base_url: format!("{url}/v1"),
            model: "gpt-x".into(),
            access_token: "tok".into(),
        };
        let mut chat = Chat::new("sys");
        let mut progress = Vec::new();
        let reply = chat
            .send(
                &backend,
                &tools,
                "where is the impeller invoice?",
                &Cancel::new(),
                &mut |s| progress.push(s.tool.clone()),
            )
            .unwrap();
        assert_eq!(reply.text, "In Boat/Engine.");
        assert_eq!(reply.model, "gpt-x");
        assert_eq!(progress, ["search_documents"]);

        let seen = seen.lock().unwrap();
        let (head, first_req) = &seen[0];
        assert!(head.starts_with("POST /v1/responses"));
        assert!(
            head.to_ascii_lowercase()
                .contains("authorization: bearer tok")
        );
        assert_eq!(first_req["instructions"], "sys");
        assert_eq!(first_req["store"], false);
        assert_eq!(first_req["stream"], true);
        assert_eq!(first_req["tools"][0]["type"], "function");
        assert!(first_req.get("temperature").is_none());
        // The second request carries everything: the question, the
        // reasoning (with its id), the call (without its id) and the result.
        let input = seen[1].1["input"].as_array().unwrap();
        assert_eq!(
            input[0],
            json!({"role": "user", "content": "where is the impeller invoice?"})
        );
        assert_eq!(input[1]["id"], "rs_1");
        assert_eq!(input[2]["call_id"], "call_1");
        assert!(input[2].get("id").is_none() && input[2].get("status").is_none());
        assert_eq!(input[3]["type"], "function_call_output");
        assert!(
            input[3]["output"]
                .as_str()
                .unwrap()
                .contains("Impeller invoice")
        );
    }

    #[test]
    fn chatgpt_plan_stops_mid_stream() {
        let (_dir, idx, sources) = library();
        let tools = LibraryTools::new(&idx, &sources);
        let (url, _seen) = mock_sse(vec![], true);
        let backend = Backend::ChatGptPlan {
            base_url: url,
            model: "gpt-x".into(),
            access_token: "tok".into(),
        };
        let cancel = Cancel::new();
        let stopper = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            stopper.cancel();
        });
        let started = std::time::Instant::now();
        let mut chat = Chat::new("sys");
        let result = chat.send(&backend, &tools, "hello?", &cancel, &mut |_| {});
        assert!(matches!(result, Err(Error::Cancelled)));
        assert!(started.elapsed() < Duration::from_secs(3));
        assert!(chat.is_empty());
    }
}
