//! Adds an invisible text layer to scanned PDF pages, so PDF readers can
//! search and select the text that OCR recognized (the same idea as
//! Tesseract's and OCRmyPDF's "sandwich" PDFs).
//!
//! - Each word is drawn with text rendering mode 3 (invisible), stretched
//!   to the box OCR found for it, so selection lines up with the scan.
//! - The font is a "glyphless" Unicode font, as Tesseract uses: a Type 0
//!   font (Identity-H, two bytes per character = the UTF-16 code unit)
//!   over a tiny embedded TrueType program whose glyphs are all empty, with
//!   fixed widths and a ToUnicode map. Every reader (PDFKit/Preview,
//!   Acrobat, PDFium, Poppler) can extract it, and every character
//!   (accents, ß, ñ, €) maps back to its Unicode value.
//! - The page's existing content is wrapped in `q … Q` and the text added
//!   after it. Nothing visible changes.
//! - Like metadata, it's written as an incremental update: the original
//!   bytes stay intact, the result is checked in memory before anything is
//!   appended, and a file that changed meanwhile is left alone.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use lopdf::{Dictionary, Document, IncrementalDocument, Object, ObjectId, Stream};

use crate::error::{Error, Result};
use crate::pdf_meta::{append, unwritable_reason};

/// Glyph width in the font's 1000-unit space; every glyph is the same.
const GLYPH_WIDTH: f64 = 500.0;
/// Glyph box from the baseline: 200 below, 800 above.
const DESCENT: f64 = 0.2;
const FONT_NAME: &str = "GlyphLessFont";

// MARK: - Input

