//! Import into Inbox (#122): many files and folders brought into the inbox
//! at once, to be read and filed like scans. Everything is a copy (the
//! originals stay), folders are taken with everything inside, and a
//! document whose content is already in the library is left out: the
//! inbox would only get a second copy of it. The content comparison is
//! here so the duplicate finder (#111) can use it too.

use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Serialize;

use crate::error::Result;
use crate::file_kind::{FileKind, is_ignored_name};
use crate::fileops;
use crate::index::Index;
use crate::listing::{CloudState, cloud_state, resolve};
use crate::source::Source;

// MARK: - What was chosen

/// One file or folder the user chose, as the Import window lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chosen {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    /// The documents it brings in (a file: 1 or 0).
    pub documents: usize,
}

/// What a set of chosen files and folders brings in.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub chosen: Vec<Chosen>,
    /// Every document to copy, folders flattened, in a steady order.
    pub files: Vec<String>,
    /// Files that aren't documents (or are hidden), left out.
    pub left_out: usize,
}

/// Whether a file name is one the inbox takes: a document, not hidden.
pub fn takes(name: &str) -> bool {
    !name.starts_with('.')
        && !is_ignored_name(name)
        && FileKind::from_path(Path::new(name)).is_document()
}

/// Lists what `paths` (files and folders) bring in.
pub fn plan(paths: &[PathBuf]) -> Plan {
    let mut plan = Plan::default();
    for path in paths {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let before = plan.files.len();
        let is_dir = path.is_dir();
        if is_dir {
            walk(path, &mut plan);
        } else if takes(&name) {
            plan.files.push(path.display().to_string());
        } else {
            plan.left_out += 1;
        }
        plan.chosen.push(Chosen {
            path: path.display().to_string(),
            name,
            is_dir,
            documents: plan.files.len() - before,
        });
    }
    plan
}

fn walk(dir: &Path, plan: &mut Plan) {
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = read.flatten().collect();
    entries.sort_by_key(|e| e.file_name().to_string_lossy().to_lowercase());
    for e in entries {
        let name = e.file_name().to_string_lossy().into_owned();
        let path = e.path();
        // Hidden folders (".rupert", ".git") and app bundles stay out.
        if name.starts_with('.') || is_ignored_name(&name) {
            continue;
        }
        match e.file_type() {
            Ok(t) if t.is_dir() => walk(&path, plan),
            Ok(t) if t.is_file() => {
                if takes(&name) {
                    plan.files.push(path.display().to_string());
                } else {
                    plan.left_out += 1;
                }
            }
            _ => {}
        }
    }
}

// MARK: - Already in the library

/// Whether two files hold the same bytes (read in step, stopping at the
/// first difference).
pub fn same_file_contents(a: &Path, b: &Path) -> io::Result<bool> {
    if fs::metadata(a)?.len() != fs::metadata(b)?.len() {
        return Ok(false);
    }
    let (mut a, mut b) = (
        BufReader::new(File::open(a)?),
        BufReader::new(File::open(b)?),
    );
    let (mut x, mut y) = ([0u8; 64 * 1024], [0u8; 64 * 1024]);
    loop {
        let n = a.read(&mut x)?;
        if n == 0 {
            return Ok(true);
        }
        b.read_exact(&mut y[..n])?;
        if x[..n] != y[..n] {
            return Ok(false);
        }
    }
}

/// Whether a file holds `bytes`.
pub fn same_contents(bytes: &[u8], file: &Path) -> io::Result<bool> {
    if fs::metadata(file)?.len() != bytes.len() as u64 {
        return Ok(false);
    }
    let mut f = BufReader::new(File::open(file)?);
    let mut buf = [0u8; 64 * 1024];
    let mut at = 0;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            return Ok(at == bytes.len());
        }
        if bytes.get(at..at + n) != Some(&buf[..n]) {
            return Ok(false);
        }
        at += n;
    }
}

/// The library's documents of `size` bytes: the only ones that can have
/// the same content (looked up apart, so the index isn't held while files
/// are compared).
pub fn same_size(index: &Index, size: u64) -> Result<Vec<(String, String)>> {
    index.find_by_size(size)
}

