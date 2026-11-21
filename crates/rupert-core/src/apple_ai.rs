//! Apple Intelligence: Apple's on-device language model, through the Swift
//! bridge in `apple/RupertAI.swift` (built by `build.rs` on macOS).
//!
//! Available on macOS 26 or later, on a Mac with Apple silicon, with Apple
//! Intelligence turned on and its model downloaded. Everything runs on the
//! Mac: it is Local AI that needs no extra app. Elsewhere (Windows, Linux,
//! older macOS, Intel Macs, or a build without the framework) [`status`]
//! says why it isn't available.
//!
//! The model is small and its context is short (4,096 tokens on macOS 26,
//! 8,192 on macOS 27 on this Mac), so callers size what they send by
//! [`Status::context_size`].

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::{Error, Result};
use crate::i18n::{tr, trf};

// MARK: - Status

/// Why Apple Intelligence can't be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Unavailable {
    /// Not a Mac (or iPhone) that can run Apple Intelligence.
    DeviceNotEligible,
    /// Turned off in System Settings ▸ Apple Intelligence & Siri.
    AppleIntelligenceNotEnabled,
    /// Still downloading or preparing its model.
    ModelNotReady,
    /// Before macOS 26.
    SystemTooOld,
    /// Not on this platform, or Rupert was built without it.
    NotSupported,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub available: bool,
    /// Set when it isn't available.
    pub reason: Option<Unavailable>,
    /// Tokens the model can handle, prompt and answer together.
    pub context_size: usize,
    /// Languages the model supports ("en", "es", "de", …).
    pub languages: Vec<String>,
}

impl Unavailable {
    /// What to tell people.
    pub fn message(self) -> &'static str {
        match self {
            Self::DeviceNotEligible => tr("This Mac can't run Apple Intelligence."),
            Self::AppleIntelligenceNotEnabled => tr(
                "Apple Intelligence is turned off. Turn it on in System Settings ▸ Apple Intelligence & Siri.",
            ),
            Self::ModelNotReady => {
                tr("Apple Intelligence is still getting ready (downloading its model).")
            }
            Self::SystemTooOld => tr("Apple Intelligence needs macOS 26 or later."),
            Self::NotSupported | Self::Unknown => tr("Apple Intelligence isn't available here."),
        }
    }
}

impl Status {
    /// This Mac can run Apple Intelligence, even if it's still turned off
    /// or downloading its model (Settings then says what to do).
    pub fn capable(&self) -> bool {
        self.available
            || matches!(
                self.reason,
                Some(Unavailable::AppleIntelligenceNotEnabled | Unavailable::ModelNotReady)
            )
    }
}

/// Whether Apple Intelligence can be used now. Cheap: asks the system, not
/// the model.
pub fn status() -> Status {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Raw {
        available: bool,
        reason: Option<Unavailable>,
        #[serde(default)]
        context_size: usize,
        #[serde(default)]
        languages: Vec<String>,
    }
    let unavailable = |reason| Status {
        available: false,
        reason: Some(reason),
        context_size: 0,
        languages: Vec::new(),
    };
    let Some(raw) = bridge::status() else {
        return unavailable(Unavailable::NotSupported);
    };
    match serde_json::from_str::<Raw>(&raw) {
        Ok(r) if r.available => Status {
            available: true,
            reason: None,
            context_size: r.context_size,
            languages: r.languages,
        },
        Ok(r) => unavailable(r.reason.unwrap_or(Unavailable::Unknown)),
        Err(_) => unavailable(Unavailable::Unknown),
    }
}

// MARK: - Generating

/// What to ask the model.
pub struct Ask<'a> {
    pub instructions: &'a str,
    pub prompt: &'a str,
    /// JSON Schema for a structured answer (objects, strings with `enum`,
    /// numbers, booleans, arrays); the answer is then JSON text.
    pub schema: Option<&'a Value>,
    pub temperature: f64,
    pub max_tokens: u32,
}

/// Asks the model and waits for its answer. Blocks: call it off the main
/// thread.
pub fn generate(ask: &Ask) -> Result<String> {
    let status = status();
    if !status.available {
        let reason = status.reason.unwrap_or(Unavailable::Unknown);
        return Err(Error::InvalidValue(reason.message().into()));
    }
    let request = json!({
        "instructions": ask.instructions,
        "prompt": ask.prompt,
        "schema": ask.schema,
        "temperature": ask.temperature,
        "maxTokens": ask.max_tokens,
    });
    let raw = bridge::generate(&request.to_string())
        .ok_or_else(|| Error::InvalidValue(Unavailable::NotSupported.message().into()))?;
    answer(&raw)
}

