//! From, amount and due date for documents already in the library (#108):
//! "Read details…" proposes them from each document's indexed text (see
//! [`crate::llm::read_details`]; nothing is downloaded), the user reviews
//! them, and [`apply`] writes the reviewed ones.

use std::path::Path;

use crate::error::Result;
use crate::listing::CloudState;
use crate::llm::Details;
use crate::meta::{self, DocMeta};

/// Writes `d` into the document's details, keeping everything else.
///
/// A local file gets them where its details live (inside a PDF, or its
/// sidecar). An online-only one isn't read: they wait in the sidecar and
/// move into the PDF on its next save, as when filing from the inbox. A
/// detail cleared in the review can't be cleared that way (the waiting
/// details only add); it stays as the file has it.
pub fn apply(path: &Path, d: &Details) -> Result<DocMeta> {
    let local =
        crate::cloud_state(path)? == CloudState::Local || crate::cloud::streams_on_demand(path);
    let with = |m: DocMeta| DocMeta {
        sender: d.sender.clone(),
        amount: d.amount.clone(),
        currency: d.currency.clone(),
        due_date: d.due_date.clone(),
        ..m
    };
    if local {
        let existing = meta::read_meta(path)?.meta;
        Ok(meta::write_meta(path, with(existing), true)?.meta)
    } else {
        let waiting = meta::read_meta_beside(path)?
            .map(|(m, _)| m)
            .unwrap_or_default();
        meta::write_meta_beside(path, with(waiting))
    }
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_the_details_and_keeps_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bill.txt");
        std::fs::write(&path, "City Electric, total 84.20 EUR").unwrap();
        meta::write_meta(
            &path,
            DocMeta {
                title: Some("Electric bill".into()),
                tags: vec!["Home".into()],
                sender: Some("Old name".into()),
                ..Default::default()
            },
            true,
        )
        .unwrap();
        let saved = apply(
            &path,
            &Details {
                sender: Some("City Electric".into()),
                amount: Some("84.20".into()),
                currency: Some("EUR".into()),
                due_date: None,
            },
        )
        .unwrap();
        assert_eq!(saved.title.as_deref(), Some("Electric bill"));
        assert_eq!(saved.tags, ["Home"]);
        assert_eq!(saved.sender.as_deref(), Some("City Electric"));
        assert_eq!(saved.amount.as_deref(), Some("84.20"));
        let read = meta::read_meta(&path).unwrap().meta;
        assert_eq!(read, saved);
    }
}
