//! Renaming the documents of a folder together (#64). New names come from
//! each document's date and title (the name pattern), "the same way" as
//! names the user typed (worked out here when it's a pattern of known
//! details), or from Local AI. Only names change, the extension stays,
//! nothing is ever replaced, and a batch can be undone.

use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{Error, Result};
use crate::fileops::{self, check_name};
use crate::i18n::{Lang, tr, trf};
use crate::index::{Index, TextState};
use crate::listing::{CloudState, cloud_state, list_dir, resolve};
use crate::llm::Engine;
use crate::meta::{
    DocMeta, is_iso_date, read_meta, read_meta_beside, write_meta, write_meta_beside,
};
use crate::settings::Settings;
use crate::source::Source;

/// Longest name Local AI may propose, in characters.
const MAX_PROPOSED: usize = 120;
/// Characters of document text sent with a request.
const MAX_TEXT: usize = 3000;

// MARK: - Rows

/// One document of the folder, as the rename dialog lists it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub rel_path: String,
    pub name: String,
    /// The name without its extension.
    pub stem: String,
    /// The extension with its dot (".pdf"), or "".
    pub ext: String,
    pub title: Option<String>,
    pub document_date: Option<String>,
    pub tags: Vec<String>,
    pub cloud: CloudState,
    /// For its picture in the preview.
    pub kind: Option<crate::FileKind>,
    pub size: u64,
    pub modified_ms: Option<i64>,
    /// Whether the index has the document's text (Local AI needs it).
    pub has_text: bool,
    /// The name from the name pattern, when the document has a title.
    pub pattern_name: Option<String>,
}

/// A file name's stem and extension (with its dot). A name without a dot,
/// or with only a leading one, has no extension.
pub fn split_name(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    }
}

/// The documents (not folders) in `rel_dir`, with what the index knows
/// about them. Nothing is read from the files.
pub fn rows(
    source: &Source,
    index: &Index,
    settings: &Settings,
    rel_dir: &str,
) -> Result<Vec<Row>> {
    let mut rows = Vec::new();
    for e in list_dir(&source.root, rel_dir, source.show_all_types)? {
        if e.is_dir {
            continue;
        }
        let info = index.document(&source.id, &e.rel_path)?;
        let (stem, ext) = split_name(&e.name);
        let (title, document_date, tags, has_text) = match info {
            Some(i) => (
                i.hit.title,
                i.hit.document_date,
                i.hit.tags,
                matches!(
                    i.text_state,
                    TextState::Extracted | TextState::Catalog | TextState::Sidecar
                ),
            ),
            None => (None, None, Vec::new(), false),
        };
        // A title that's a file name ("Roof warranty.html", from the PDF's
        // own details) without its extension.
        let title = title.map(|t| match split_name(&t) {
            (stem, ext)
                if !ext.is_empty()
                    && (crate::FileKind::from_path(Path::new(&t)) != crate::FileKind::Other
                        || matches!(ext.to_lowercase().as_str(), ".html" | ".htm")) =>
            {
                stem.to_string()
            }
            _ => t,
        });
        let pattern_name = title
            .as_deref()
            .map(|t| {
                fileops::clean_name(
                    &settings.file_name(document_date.as_deref(), t),
                    ext.trim_start_matches('.'),
                )
            })
            .filter(|n| !n.is_empty());
        rows.push(Row {
            rel_path: e.rel_path,
            stem: stem.to_string(),
            ext: ext.to_string(),
            name: e.name,
            title,
            document_date,
            tags,
            cloud: e.cloud,
            kind: e.kind,
            size: e.size,
            modified_ms: e.modified_ms,
            has_text,
            pattern_name,
        });
    }
    Ok(rows)
}

// MARK: - The same way

/// What a new name can be made of: a document's details.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Details {
    /// The current name without its extension.
    pub stem: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub document_date: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// How a date is written in a name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DateStyle {
    /// 2026-03-01
    Iso,
    /// 2026_03_01
    Underscored,
    /// 01.03.2026
    Dotted,
    /// 01-03-2026
    DayFirst,
    /// 03-01-2026
    MonthFirst,
    /// 20260301
    Compact,
    /// 2026-03
    YearMonth,
    /// 2026
    Year,
}