/// A conversation turn where the model may use tools.
pub struct Turn<'a> {
    pub instructions: &'a str,
    pub prompt: &'a str,
    /// `{"name", "description", "schema"}` for each tool.
    pub tools: &'a [Value],
    pub temperature: f64,
    pub max_tokens: u32,
}

/// Runs a conversation turn: the model calls `on_tool(name, arguments)`
/// for each tool it uses (one at a time, possibly from another thread while
/// this call blocks) and then answers. Blocks: call it off the main thread.
pub fn chat(turn: &Turn, on_tool: &mut dyn FnMut(&str, &Value) -> String) -> Result<String> {
    let status = status();
    if !status.available {
        let reason = status.reason.unwrap_or(Unavailable::Unknown);
        return Err(Error::InvalidValue(reason.message().into()));
    }
    let request = json!({
        "instructions": turn.instructions,
        "prompt": turn.prompt,
        "tools": turn.tools,
        "temperature": turn.temperature,
        "maxTokens": turn.max_tokens,
    });
    let raw = bridge::chat(&request.to_string(), on_tool)
        .ok_or_else(|| Error::InvalidValue(Unavailable::NotSupported.message().into()))?;
    answer(&raw)
}

/// The bridge's `{"content"}` or `{"error", "message"}`.
fn answer(raw: &str) -> Result<String> {
    #[derive(Deserialize)]
    struct Reply {
        content: Option<String>,
        error: Option<String>,
        message: Option<String>,
    }
    let reply: Reply = serde_json::from_str(raw)
        .map_err(|e| Error::InvalidValue(trf("Apple Intelligence couldn't answer: {0}", &[&e])))?;
    if let Some(content) = reply.content {
        return Ok(content);
    }
    let message = match reply.error.as_deref() {
        Some("contextWindow") => tr("The document is too long for Apple Intelligence.").into(),
        Some("guardrail" | "refusal") => {
            tr("Apple Intelligence declined to answer for this document.").into()
        }
        Some("unsupportedLanguage") => {
            tr("Apple Intelligence doesn't support this document's language.").into()
        }
        Some("assetsUnavailable") => Unavailable::ModelNotReady.message().into(),
        Some("busy" | "rateLimited" | "concurrentRequests") => {
            tr("Apple Intelligence is busy. Try again in a moment.").into()
        }
        _ => trf(
            "Apple Intelligence couldn't answer: {0}",
            &[&reply.message.unwrap_or_default()],
        ),
    };
    Err(Error::InvalidValue(message))
}

// MARK: - Bridge

#[cfg(apple_ai)]
mod bridge {
    use std::ffi::{CStr, CString, c_char, c_void};

    use serde_json::{Value, json};

    type ToolFn = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> *mut c_char;

    unsafe extern "C" {
        fn rupert_ai_status() -> *mut c_char;
        fn rupert_ai_generate(request: *const c_char) -> *mut c_char;
        fn rupert_ai_chat(
            request: *const c_char,
            context: *mut c_void,
            callback: ToolFn,
        ) -> *mut c_char;
        fn rupert_ai_free(pointer: *mut c_char);
        fn strdup(s: *const c_char) -> *mut c_char;
    }

    type OnTool<'a> = &'a mut dyn FnMut(&str, &Value) -> String;

