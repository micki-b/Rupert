//! Filing suggestions from Local AI: a model on an OpenAI-compatible server
//! on this computer or the local network (LM Studio at
//! `http://localhost:1234` by default; Ollama, llama.cpp's `llama-server`
//! and others speak the same API). Nothing goes to the internet: an address
//! elsewhere is refused ([`reach`]). If the server isn't running,
//! suggestions are simply unavailable.
//!
//! The model gets the document's text (the start of it), its file name,
//! the folders where similar documents already live, and the tags already
//! in use, and answers with JSON matching a schema (the server enforces it).

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::apple_ai;
use crate::error::{Error, Result};
use crate::i18n::{Lang, tr, trf};
use crate::meta::{is_iso_date, parse_amount, parse_currency};

/// LM Studio's server, the default.
pub const DEFAULT_URL: &str = "http://localhost:1234";
/// Characters of document text sent to the model.
const MAX_TEXT: usize = 6000;

// MARK: - Types

/// What the model is told about a document.
#[derive(Debug, Clone, Default)]
pub struct Request<'a> {
    pub text: &'a str,
    pub file_name: &'a str,
    /// The current year; helps with dates printed without one. (Not the
    /// full date: models tend to copy it as the document date.)
    pub year: &'a str,
    /// Existing folders to choose from, as shown to the user ("Source/Dir").
    pub folders: &'a [String],
    pub tags: &'a [String],
}

/// What the model suggests. Everything is optional; the user reviews it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub title: Option<String>,
    pub document_date: Option<String>,
    pub tags: Vec<String>,
    /// One of the folders offered, exactly as given.
    pub folder: Option<String>,
    /// The model's one-line reason, shown in the review.
    pub reason: Option<String>,
    /// Who sent or issued it, the amount on it with its currency, and the
    /// date to pay by or valid until (#108). Each one only if it can be
    /// found in the document's text.
    pub sender: Option<String>,
    pub amount: Option<String>,
    pub currency: Option<String>,
    pub due_date: Option<String>,
    /// The model that made it.
    pub model: String,
}

// MARK: - Where the server is

/// Where a Local AI address points, judged from the address alone (no DNS
/// lookup, so the answer is predictable and nothing is sent anywhere).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Reach {
    /// `localhost` or a loopback address.
    ThisComputer,
    /// A private, link-local or shared (e.g. Tailscale) address, or a name
    /// only the local network knows (`studio`, `mac-mini.local`, `*.lan`).
    LocalNetwork,
    /// Anywhere else: treated like a cloud service.
    Elsewhere,
}

/// Where `url` points. Unparseable addresses count as elsewhere.
pub fn reach(url: &str) -> Reach {
    use std::net::IpAddr;
    let Some(host) = host(url) else {
        return Reach::Elsewhere;
    };
    if let Ok(ip) = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .parse::<IpAddr>()
    {
        return match ip {
            IpAddr::V4(v4) if v4.is_loopback() => Reach::ThisComputer,
            IpAddr::V4(v4)
                if v4.is_private()
                    || v4.is_link_local()
                    // 100.64.0.0/10, shared address space (Tailscale).
                    || (v4.octets()[0] == 100 && (v4.octets()[1] & 0xc0) == 64) =>
            {
                Reach::LocalNetwork
            }
            IpAddr::V6(v6) if v6.is_loopback() => Reach::ThisComputer,
            IpAddr::V6(v6)
                if (v6.segments()[0] & 0xfe00) == 0xfc00 // unique local
                    || (v6.segments()[0] & 0xffc0) == 0xfe80 =>
            // link-local
            {
                Reach::LocalNetwork
            }
            _ => Reach::Elsewhere,
        };
    }
    let host = host.to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") {
        Reach::ThisComputer
    } else if !host.contains('.')
        || [".local", ".lan", ".home.arpa", ".internal"]
            .iter()
            .any(|s| host.ends_with(s))
    {
        Reach::LocalNetwork
    } else {
        Reach::Elsewhere
    }
}