/// A word and its corners in PDF user space: top-left, top-right,
/// bottom-right, bottom-left (as read, i.e. "top" is up on the screen).
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedWord {
    pub text: String,
    pub quad: [(f64, f64); 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageWords {
    /// 0-based page index.
    pub index: usize,
    pub words: Vec<PlacedWord>,
}

// MARK: - Writing

/// Appends the text layer for `pages` to the PDF at `path`. `extract`
/// returns the text of a whole PDF given its bytes; it's used to check that
/// the updated file reads back with the new text before it is written.
pub fn add(
    path: &Path,
    pages: &[PageWords],
    extract: impl Fn(Vec<u8>) -> Result<String>,
) -> Result<()> {
    let before = fs::metadata(path)?;
    let mut inc = IncrementalDocument::load(path).map_err(pdf_err)?;
    let prev = inc.get_prev_documents();
    if let Some(reason) = unwritable_reason(prev) {
        return Err(Error::Unsupported(reason.replace(
            "details are kept in a sidecar file",
            "Rupert won't add a text layer to it",
        )));
    }
    let page_ids = prev.get_pages(); // 1-based page number → object id

    let font = glyphless_font(&mut inc.new_document);

    for page in pages {
        let Some(&page_id) = page_ids.get(&(page.index as u32 + 1)) else {
            continue;
        };
        let words: Vec<&PlacedWord> = page
            .words
            .iter()
            .filter(|w| !w.text.trim().is_empty())
            .collect();
        if words.is_empty() {
            continue;
        }
        add_to_page(&mut inc, page_id, &words, font)?;
    }

    // Trailer for the update: only the keys that belong in it.
    let prev_trailer = inc.get_prev_documents().trailer.clone();
    let mut trailer = Dictionary::new();
    for key in [&b"Root"[..], b"ID", b"Prev", b"Info"] {
        if let Ok(value) = inc
            .new_document
            .trailer
            .get(key)
            .or_else(|_| prev_trailer.get(key))
        {
            trailer.set(key.to_vec(), value.clone());
        }
    }
    inc.new_document.trailer = trailer;

    let mut updated = Vec::new();
    inc.save_to(&mut updated).map_err(pdf_err)?;
    let prev_len = inc.get_prev_documents_bytes().len();
    verify(&updated, pages, &extract)?;

    let now = fs::metadata(path)?;
    if now.len() != before.len() || now.modified().ok() != before.modified().ok() {
        return Err(Error::InvalidValue(
            "the file changed while recognizing its text; please try again".into(),
        ));
    }
    append(path, prev_len as u64, &updated[prev_len..])
}

fn add_to_page(
    inc: &mut IncrementalDocument,
    page_id: ObjectId,
    words: &[&PlacedWord],
    font: ObjectId,
) -> Result<()> {
    // The font, under a name that can't clash with the page's own.
    let prev = inc.get_prev_documents();
    let mut resources = effective_resources(prev, page_id);
    let mut fonts = match resources.get(b"Font") {
        Ok(Object::Reference(id)) => prev.get_dictionary(*id).cloned().unwrap_or_default(),
        Ok(Object::Dictionary(d)) => d.clone(),
        _ => Dictionary::new(),
    };
    let mut name = String::from("RupertOCR");
    while fonts.has(name.as_bytes()) {
        name.push('x');
    }
    fonts.set(name.clone(), Object::Reference(font));
    resources.set("Font", Object::Dictionary(fonts));

    // The invisible text: one positioned run per word, plus a space.
    let mut content = format!("BT\n3 Tr\n/{name} 1 Tf\n");
    for w in words {
        let Some(tm) = text_matrix(w) else { continue };
        let hex: String = w
            .text
            .chars()
            .chain([' '])
            .map(|c| {
                let mut units = [0u16; 2];
                match c.encode_utf16(&mut units) {
                    [u] => format!("{u:04X}"),
                    _ => "FFFD".into(), // outside the BMP: rare in documents
                }
            })
            .collect();
        let _ = writeln!(
            content,
            "{} {} {} {} {} {} Tm <{hex}> Tj",
            num(tm[0]),
            num(tm[1]),
            num(tm[2]),
            num(tm[3]),
            num(tm[4]),
            num(tm[5])
        );
    }
    content.push_str("ET\n");

    let mut text_stream = Stream::new(Dictionary::new(), content.into_bytes());
    let _ = text_stream.compress();
    let text_id = inc.new_document.add_object(text_stream);
    let open = inc
        .new_document
        .add_object(Stream::new(Dictionary::new(), b"q\n".to_vec()));
    let close = inc
        .new_document
        .add_object(Stream::new(Dictionary::new(), b"Q\n".to_vec()));

    // Contents: [q, original…, Q, text].
    let prev = inc.get_prev_documents();
    let page = prev.get_dictionary(page_id).map_err(pdf_err)?;
    let mut contents: Vec<Object> = vec![Object::Reference(open)];
    match page.get(b"Contents") {
        Ok(Object::Reference(id)) => match prev.get_object(*id) {
            Ok(Object::Array(items)) => contents.extend(items.iter().cloned()),
            _ => contents.push(Object::Reference(*id)),
        },
        Ok(Object::Array(items)) => contents.extend(items.iter().cloned()),
        _ => {}
    }
    contents.push(Object::Reference(close));
    contents.push(Object::Reference(text_id));

    inc.opt_clone_object_to_new_document(page_id)
        .map_err(pdf_err)?;
    let page = inc
        .new_document
        .get_object_mut(page_id)
        .and_then(Object::as_dict_mut)
        .map_err(pdf_err)?;
    page.set("Contents", Object::Array(contents));
    page.set("Resources", Object::Dictionary(resources));
    Ok(())
}

/// The page's resources, including ones inherited from the page tree.
fn effective_resources(doc: &Document, page_id: ObjectId) -> Dictionary {
    let mut node = doc.get_dictionary(page_id).ok();
    for _ in 0..64 {
        let Some(dict) = node else { break };
        match dict.get(b"Resources") {
            Ok(Object::Reference(id)) => {
                return doc.get_dictionary(*id).cloned().unwrap_or_default();
            }
            Ok(Object::Dictionary(d)) => return d.clone(),
            _ => {}
        }
        node = dict
            .get(b"Parent")
            .and_then(Object::as_reference)
            .ok()
            .and_then(|id| doc.get_dictionary(id).ok());
    }
    Dictionary::new()
}

// MARK: - The glyphless font

/// Adds the Type 0 font (and its parts) to `doc`; returns the font's id.
fn glyphless_font(doc: &mut Document) -> ObjectId {
    let int = |v: i64| Object::Integer(v);
    let name = |n: &str| Object::Name(n.as_bytes().to_vec());

    let program = glyphless_ttf();
    let mut font_file = Stream::new(
        Dictionary::from_iter([("Length1", int(program.len() as i64))]),
        program,
    );
    let _ = font_file.compress();
    let font_file = doc.add_object(font_file);

    let descriptor = doc.add_object(Dictionary::from_iter([
        ("Type", name("FontDescriptor")),
        ("FontName", name(FONT_NAME)),
        ("Flags", int(5)), // fixed pitch, symbolic
        (
            "FontBBox",
            Object::Array(vec![int(0), int(-200), int(500), int(800)]),
        ),
        ("ItalicAngle", int(0)),
        ("Ascent", int(800)),
        ("Descent", int(-200)),
        ("CapHeight", int(800)),
        ("StemV", int(80)),
        ("FontFile2", Object::Reference(font_file)),
    ]));

    // Every code (CID) draws glyph 1, which is empty.
    let mut cid_to_gid = Stream::new(Dictionary::new(), [0u8, 1].repeat(65536));
    let _ = cid_to_gid.compress();
    let cid_to_gid = doc.add_object(cid_to_gid);

    let cid_font = doc.add_object(Dictionary::from_iter([
        ("Type", name("Font")),
        ("Subtype", name("CIDFontType2")),
        ("BaseFont", name(FONT_NAME)),
        (
            "CIDSystemInfo",
            Object::Dictionary(Dictionary::from_iter([
                ("Registry", Object::string_literal("Adobe")),
                ("Ordering", Object::string_literal("Identity")),
                ("Supplement", int(0)),
            ])),
        ),
        ("FontDescriptor", Object::Reference(descriptor)),
        ("DW", int(GLYPH_WIDTH as i64)),
        ("CIDToGIDMap", Object::Reference(cid_to_gid)),
    ]));

    let mut to_unicode = Stream::new(Dictionary::new(), to_unicode_cmap().into_bytes());
    let _ = to_unicode.compress();
    let to_unicode = doc.add_object(to_unicode);

    doc.add_object(Dictionary::from_iter([
        ("Type", name("Font")),
        ("Subtype", name("Type0")),
        ("BaseFont", name(FONT_NAME)),
        ("Encoding", name("Identity-H")),
        (
            "DescendantFonts",
            Object::Array(vec![Object::Reference(cid_font)]),
        ),
        ("ToUnicode", Object::Reference(to_unicode)),
    ]))
}

/// Code = UTF-16 code unit, mapped to itself (surrogates excluded).
fn to_unicode_cmap() -> String {
    let ranges: Vec<u16> = (0u16..=0xFF)
        .filter(|h| !(0xD8..=0xDF).contains(h))
        .collect();
    let mut cmap = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n\
         1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    for block in ranges.chunks(100) {
        let _ = writeln!(cmap, "{} beginbfrange", block.len());
        for hi in block {
            let _ = writeln!(cmap, "<{hi:02X}00> <{hi:02X}FF> <{hi:02X}00>");
        }
        cmap.push_str("endbfrange\n");
    }
    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    cmap
}

/// A minimal TrueType font with two empty glyphs (.notdef and one blank
/// glyph), 1000 units per em, every glyph 500 wide.
fn glyphless_ttf() -> Vec<u8> {
    fn be16(v: &mut Vec<u8>, x: u16) {
        v.extend_from_slice(&x.to_be_bytes());
    }
    fn be32(v: &mut Vec<u8>, x: u32) {
        v.extend_from_slice(&x.to_be_bytes());
    }
    let i16 = |x: i16| x as u16;

    let mut head = Vec::new();
    be32(&mut head, 0x0001_0000); // version
    be32(&mut head, 0x0001_0000); // fontRevision
    be32(&mut head, 0); // checkSumAdjustment (set below)
    be32(&mut head, 0x5F0F_3CF5); // magic
    be16(&mut head, 0b1011); // flags
    be16(&mut head, 1000); // unitsPerEm
    head.extend_from_slice(&[0; 16]); // created, modified
    for v in [0i16, -200, 500, 800] {
        be16(&mut head, i16(v)); // xMin, yMin, xMax, yMax
    }
    be16(&mut head, 0); // macStyle
    be16(&mut head, 8); // lowestRecPPEM
    be16(&mut head, 2); // fontDirectionHint
    be16(&mut head, 0); // indexToLocFormat: short
    be16(&mut head, 0); // glyphDataFormat

    let mut hhea = Vec::new();
    be32(&mut hhea, 0x0001_0000);
    for v in [800i16, -200, 0] {
        be16(&mut hhea, i16(v)); // ascender, descender, lineGap
    }
    be16(&mut hhea, 500); // advanceWidthMax
    for v in [0i16, 0, 500, 1, 0, 0, 0, 0, 0, 0, 0] {
        // minLSB, minRSB, xMaxExtent, caretSlopeRise/Run, caretOffset, reserved×4, metricDataFormat
        be16(&mut hhea, i16(v));
    }
    be16(&mut hhea, 2); // numberOfHMetrics

    let mut maxp = Vec::new();
    be32(&mut maxp, 0x0001_0000);
    be16(&mut maxp, 2); // numGlyphs
    for v in [2u16, 1, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0] {
        be16(&mut maxp, v); // maxPoints … maxComponentDepth (maxZones = 2)
    }

    let mut hmtx = Vec::new();
    for _ in 0..2 {
        be16(&mut hmtx, 500);
        be16(&mut hmtx, 0);
    }

    // Glyph 0 (.notdef) is empty. Glyph 1, the one every code draws, is a
    // single line from corner to corner of the cell: it encloses no area,
    // so it paints nothing even where invisible text were drawn, but it
    // gives the glyph a real bounding box. Without one, text whose baseline
    // runs up or down the page (a page shown turned, `/Rotate`) has no
    // width, and PDFium leaves it out of the page's text.
    let mut glyf = Vec::new();
    be16(&mut glyf, 1); // numberOfContours
    for v in [0i16, -200, 500, 800] {
        be16(&mut glyf, i16(v)); // xMin, yMin, xMax, yMax
    }
    be16(&mut glyf, 1); // endPtsOfContours: points 0 and 1
    be16(&mut glyf, 0); // instructionLength
    glyf.extend_from_slice(&[1, 1]); // flags: both on the curve, 16-bit deltas
    for v in [0i16, 500, -200, 1000] {
        be16(&mut glyf, i16(v)); // x deltas, then y deltas
    }
    debug_assert_eq!(glyf.len(), 24);
    // Short offsets (in units of 2 bytes): .notdef 0..0, glyph 1 0..24.
    let mut loca = Vec::new();
    for v in [0u16, 0, 12] {
        be16(&mut loca, v);
    }

    let mut cmap = Vec::new();
    be16(&mut cmap, 0); // version
    be16(&mut cmap, 1); // numTables
    be16(&mut cmap, 3); // Windows
    be16(&mut cmap, 1); // Unicode BMP
    be32(&mut cmap, 12);
    for v in [4u16, 24, 0, 2, 2, 0, 0, 0xFFFF, 0, 0xFFFF, 1, 0] {
        // format 4 with only the closing segment
        be16(&mut cmap, v);
    }

    let mut name = Vec::new();
    let strings: Vec<(u16, Vec<u8>)> = [
        (1u16, FONT_NAME),
        (2, "Regular"),
        (4, FONT_NAME),
        (6, FONT_NAME),
    ]
    .into_iter()
    .map(|(id, s)| (id, s.encode_utf16().flat_map(u16::to_be_bytes).collect()))
    .collect();
    be16(&mut name, 0);
    be16(&mut name, strings.len() as u16);
    be16(&mut name, 6 + 12 * strings.len() as u16);
    let mut offset = 0u16;
    for (id, bytes) in &strings {
        for v in [3u16, 1, 0x409, *id, bytes.len() as u16, offset] {
            be16(&mut name, v);
        }
        offset += bytes.len() as u16;
    }
    for (_, bytes) in &strings {
        name.extend_from_slice(bytes);
    }

    let mut os2 = Vec::new();
    be16(&mut os2, 0); // version 0
    for v in [500i16, 400, 5, 0] {
        be16(&mut os2, i16(v)); // xAvgCharWidth, weight, width, fsType
    }
    os2.extend_from_slice(&[0; 20]); // sub/superscript, strikeout
    be16(&mut os2, 0); // sFamilyClass
    os2.extend_from_slice(&[0; 10]); // panose
    os2.extend_from_slice(&[0; 16]); // ulUnicodeRange1–4
    os2.extend_from_slice(b"RUPT");
    be16(&mut os2, 0x40); // fsSelection: regular
    be16(&mut os2, 0); // usFirstCharIndex
    be16(&mut os2, 0xFFFF); // usLastCharIndex
    for v in [800i16, -200, 0] {
        be16(&mut os2, i16(v)); // typo ascender, descender, line gap
    }
    be16(&mut os2, 800); // usWinAscent
    be16(&mut os2, 200); // usWinDescent

    let mut post = Vec::new();
    be32(&mut post, 0x0003_0000); // version 3: no glyph names
    be32(&mut post, 0); // italicAngle
    be16(&mut post, i16(-100));
    be16(&mut post, 50);
    be32(&mut post, 1); // isFixedPitch
    post.extend_from_slice(&[0; 16]);

    let mut tables: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"OS/2", os2),
        (b"cmap", cmap),
        (b"glyf", glyf),
        (b"head", head),
        (b"hhea", hhea),
        (b"hmtx", hmtx),
        (b"loca", loca),
        (b"maxp", maxp),
        (b"name", name),
        (b"post", post),
    ];
    tables.sort_by_key(|(tag, _)| **tag);

    let checksum = |data: &[u8]| -> u32 {
        data.chunks(4).fold(0u32, |sum, c| {
            let mut word = [0u8; 4];
            word[..c.len()].copy_from_slice(c);
            sum.wrapping_add(u32::from_be_bytes(word))
        })
    };
    let n = tables.len() as u16;
    let mut font = Vec::new();
    be32(&mut font, 0x0001_0000);
    be16(&mut font, n);
    let entry_selector = 15 - n.leading_zeros() as u16; // floor(log2 n)
    be16(&mut font, 16 << entry_selector); // searchRange
    be16(&mut font, entry_selector);
    be16(&mut font, n * 16 - (16 << entry_selector)); // rangeShift
    let mut offset = 12 + 16 * tables.len();
    let mut head_offset = 0;
    for (tag, data) in &tables {
        font.extend_from_slice(*tag);
        be32(&mut font, checksum(data));
        be32(&mut font, offset as u32);
        be32(&mut font, data.len() as u32);
        if *tag == b"head" {
            head_offset = offset;
        }
        offset += data.len().div_ceil(4) * 4;
    }
    for (_, data) in &tables {
        font.extend_from_slice(data);
        font.resize(font.len().div_ceil(4) * 4, 0);
    }
    let adjustment = 0xB1B0_AFBAu32.wrapping_sub(checksum(&font));
    font[head_offset + 8..head_offset + 12].copy_from_slice(&adjustment.to_be_bytes());
    font
}

