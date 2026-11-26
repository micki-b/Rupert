//! The tag manager: rename, merge or delete a tag across every document
//! that has it. The same edits, plus adding tags, work on a chosen set of
//! documents (several selected in the window, #102). Changes go where each
//! document's details live, as when editing one document:
//!
//! - local PDFs: inside the file (XMP and Info dictionary, appended);
//! - other formats: their sidecar (the file itself isn't read);
//! - online-only PDFs: pending details in the sidecar, so nothing is
//!   downloaded. Pending details can only add or replace, so removing a
//!   PDF's last tag that way wouldn't stick: those files are changed in
//!   place, which downloads them, and the preview says how many.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::file_kind::FileKind;
use crate::i18n::tr;
use crate::index::Index;
use crate::listing::{CloudState, cloud_state, resolve};
use crate::meta::{self, DocMeta};
use crate::source::Source;

// MARK: - Edits

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum TagAction {
    /// Replace the tags with this one (several tags: a merge). Also fixes
    /// spelling and capitalization.
    Rename { to: String },
    /// Remove the tags.
    Delete,
    /// Add these tags where they're missing (#102; `from` isn't used).
    Add { tags: Vec<String> },
    /// Undo (#104): put `before` back, but only where the tags are still
    /// exactly `after`, as the edit left them (`from` isn't used).
    Restore {
        before: Vec<String>,
        after: Vec<String>,
    },
}

/// What an edit did to one document, for Undo (#104).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagChange {
    pub source_id: String,
    pub rel_path: String,
    pub before: Vec<String>,
    pub after: Vec<String>,
}

/// A document, by source and path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocRef {
    pub source_id: String,
    pub rel_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagEdit {
    /// The tags to change (matched ignoring case).
    pub from: Vec<String>,
    pub action: TagAction,
    /// Only these documents (#102); every document with the tags otherwise.
    #[serde(default)]
    pub only: Option<Vec<DocRef>>,
}

impl TagEdit {
    /// Checks the edit makes sense before touching anything.
    pub fn validate(&self) -> Result<()> {
        let none = |tags: &[String]| tags.iter().all(|t| t.trim().is_empty());
        let empty = match &self.action {
            TagAction::Add { tags } => none(tags),
            TagAction::Restore { .. } => false,
            _ => none(&self.from),
        };
        if empty {
            return Err(Error::InvalidValue(tr("choose at least one tag").into()));
        }
        if let TagAction::Rename { to } = &self.action
            && to.trim().is_empty()
        {
            return Err(Error::InvalidValue(tr("the new tag name is empty").into()));
        }
        Ok(())
    }

    /// The document's tags after the edit, or `None` if nothing changes.
    /// The renamed tag takes the place of the first tag it replaces, and
    /// appears only once.
    pub fn apply(&self, tags: &[String]) -> Option<Vec<String>> {
        if let TagAction::Restore { before, after } = &self.action {
            return (tags == after.as_slice() && before != after).then(|| before.clone());
        }
        if let TagAction::Add { tags: add } = &self.action {
            let mut out = tags.to_vec();
            for t in add.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
                if !out.iter().any(|o| o.trim().eq_ignore_ascii_case(t)) {
                    out.push(t.to_string());
                }
            }
            return (out != tags).then_some(out);
        }
        let from: Vec<String> = self.from.iter().map(|t| t.trim().to_lowercase()).collect();
        let hit = |t: &str| from.contains(&t.trim().to_lowercase());
        if !tags.iter().any(|t| hit(t)) {
            return None;
        }
        let mut out: Vec<String> = Vec::with_capacity(tags.len());
        match &self.action {
            TagAction::Delete => out.extend(tags.iter().filter(|t| !hit(t)).cloned()),
            TagAction::Add { .. } | TagAction::Restore { .. } => unreachable!("handled above"),
            TagAction::Rename { to } => {
                let to = to.trim();
                let mut placed = false;
                for t in tags {
                    if hit(t) || t.trim().eq_ignore_ascii_case(to) {
                        if !placed {
                            out.push(to.to_string());
                            placed = true;
                        }
                    } else {
                        out.push(t.clone());
                    }
                }
            }
        }
        (out != tags).then_some(out)
    }
}

// MARK: - Plan