/// The host part of an address (`http://host:port/path` → `host`).
pub fn host(url: &str) -> Option<&str> {
    let rest = url.trim().split_once("://").map_or(url.trim(), |(_, r)| r);
    let authority = rest.split(['/', '?', '#']).next()?;
    let authority = authority.rsplit_once('@').map_or(authority, |(_, a)| a);
    let host = if authority.starts_with('[') {
        &authority[..=authority.find(']')?]
    } else {
        authority.split(':').next()?
    };
    (!host.is_empty()).then_some(host)
}

// MARK: - Client

#[derive(Debug, Clone)]
pub struct LocalAi {
    pub url: String,
    /// `None` = the first chat model the server lists (LM Studio lists the
    /// loaded one first; Ollama lists every installed model).
    pub model: Option<String>,
}

impl LocalAi {
    fn agent(timeout: Duration) -> ureq::Agent {
        ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .build()
            .into()
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/v1/{path}", self.url.trim_end_matches('/'))
    }

    /// Chat models the server offers (embedding models left out). Fails fast
    /// when it isn't running.
    pub fn models(&self) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        struct List {
            data: Vec<Model>,
        }
        #[derive(Deserialize)]
        struct Model {
            id: String,
        }
        let list: List = Self::agent(Duration::from_secs(3))
            .get(&self.endpoint("models"))
            .call()
            .map_err(unreachable)?
            .body_mut()
            .read_json()
            .map_err(unreachable)?;
        Ok(list
            .data
            .into_iter()
            .map(|m| m.id)
            .filter(|id| !id.contains("embed"))
            .collect())
    }

    /// Asks the model for a title, date, tags and folder.
    pub fn suggest(&self, req: &Request) -> Result<Suggestion> {
        let user = user_prompt(req, MAX_TEXT, crate::i18n::current());
        let (content, model) = self.chat_json(SYSTEM_PROMPT, &user, "filing", &schema(), 8000)?;
        parse(&content, req, model)
    }

    /// Sends one request and returns the answer's JSON text (as the model
    /// wrote it) and the model's name. The server enforces `schema`.
    pub fn chat_json(
        &self,
        system: &str,
        user: &str,
        name: &str,
        schema: &serde_json::Value,
        max_tokens: u32,
    ) -> Result<(String, String)> {
        let model = match &self.model {
            Some(m) => m.clone(),
            None => self.models()?.into_iter().next().ok_or_else(|| {
                Error::InvalidValue(tr("The Local AI server has no chat model").into())
            })?,
        };
        let body = json!({
            "model": model,
            "temperature": 0.1,
            // Room for thinking models, which can spend thousands of
            // tokens before the answer.
            "max_tokens": max_tokens,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": {"name": name, "strict": true, "schema": schema},
            },
        });
        #[derive(Deserialize)]
        struct Reply {
            choices: Vec<Choice>,
        }
        #[derive(Deserialize)]
        struct Choice {
            message: Message,
            finish_reason: Option<String>,
        }
        #[derive(Deserialize)]
        struct Message {
            content: Option<String>,
            // Reasoning models: the server may put the (schema-constrained)
            // answer here and leave `content` empty.
            reasoning_content: Option<String>,
            reasoning: Option<String>,
        }
        let reply: Reply = Self::agent(Duration::from_secs(180))
            .post(&self.endpoint("chat/completions"))
            .send_json(&body)
            .map_err(unreachable)?
            .body_mut()
            .read_json()
            .map_err(|e| {
                Error::InvalidValue(trf("The Local AI server answered oddly: {0}", &[&e]))
            })?;
        let choice = reply.choices.into_iter().next();
        let ran_out = choice
            .as_ref()
            .is_some_and(|c| c.finish_reason.as_deref() == Some("length"));
        let content = choice
            .map(|c| c.message)
            .map(|m| {
                [m.content, m.reasoning_content, m.reasoning]
                    .into_iter()
                    .flatten()
                    .find(|t| t.contains('{'))
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        if ran_out && !content.contains('{') {
            return Err(Error::InvalidValue(
                tr("The model ran out of room before answering (thinking models can). Choose a model that doesn't think, or thinks less.").into(),
            ));
        }
        Ok((content, model))
    }
}