/// Text matrix that stretches the word (at font size 1) over its box:
/// along the bottom edge, as tall as the box, baseline a little above the
/// bottom so the glyph boxes cover the word.
fn text_matrix(w: &PlacedWord) -> Option<[f64; 6]> {
    let [tl, _tr, br, bl] = w.quad;
    let (bx, by) = (br.0 - bl.0, br.1 - bl.1);
    let (ux, uy) = (tl.0 - bl.0, tl.1 - bl.1);
    let len = bx.hypot(by);
    let height = ux.hypot(uy);
    let chars = w.text.chars().count() as f64;
    if len < 0.1 || height < 0.1 || chars == 0.0 {
        return None;
    }
    let sx = len / (chars * GLYPH_WIDTH / 1000.0);
    let (bx, by) = (bx / len, by / len);
    let (ux, uy) = (ux / height, uy / height);
    Some([
        bx * sx,
        by * sx,
        ux * height,
        uy * height,
        bl.0 + ux * height * DESCENT,
        bl.1 + uy * height * DESCENT,
    ])
}

fn num(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

/// The updated PDF must parse, and its text must contain most of the
/// words that were added.
fn verify(
    updated: &[u8],
    pages: &[PageWords],
    extract: &impl Fn(Vec<u8>) -> Result<String>,
) -> Result<()> {
    Document::load_mem(updated)
        .map_err(|e| Error::Pdf(format!("updated PDF didn't parse: {e}")))?;
    let text = extract(updated.to_vec())?.to_lowercase();
    let sample: Vec<String> = pages
        .iter()
        .flat_map(|p| &p.words)
        .map(|w| w.text.to_lowercase())
        .filter(|t| t.chars().count() >= 4)
        .take(200)
        .collect();
    if sample.is_empty() {
        return Ok(());
    }
    let found = sample.iter().filter(|w| text.contains(w.as_str())).count();
    if found * 10 < sample.len() * 9 {
        return Err(Error::Pdf(format!(
            "the text layer didn't read back ({found} of {} words)",
            sample.len()
        )));
    }
    Ok(())
}

fn pdf_err(e: impl std::fmt::Display) -> Error {
    Error::Pdf(e.to_string())
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, x: f64, y: f64, w: f64, h: f64) -> PlacedWord {
        PlacedWord {
            text: text.into(),
            quad: [(x, y + h), (x + w, y + h), (x + w, y), (x, y)],
        }
    }

    #[test]
    fn matrix_stretches_word_over_its_box() {
        // "Rent" (4 glyphs × 0.5) over 100 × 20 points at (50, 700).
        let m = text_matrix(&word("Rent", 50.0, 700.0, 100.0, 20.0)).unwrap();
        assert_eq!(
            m.map(|v| (v * 1000.0).round() / 1000.0),
            [50.0, 0.0, 0.0, 20.0, 50.0, 704.0]
        );
        // Rotated 90°: the baseline runs up the page.
        let rotated = PlacedWord {
            text: "Rent".into(),
            quad: [(80.0, 0.0), (80.0, 100.0), (100.0, 100.0), (100.0, 0.0)],
        };
        let m = text_matrix(&rotated).unwrap();
        assert_eq!(
            m.map(|v| (v * 1000.0).round() / 1000.0),
            [0.0, 50.0, -20.0, 0.0, 96.0, 0.0]
        );
    }

    #[test]
    fn writes_a_readable_layer_and_keeps_the_original_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scan.pdf");
        crate::pdf_meta::tests::make_pdf(&path, None);
        let original = fs::read(&path).unwrap();

        let pages = [PageWords {
            index: 0,
            words: vec![
                word("Grüße", 72.0, 700.0, 80.0, 14.0),
                word("Señor", 160.0, 700.0, 70.0, 14.0),
                word("€42", 240.0, 700.0, 40.0, 14.0),
            ],
        }];
        // With PDFium (fetched into src-tauri/pdfium) the real check runs;
        // without it only the structure is checked below.
        let pdfium = crate::text::tests::shared_pdfium();
        add(&path, &pages, |bytes| match pdfium {
            Some(p) => p.extract_pdf_bytes(bytes),
            None => Ok("grüße señor €42".into()),
        })
        .unwrap_or_else(|e| panic!("{e}"));
        if let Some(p) = pdfium {
            let text = p.extract(&path).unwrap().unwrap();
            assert!(
                text.contains("Grüße") && text.contains("Señor") && text.contains("€42"),
                "{text}"
            );
        }

        let updated = fs::read(&path).unwrap();
        assert!(updated.starts_with(&original), "original bytes kept");
        let doc = Document::load(&path).unwrap();
        let page = doc.get_dictionary(doc.get_pages()[&1]).unwrap();
        let contents = page.get(b"Contents").unwrap().as_array().unwrap();
        assert_eq!(contents.len(), 4, "q, original, Q, text");
        let fonts = page
            .get(b"Resources")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"Font")
            .unwrap()
            .as_dict()
            .unwrap();
        assert!(fonts.has(b"RupertOCR"));
    }

    #[test]
    fn refuses_unchanged_text_check() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scan.pdf");
        crate::pdf_meta::tests::make_pdf(&path, None);
        let original = fs::read(&path).unwrap();
        let pages = [PageWords {
            index: 0,
            words: vec![word("Invoice", 72.0, 700.0, 80.0, 14.0)],
        }];
        let err = add(&path, &pages, |_| Ok(String::new())).unwrap_err();
        assert!(err.to_string().contains("didn't read back"), "{err}");
        assert_eq!(fs::read(&path).unwrap(), original, "file untouched");
    }

    /// A page shown turned (`/Rotate 270`, e.g. a landscape scan turned in
    /// Preview): its words run down the page in PDF space, and must still
    /// read back as words.
    #[test]
    fn rotated_page_reads_back() {
        let Some(pdfium) = crate::text::tests::shared_pdfium() else {
            return;
        };
        for rotate in [0, 90, 180, 270] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("turned.pdf");
            crate::pdf_meta::tests::make_pdf(&path, None);
            let mut doc = Document::load(&path).unwrap();
            let page_id = doc.get_pages()[&1];
            doc.get_object_mut(page_id)
                .and_then(Object::as_dict_mut)
                .unwrap()
                .set("Rotate", rotate);
            doc.save(&path).unwrap();

            // Lines of words as OCR on the turned page places them.
            let names = [
                "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf",
            ];
            let mut words = Vec::new();
            for line in 0..6 {
                let mut along = 40.0;
                for n in names {
                    let len = n.len() as f64 * 6.0;
                    let across = 100.0 + line as f64 * 16.0;
                    let h = 10.0;
                    // Display direction → PDF space for each rotation.
                    let pt = |a: f64, c: f64| match rotate {
                        90 => (c, a), // text runs up the page
                        180 => (500.0 - a, 700.0 - c),
                        270 => (500.0 - c, 700.0 - a), // runs down
                        _ => (a, 700.0 - c),
                    };
                    let bl = pt(along, across);
                    let br = pt(along + len, across);
                    let tl = pt(along, across - h);
                    let tr = pt(along + len, across - h);
                    words.push(PlacedWord {
                        text: format!("{n}{line}"),
                        quad: [tl, tr, br, bl],
                    });
                    along += len + 5.0;
                }
            }
            let result = add(&path, &[PageWords { index: 0, words }], |b| {
                pdfium.extract_pdf_bytes(b)
            });
            let text = pdfium.extract(&path).unwrap().unwrap_or_default();
            result.unwrap_or_else(|e| panic!("rotate {rotate}: {e}\n{text}"));
            assert!(
                text.contains("alpha0") && text.contains("golf5"),
                "rotate {rotate}: {text}"
            );
        }
    }
}