impl DateStyle {
    /// Longer forms first, so "2026-03-01" isn't taken for "2026-03".
    const ALL: [Self; 8] = [
        Self::Iso,
        Self::Underscored,
        Self::Dotted,
        Self::DayFirst,
        Self::MonthFirst,
        Self::Compact,
        Self::YearMonth,
        Self::Year,
    ];

    /// `date` (YYYY-MM-DD) written this way.
    fn format(self, date: &str) -> Option<String> {
        if !is_iso_date(date) {
            return None;
        }
        let (y, m, d) = (&date[..4], &date[5..7], &date[8..10]);
        Some(match self {
            Self::Iso => date.to_string(),
            Self::Underscored => format!("{y}_{m}_{d}"),
            Self::Dotted => format!("{d}.{m}.{y}"),
            Self::DayFirst => format!("{d}-{m}-{y}"),
            Self::MonthFirst => format!("{m}-{d}-{y}"),
            Self::Compact => format!("{y}{m}{d}"),
            Self::YearMonth => format!("{y}-{m}"),
            Self::Year => y.to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Text(String),
    Title,
    Date(DateStyle),
}

/// A rule for names, worked out from one the user typed: the title and
/// date in it, with whatever is around them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template(Vec<Part>);

/// Where `needle` is in `hay`, ignoring case: the byte range in `hay`.
fn find_ignoring_case(hay: &str, needle: &str) -> Option<(usize, usize)> {
    if needle.is_empty() {
        return None;
    }
    for (start, _) in hay.char_indices() {
        let mut rest = hay[start..].char_indices();
        let mut end = start;
        let mut matched = true;
        for n in needle.chars() {
            match rest.next() {
                Some((i, h)) if h.to_lowercase().eq(n.to_lowercase()) => {
                    end = start + i + h.len_utf8();
                }
                _ => {
                    matched = false;
                    break;
                }
            }
        }
        if matched {
            return Some((start, end));
        }
    }
    None
}

/// Separators left over around a detail that's missing.
fn is_separator(c: char) -> bool {
    c.is_whitespace() || matches!(c, '-' | '_' | '.' | ',' | '–' | '—')
}

/// The rule behind `name`, typed for the document `example`: its title and
/// document date (in any of the usual ways of writing a date) stand for
/// those of any document. `None` when the title isn't in the name as it
/// is: then the name isn't a pattern of details (Local AI can still follow
/// it as an example).
pub fn infer(example: &Details, name: &str) -> Option<Template> {
    let title = example
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())?;
    let (start, end) = find_ignoring_case(name, title)?;
    let mut parts = Vec::new();
    let mut texts = vec![
        (name[..start].to_string(), true),
        (name[end..].to_string(), false),
    ];
    // The date, once, in the text before or after the title.
    let mut date_part: Option<(bool, String, DateStyle, String)> = None;
    if let Some(date) = example.document_date.as_deref() {
        'styles: for style in DateStyle::ALL {
            let Some(written) = style.format(date) else {
                continue;
            };
            for (text, before) in &texts {
                if let Some(at) = text.find(&written) {
                    date_part = Some((
                        *before,
                        text[..at].to_string(),
                        style,
                        text[at + written.len()..].to_string(),
                    ));
                    break 'styles;
                }
            }
        }
    }
    let push_text = |parts: &mut Vec<Part>, t: String| {
        if !t.is_empty() {
            parts.push(Part::Text(t));
        }
    };
    let (before, after) = (
        std::mem::take(&mut texts[0].0),
        std::mem::take(&mut texts[1].0),
    );
    match date_part {
        Some((true, a, style, b)) => {
            push_text(&mut parts, a);
            parts.push(Part::Date(style));
            push_text(&mut parts, b);
            parts.push(Part::Title);
            push_text(&mut parts, after);
        }
        Some((false, a, style, b)) => {
            push_text(&mut parts, before);
            parts.push(Part::Title);
            push_text(&mut parts, a);
            parts.push(Part::Date(style));
            push_text(&mut parts, b);
        }
        None => {
            push_text(&mut parts, before);
            parts.push(Part::Title);
            push_text(&mut parts, after);
        }
    }
    Some(Template(parts))
}

impl Template {
    /// The name for a document by this rule; `None` if it has no title. A
    /// missing date is left out along with the separator before it.
    pub fn apply(&self, d: &Details) -> Option<String> {
        let title = d
            .title
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())?;
        let mut out = String::new();
        for part in &self.0 {
            match part {
                Part::Text(t) => out.push_str(t),
                Part::Title => out.push_str(title),
                Part::Date(style) => {
                    match d.document_date.as_deref().and_then(|x| style.format(x)) {
                        Some(written) => out.push_str(&written),
                        None => out.truncate(out.trim_end_matches(is_separator).len()),
                    }
                }
            }
        }
        // Brackets that held a missing date.
        let out = out.replace("()", "").replace("[]", "");
        let out = out.split_whitespace().collect::<Vec<_>>().join(" ");
        let out = out.trim_matches(is_separator).to_string();
        (!out.is_empty()).then_some(out)
    }
}