// MARK: - Which Local AI

/// The Local AI the settings choose: Apple Intelligence or a server.
#[derive(Debug, Clone)]
pub enum Engine {
    Apple,
    Server(LocalAi),
}

impl Engine {
    /// The Local AI chosen in Settings, if it can be used: Apple
    /// Intelligence on a Mac that runs it, or a server on this computer or
    /// the local network (never one elsewhere). Whether the server is
    /// running isn't checked here.
    pub fn from_settings(settings: &crate::settings::Settings) -> Option<Self> {
        match settings.local_engine {
            crate::settings::LocalEngine::Apple => {
                apple_ai::status().capable().then_some(Self::Apple)
            }
            crate::settings::LocalEngine::Server => {
                (reach(&settings.local_url) != Reach::Elsewhere).then(|| {
                    Self::Server(LocalAi {
                        url: settings.local_url.clone(),
                        model: settings.local_model.clone(),
                    })
                })
            }
        }
    }

    /// Characters of document text that fit in a request.
    pub fn max_text(&self, server: usize) -> usize {
        match self {
            Self::Apple if apple_ai::status().context_size < 8192 => server.min(2000),
            _ => server,
        }
    }

    /// One request answered with JSON matching `schema`: the answer's text.
    pub fn json(
        &self,
        system: &str,
        user: &str,
        name: &str,
        schema: &serde_json::Value,
        max_tokens: u32,
    ) -> Result<String> {
        match self {
            Self::Apple => apple_ai::generate(&apple_ai::Ask {
                instructions: system,
                prompt: user,
                schema: Some(schema),
                temperature: 0.1,
                max_tokens,
            }),
            Self::Server(ai) => ai
                .chat_json(system, user, name, schema, max_tokens.max(8000))
                .map(|(content, _)| content),
        }
    }
}

// MARK: - Apple Intelligence

/// The model name recorded with suggestions from Apple Intelligence.
pub const APPLE_MODEL: &str = "Apple Intelligence";

/// Suggestions from Apple Intelligence, Apple's on-device model (macOS 26
/// or later; see [`apple_ai`]). The same prompt and checks as [`LocalAi`],
/// sized for its shorter context, with the folder chosen from the list
/// rather than copied (easier for a small model).
pub struct AppleAi;

impl AppleAi {
    pub fn suggest(&self, req: &Request) -> Result<Suggestion> {
        // About 4 characters a token; leave room for the instructions,
        // folders, tags and the answer.
        let max_text = if apple_ai::status().context_size >= 8192 {
            MAX_TEXT
        } else {
            2500
        };
        let mut schema = schema();
        let mut folders: Vec<&str> = req.folders.iter().map(String::as_str).collect();
        folders.push("");
        schema["properties"]["folder"] = json!({"type": "string", "enum": folders});
        // Apple's framework shows the model these descriptions; without
        // them it fills every tag slot and dates notes from the file name.
        schema["properties"]["tags"]["description"] = json!(
            "Only tags that clearly describe this document; often just one or two. Never pad the list."
        );
        schema["properties"]["document_date"]["description"] = json!(
            "A date printed in the document text, as YYYY-MM-DD. \"\" if the text shows none. Never the file name's date."
        );
        schema["properties"]["amount"]["description"] = json!(
            "The total to pay or paid, copied from the document text, e.g. \"1,240.50\". \"\" if the text shows none."
        );
        schema["properties"]["due_date"]["description"] = json!(
            "The date to pay by, or valid until, printed in the document text, as YYYY-MM-DD. \"\" if none."
        );
        let content = apple_ai::generate(&apple_ai::Ask {
            instructions: SYSTEM_PROMPT,
            prompt: &user_prompt(req, max_text, crate::i18n::current()),
            schema: Some(&schema),
            temperature: 0.1,
            max_tokens: 600,
        })?;
        parse(&content, req, APPLE_MODEL.into())
    }
}