/// How a document's change is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Method {
    /// Inside the (local) PDF.
    InFile,
    /// In the sidecar, which holds this format's details.
    Sidecar,
    /// As pending details beside an online-only PDF.
    Pending,
    /// The online-only PDF is downloaded and changed in place.
    Download,
}

fn method(path: &Path, cloud: CloudState, new_tags: &[String]) -> Method {
    match (FileKind::from_path(path) == FileKind::Pdf, cloud) {
        (false, _) => Method::Sidecar,
        (true, CloudState::Local) => Method::InFile,
        (true, CloudState::CloudOnly) if new_tags.is_empty() => Method::Download,
        (true, CloudState::CloudOnly) => Method::Pending,
    }
}

/// What an edit would do, before doing it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub documents: usize,
    pub in_file: usize,
    pub sidecar: usize,
    pub pending: usize,
    pub download: usize,
}

/// One document the edit touches.
#[derive(Debug, Clone)]
pub struct Target {
    pub source: Source,
    pub rel_path: String,
    cloud: CloudState,
    method: Method,
}

/// The documents an edit would change, from the index.
pub fn plan(index: &Index, sources: &[Source], edit: &TagEdit) -> Result<(Vec<Target>, Preview)> {
    edit.validate()?;
    let mut targets = Vec::new();
    let mut preview = Preview::default();
    let candidates: Vec<(String, String, Option<CloudState>)> = match &edit.only {
        // Their cloud state is read below, from the file system (it doesn't
        // download anything).
        Some(docs) => docs
            .iter()
            .map(|d| (d.source_id.clone(), d.rel_path.clone(), None))
            .collect(),
        None => index
            .documents_with_tags(&edit.from)?
            .into_iter()
            .map(|(s, r, c)| (s, r, Some(c)))
            .collect(),
    };
    for (source_id, rel_path, cloud) in candidates {
        let Some(source) = sources.iter().find(|s| s.id == source_id) else {
            continue;
        };
        let Some(current) = index.doc_meta(&source_id, &rel_path)? else {
            continue;
        };
        let Some(new_tags) = edit.apply(&current.tags) else {
            continue;
        };
        let path = resolve(&source.root, &rel_path)?;
        let cloud = match cloud {
            Some(c) => c,
            None => cloud_state(&path)?,
        };
        let m = method(&path, cloud, &new_tags);
        match m {
            Method::InFile => preview.in_file += 1,
            Method::Sidecar => preview.sidecar += 1,
            Method::Pending => preview.pending += 1,
            Method::Download => preview.download += 1,
        }
        preview.documents += 1;
        targets.push(Target {
            source: source.clone(),
            rel_path,
            cloud,
            method: m,
        });
    }
    Ok((targets, preview))
}

// MARK: - Apply