/// New names for `targets` "the same way" as the names the user typed
/// (`examples`, the latest last), when that's a pattern of their details:
/// the rule from the latest one, if it also gives every other typed name.
/// Per target, `None` where the rule can't make a name (no title).
pub fn same_way(
    examples: &[(Details, String)],
    targets: &[Details],
) -> Option<Vec<Option<String>>> {
    let (latest, name) = examples.last()?;
    let rule = infer(latest, name)?;
    let agrees = examples.iter().all(|(d, n)| {
        rule.apply(d)
            .is_some_and(|made| made.to_lowercase() == n.trim().to_lowercase())
    });
    agrees.then(|| targets.iter().map(|t| rule.apply(t)).collect())
}

// MARK: - Local AI

/// A name the user typed, for Local AI to follow.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Example {
    pub details: Details,
    pub name: String,
}

/// What Local AI is asked for one document.
#[derive(Debug, Clone)]
pub struct Ask<'a> {
    /// How the names should look, in the user's words (may be empty).
    pub instruction: &'a str,
    pub examples: &'a [Example],
    pub target: &'a Details,
    pub text: &'a str,
    /// The extension, without its dot ("pdf").
    pub ext: &'a str,
}

const SYSTEM_PROMPT: &str = "You name the files of scanned personal documents (bills, \
statements, letters, receipts, forms, manuals). You get a document's details and text, which \
may contain OCR errors, and answer only with JSON.";

fn describe(d: &Details) -> String {
    let mut s = format!("current file name \"{}\"", d.stem);
    if let Some(t) = d.title.as_deref().filter(|t| !t.trim().is_empty()) {
        s.push_str(&format!(", title \"{t}\""));
    }
    if let Some(date) = d.document_date.as_deref() {
        s.push_str(&format!(", document date {date}"));
    }
    if !d.tags.is_empty() {
        s.push_str(&format!(", tags {}", d.tags.join(", ")));
    }
    s
}

fn prompt(ask: &Ask, max_text: usize, lang: Lang) -> String {
    let mut p = String::from("Propose a new file name (without extension) for this document.\n");
    let instruction = ask.instruction.trim();
    if !instruction.is_empty() {
        p.push_str(&format!(
            "How the names should look, in the user's words (they may write in {}): {instruction}\n\
             Follow that exactly: every part it names, in that order, with the separators it \
             shows. A year or a date comes from the document date.\n",
            lang.english_name()
        ));
    }
    if !ask.examples.is_empty() {
        p.push_str(
            "Name it the same way as these names the user chose for other documents in the \
             folder: the same order of parts, separators, date format, capitalization and \
             wording style.\n",
        );
        for e in ask.examples {
            p.push_str(&format!(
                "- {} → \"{}\"\n",
                describe(&e.details),
                e.name.trim()
            ));
        }
    }
    if instruction.is_empty() && ask.examples.is_empty() {
        p.push_str(
            "Make it short and specific, like a person would name the file: the document date \
             as YYYY-MM-DD if the document shows one, then who it's from and what it is.\n",
        );
    }
    let text: String = ask.text.chars().take(max_text).collect();
    p.push_str(&format!(
        "Rules: use only what the document's details and text show; never invent a date, a \
         name or a number. Words taken from the document stay in its language. At most 80 \
         characters. No file extension. None of these characters: / \\ : * ? \" < > |\n\n\
         This document: {}\n\nDocument text:\n\"\"\"\n{text}\n\"\"\"",
        describe(ask.target)
    ));
    p
}