fn unreachable(e: impl std::fmt::Display) -> Error {
    Error::InvalidValue(trf("The Local AI server isn't reachable ({0})", &[&e]))
}

// MARK: - Prompt

const SYSTEM_PROMPT: &str = "You are a careful archivist filing scanned personal documents \
(bills, statements, letters, receipts, forms, manuals). You get a document's text, which may \
contain OCR errors, and answer only with JSON.";

/// The request for one document. The reason is shown to the user, so it's
/// asked for in `lang`, Rupert's language; the title stays in the document's
/// language, since it becomes the file name.
fn user_prompt(req: &Request, max_text: usize, lang: Lang) -> String {
    let text: String = req.text.chars().take(max_text).collect();
    let folders = if req.folders.is_empty() {
        "(none known)".to_string()
    } else {
        req.folders
            .iter()
            .map(|f| format!("- {f}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let tags = if req.tags.is_empty() {
        "(none yet)".to_string()
    } else {
        req.tags.join(", ")
    };
    format!(
        "The current year is {year}. File name: {name}\n\n\
         Suggest how to file this document:\n\
         - document_language: the language the document text is written in, e.g. \"English\".\n\
         - title: short and specific, like a person would name the file: who it's from and what \
           it is, e.g. \"City Electric bill\" or \"Acme auto policy cancellation\". At most 60 \
           characters. No date in the title. The whole title in document_language: never \
           translated, never mixing languages.\n\
         - document_date: the date the document was issued (statement, invoice, letter or \
           receipt date) as YYYY-MM-DD, or \"\" if the document shows none. Only use a date \
           printed on the document. Not a due date or a date of birth.\n\
         - tags: 0 to 3 short tags for the kind of document or its subject (e.g. \"Insurance\", \
           \"Tax\", \"Medical\"). Reuse a tag in use only if it clearly fits; never add one just \
           because it exists.\n\
         - sender: who sent or issued it (a company, office or person), a short name as printed, \
           e.g. \"City Electric\", or \"\" if unclear.\n\
         - amount: the total to pay or that was paid (invoice or receipt total, balance due), \
           copied as printed, e.g. \"1,240.50\", or \"\" if there's none. Never computed or guessed.\n\
         - currency: the amount's currency as a code like \"EUR\" or \"USD\", or \"\".\n\
         - due_date: the date to pay by, or the date it's valid until (expiry), as YYYY-MM-DD, \
           or \"\" if the document shows none.\n\
         - folder: the folder it belongs in, copied exactly from the list, or \"\" if none fits.\n\
         - reason: one short sentence on why that folder, written in {language}. Only the \
           reason is in {language}.\n\n\
         Folders where similar documents are filed:\n{folders}\n\n\
         Tags in use: {tags}\n\n\
         Document text:\n\"\"\"\n{text}\n\"\"\"",
        year = req.year,
        name = req.file_name,
        language = lang.english_name(),
    )
}

fn schema() -> serde_json::Value {
    json!({
        "type": "object",
        // document_language comes first: a model that has named the
        // document's language keeps the title in it (#58), even when the
        // reason is asked for in another one. It isn't used otherwise.
        "properties": {
            "document_language": {"type": "string"},
            "title": {"type": "string"},
            "document_date": {"type": "string"},
            "tags": {"type": "array", "items": {"type": "string"}},
            "sender": {"type": "string"},
            "amount": {"type": "string"},
            "currency": {"type": "string"},
            "due_date": {"type": "string"},
            "folder": {"type": "string"},
            "reason": {"type": "string"},
        },
        "required": [
            "document_language", "title", "document_date", "tags", "sender", "amount",
            "currency", "due_date", "folder", "reason"
        ],
        "additionalProperties": false,
    })
}

/// Turns the model's JSON into a suggestion, dropping anything malformed:
/// bad dates, folders not offered, empty or overlong values.
fn parse(content: &str, req: &Request, model: String) -> Result<Suggestion> {
    #[derive(Deserialize)]
    struct Raw {
        #[serde(default)]
        title: String,
        #[serde(default)]
        document_date: String,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        folder: String,
        #[serde(default)]
        reason: String,
        #[serde(default)]
        sender: String,
        #[serde(default)]
        amount: String,
        #[serde(default)]
        currency: String,
        #[serde(default)]
        due_date: String,
    }
    // Some models wrap JSON in prose or code fences; take the object.
    let start = content.find('{');
    let end = content.rfind('}');
    let json = match (start, end) {
        (Some(s), Some(e)) if e > s => &content[s..=e],
        _ => {
            return Err(Error::InvalidValue(
                tr("the model didn't answer with JSON").into(),
            ));
        }
    };
    let raw: Raw = serde_json::from_str(json)
        .map_err(|e| Error::InvalidValue(trf("the model's answer didn't parse: {0}", &[&e])))?;
    let clean = |s: String, max: usize| {
        let s = s.trim().trim_matches('"').trim().to_string();
        (!s.is_empty()).then(|| s.chars().take(max).collect::<String>())
    };
    let mut tags: Vec<String> = Vec::new();
    for t in raw.tags {
        if let Some(t) = clean(t, 40) {
            // Prefer the existing spelling of a tag.
            let t = req
                .tags
                .iter()
                .find(|e| e.eq_ignore_ascii_case(&t))
                .cloned()
                .unwrap_or(t);
            if !tags.iter().any(|x| x.eq_ignore_ascii_case(&t)) && tags.len() < 4 {
                tags.push(t);
            }
        }
    }
    let details = checked(
        Details {
            sender: clean(raw.sender, 80),
            amount: clean(raw.amount, 40),
            currency: clean(raw.currency, 10),
            due_date: clean(raw.due_date, 10),
        },
        req.text,
    );
    Ok(Suggestion {
        title: clean(raw.title, 80),
        document_date: clean(raw.document_date, 10).filter(|d| is_iso_date(d)),
        tags,
        folder: clean(raw.folder, 400).filter(|f| req.folders.contains(f)),
        reason: clean(raw.reason, 300),
        model,
        sender: details.sender,
        amount: details.amount,
        currency: details.currency,
        due_date: details.due_date,
    })
}

// MARK: - From, amount and due date (#108)

/// Who a document is from, its amount and its due date: suggested with the
/// inbox's other fields, or read for documents already in the library.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Details {
    pub sender: Option<String>,
    pub amount: Option<String>,
    pub currency: Option<String>,
    pub due_date: Option<String>,
}

/// Keeps only what `text` shows. Small models invent amounts and dates
/// (#57), so each is looked for in the document before it's kept; the
/// currency only goes with a kept amount.
fn checked(d: Details, text: &str) -> Details {
    let amount = d
        .amount
        .and_then(|a| parse_amount(&a))
        .filter(|a| amount_in_text(a, text));
    Details {
        sender: d.sender.filter(|s| sender_in_text(s, text)),
        currency: amount
            .is_some()
            .then(|| d.currency.and_then(|c| parse_currency(&c)))
            .flatten(),
        amount,
        due_date: d
            .due_date
            .filter(|x| is_iso_date(x) && date_in_text(x, text)),
    }
}

/// Reads who a document is from, its amount and due date from its text
/// (#108: for documents already in the library). One request; nothing but
/// what the text shows is kept.
pub fn read_details(engine: &Engine, text: &str, file_name: &str) -> Result<Details> {
    let text: String = text.chars().take(engine.max_text(MAX_TEXT)).collect();
    let user = format!(
        "File name: {file_name}\n\n\
         Read these details from the document:\n\
         - sender: who sent or issued it (a company, office or person), a short name as printed, \
           e.g. \"City Electric\", or \"\" if unclear.\n\
         - amount: the total to pay or that was paid (invoice or receipt total, balance due), \
           copied as printed, e.g. \"1,240.50\", or \"\" if there's none. Never computed or guessed.\n\
         - currency: the amount's currency as a code like \"EUR\" or \"USD\", or \"\".\n\
         - due_date: the date to pay by, or the date it's valid until (expiry), as YYYY-MM-DD, \
           or \"\" if the document shows none.\n\n\
         Document text:\n\"\"\"\n{text}\n\"\"\""
    );
    let schema = json!({
        "type": "object",
        "properties": {
            "sender": {"type": "string"},
            "amount": {"type": "string"},
            "currency": {"type": "string"},
            "due_date": {"type": "string"},
        },
        "required": ["sender", "amount", "currency", "due_date"],
        "additionalProperties": false,
    });
    let content = engine.json(SYSTEM_PROMPT, &user, "details", &schema, 300)?;
    let json = match (content.find('{'), content.rfind('}')) {
        (Some(s), Some(e)) if e > s => &content[s..=e],
        _ => {
            return Err(Error::InvalidValue(
                tr("the model didn't answer with JSON").into(),
            ));
        }
    };
    #[derive(Deserialize)]
    struct Raw {
        #[serde(default)]
        sender: String,
        #[serde(default)]
        amount: String,
        #[serde(default)]
        currency: String,
        #[serde(default)]
        due_date: String,
    }
    let raw: Raw = serde_json::from_str(json)
        .map_err(|e| Error::InvalidValue(trf("the model's answer didn't parse: {0}", &[&e])))?;
    let clean = |s: String, max: usize| {
        let s = s.trim().trim_matches('"').trim().to_string();
        (!s.is_empty()).then(|| s.chars().take(max).collect::<String>())
    };
    Ok(checked(
        Details {
            sender: clean(raw.sender, 80),
            amount: clean(raw.amount, 40),
            currency: clean(raw.currency, 10),
            due_date: clean(raw.due_date, 10),
        },
        &text,
    ))
}

// MARK: - Checking against the text (#108)

/// Whether `amount` ("1240.50") is one of the numbers in `text`, written
/// any common way ("1,240.50", "1.240,50", "1240,5").
pub fn amount_in_text(amount: &str, text: &str) -> bool {
    let want = amount.trim_start_matches('-');
    let same = |found: &str| {
        let found = found.trim_start_matches('-');
        found == want
            || match (found.split_once('.'), want.split_once('.')) {
                // "1240.5" and "1240.50" are the same amount.
                (Some((a, x)), Some((b, y))) => {
                    a == b && x.trim_end_matches('0') == y.trim_end_matches('0')
                }
                (Some((a, x)), None) | (None, Some((a, x))) => {
                    x.trim_end_matches('0').is_empty() && (a == want || a == found)
                }
                _ => false,
            }
    };
    // Numbers in the text: digits with the separators between them.
    let mut current = String::new();
    let check = |run: &mut String| {
        let t = run.trim_end_matches(['.', ',', ' ']).to_string();
        run.clear();
        !t.is_empty() && crate::meta::parse_amount(&t).is_some_and(|a| same(&a))
    };
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        let joins = matches!(c, '.' | ',' | '\'' | '\u{a0}' | '\u{202f}')
            || (c == ' '
                && i > 0
                && chars[i - 1].is_ascii_digit()
                && chars
                    .get(i + 1..i + 4)
                    .is_some_and(|n| n.iter().all(char::is_ascii_digit))
                && chars.get(i + 4).is_none_or(|n| !n.is_ascii_digit()));
        if c.is_ascii_digit() || (joins && !current.is_empty()) {
            current.push(c);
        } else if check(&mut current) {
            return true;
        }
    }
    check(&mut current)
}

