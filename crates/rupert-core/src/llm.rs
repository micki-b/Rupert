//! Filing suggestions from a local model in LM Studio (its OpenAI-compatible
//! server, `http://localhost:1234` by default). Nothing leaves the machine;
//! if LM Studio isn't running, suggestions are simply unavailable.
//!
//! The model gets the document's text (the start of it), its file name,
//! the folders where similar documents already live, and the tags already
//! in use, and answers with JSON matching a schema (LM Studio enforces it).

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{Error, Result};
use crate::meta::is_iso_date;

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
    /// The model that made it.
    pub model: String,
}

// MARK: - Client

#[derive(Debug, Clone)]
pub struct LmStudio {
    pub url: String,
    /// `None` = the first chat model LM Studio lists (normally the loaded one).
    pub model: Option<String>,
}

impl LmStudio {
    fn agent(timeout: Duration) -> ureq::Agent {
        ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .build()
            .into()
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}/v1/{path}", self.url.trim_end_matches('/'))
    }

    /// Chat models LM Studio offers (embedding models left out). Fails fast
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
        let model = match &self.model {
            Some(m) => m.clone(),
            None => self
                .models()?
                .into_iter()
                .next()
                .ok_or_else(|| Error::InvalidValue("LM Studio has no chat model".into()))?,
        };
        let body = json!({
            "model": model,
            "temperature": 0.1,
            "max_tokens": 4000,
            "messages": [
                {"role": "system", "content": SYSTEM_PROMPT},
                {"role": "user", "content": user_prompt(req)},
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": {"name": "filing", "strict": true, "schema": schema()},
            },
        });
        #[derive(Deserialize)]
        struct Reply {
            choices: Vec<Choice>,
        }
        #[derive(Deserialize)]
        struct Choice {
            message: Message,
        }
        #[derive(Deserialize)]
        struct Message {
            content: Option<String>,
            // Reasoning models: LM Studio may put the (schema-constrained)
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
            .map_err(|e| Error::InvalidValue(format!("LM Studio answered oddly: {e}")))?;
        let message = reply.choices.into_iter().next().map(|c| c.message);
        let content = message
            .map(|m| {
                [m.content, m.reasoning_content, m.reasoning]
                    .into_iter()
                    .flatten()
                    .find(|t| t.contains('{'))
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        parse(&content, req, model)
    }
}

fn unreachable(e: impl std::fmt::Display) -> Error {
    Error::InvalidValue(format!("LM Studio isn't reachable ({e})"))
}

// MARK: - Prompt

const SYSTEM_PROMPT: &str = "You are a careful archivist filing scanned personal documents \
(bills, statements, letters, receipts, forms, manuals). You get a document's text, which may \
contain OCR errors, and answer only with JSON.";

fn user_prompt(req: &Request) -> String {
    let text: String = req.text.chars().take(MAX_TEXT).collect();
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
         - title: short and specific, like a person would name the file: who it's from and what \
           it is, e.g. \"City Electric bill\" or \"Acme auto policy cancellation\". At most 60 \
           characters. No date in the title. Same language as the document.\n\
         - document_date: the date the document was issued (statement, invoice, letter or \
           receipt date) as YYYY-MM-DD, or \"\" if the document shows none. Only use a date \
           printed on the document. Not a due date or a date of birth.\n\
         - tags: 0 to 3 short tags for the kind of document or its subject (e.g. \"Insurance\", \
           \"Tax\", \"Medical\"). Reuse a tag in use only if it clearly fits; never add one just \
           because it exists.\n\
         - folder: the folder it belongs in, copied exactly from the list, or \"\" if none fits.\n\
         - reason: one short sentence on why that folder.\n\n\
         Folders where similar documents are filed:\n{folders}\n\n\
         Tags in use: {tags}\n\n\
         Document text:\n\"\"\"\n{text}\n\"\"\"",
        year = req.year,
        name = req.file_name,
    )
}

fn schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "title": {"type": "string"},
            "document_date": {"type": "string"},
            "tags": {"type": "array", "items": {"type": "string"}},
            "folder": {"type": "string"},
            "reason": {"type": "string"},
        },
        "required": ["title", "document_date", "tags", "folder", "reason"],
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
    }
    // Some models wrap JSON in prose or code fences; take the object.
    let start = content.find('{');
    let end = content.rfind('}');
    let json = match (start, end) {
        (Some(s), Some(e)) if e > s => &content[s..=e],
        _ => {
            return Err(Error::InvalidValue(
                "the model didn't answer with JSON".into(),
            ));
        }
    };
    let raw: Raw = serde_json::from_str(json)
        .map_err(|e| Error::InvalidValue(format!("the model's answer didn't parse: {e}")))?;
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
    Ok(Suggestion {
        title: clean(raw.title, 80),
        document_date: clean(raw.document_date, 10).filter(|d| is_iso_date(d)),
        tags,
        folder: clean(raw.folder, 400).filter(|f| req.folders.contains(f)),
        reason: clean(raw.reason, 300),
        model,
    })
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

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
    fn unreachable_server_fails_fast() {
        let lm = LmStudio {
            url: "http://127.0.0.1:9".into(),
            model: None,
        };
        let t = std::time::Instant::now();
        assert!(lm.models().is_err());
        assert!(t.elapsed() < Duration::from_secs(5));
    }
}
