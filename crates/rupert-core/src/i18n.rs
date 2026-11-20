//! Rupert's language for messages that come from Rust: errors, the macOS
//! menu, window titles. (The screens have their own catalogs in
//! `src/lib/i18n/`.)
//!
//! Messages are written in English at the call site, `tr("…")` or
//! `trf("… {0} …", &[&value])`, and looked up in the current language's
//! table. A test reads the source tree and checks that every message has a
//! translation in every language, with the same `{n}` placeholders.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

mod de;
mod es;

// MARK: - Languages

/// A language Rupert speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    En,
    Es,
    De,
}

impl Lang {
    pub const ALL: [Lang; 3] = [Lang::En, Lang::Es, Lang::De];

    /// The code used in settings and by the screens' catalogs.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Es => "es",
            Lang::De => "de",
        }
    }

    /// From a setting or a system locale: "es", "es-AR", "es_419", "de-CH"…
    /// Only the language part counts (every Spanish gets Rupert's Spanish).
    pub fn from_code(code: &str) -> Option<Lang> {
        let lang = code
            .split(['-', '_', '.', '@'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        match lang.as_str() {
            "en" => Some(Lang::En),
            "es" => Some(Lang::Es),
            "de" => Some(Lang::De),
            _ => None,
        }
    }

    /// The language's name in English (for the assistant's instructions).
    pub fn english_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Es => "Spanish (Rioplatense, as spoken in Argentina)",
            Lang::De => "German (formal \"Sie\")",
        }
    }

    fn table(self) -> Option<&'static HashMap<&'static str, &'static str>> {
        static ES: OnceLock<HashMap<&str, &str>> = OnceLock::new();
        static DE: OnceLock<HashMap<&str, &str>> = OnceLock::new();
        match self {
            Lang::En => None,
            Lang::Es => Some(ES.get_or_init(|| es::MESSAGES.iter().copied().collect())),
            Lang::De => Some(DE.get_or_init(|| de::MESSAGES.iter().copied().collect())),
        }
    }
}

/// The first of the system's preferred languages that Rupert speaks;
/// English if none. (On iPhone this includes the per-app language chosen
/// in iOS Settings.)
pub fn system() -> Lang {
    sys_locale::get_locales()
        .find_map(|l| Lang::from_code(&l))
        .unwrap_or(Lang::En)
}

/// The language to use: the setting (`None` or unknown = automatic) or the
/// system's.
pub fn resolve(setting: Option<&str>) -> Lang {
    setting.and_then(Lang::from_code).unwrap_or_else(system)
}

// MARK: - Current language

static CURRENT: AtomicU8 = AtomicU8::new(0);

/// Set the language for messages from now on (at startup and when the
/// setting changes).
pub fn set(lang: Lang) {
    let i = Lang::ALL.iter().position(|l| *l == lang).unwrap_or(0);
    CURRENT.store(i as u8, Ordering::Relaxed);
}

pub fn current() -> Lang {
    Lang::ALL
        .get(CURRENT.load(Ordering::Relaxed) as usize)
        .copied()
        .unwrap_or(Lang::En)
}

// MARK: - Messages