fn schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {"name": {"type": "string"}},
        "required": ["name"],
        "additionalProperties": false,
    })
}

/// The proposed name from the model's JSON, made safe for a file name.
fn parse_name(content: &str, ext: &str) -> Result<String> {
    #[derive(Deserialize)]
    struct Raw {
        #[serde(default)]
        name: String,
    }
    let json = match (content.find('{'), content.rfind('}')) {
        (Some(s), Some(e)) if e > s => &content[s..=e],
        _ => {
            return Err(Error::InvalidValue(
                tr("the model didn't answer with JSON").into(),
            ));
        }
    };
    let raw: Raw = serde_json::from_str(json)
        .map_err(|_| Error::InvalidValue(tr("the model didn't answer with JSON").into()))?;
    let name: String = fileops::clean_name(raw.name.trim().trim_matches('"'), ext)
        .chars()
        .take(MAX_PROPOSED)
        .collect();
    let name = name.trim_matches(is_separator).to_string();
    if name.is_empty() {
        return Err(Error::InvalidValue(
            tr("The model didn't propose a name.").into(),
        ));
    }
    Ok(name)
}

/// Asks Local AI for one document's new name (without extension).
pub fn propose(engine: &Engine, ask: &Ask) -> Result<String> {
    if ask.text.trim().is_empty() && ask.target.title.is_none() {
        return Err(Error::InvalidValue(tr("No text to name it from.").into()));
    }
    let user = prompt(ask, engine.max_text(MAX_TEXT), crate::i18n::current());
    let content = engine.json(SYSTEM_PROMPT, &user, "file_name", &schema(), 300)?;
    parse_name(&content, ask.ext)
}

// MARK: - Checking names

/// A new name the user is about to give (without the extension).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Planned {
    pub rel_path: String,
    pub stem: String,
}

/// Why a planned name can't be used as it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Problem {
    /// Same as the current name.
    Unchanged,
    /// Taken (by another file, or another planned name): " (2)" added.
    Taken,
    /// Not allowed as a file name.
    Invalid { message: String },
}

/// The name a document would really get.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Checked {
    pub rel_path: String,
    /// The full new name, with the extension (and " (2)" if needed).
    pub name: String,
    pub problem: Option<Problem>,
}

fn file_name_of(rel_path: &str) -> &str {
    rel_path.rsplit('/').next().unwrap_or(rel_path)
}