    /// Called by the bridge for each tool call; answers with a malloc'd
    /// string the bridge frees.
    unsafe extern "C" fn tool_call(
        context: *mut c_void,
        name: *const c_char,
        arguments: *const c_char,
    ) -> *mut c_char {
        let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // SAFETY: `context` is the `OnTool` that `chat` passed, alive and
            // otherwise unused while `rupert_ai_chat` blocks; the bridge
            // makes these calls one at a time, so access is exclusive.
            let on_tool = unsafe { &mut *context.cast::<OnTool>() };
            // SAFETY: NUL-terminated strings valid for this call.
            let name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
            let arguments = unsafe { CStr::from_ptr(arguments) }.to_string_lossy();
            let arguments = serde_json::from_str(&arguments).unwrap_or(json!({}));
            on_tool(&name, &arguments)
        }))
        .unwrap_or_else(|_| "The tool failed.".to_string());
        let out = CString::new(out.replace('\0', "")).unwrap_or_default();
        // SAFETY: copies into malloc'd memory, which the bridge frees.
        unsafe { strdup(out.as_ptr()) }
    }

    pub fn chat(request: &str, on_tool: OnTool) -> Option<String> {
        let request = CString::new(request).ok()?;
        let mut on_tool = on_tool;
        let context = (&raw mut on_tool).cast::<c_void>();
        // SAFETY: `request` and `on_tool` outlive the call, which blocks
        // until the model is done; the result is owned (`take`).
        take(unsafe { rupert_ai_chat(request.as_ptr(), context, tool_call) })
    }

    /// Takes ownership of a string the bridge allocated.
    fn take(pointer: *mut c_char) -> Option<String> {
        if pointer.is_null() {
            return None;
        }
        // SAFETY: the bridge returns a NUL-terminated string from strdup,
        // which we read once and free with its own free function.
        let text = unsafe { CStr::from_ptr(pointer) }
            .to_string_lossy()
            .into_owned();
        unsafe { rupert_ai_free(pointer) };
        Some(text)
    }

    pub fn status() -> Option<String> {
        // SAFETY: no arguments; returns an owned string (see `take`).
        take(unsafe { rupert_ai_status() })
    }

    pub fn generate(request: &str) -> Option<String> {
        let request = CString::new(request).ok()?;
        // SAFETY: `request` outlives the call; the result is owned (`take`).
        take(unsafe { rupert_ai_generate(request.as_ptr()) })
    }
}

#[cfg(not(apple_ai))]
mod bridge {
    use serde_json::Value;

    pub fn chat(
        _request: &str,
        _on_tool: &mut dyn FnMut(&str, &Value) -> String,
    ) -> Option<String> {
        None
    }

    pub fn status() -> Option<String> {
        None
    }

    pub fn generate(_request: &str) -> Option<String> {
        None
    }
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_is_consistent() {
        let s = status();
        assert_eq!(s.available, s.reason.is_none(), "{s:?}");
        if s.available {
            assert!(s.context_size >= 4096, "{s:?}");
        }
    }

    /// Runs only where Apple Intelligence is available.
    #[test]
    fn answers_with_json_matching_a_schema() {
        if !status().available {
            return;
        }
        let schema = json!({
            "type": "object",
            "properties": {
                "color": {"type": "string", "enum": ["red", "green", "blue"]},
                "count": {"type": "integer"},
            },
            "required": ["color", "count"],
        });
        let answer = generate(&Ask {
            instructions: "Answer briefly.",
            prompt: "The sky on a clear day, and how many suns we have.",
            schema: Some(&schema),
            temperature: 0.0,
            max_tokens: 200,
        })
        .unwrap();
        let v: Value = serde_json::from_str(&answer).unwrap();
        assert!(
            ["red", "green", "blue"].contains(&v["color"].as_str().unwrap()),
            "{v}"
        );
        assert!(v["count"].is_i64(), "{v}");
    }

    /// Runs only where Apple Intelligence is available.
    #[test]
    fn uses_a_tool_and_answers_from_it() {
        if !status().available {
            return;
        }
        let tools = [json!({
            "name": "lookup_locker",
            "description": "Looks up the code of a locker by its number.",
            "schema": {
                "type": "object",
                "properties": {"number": {"type": "integer"}},
                "required": ["number"],
            },
        })];
        let mut calls = Vec::new();
        let answer = chat(
            &Turn {
                instructions: "Use the tools to answer. Answer in one sentence.",
                prompt: "What's the code of locker 17?",
                tools: &tools,
                temperature: 0.0,
                max_tokens: 200,
            },
            &mut |name, args| {
                calls.push((name.to_string(), args.clone()));
                "Locker 17's code is 4931.".to_string()
            },
        )
        .unwrap();
        assert_eq!(calls.len(), 1, "{calls:?}");
        assert_eq!(calls[0].0, "lookup_locker");
        assert_eq!(calls[0].1["number"], 17);
        assert!(answer.contains("4931"), "{answer}");
    }
}