/// `english` in the current language (itself if there's no translation).
pub fn tr(english: &'static str) -> &'static str {
    tr_in(current(), english)
}

/// `english` in `lang`.
pub fn tr_in(lang: Lang, english: &'static str) -> &'static str {
    lang.table()
        .and_then(|t| t.get(english).copied())
        .unwrap_or(english)
}

/// A message with values: `{0}`, `{1}`… are replaced by `args`.
pub fn trf(english: &'static str, args: &[&dyn Display]) -> String {
    fill(tr(english), args)
}

fn fill(text: &str, args: &[&dyn Display]) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let arg = after.find('}').and_then(|close| {
            let n: usize = after[..close].parse().ok()?;
            Some((args.get(n)?, close))
        });
        match arg {
            Some((value, close)) => {
                out.push_str(&value.to_string());
                rest = &after[close + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn codes() {
        assert_eq!(Lang::from_code("es-AR"), Some(Lang::Es));
        assert_eq!(Lang::from_code("es_419"), Some(Lang::Es));
        assert_eq!(Lang::from_code("ES"), Some(Lang::Es));
        assert_eq!(Lang::from_code("en-US"), Some(Lang::En));
        assert_eq!(Lang::from_code("de-CH"), Some(Lang::De));
        assert_eq!(Lang::from_code("fr-FR"), None);
        assert_eq!(Lang::from_code(""), None);
        assert_eq!(resolve(Some("es")), Lang::Es);
        for l in Lang::ALL {
            assert_eq!(Lang::from_code(l.code()), Some(l));
        }
    }

    #[test]
    fn filling() {
        assert_eq!(fill("a {0} b {1}", &[&1, &"x"]), "a 1 b x");
        assert_eq!(fill("{1} before {0}", &[&"a", &"b"]), "b before a");
        assert_eq!(fill("no {braces} {9}", &[&1]), "no {braces} {9}");
        assert_eq!(tr_in(Lang::En, "unknown source"), "unknown source");
        assert_eq!(tr_in(Lang::Es, "not in any table"), "not in any table");
    }

    /// Every `tr("…")` / `trf("…")` in the code base has a translation in
    /// every language, with the same placeholders.
    #[test]
    fn every_message_is_translated() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut files = Vec::new();
        for dir in ["crates/rupert-core/src", "src-tauri/src", "mobile/src"] {
            rust_files(&root.join(dir), &mut files);
        }
        let mut used = Vec::new();
        for file in &files {
            let text = std::fs::read_to_string(file).unwrap();
            for message in calls(&text) {
                used.push((file.clone(), message));
            }
        }
        assert!(used.len() > 20, "found only {} messages", used.len());

        let mut problems = Vec::new();
        for lang in Lang::ALL.into_iter().filter(|l| *l != Lang::En) {
            let table = lang.table().unwrap();
            for (file, message) in &used {
                match table.get(message.as_str()) {
                    None => problems.push(format!(
                        "{}: no {} for {message:?}",
                        file.display(),
                        lang.code()
                    )),
                    Some(t) if placeholders(t) != placeholders(message) => problems.push(format!(
                        "{}: placeholders differ in {} for {message:?}",
                        file.display(),
                        lang.code()
                    )),
                    _ => {}
                }
            }
            // And nothing in the table that isn't used any more.
            for key in table.keys() {
                if !used.iter().any(|(_, m)| m == key) {
                    problems.push(format!("{}: unused message {key:?}", lang.code()));
                }
            }
        }
        assert!(problems.is_empty(), "\n{}", problems.join("\n"));
    }

    fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                rust_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs")
                && !path.ends_with("i18n/es.rs")
                && !path.ends_with("i18n/de.rs")
                && !path.ends_with("i18n.rs")
            {
                out.push(path);
            }
        }
    }

    /// The string literals passed to `tr(` / `trf(` / `tr_in(…, `.
    fn calls(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        for (at, _) in text.match_indices("tr") {
            let before = text[..at].chars().next_back();
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            let rest = &text[at + 2..];
            let rest = if let Some(r) = rest.strip_prefix("f(") {
                r
            } else if let Some(r) = rest.strip_prefix("(") {
                r
            } else if let Some(r) = rest.strip_prefix("_in(") {
                match r.find(',') {
                    Some(comma) => &r[comma + 1..],
                    None => continue,
                }
            } else {
                continue;
            };
            let rest = rest.trim_start();
            if let Some(literal) = rest.strip_prefix('"') {
                out.push(unescape(literal));
            }
        }
        out
    }

    /// A Rust string literal's value, up to its closing quote.
    fn unescape(literal: &str) -> String {
        let mut out = String::new();
        let mut chars = literal.chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => break,
                '\\' => match chars.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('\n') => {
                        // Line continuation: skip the next line's indent.
                        let rest: String = chars.clone().collect();
                        let skip = rest.len() - rest.trim_start().len();
                        for _ in 0..rest[..skip].chars().count() {
                            chars.next();
                        }
                    }
                    Some(other) => out.push(other),
                    None => break,
                },
                c => out.push(c),
            }
        }
        out
    }

    fn placeholders(text: &str) -> Vec<String> {
        let mut found: Vec<String> = text
            .match_indices('{')
            .filter_map(|(i, _)| {
                let rest = &text[i + 1..];
                let close = rest.find('}')?;
                rest[..close]
                    .parse::<usize>()
                    .ok()
                    .map(|n| format!("{{{n}}}"))
            })
            .collect();
        found.sort();
        found
    }
}
