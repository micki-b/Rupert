//! Finding words on a document's pages (#106): where search hits and cited
//! passages are, as rectangles on each page, so the page viewer can mark
//! them. Matching works like search: case and accents don't matter, and
//! each word may be the start of a longer one ("impel" finds "impeller").
//!
//! The matching here is plain Rust over a page's characters; reading them
//! (and their boxes) from a PDF is `TextExtractor::find`.

use serde::Serialize;
use unicode_normalization::UnicodeNormalization;

/// At most this many hits in one document.
pub const MAX_HITS: usize = 1000;

// MARK: - Query

/// What to look for: each entry is a run of words that must follow each
/// other (only spaces or punctuation between them).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindQuery {
    runs: Vec<Vec<Vec<char>>>,
}

impl FindQuery {
    /// Every word on its own, as search matches them.
    pub fn words(text: &str) -> Self {
        Self {
            runs: words(text).into_iter().map(|w| vec![w]).collect(),
        }
    }

    /// The words together, in this order: a passage.
    pub fn phrase(text: &str) -> Self {
        let w = words(text);
        Self {
            runs: if w.is_empty() { Vec::new() } else { vec![w] },
        }
    }

    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }
}

/// The folded words of `text` (letters and digits only).
fn words(text: &str) -> Vec<Vec<char>> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.chars().flat_map(fold).collect())
        .collect()
}

/// A character as matching sees it: lowercase, without accents ("Ä" → "a").
pub fn fold(c: char) -> impl Iterator<Item = char> {
    c.to_lowercase()
        .flat_map(|l| std::iter::once(l).nfd())
        .filter(|c| !is_mark(*c))
}

/// Combining marks (the accents NFD splits off).
fn is_mark(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F)
}

// MARK: - Matching

/// One hit: its page (0-based) and where it is on it, as rectangles
/// `[x, y, width, height]` in fractions of the page as shown (from the top
/// left), one per line it covers.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FoundHit {
    pub page: usize,
    pub rects: Vec<[f32; 4]>,
}

/// The hits in a page's text (`chars`, in reading order) as ranges of
/// character positions, in order.
pub fn match_ranges(chars: &[char], query: &FindQuery) -> Vec<(usize, usize)> {
    let mut folded: Vec<char> = Vec::with_capacity(chars.len());
    let mut owner: Vec<usize> = Vec::with_capacity(chars.len());
    for (i, c) in chars.iter().enumerate() {
        for f in fold(*c) {
            folded.push(f);
            owner.push(i);
        }
    }
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < folded.len() {
        let at_word_start = i == 0 || !folded[i - 1].is_alphanumeric();
        let hit = at_word_start
            .then(|| query.runs.iter().find_map(|run| run_at(&folded, i, run)))
            .flatten();
        match hit {
            Some(end) => {
                out.push((owner[i], owner[end - 1] + 1));
                i = end;
            }
            None => i += 1,
        }
    }
    out.dedup();
    out
}

/// Where `run` ends if it starts at `i` (each word may continue into a
/// longer one; between words, anything but letters and digits).
fn run_at(folded: &[char], i: usize, run: &[Vec<char>]) -> Option<usize> {
    let mut pos = i;
    for (k, word) in run.iter().enumerate() {
        if k > 0 {
            // To the end of the previous word, then over what separates.
            while pos < folded.len() && folded[pos].is_alphanumeric() {
                pos += 1;
            }
            while pos < folded.len() && !folded[pos].is_alphanumeric() {
                pos += 1;
            }
        }
        if folded.get(pos..pos + word.len())? != word.as_slice() {
            return None;
        }
        pos += word.len();
    }
    Some(pos)
}

/// The boxes of a hit's characters (`[left, top, right, bottom]`, as
/// fractions of the page) joined into one rectangle per line.
pub fn line_rects(boxes: impl IntoIterator<Item = [f32; 4]>) -> Vec<[f32; 4]> {
    let mut lines: Vec<[f32; 4]> = Vec::new();
    for b in boxes {
        if b[2] <= b[0] || b[3] <= b[1] {
            continue; // a space, or a character without a box
        }
        match lines.last_mut() {
            // On the same line: the boxes overlap by over half their height.
            Some(l) if overlap(l[1], l[3], b[1], b[3]) > 0.5 * (b[3] - b[1]).min(l[3] - l[1]) => {
                l[0] = l[0].min(b[0]);
                l[1] = l[1].min(b[1]);
                l[2] = l[2].max(b[2]);
                l[3] = l[3].max(b[3]);
            }
            _ => lines.push(b),
        }
    }
    lines
        .into_iter()
        .map(|[l, t, r, b]| [l, t, r - l, b - t])
        .collect()
}

fn overlap(a0: f32, a1: f32, b0: f32, b1: f32) -> f32 {
    (a1.min(b1) - a0.max(b0)).max(0.0)
}

// MARK: - Excerpts