/// Whether the date `iso` ("2026-12-01") is printed in `text`: its year,
/// day and month close together, the month as a number or a name (English,
/// Spanish or German, also abbreviated).
pub fn date_in_text(iso: &str, text: &str) -> bool {
    let (Ok(year), Ok(month), Ok(day)) = (
        iso[..4].parse::<u32>(),
        iso[5..7].parse::<u32>(),
        iso[8..10].parse::<u32>(),
    ) else {
        return false;
    };
    const MONTHS: [&[&str]; 12] = [
        &["jan", "ene", "jän"],
        &["feb"],
        &["mar", "mär"],
        &["apr", "abr"],
        &["may", "mai"],
        &["jun"],
        &["jul"],
        &["aug", "ago"],
        &["sep", "set"],
        &["oct", "okt"],
        &["nov"],
        &["dec", "dic", "dez"],
    ];
    let lower = text.to_lowercase();
    // Tokens: numbers and words, with their positions.
    let mut tokens: Vec<(usize, String)> = Vec::new();
    let mut start = None;
    for (i, c) in lower
        .char_indices()
        .chain(std::iter::once((lower.len(), ' ')))
    {
        let part = c.is_alphanumeric();
        match (start, part) {
            (None, true) => start = Some(i),
            (Some(s), false) => {
                tokens.push((s, lower[s..i].to_string()));
                start = None;
            }
            _ => {}
        }
    }
    let is_num =
        |t: &str, n: u32| t.chars().all(|c| c.is_ascii_digit()) && t.parse::<u32>() == Ok(n);
    let is_month = |t: &str| {
        is_num(t, month)
            || (t.len() >= 3 && MONTHS[month as usize - 1].iter().any(|m| t.starts_with(m)))
    };
    for (k, (_, t)) in tokens.iter().enumerate() {
        let year_here = is_num(t, year) || (t.len() == 2 && is_num(t, year % 100));
        if !year_here {
            continue;
        }
        // The day and month within three tokens of the year, either side.
        let near = &tokens[k.saturating_sub(3)..(k + 4).min(tokens.len())];
        let has_day = near.iter().any(|(_, x)| {
            is_num(x, day)
                || x == &format!("{day}st")
                || x == &format!("{day}nd")
                || x == &format!("{day}rd")
                || x == &format!("{day}th")
        });
        let has_month = near.iter().any(|(_, x)| is_month(x));
        // Day and month are both numbers below 13: either order counts.
        if has_day && has_month {
            return true;
        }
    }
    false
}