/// Among `candidates` (same size, from [`same_size`]), a document with the
/// same content as the one being imported (named `name`; `same` compares
/// its content with a library file). A library file that's online-only
/// isn't read (that would download it): there the same name counts.
pub fn find_in_library(
    candidates: Vec<(String, String)>,
    sources: &[Source],
    name: &str,
    same: &dyn Fn(&Path) -> bool,
) -> Option<(String, String)> {
    for (source_id, rel_path) in candidates {
        let Some(source) = sources.iter().find(|s| s.id == source_id) else {
            continue;
        };
        let Ok(path) = resolve(&source.root, &rel_path) else {
            continue;
        };
        let hit = match cloud_state(&path) {
            Ok(CloudState::Local) => same(&path),
            Ok(CloudState::CloudOnly) => {
                let theirs = rel_path.rsplit('/').next().unwrap_or(&rel_path);
                theirs.eq_ignore_ascii_case(name)
            }
            Err(_) => false,
        };
        if hit {
            return Some((source_id, rel_path));
        }
    }
    None
}

// MARK: - Copying in

/// Copies the file at `from` into `inbox`, keeping its name (" (2)" if
/// taken) and modification time. Reading it downloads an online-only one.
pub fn copy_file_in(from: &Path, inbox: &Path) -> Result<PathBuf> {
    let name = from
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let contents = fs::read(from)?;
    let modified: Option<SystemTime> = fs::metadata(from).and_then(|m| m.modified()).ok();
    fileops::copy_in(inbox, &name, &contents, modified, true)
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_files_and_folders() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("Old scans");
        fs::create_dir_all(old.join("2021")).unwrap();
        fs::create_dir_all(old.join(".rupert")).unwrap();
        for (p, body) in [
            ("Old scans/a.pdf", "A"),
            ("Old scans/2021/b.pdf", "B"),
            ("Old scans/2021/photo.jpg", "P"),
            ("Old scans/.DS_Store", "x"),
            ("Old scans/.rupert/meta.json", "{}"),
            ("Old scans/tool.exe", "x"),
            ("Statement.pdf", "S"),
            ("song.mp3", "M"),
        ] {
            fs::write(dir.path().join(p), body).unwrap();
        }
        let plan = plan(&[
            old.clone(),
            dir.path().join("Statement.pdf"),
            dir.path().join("song.mp3"),
        ]);
        let names: Vec<_> = plan
            .chosen
            .iter()
            .map(|c| (c.name.as_str(), c.is_dir, c.documents))
            .collect();
        assert_eq!(
            names,
            [
                ("Old scans", true, 3),
                ("Statement.pdf", false, 1),
                ("song.mp3", false, 0)
            ]
        );
        assert_eq!(plan.files.len(), 4);
        assert!(plan.files[0].ends_with("Old scans/2021/b.pdf"));
        // tool.exe and song.mp3 aren't documents; hidden files don't count.
        assert_eq!(plan.left_out, 2);
    }

    #[test]
    fn compares_contents() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b, c) = (
            dir.path().join("a"),
            dir.path().join("b"),
            dir.path().join("c"),
        );
        let big: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        fs::write(&a, &big).unwrap();
        fs::write(&b, &big).unwrap();
        let mut other = big.clone();
        other[150_000] ^= 1;
        fs::write(&c, &other).unwrap();
        assert!(same_file_contents(&a, &b).unwrap());
        assert!(!same_file_contents(&a, &c).unwrap());
        assert!(same_contents(&big, &b).unwrap());
        assert!(!same_contents(&other, &b).unwrap());
        assert!(!same_contents(b"short", &b).unwrap());
    }

    #[test]
    fn copies_in_keeping_name_and_date() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = dir.path().join("Inbox");
        fs::create_dir(&inbox).unwrap();
        let from = dir.path().join("Water bill.pdf");
        fs::write(&from, "%PDF").unwrap();
        let when = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        File::options()
            .write(true)
            .open(&from)
            .unwrap()
            .set_modified(when)
            .unwrap();
        let first = copy_file_in(&from, &inbox).unwrap();
        let second = copy_file_in(&from, &inbox).unwrap();
        assert_eq!(first.file_name().unwrap(), "Water bill.pdf");
        assert_eq!(second.file_name().unwrap(), "Water bill (2).pdf");
        assert_eq!(fs::metadata(&first).unwrap().modified().unwrap(), when);
        assert!(from.exists());
    }
}