/// A short excerpt of `text` around the first hit of `query`, about
/// `words` words long, with every hit in it wrapped in `marks` and "…"
/// where it's cut. Without a hit, the start of the text. For search
/// results: SQLite's own `snippet()` reads a document's whole text to
/// choose its excerpt, which took seconds over long documents.
pub fn excerpt(text: &str, query: &FindQuery, words: usize, marks: (char, char)) -> Option<String> {
    // The first hit is usually near the start: look there first, and at
    // more of the text only when it isn't (a long text costs time).
    let mut chars: Vec<char> = Vec::new();
    let mut hits = Vec::new();
    for size in [2_000, 20_000, usize::MAX] {
        chars = text.chars().take(size).collect();
        hits = match_ranges(&chars, query);
        let whole = chars.len() < size;
        // A hit, with room after it for the rest of the excerpt.
        if whole || hits.first().is_some_and(|h| h.1 + 400 < chars.len()) {
            break;
        }
    }
    // Word starts, to cut at whole words.
    let starts: Vec<usize> = (0..chars.len())
        .filter(|&i| chars[i].is_alphanumeric() && (i == 0 || !chars[i - 1].is_alphanumeric()))
        .collect();
    if starts.is_empty() {
        return None;
    }
    let at = hits.first().map_or(0, |h| h.0);
    let first_word = starts.partition_point(|&s| s <= at).saturating_sub(1);
    // A few words before the hit, the rest after it.
    let from = first_word.saturating_sub(words / 4);
    let to = (from + words).min(starts.len());
    let begin = starts[from];
    let end = starts
        .get(to)
        .copied()
        .unwrap_or(chars.len())
        .min(chars.len());
    let mut out = String::new();
    if from > 0 {
        out.push('…');
    }
    let mut i = begin;
    let mut space = false;
    for &(a, b) in hits.iter().filter(|h| h.1 > begin && h.0 < end) {
        push_plain(&mut out, &chars[i..a.max(i)], &mut space);
        if space && !out.is_empty() && !out.ends_with('…') {
            out.push(' ');
        }
        out.push(marks.0);
        out.extend(&chars[a.max(begin)..b.min(end)]);
        out.push(marks.1);
        space = false;
        i = b.min(end);
    }
    push_plain(&mut out, &chars[i.min(end)..end], &mut space);
    let mut out = out
        .trim_end()
        .trim_end_matches(|c: char| !c.is_alphanumeric() && c != marks.1 && c != '…')
        .to_string();
    if to < starts.len() {
        out.push('…');
    }
    Some(out)
}

/// Appends text with every run of white space (lines, page breaks) as one
/// space.
fn push_plain(out: &mut String, chars: &[char], space: &mut bool) {
    for &c in chars {
        if c.is_whitespace() {
            *space = true;
        } else {
            if *space && !out.is_empty() && !out.ends_with('…') {
                out.push(' ');
            }
            *space = false;
            out.push(c);
        }
    }
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    fn found(text: &str, q: &FindQuery) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        match_ranges(&chars, q)
            .into_iter()
            .map(|(a, b)| chars[a..b].iter().collect())
            .collect()
    }

    #[test]
    fn words_match_at_word_starts_ignoring_case_and_accents() {
        let q = FindQuery::words("impel MARZ");
        assert_eq!(
            found("The Impeller was replaced in März; reimpel nothing", &q),
            ["Impel", "März"]
        );
        assert!(FindQuery::words("  ,. ").is_empty());
    }

    #[test]
    fn a_phrase_needs_its_words_in_order() {
        let q = FindQuery::phrase("monthly rent: 1,200");
        assert_eq!(
            found(
                "The monthly  rent of 1200. The monthly rent - 1,200 USD.",
                &q
            ),
            ["monthly rent - 1,200"]
        );
        assert_eq!(found("rent monthly 1,200", &q), Vec::<String>::new());
    }

    #[test]
    fn excerpts_show_the_first_hit_with_its_words_marked() {
        let text = "Harbor Marine Supply\nInvoice 4179\n\u{c}\nRaw water impeller for the Yanmar engine, \
                    replaced in March. Impeller kit and gasket. Total due $412.50 within thirty days.";
        let q = FindQuery::words("impel");
        let e = excerpt(text, &q, 8, ('[', ']')).unwrap();
        assert_eq!(e, "…Raw water [impel]ler for the Yanmar engine, replaced…");
        // From the start, and every hit marked.
        let e = excerpt(text, &FindQuery::words("harbor"), 4, ('[', ']')).unwrap();
        assert_eq!(e, "[Harbor] Marine Supply Invoice…");
        let e = excerpt(text, &q, 20, ('[', ']')).unwrap();
        assert!(e.contains("[Impel]ler kit"), "{e}");
        // No hit in the text (the title matched): its start.
        let e = excerpt(text, &FindQuery::words("lease"), 3, ('[', ']')).unwrap();
        assert_eq!(e, "Harbor Marine Supply…");
        assert_eq!(excerpt(" \n ", &q, 5, ('[', ']')), None);
    }

    #[test]
    fn boxes_join_into_one_rectangle_per_line() {
        let rects = line_rects([
            [0.10, 0.20, 0.12, 0.22],
            [0.12, 0.20, 0.14, 0.22],
            [0.0, 0.0, 0.0, 0.0], // a space
            [0.15, 0.201, 0.17, 0.221],
            [0.05, 0.25, 0.07, 0.27], // next line
        ]);
        assert_eq!(rects.len(), 2);
        let [x, y, w, h] = rects[0];
        assert!((x - 0.10).abs() < 1e-6 && (y - 0.20).abs() < 1e-6);
        assert!((w - 0.07).abs() < 1e-6 && (h - 0.021).abs() < 1e-6);
    }
}
