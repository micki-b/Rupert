//! Export (#107): copies of a set of documents handed over together, in a
//! folder or a zip, with a list of what's in it (`index.csv` for a
//! spreadsheet, `index.html` to read): file, title, date, tags, notes and
//! where it came from. The originals aren't touched.
//!
//! Copying an online-only document reads it, which downloads it; the
//! window says how many first and asks.

use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::file_kind::FileKind;
use crate::fileops::clean_name;
use crate::i18n::tr;
use crate::meta::DocMeta;
use crate::rename::split_name;

/// The list's names inside the bundle.
pub const CSV_NAME: &str = "index.csv";
pub const HTML_NAME: &str = "index.html";

// MARK: - What goes in

/// One document to export.
#[derive(Debug, Clone)]
pub struct ExportDoc {
    /// The source's name (the top folder when several sources are mixed).
    pub source_name: String,
    /// Its path inside the source.
    pub rel_path: String,
    /// Where it is on disk.
    pub path: PathBuf,
    pub meta: DocMeta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    Zip,
    Folder,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    pub format: Format,
    /// Notes go along: in the list, and inside PDFs (as Share… does).
    pub include_notes: bool,
}

// MARK: - Names in the bundle

/// Each document's path inside the bundle, in order: its folders below the
/// one all of them share (with the source's name first when they come from
/// several sources), and its own name. Two that would be the same get
/// " (2)", " (3)"…
pub fn bundle_paths(docs: &[ExportDoc]) -> Vec<String> {
    let sources: HashSet<&str> = docs.iter().map(|d| d.source_name.as_str()).collect();
    let several = sources.len() > 1;
    let dir_of = |rel: &str| rel.rsplit_once('/').map_or("", |(d, _)| d).to_string();
    // The folder every document is in (per source when there are several).
    let shared = |source: &str| -> Vec<String> {
        let mut dirs = docs
            .iter()
            .filter(|d| !several || d.source_name == source)
            .map(|d| dir_of(&d.rel_path));
        let first: Vec<String> = dirs
            .next()
            .map(|d| {
                d.split('/')
                    .filter(|p| !p.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        dirs.fold(first, |common, d| {
            let parts: Vec<&str> = d.split('/').filter(|p| !p.is_empty()).collect();
            common
                .into_iter()
                .zip(parts)
                .take_while(|(a, b)| a == b)
                .map(|(a, _)| a)
                .collect()
        })
    };
    let mut taken: HashSet<String> = [CSV_NAME, HTML_NAME]
        .iter()
        .map(|s| s.to_lowercase())
        .collect();
    docs.iter()
        .map(|d| {
            let file = d.rel_path.rsplit('/').next().unwrap_or(&d.rel_path);
            let (name, ext) = split_name(file);
            let folder = {
                let common = shared(&d.source_name);
                let mut parts: Vec<String> = Vec::new();
                if several {
                    parts.push(clean_name(&d.source_name, ""));
                }
                parts.extend(
                    dir_of(&d.rel_path)
                        .split('/')
                        .filter(|p| !p.is_empty())
                        .skip(common.len())
                        .map(String::from),
                );
                parts.join("/")
            };
            let at = |n: usize| {
                let base = if n == 1 {
                    name.to_string()
                } else {
                    format!("{name} ({n})")
                };
                if folder.is_empty() {
                    format!("{base}{ext}")
                } else {
                    format!("{folder}/{base}{ext}")
                }
            };
            let mut n = 1;
            while taken.contains(&at(n).to_lowercase()) {
                n += 1;
            }
            let path = at(n);
            taken.insert(path.to_lowercase());
            path
        })
        .collect()
}

// MARK: - The list

/// The list's column names, in the app's language.
fn headers() -> [&'static str; 7] {
    [
        tr("File name"),
        tr("Title"),
        tr("Date"),
        tr("Tags"),
        tr("Notes"),
        tr("Original folder"),
        tr("Original name"),
    ]
}

/// One row of the list for each document.
fn rows(docs: &[ExportDoc], paths: &[String], include_notes: bool) -> Vec<[String; 7]> {
    docs.iter()
        .zip(paths)
        .map(|(d, p)| {
            let (dir, name) = d.rel_path.rsplit_once('/').unwrap_or(("", &d.rel_path));
            let folder = if dir.is_empty() {
                d.source_name.clone()
            } else {
                format!("{}/{dir}", d.source_name)
            };
            [
                p.clone(),
                d.meta.title.clone().unwrap_or_default(),
                d.meta.document_date.clone().unwrap_or_default(),
                d.meta.tags.join(", "),
                if include_notes {
                    d.meta.notes.clone().unwrap_or_default()
                } else {
                    String::new()
                },
                folder,
                name.to_string(),
            ]
        })
        .collect()
}

/// `index.csv`: UTF-8 with a byte order mark (so Excel reads accents),
/// fields quoted where needed. Without notes, the column is left out.
pub fn index_csv(docs: &[ExportDoc], paths: &[String], include_notes: bool) -> String {
    let field = |s: &str| {
        if s.contains([',', '"', '\n', '\r', ';']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };
    let keep = |i: usize| include_notes || i != 4;
    let line = |cells: &[String]| {
        cells
            .iter()
            .enumerate()
            .filter(|(i, _)| keep(*i))
            .map(|(_, c)| field(c))
            .collect::<Vec<_>>()
            .join(",")
    };
    let mut out = String::from("\u{feff}");
    out.push_str(&line(&headers().map(String::from)));
    out.push_str("\r\n");
    for r in rows(docs, paths, include_notes) {
        out.push_str(&line(&r));
        out.push_str("\r\n");
    }
    out
}

/// `index.html`: the same list as a page, each file a link.
pub fn index_html(
    docs: &[ExportDoc],
    paths: &[String],
    include_notes: bool,
    title: &str,
) -> String {
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };
    let href = |p: &str| {
        p.split('/')
            .map(|part| {
                part.bytes()
                    .map(|b| match b {
                        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                            (b as char).to_string()
                        }
                        _ => format!("%{b:02X}"),
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("/")
    };
    let keep = |i: usize| include_notes || i != 4;
    let mut out = format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>{t}</title>\n<style>\
         body{{font:14px -apple-system,system-ui,sans-serif;margin:24px;color:#222}}\
         table{{border-collapse:collapse;width:100%}}th,td{{text-align:left;vertical-align:top;\
         padding:6px 10px;border-bottom:1px solid #ddd}}th{{font-size:12px;color:#666}}\
         td.notes{{white-space:pre-wrap}}</style></head><body>\n<h1>{t}</h1>\n<p>{n}</p>\n<table>\n<tr>",
        t = esc(title),
        n = esc(&crate::i18n::trf(
            "{0} documents",
            &[&docs.len().to_string()]
        )),
    );
    for (i, h) in headers().iter().enumerate().filter(|(i, _)| keep(*i)) {
        let _ = i;
        out.push_str(&format!("<th>{}</th>", esc(h)));
    }
    out.push_str("</tr>\n");
    for r in rows(docs, paths, include_notes) {
        out.push_str("<tr>");
        for (i, cell) in r.iter().enumerate().filter(|(i, _)| keep(*i)) {
            match i {
                0 => out.push_str(&format!(
                    "<td><a href=\"{}\">{}</a></td>",
                    href(cell),
                    esc(cell)
                )),
                4 => out.push_str(&format!("<td class=\"notes\">{}</td>", esc(cell))),
                _ => out.push_str(&format!("<td>{}</td>", esc(cell))),
            }
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</table>\n</body></html>\n");
    out
}

// MARK: - Writing

/// What happened to each document.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub copied: usize,
    /// Documents that couldn't be copied, with why.
    pub failed: Vec<(String, String)>,
}

/// Writes the bundle to `dest` (a new folder, or the zip file), calling
/// `before(i)` before each document. `prepare` turns a document's file
/// into the one to copy (the same path, or a copy without notes); it may
/// download an online-only file. `stop` is checked between documents.
pub fn write(
    dest: &Path,
    docs: &[ExportDoc],
    opts: &Options,
    title: &str,
    mut before: impl FnMut(usize),
    prepare: impl Fn(&ExportDoc) -> Result<PathBuf>,
    stop: impl Fn() -> bool,
) -> Result<Report> {
    let paths = bundle_paths(docs);
    let mut report = Report::default();
    let csv = index_csv(docs, &paths, opts.include_notes);
    let html = index_html(docs, &paths, opts.include_notes, title);
    match opts.format {
        Format::Folder => {
            if dest.exists() {
                return Err(Error::InvalidValue(crate::i18n::trf(
                    "There's already something called “{0}” there.",
                    &[&dest.file_name().unwrap_or_default().to_string_lossy()],
                )));
            }
            fs::create_dir_all(dest)?;
            for (i, (d, p)) in docs.iter().zip(&paths).enumerate() {
                if stop() {
                    return Err(Error::Cancelled);
                }
                before(i);
                let to = dest.join(p);
                let result = (|| {
                    let from = prepare(d)?;
                    if let Some(dir) = to.parent() {
                        fs::create_dir_all(dir)?;
                    }
                    fs::copy(&from, &to)?;
                    // The file's own date, not today's.
                    if let Ok(modified) = fs::metadata(&d.path).and_then(|m| m.modified()) {
                        let _ = File::options()
                            .write(true)
                            .open(&to)
                            .and_then(|f| f.set_modified(modified));
                    }
                    Ok::<_, Error>(())
                })();
                match result {
                    Ok(()) => report.copied += 1,
                    Err(e) => report.failed.push((d.rel_path.clone(), e.to_string())),
                }
            }
            fs::write(dest.join(CSV_NAME), csv)?;
            fs::write(dest.join(HTML_NAME), html)?;
        }
        Format::Zip => {
            // Straight to the file chosen (the Mac App Store edition may
            // write only that one), removed again if it doesn't finish.
            let result = (|| {
                let mut zip = zip::ZipWriter::new(BufWriter::new(File::create(dest)?));
                let stored = zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored);
                let deflated = zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated);
                for (i, (d, p)) in docs.iter().zip(&paths).enumerate() {
                    if stop() {
                        return Err(Error::Cancelled);
                    }
                    before(i);
                    let from = match prepare(d) {
                        Ok(f) => f,
                        Err(e) => {
                            report.failed.push((d.rel_path.clone(), e.to_string()));
                            continue;
                        }
                    };
                    let meta = fs::metadata(&from)?;
                    // PDFs and pictures are compressed already.
                    let packed =
                        matches!(FileKind::from_path(&from), FileKind::Pdf | FileKind::Image);
                    let mut options = if packed { stored } else { deflated }
                        .large_file(meta.len() >= u32::MAX as u64);
                    if let Some(t) = fs::metadata(&d.path)
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(zip_time)
                    {
                        options = options.last_modified_time(t);
                    }
                    zip.start_file(p.as_str(), options).map_err(zip_err)?;
                    io::copy(&mut File::open(&from)?, &mut zip)?;
                    report.copied += 1;
                }
                zip.start_file(CSV_NAME, deflated).map_err(zip_err)?;
                zip.write_all(csv.as_bytes())?;
                zip.start_file(HTML_NAME, deflated).map_err(zip_err)?;
                zip.write_all(html.as_bytes())?;
                zip.finish().map_err(zip_err)?.flush()?;
                Ok(())
            })();
            if let Err(e) = result {
                let _ = fs::remove_file(dest);
                return Err(e);
            }
        }
    }
    Ok(report)
}

fn zip_err(e: zip::result::ZipError) -> Error {
    Error::Io(io::Error::other(e.to_string()))
}

/// A file date for the zip (it keeps local-looking times without a zone;
/// UTC is close enough for a list of documents).
fn zip_time(t: SystemTime) -> Option<zip::DateTime> {
    let secs = t.duration_since(UNIX_EPOCH).ok()?.as_secs();
    let s = crate::pdf_meta::format_utc(secs);
    let n = |a: usize, b: usize| s.get(a..b)?.parse::<u16>().ok();
    zip::DateTime::from_date_and_time(
        n(0, 4)?,
        n(5, 7)? as u8,
        n(8, 10)? as u8,
        n(11, 13)? as u8,
        n(14, 16)? as u8,
        n(17, 19)? as u8,
    )
    .ok()
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(source: &str, rel: &str, title: Option<&str>, date: Option<&str>) -> ExportDoc {
        ExportDoc {
            source_name: source.into(),
            rel_path: rel.into(),
            path: PathBuf::from(rel),
            meta: DocMeta {
                title: title.map(Into::into),
                document_date: date.map(Into::into),
                tags: vec!["Taxes".into(), "2025".into()],
                notes: Some("Paid, \"see\" bank".into()),
                ..Default::default()
            },
        }
    }

    fn opts() -> Options {
        Options {
            format: Format::Folder,
            include_notes: true,
        }
    }

    #[test]
    fn paths_keep_folders_below_the_shared_one() {
        let docs = [
            doc("Docs", "Taxes/2025/W-2.pdf", None, None),
            doc("Docs", "Taxes/2025/Receipts/a.pdf", None, None),
            doc("Docs", "Taxes/2025/Receipts/A.pdf", None, None),
            doc("Docs", "Taxes/2025/index.csv", None, None),
        ];
        assert_eq!(
            bundle_paths(&docs),
            ["W-2.pdf", "Receipts/a.pdf", "Receipts/A (2).pdf", "index (2).csv"]
        );
        // From two sources: each under its source's name.
        let mixed = [
            doc("Docs", "Car/title.pdf", None, None),
            doc("iCloud", "Car/Sale/bill.pdf", None, None),
        ];
        assert_eq!(bundle_paths(&mixed), ["Docs/title.pdf", "iCloud/bill.pdf"]);
    }

    #[test]
    fn the_list_quotes_fields_and_leaves_out_notes_unless_asked() {
        let docs = [doc(
            "Docs",
            "Taxes/W-2.pdf",
            Some("W-2, 2025"),
            Some("2026-01-30"),
        )];
        let paths = bundle_paths(&docs);
        let csv = index_csv(&docs, &paths, true);
        assert!(csv.starts_with('\u{feff}'));
        let lines: Vec<&str> = csv.trim_start_matches('\u{feff}').lines().collect();
        assert_eq!(
            lines[0],
            "File name,Title,Date,Tags,Notes,Original folder,Original name"
        );
        assert_eq!(
            lines[1],
            "W-2.pdf,\"W-2, 2025\",2026-01-30,\"Taxes, 2025\",\"Paid, \"\"see\"\" bank\",Docs/Taxes,W-2.pdf"
        );
        let without = index_csv(&docs, &paths, false);
        assert!(!without.contains("Notes") && !without.contains("bank"));
        let html = index_html(&docs, &paths, false, "Taxes & more");
        assert!(html.contains("<title>Taxes &amp; more</title>"));
        assert!(html.contains("<a href=\"W-2.pdf\">W-2.pdf</a>"));
        assert!(!html.contains("bank"));
    }

    #[test]
    fn writes_a_folder_or_a_zip_and_leaves_the_originals() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(src.join("Car")).unwrap();
        fs::write(src.join("Car/title.pdf"), b"%PDF-title").unwrap();
        fs::write(src.join("Car/notes.txt"), b"oil change").unwrap();
        let docs: Vec<ExportDoc> = ["Car/title.pdf", "Car/notes.txt"]
            .iter()
            .map(|r| ExportDoc {
                path: src.join(r),
                ..doc("Docs", r, None, None)
            })
            .collect();
        let prepare = |d: &ExportDoc| Ok(d.path.clone());

        let folder = dir.path().join("Car export");
        let mut seen = Vec::new();
        let report = write(
            &folder,
            &docs,
            &opts(),
            "Car",
            |i| seen.push(i),
            prepare,
            || false,
        )
        .unwrap();
        assert_eq!((report.copied, seen), (2, vec![0, 1]));
        assert_eq!(fs::read(folder.join("title.pdf")).unwrap(), b"%PDF-title");
        assert!(folder.join(CSV_NAME).is_file() && folder.join(HTML_NAME).is_file());
        // Not over an existing folder.
        assert!(
            write(
                &folder,
                &docs,
                &opts(),
                "Car",
                |_| {},
                prepare,
                || false
            )
            .is_err()
        );

        let zip_path = dir.path().join("Car.zip");
        let o = Options {
            format: Format::Zip,
            ..opts()
        };
        write(&zip_path, &docs, &o, "Car", |_| {}, prepare, || false).unwrap();
        let mut archive = zip::ZipArchive::new(File::open(&zip_path).unwrap()).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().unwrap().to_string())
            .collect();
        assert_eq!(names, ["title.pdf", "notes.txt", CSV_NAME, HTML_NAME]);
        let mut text = String::new();
        io::Read::read_to_string(&mut archive.by_name("notes.txt").unwrap(), &mut text).unwrap();
        assert_eq!(text, "oil change");

        // Stopped: nothing is left behind.
        let stopped = dir.path().join("Stopped.zip");
        assert!(write(&stopped, &docs, &o, "Car", |_| {}, prepare, || true).is_err());
        assert!(!stopped.exists());
        assert_eq!(fs::read(src.join("Car/title.pdf")).unwrap(), b"%PDF-title");
    }
}