/// Checks planned names before anything happens: names that aren't
/// allowed, unchanged ones, and taken ones (which get " (2)", " (3)", …,
/// in the order given). A name another planned document is leaving counts
/// as free.
pub fn check(source: &Source, rel_dir: &str, planned: &[Planned]) -> Result<Vec<Checked>> {
    let existing: HashSet<String> = list_dir(&source.root, rel_dir, true)?
        .into_iter()
        .map(|e| e.name.to_lowercase())
        .collect();
    // First what each one would be on its own.
    let mut out: Vec<Checked> = planned
        .iter()
        .map(|p| {
            let current = file_name_of(&p.rel_path);
            let (stem, ext) = split_name(current);
            let new_stem = p.stem.trim();
            let name = format!("{new_stem}{ext}");
            let problem = if new_stem == stem {
                Some(Problem::Unchanged)
            } else {
                check_name(&name).err().map(|e| Problem::Invalid {
                    message: match e {
                        Error::InvalidValue(m) => m,
                        other => other.to_string(),
                    },
                })
            };
            Checked {
                rel_path: p.rel_path.clone(),
                name: if new_stem == stem {
                    current.to_string()
                } else {
                    name
                },
                problem,
            }
        })
        .collect();
    let leaving: HashSet<String> = out
        .iter()
        .filter(|c| c.problem.is_none())
        .map(|c| file_name_of(&c.rel_path).to_lowercase())
        .collect();
    let mut taken: HashSet<String> = existing.difference(&leaving).cloned().collect();
    // Names staying put keep theirs.
    for c in out.iter().filter(|c| c.problem.is_some()) {
        taken.insert(file_name_of(&c.rel_path).to_lowercase());
    }
    for c in out.iter_mut().filter(|c| c.problem.is_none()) {
        let (stem, ext) = split_name(&c.name);
        let (stem, ext) = (stem.to_string(), ext.to_string());
        let mut n = 1;
        while taken.contains(&c.name.to_lowercase()) {
            n += 1;
            c.name = format!("{stem} ({n}){ext}");
            c.problem = Some(Problem::Taken);
        }
        taken.insert(c.name.to_lowercase());
    }
    Ok(out)
}

// MARK: - Renaming

/// One document to rename: its path and full new name (with extension).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub rel_path: String,
    pub name: String,
}

/// A document that was renamed: enough to undo it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Renamed {
    pub from: String,
    pub to: String,
    /// Its title was set from the new name; `old_title` is the one before.
    #[serde(default)]
    pub title_changed: bool,
    #[serde(default)]
    pub old_title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failed {
    pub rel_path: String,
    pub message: String,
}

/// What a batch did.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub renamed: Vec<Renamed>,
    pub failed: Vec<Failed>,
}

fn message(e: Error) -> String {
    match e {
        Error::InvalidValue(m) => m,
        other => other.to_string(),
    }
}

/// Sets a document's title, keeping its other details: read from the file
/// when it's on this computer; for an online-only file (never read), what
/// the index knows (`known`) or its sidecar has.
fn set_title(path: &Path, known: Option<DocMeta>, title: Option<String>) -> Result<()> {
    if cloud_state(path)? == CloudState::Local {
        let mut meta = read_meta(path)?.meta;
        meta.title = title;
        write_meta(path, meta, true)?;
    } else {
        let mut meta = match known {
            Some(m) => m,
            None => read_meta_beside(path)?.map(|(m, _)| m).unwrap_or_default(),
        };
        meta.title = title;
        write_meta_beside(path, meta)?;
    }
    Ok(())
}

/// What the index knows about a document, by its path.
pub type MetaOf<'a> = &'a dyn Fn(&str) -> Option<DocMeta>;