/// Whether the sender's name shows in `text`: one of its words of three or
/// more letters (ignoring case), so "Allianz Versicherungs-AG" counts for
/// "Allianz".
pub fn sender_in_text(sender: &str, text: &str) -> bool {
    let text = text.to_lowercase();
    sender
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3)
        .any(|w| text.contains(&w.to_lowercase()))
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amounts_dates_and_senders_must_be_in_the_text() {
        let text = "Allianz Versicherungs-AG\nRechnung vom 03.11.2026\nGesamtbetrag 1.240,50 EUR\n\
                    Zahlbar bis 1. Dezember 2026. Invoice total $ 89.00, due Dec 15, 2026. \
                    Card 4 500 012";
        assert!(amount_in_text("1240.50", text));
        assert!(amount_in_text("1240.5", text));
        assert!(amount_in_text("89", text));
        assert!(!amount_in_text("1240", text));
        assert!(!amount_in_text("999.99", text));
        assert!(date_in_text("2026-12-01", text));
        assert!(date_in_text("2026-12-15", text));
        assert!(date_in_text("2026-11-03", text));
        assert!(!date_in_text("2026-12-02", text));
        assert!(!date_in_text("2027-12-01", text));
        assert!(sender_in_text("Allianz", text));
        assert!(!sender_in_text("Acme Insurance", text));
    }

    #[test]
    fn parses_and_cleans_answers() {
        let folders = vec!["Documents/Utilities/City Electric".to_string()];
        let tags = vec!["Utilities".to_string()];
        let req = Request {
            folders: &folders,
            tags: &tags,
            ..Default::default()
        };
        let answer = "```json\n{\"title\": \" City Electric bill \", \"document_date\": \"2026-10-03\", \
                      \"tags\": [\"utilities\", \"Electric\", \"\", \"Electric\"], \
                      \"folder\": \"Documents/Utilities/City Electric\", \"reason\": \"Same utility\"}\n```";
        let s = parse(answer, &req, "m".into()).unwrap();
        assert_eq!(s.title.as_deref(), Some("City Electric bill"));
        assert_eq!(s.document_date.as_deref(), Some("2026-10-03"));
        assert_eq!(s.tags, ["Utilities", "Electric"]);
        assert_eq!(
            s.folder.as_deref(),
            Some("Documents/Utilities/City Electric")
        );

        let odd = "{\"title\": \"\", \"document_date\": \"10/03/2026\", \"tags\": [], \
                   \"folder\": \"Somewhere/Else\", \"reason\": \"\"}";
        let s = parse(odd, &req, "m".into()).unwrap();
        assert_eq!((s.title, s.document_date, s.folder), (None, None, None));
        assert!(parse("no idea", &req, "m".into()).is_err());
    }

    #[test]
    fn asks_for_the_reason_in_ruperts_language() {
        let req = Request::default();
        for (lang, want) in [
            (Lang::En, "written in English"),
            (Lang::Es, "written in Spanish (Rioplatense"),
            (Lang::De, "written in German (formal"),
        ] {
            let prompt = user_prompt(&req, 100, lang);
            assert!(
                prompt.contains(&format!("why that folder, {want}")),
                "{lang:?}: {prompt}"
            );
            // The title follows the document, not the screen.
            assert!(prompt.contains("The whole title in document_language"));
            assert!(prompt.contains(&format!("Only the reason is in {}.", lang.english_name())));
        }
        // The model fills the fields in this order: the document's language
        // before the title, the reason last.
        let schema = schema();
        let keys: Vec<&String> = schema["properties"].as_object().unwrap().keys().collect();
        assert_eq!(
            keys,
            [
                "document_language",
                "title",
                "document_date",
                "tags",
                "sender",
                "amount",
                "currency",
                "due_date",
                "folder",
                "reason"
            ]
        );
    }

    #[test]
    fn tells_where_a_server_is() {
        use Reach::*;
        for (url, want) in [
            ("http://localhost:1234", ThisComputer),
            ("http://127.0.0.1:11434/", ThisComputer),
            ("http://[::1]:8080", ThisComputer),
            ("localhost:1234", ThisComputer),
            ("http://192.168.1.20:1234", LocalNetwork),
            ("http://10.0.0.5", LocalNetwork),
            ("http://172.20.1.1:11434", LocalNetwork),
            ("http://100.101.102.103:11434", LocalNetwork),
            ("http://[fd12:3456::1]:1234", LocalNetwork),
            ("http://mac-mini.local:11434", LocalNetwork),
            ("http://studio:1234", LocalNetwork),
            ("http://user@nas.lan:8080/v1", LocalNetwork),
            ("http://172.32.0.1", Elsewhere),
            ("http://100.128.0.1", Elsewhere),
            ("https://api.example.com", Elsewhere),
            ("http://8.8.8.8:1234", Elsewhere),
            ("", Elsewhere),
        ] {
            assert_eq!(reach(url), want, "{url}");
        }
        assert_eq!(host("http://[::1]:8080/v1"), Some("[::1]"));
        assert_eq!(host("https://example.com/v1?x"), Some("example.com"));
    }

    #[test]
    fn unreachable_server_fails_fast() {
        let lm = LocalAi {
            url: "http://127.0.0.1:9".into(),
            model: None,
        };
        let t = std::time::Instant::now();
        assert!(lm.models().is_err());
        assert!(t.elapsed() < Duration::from_secs(5));
    }
}