/// Changes one document: what changed, or `None` if its tags turned out
/// not to need a change (e.g. edited meanwhile).
pub fn apply_one(index: &Index, target: &Target, edit: &TagEdit) -> Result<Option<TagChange>> {
    let path = resolve(&target.source.root, &target.rel_path)?;
    // Current details: from the file where that's safe, otherwise as indexed.
    let current: DocMeta = match target.method {
        Method::Pending => index
            .doc_meta(&target.source.id, &target.rel_path)?
            .unwrap_or_default(),
        _ => meta::read_meta(&path)?.meta,
    };
    let Some(tags) = edit.apply(&current.tags) else {
        return Ok(None);
    };
    let change = TagChange {
        source_id: target.source.id.clone(),
        rel_path: target.rel_path.clone(),
        before: current.tags.clone(),
        after: tags.clone(),
    };
    let updated = DocMeta { tags, ..current };
    match target.method {
        Method::Pending => {
            meta::write_meta_beside(&path, updated)?;
        }
        _ => {
            meta::write_meta(&path, updated, target.cloud == CloudState::Local)?;
        }
    }
    Ok(Some(change))
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::SearchQuery;
    use crate::indexer::tests::index_as_local;
    use crate::source::Provider;
    use std::fs;

    fn edit(from: &[&str], action: TagAction) -> TagEdit {
        TagEdit {
            from: from.iter().map(|s| s.to_string()).collect(),
            action,
            only: None,
        }
    }

    fn tags(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn rename_merge_and_delete_tag_lists() {
        let rename = edit(
            &["insurance"],
            TagAction::Rename {
                to: "Insurance".into(),
            },
        );
        assert_eq!(
            rename.apply(&tags(&["Boat", "insurance"])),
            Some(tags(&["Boat", "Insurance"]))
        );
        assert_eq!(
            rename.apply(&tags(&["Boat", "Insurance"])),
            None,
            "already right"
        );
        assert_eq!(rename.apply(&tags(&["Boat"])), None);

        let merge = edit(
            &["Ins.", "insurance policy"],
            TagAction::Rename {
                to: "Insurance".into(),
            },
        );
        assert_eq!(
            merge.apply(&tags(&["insurance policy", "Boat", "Ins.", "Insurance"])),
            Some(tags(&["Insurance", "Boat"])),
            "one copy, where the first one was"
        );

        let delete = edit(&["TEMP"], TagAction::Delete);
        assert_eq!(
            delete.apply(&tags(&["temp", "Boat"])),
            Some(tags(&["Boat"]))
        );
        assert_eq!(delete.apply(&tags(&["temp"])), Some(vec![]));

        assert!(edit(&[" "], TagAction::Delete).validate().is_err());
        assert!(
            edit(&["a"], TagAction::Rename { to: " ".into() })
                .validate()
                .is_err()
        );
    }

    #[test]
    fn edits_pdfs_in_place_and_others_in_sidecars() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        crate::pdf_meta::tests::make_pdf(&root.join("hull.pdf"), None);
        fs::write(root.join("notes.txt"), "Haul-out notes").unwrap();
        let tagged = |file: &str, t: &[&str]| {
            meta::write_meta(
                &root.join(file),
                DocMeta {
                    title: Some(file.into()),
                    tags: tags(t),
                    ..Default::default()
                },
                true,
            )
            .unwrap();
        };
        tagged("hull.pdf", &["Boat", "ins."]);
        tagged("notes.txt", &["ins."]);

        let source = Source {
            id: "src".into(),
            name: "Test".into(),
            root: root.to_path_buf(),
            provider: Provider::Local,
            show_all_types: false,
            download_online_only: false,
            bookmark: None,
        };
        let index = index_as_local(root, &source);
        let merge = edit(
            &["Ins."],
            TagAction::Rename {
                to: "Insurance".into(),
            },
        );
        let (targets, preview) = plan(&index, std::slice::from_ref(&source), &merge).unwrap();
        assert_eq!(
            preview,
            Preview {
                documents: 2,
                in_file: 1,
                sidecar: 1,
                ..Default::default()
            }
        );
        for t in &targets {
            assert!(apply_one(&index, t, &merge).unwrap().is_some());
        }
        let pdf = meta::read_meta(&root.join("hull.pdf")).unwrap();
        assert_eq!(pdf.meta.tags, tags(&["Boat", "Insurance"]));
        assert_eq!(
            pdf.meta.title.as_deref(),
            Some("hull.pdf"),
            "other details kept"
        );
        assert_eq!(pdf.storage, crate::meta::Storage::Embedded);
        let txt = meta::read_meta(&root.join("notes.txt")).unwrap();
        assert_eq!(txt.meta.tags, tags(&["Insurance"]));

        // Re-indexed, the old tag is gone and the new one has both.
        let index = index_as_local(root, &source);
        let found = index
            .search(&SearchQuery {
                tags: tags(&["insurance"]),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(found.len(), 2);
        assert!(
            index
                .documents_with_tags(&tags(&["ins."]))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn adds_missing_tags_only() {
        let add = edit(
            &[],
            TagAction::Add {
                tags: tags(&["Insurance", " 2026 ", ""]),
            },
        );
        assert_eq!(
            add.apply(&tags(&["insurance", "Car"])),
            Some(tags(&["insurance", "Car", "2026"])),
            "a tag already there in other capitals isn't added again"
        );
        assert_eq!(add.apply(&tags(&["Insurance", "2026"])), None);
        assert_eq!(add.apply(&[]), Some(tags(&["Insurance", "2026"])));
        assert!(
            edit(&[], TagAction::Add { tags: tags(&[" "]) })
                .validate()
                .is_err()
        );
        assert!(add.validate().is_ok(), "Add needs no `from`");
    }

    #[test]
    fn restores_only_tags_left_as_the_edit_made_them() {
        let undo = edit(
            &[],
            TagAction::Restore {
                before: tags(&["Receipt"]),
                after: tags(&["Receipt", "2026"]),
            },
        );
        assert!(undo.validate().is_ok());
        assert_eq!(
            undo.apply(&tags(&["Receipt", "2026"])),
            Some(tags(&["Receipt"]))
        );
        assert_eq!(
            undo.apply(&tags(&["Receipt", "2026", "Car"])),
            None,
            "changed since: left alone"
        );
    }

    #[test]
    fn edits_only_the_chosen_documents() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for f in ["a.txt", "b.txt", "c.txt"] {
            fs::write(root.join(f), f).unwrap();
            meta::write_meta(
                &root.join(f),
                DocMeta {
                    tags: tags(&["Receipts"]),
                    ..Default::default()
                },
                true,
            )
            .unwrap();
        }
        let source = Source {
            id: "src".into(),
            name: "Test".into(),
            root: root.to_path_buf(),
            provider: Provider::Local,
            show_all_types: false,
            download_online_only: false,
            bookmark: None,
        };
        let index = index_as_local(root, &source);
        let only = |files: &[&str]| {
            Some(
                files
                    .iter()
                    .map(|f| DocRef {
                        source_id: "src".into(),
                        rel_path: f.to_string(),
                    })
                    .collect(),
            )
        };
        let sources = std::slice::from_ref(&source);
        let mut add = edit(
            &[],
            TagAction::Add {
                tags: tags(&["2026"]),
            },
        );
        add.only = only(&["a.txt", "b.txt"]);
        let (targets, preview) = plan(&index, sources, &add).unwrap();
        assert_eq!(preview.documents, 2);
        assert_eq!(preview.sidecar, 2);
        for t in &targets {
            assert!(apply_one(&index, t, &add).unwrap().is_some());
        }
        let mut remove = edit(&["receipts"], TagAction::Delete);
        remove.only = only(&["b.txt"]);
        for t in &plan(&index, sources, &remove).unwrap().0 {
            assert!(apply_one(&index, t, &remove).unwrap().is_some());
        }
        let tags_of = |f: &str| meta::read_meta(&root.join(f)).unwrap().meta.tags;
        assert_eq!(tags_of("a.txt"), tags(&["Receipts", "2026"]));
        assert_eq!(tags_of("b.txt"), tags(&["2026"]));
        assert_eq!(
            tags_of("c.txt"),
            tags(&["Receipts"]),
            "not chosen: untouched"
        );
    }

    #[test]
    fn pending_details_change_tags_without_reading_the_pdf() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        crate::pdf_meta::tests::make_pdf(&root.join("deed.pdf"), None);
        let source = Source {
            id: "src".into(),
            name: "Test".into(),
            root: root.to_path_buf(),
            provider: Provider::OneDrive,
            show_all_types: false,
            download_online_only: false,
            bookmark: None,
        };
        // As if imported for an online-only PDF: details pending beside it.
        meta::write_meta_beside(
            &root.join("deed.pdf"),
            DocMeta {
                title: Some("Slip deed".into()),
                tags: tags(&["ins.", "Boat"]),
                ..Default::default()
            },
        )
        .unwrap();
        let index = index_as_local(root, &source);
        let target = Target {
            source: source.clone(),
            rel_path: "deed.pdf".into(),
            cloud: CloudState::CloudOnly,
            method: Method::Pending,
        };
        let merge = edit(
            &["ins."],
            TagAction::Rename {
                to: "Insurance".into(),
            },
        );
        assert!(apply_one(&index, &target, &merge).unwrap().is_some());
        let (side, pending) = meta::read_meta_beside(&root.join("deed.pdf"))
            .unwrap()
            .unwrap();
        assert!(pending);
        assert_eq!(side.tags, tags(&["Insurance", "Boat"]));
        assert_eq!(side.title.as_deref(), Some("Slip deed"));
    }

    #[test]
    fn online_only_pdfs_get_pending_details_unless_left_without_tags() {
        let p = Path::new("/lib/a.pdf");
        assert_eq!(
            method(p, CloudState::CloudOnly, &tags(&["Boat"])),
            Method::Pending
        );
        assert_eq!(method(p, CloudState::CloudOnly, &[]), Method::Download);
        assert_eq!(method(p, CloudState::Local, &[]), Method::InFile);
        assert_eq!(
            method(Path::new("/lib/a.txt"), CloudState::CloudOnly, &[]),
            Method::Sidecar
        );
    }
}