/// Renames documents in one folder, in an order that never needs a taken
/// name (names that swap go through a temporary one). Never replaces a
/// file: a name that's taken by the time it's used fails that document
/// only. With `meta_of` (what the index knows, by path), each document's
/// title is also set from its new name.
pub fn apply(source: &Source, steps: &[Step], meta_of: Option<MetaOf>) -> Outcome {
    let mut outcome = Outcome::default();
    // (path now, new name, path at the start)
    let mut pending: Vec<(String, String, String)> = steps
        .iter()
        .filter(|s| file_name_of(&s.rel_path) != s.name)
        .map(|s| (s.rel_path.clone(), s.name.clone(), s.rel_path.clone()))
        .collect();
    while !pending.is_empty() {
        let names: Vec<String> = pending
            .iter()
            .map(|(now, _, _)| file_name_of(now).to_lowercase())
            .collect();
        let ready = pending.iter().position(|(now, name, _)| {
            let own = file_name_of(now).to_lowercase();
            let want = name.to_lowercase();
            want == own || !names.contains(&want)
        });
        let Some(i) = ready else {
            // Names going round in a circle: one steps aside first.
            let (now, _, _) = &mut pending[0];
            let (stem, ext) = split_name(file_name_of(now));
            let (stem, ext) = (stem.to_string(), ext.to_string());
            let mut moved = None;
            for n in 1..100 {
                if let Ok(m) = fileops::rename_entry(source, now, &format!("{stem} ~{n}{ext}")) {
                    moved = Some(m.rel_path);
                    break;
                }
            }
            match moved {
                Some(rel) => *now = rel,
                None => {
                    let (now, _, start) = pending.remove(0);
                    outcome.failed.push(Failed {
                        rel_path: start,
                        message: trf("Couldn't rename “{0}”.", &[&file_name_of(&now)]),
                    });
                }
            }
            continue;
        };
        let (now, name, start) = pending.remove(i);
        match fileops::rename_entry(source, &now, &name) {
            Ok(moved) => {
                let mut done = Renamed {
                    from: start.clone(),
                    to: moved.rel_path.clone(),
                    title_changed: false,
                    old_title: None,
                };
                if let Some(meta_of) = meta_of {
                    let known = meta_of(&start);
                    let path = resolve(&source.root, &moved.rel_path);
                    // The title before, from the file where it can be read.
                    let old = match &path {
                        Ok(p) if cloud_state(p).ok() == Some(CloudState::Local) => {
                            read_meta(p).ok().and_then(|r| r.meta.title)
                        }
                        _ => known.as_ref().and_then(|m| m.title.clone()),
                    };
                    let title = split_name(&name).0.to_string();
                    match path.and_then(|p| set_title(&p, known, Some(title))) {
                        Ok(()) => {
                            done.title_changed = true;
                            done.old_title = old;
                        }
                        Err(e) => outcome.failed.push(Failed {
                            rel_path: moved.rel_path.clone(),
                            message: message(e),
                        }),
                    }
                }
                outcome.renamed.push(done);
            }
            Err(e) => outcome.failed.push(Failed {
                rel_path: start,
                message: message(e),
            }),
        }
    }
    outcome
}

/// Undoes a batch: each document gets its old name back (and its old
/// title, where that changed). `meta_of` is what the index knows, by path.
pub fn undo(source: &Source, done: &[Renamed], meta_of: MetaOf) -> Outcome {
    let steps: Vec<Step> = done
        .iter()
        .map(|r| Step {
            rel_path: r.to.clone(),
            name: file_name_of(&r.from).to_string(),
        })
        .collect();
    let mut outcome = apply(source, &steps, None);
    for r in done.iter().filter(|r| r.title_changed) {
        // Back where it was, if that worked.
        let Some(back) = outcome.renamed.iter().find(|b| b.from == r.to) else {
            continue;
        };
        let known = meta_of(&r.to).or_else(|| meta_of(&r.from));
        let path = resolve(&source.root, &back.to);
        if let Err(e) = path.and_then(|p| set_title(&p, known, r.old_title.clone())) {
            outcome.failed.push(Failed {
                rel_path: back.to.clone(),
                message: message(e),
            });
        }
    }
    outcome
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn details(title: &str, date: Option<&str>) -> Details {
        Details {
            stem: "Scan".into(),
            title: Some(title.into()),
            document_date: date.map(Into::into),
            tags: Vec::new(),
        }
    }

    #[test]
    fn works_out_a_rule_from_a_typed_name() {
        let kia = details("Kia Finance statement", Some("2026-03-01"));
        let dmv = details("DMV registration renewal", Some("2026-01-15"));
        let no_date = details("Owner's manual", None);
        for (typed, want_dmv, want_manual) in [
            (
                "2026-03-01 Kia Finance statement",
                "2026-01-15 DMV registration renewal",
                "Owner's manual",
            ),
            (
                "Kia Finance statement - 2026-03",
                "DMV registration renewal - 2026-01",
                "Owner's manual",
            ),
            (
                "Car - kia finance statement (01.03.2026)",
                "Car - DMV registration renewal (15.01.2026)",
                "Car - Owner's manual",
            ),
            (
                "20260301_Kia Finance statement",
                "20260115_DMV registration renewal",
                "Owner's manual",
            ),
            (
                "Kia Finance statement 2026",
                "DMV registration renewal 2026",
                "Owner's manual",
            ),
        ] {
            let rule = infer(&kia, typed).unwrap_or_else(|| panic!("{typed}"));
            assert_eq!(rule.apply(&dmv).as_deref(), Some(want_dmv), "{typed}");
            assert_eq!(
                rule.apply(&no_date).as_deref(),
                Some(want_manual),
                "{typed}"
            );
        }
        // Not a pattern of the details: the title isn't in it.
        assert_eq!(infer(&kia, "Kia Finance - statement - 2026-03"), None);
        // No title, no rule; and nothing to make a name from.
        assert_eq!(infer(&Details::default(), "anything"), None);
        let rule = infer(&kia, "2026-03-01 Kia Finance statement").unwrap();
        assert_eq!(rule.apply(&Details::default()), None);
    }

    #[test]
    fn same_way_follows_the_latest_name_if_it_fits_the_others() {
        let a = details("Kia Finance statement", Some("2026-03-01"));
        let b = details("Progressive insurance card", Some("2026-03-28"));
        let t = details("DMV registration renewal", Some("2026-01-15"));
        let both = [
            (a.clone(), "Kia Finance statement 2026-03".to_string()),
            (b.clone(), "Progressive insurance card 2026-03".to_string()),
        ];
        assert_eq!(
            same_way(&both, std::slice::from_ref(&t)),
            Some(vec![Some("DMV registration renewal 2026-01".into())])
        );
        // The two typed names don't follow one rule.
        let mixed = [
            (a, "Kia Finance statement 2026-03".to_string()),
            (b, "2026-03-28 Progressive insurance card".to_string()),
        ];
        assert_eq!(same_way(&mixed, &[t]), None);
    }

    #[test]
    fn reads_the_proposed_name() {
        assert_eq!(
            parse_name("{\"name\": \"2026-03-01 Kia: statement.pdf\"}", "pdf").unwrap(),
            "2026-03-01 Kia statement"
        );
        assert!(parse_name("{\"name\": \"  \"}", "pdf").is_err());
        assert!(parse_name("no idea", "pdf").is_err());
    }

    #[test]
    fn asks_with_the_instruction_and_the_examples() {
        let target = details("DMV registration renewal", Some("2026-01-15"));
        let examples = [Example {
            details: details("Kia Finance statement", Some("2026-03-01")),
            name: "Kia Finance - statement - 2026-03".into(),
        }];
        let p = prompt(
            &Ask {
                instruction: "issuer - what - year-month",
                examples: &examples,
                target: &target,
                text: "DMV RENEWAL NOTICE",
                ext: "pdf",
            },
            100,
            Lang::De,
        );
        assert!(p.contains("issuer - what - year-month"));
        assert!(p.contains("they may write in German"));
        assert!(p.contains("→ \"Kia Finance - statement - 2026-03\""));
        assert!(p.contains("title \"DMV registration renewal\", document date 2026-01-15"));
        assert!(p.contains("DMV RENEWAL NOTICE"));
        // Neither: the default kind of name.
        let p = prompt(
            &Ask {
                instruction: "",
                examples: &[],
                target: &target,
                text: "",
                ext: "pdf",
            },
            100,
            Lang::En,
        );
        assert!(p.contains("then who it's from and what it is"));
    }

    fn library() -> (tempfile::TempDir, Source) {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("Car")).unwrap();
        for (name, body) in [
            ("a.pdf", "A"),
            ("b.pdf", "B"),
            ("c.txt", "C"),
            ("Taken.pdf", "T"),
        ] {
            fs::write(dir.path().join("Car").join(name), body).unwrap();
        }
        let source = Source {
            id: "test".into(),
            name: "Test".into(),
            root: dir.path().to_path_buf(),
            provider: crate::source::Provider::Local,
            show_all_types: true,
            download_online_only: false,
            bookmark: None,
        };
        (dir, source)
    }

    fn planned(rel: &str, stem: &str) -> Planned {
        Planned {
            rel_path: rel.into(),
            stem: stem.into(),
        }
    }

    #[test]
    fn checks_names_before_renaming() {
        let (_dir, source) = library();
        let checked = check(
            &source,
            "Car",
            &[
                planned("Car/a.pdf", "Taken"),
                planned("Car/b.pdf", "b"),
                planned("Car/c.txt", "Taken"),
                planned("Car/Taken.pdf", "Bad: name"),
            ],
        )
        .unwrap();
        let got: Vec<(&str, Option<&Problem>)> = checked
            .iter()
            .map(|c| (c.name.as_str(), c.problem.as_ref()))
            .collect();
        assert_eq!(got[0], ("Taken (2).pdf", Some(&Problem::Taken)));
        assert_eq!(got[1], ("b.pdf", Some(&Problem::Unchanged)));
        // Another extension: no clash.
        assert_eq!(got[2], ("Taken.txt", None));
        assert!(matches!(got[3].1, Some(Problem::Invalid { .. })));

        // Two documents swapping names: both free.
        let checked = check(
            &source,
            "Car",
            &[planned("Car/a.pdf", "b"), planned("Car/b.pdf", "a")],
        )
        .unwrap();
        assert!(checked.iter().all(|c| c.problem.is_none()), "{checked:?}");
        // Two planned names the same: the second gets " (2)".
        let checked = check(
            &source,
            "Car",
            &[planned("Car/a.pdf", "x"), planned("Car/b.pdf", "X")],
        )
        .unwrap();
        assert_eq!(checked[1].name, "X (2).pdf");
    }

    #[test]
    fn renames_swaps_and_undoes() {
        let (dir, source) = library();
        let car = dir.path().join("Car");
        let steps = [
            Step {
                rel_path: "Car/a.pdf".into(),
                name: "b.pdf".into(),
            },
            Step {
                rel_path: "Car/b.pdf".into(),
                name: "a.pdf".into(),
            },
            Step {
                rel_path: "Car/c.txt".into(),
                name: "Taken.txt".into(),
            },
        ];
        let done = apply(&source, &steps, None);
        assert!(done.failed.is_empty(), "{:?}", done.failed);
        assert_eq!(fs::read_to_string(car.join("a.pdf")).unwrap(), "B");
        assert_eq!(fs::read_to_string(car.join("b.pdf")).unwrap(), "A");
        assert_eq!(fs::read_to_string(car.join("Taken.txt")).unwrap(), "C");
        assert_eq!(fs::read_to_string(car.join("Taken.pdf")).unwrap(), "T");

        let back = undo(&source, &done.renamed, &|_| None);
        assert!(back.failed.is_empty(), "{:?}", back.failed);
        assert_eq!(fs::read_to_string(car.join("a.pdf")).unwrap(), "A");
        assert_eq!(fs::read_to_string(car.join("b.pdf")).unwrap(), "B");
        assert_eq!(fs::read_to_string(car.join("c.txt")).unwrap(), "C");

        // The title can follow the new name, keeping the other details.
        let note = car.join("c.txt");
        let tagged = DocMeta {
            tags: vec!["Car".into()],
            ..Default::default()
        };
        write_meta(&note, tagged, true).unwrap();
        let none = |_: &str| None;
        let titled = apply(
            &source,
            &[Step {
                rel_path: "Car/c.txt".into(),
                name: "Oil change.txt".into(),
            }],
            Some(&none),
        );
        assert!(titled.failed.is_empty(), "{:?}", titled.failed);
        assert!(titled.renamed[0].title_changed);
        let meta = read_meta(&car.join("Oil change.txt")).unwrap().meta;
        assert_eq!(
            (meta.title.as_deref(), meta.tags.as_slice()),
            (Some("Oil change"), &["Car".to_string()][..])
        );
        undo(&source, &titled.renamed, &none);
        let meta = read_meta(&note).unwrap().meta;
        assert_eq!((meta.title, meta.tags), (None, vec!["Car".to_string()]));

        // A name taken meanwhile is never replaced.
        let clash = apply(
            &source,
            &[Step {
                rel_path: "Car/a.pdf".into(),
                name: "Taken.pdf".into(),
            }],
            None,
        );
        assert_eq!(clash.failed.len(), 1);
        assert_eq!(fs::read_to_string(car.join("Taken.pdf")).unwrap(), "T");
    }
}
