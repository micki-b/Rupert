//! XMP packets: reading Rupert's fields, and writing them into a packet while
//! keeping everything else other software put there.
//!
//! Field mapping (see docs/DESIGN.md):
//! title → `dc:title`, tags → `dc:subject` + `pdf:Keywords`,
//! notes → `dc:description`, the rest → `rupert:*`.

use quick_xml::NsReader;
use quick_xml::escape::escape;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::writer::Writer;

use crate::meta::{DocMeta, PartialMeta, split_keywords};

pub const NS_RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
pub const NS_DC: &str = "http://purl.org/dc/elements/1.1/";
pub const NS_PDF: &str = "http://ns.adobe.com/pdf/1.3/";
pub const NS_XMP: &str = "http://ns.adobe.com/xap/1.0/";
pub const NS_RUPERT: &str = "https://github.com/micki-b/Rupert/ns/1.0/";
const NS_XML: &str = "http://www.w3.org/XML/1998/namespace";

// MARK: - Properties

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Prop {
    Title,
    Subject,
    Description,
    Keywords,
    MetadataDate,
    RupertId,
    RupertDate,
    RupertFavorite,
    /// Any other `rupert:` property (written by a newer Rupert). Managed, so
    /// it is replaced rather than duplicated.
    RupertOther,
}

fn prop(ns: &str, local: &str) -> Option<Prop> {
    Some(match (ns, local) {
        (NS_DC, "title") => Prop::Title,
        (NS_DC, "subject") => Prop::Subject,
        (NS_DC, "description") => Prop::Description,
        (NS_PDF, "Keywords") => Prop::Keywords,
        (NS_XMP, "MetadataDate") => Prop::MetadataDate,
        (NS_RUPERT, "id") => Prop::RupertId,
        (NS_RUPERT, "documentDate") => Prop::RupertDate,
        (NS_RUPERT, "favorite") => Prop::RupertFavorite,
        (NS_RUPERT, _) => Prop::RupertOther,
        _ => return None,
    })
}

fn bound<'a>(r: &'a ResolveResult) -> &'a str {
    match r {
        ResolveResult::Bound(ns) => ns.as_ref(),
        _ => "",
    }
}

/// Unescapes an attribute value (`&amp;` → `&`).
fn unescape_attr(a: &quick_xml::events::attributes::Attribute) -> String {
    quick_xml::escape::unescape(&a.value)
        .map(|v| v.into_owned())
        .unwrap_or_else(|_| a.value.to_string())
}

fn entity(name: &str) -> Option<char> {
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        _ => return None,
    })
}

// MARK: - Reading

#[derive(Default)]
struct Collecting {
    prop: Option<Prop>,
    depth: usize,
    text: String,
    items: Vec<(bool, String)>, // (is x-default, value)
    item: Option<(bool, String)>,
}

/// Extracts Rupert's fields from an XMP packet. Malformed XML yields what
/// was read before the error.
pub fn read(xml: &str) -> PartialMeta {
    let mut out = PartialMeta::default();
    let mut reader = NsReader::from_str(xml);
    let mut depth = 0usize;
    let mut c = Collecting::default();

    while let Ok((ns, event)) = reader.read_resolved_event() {
        let ns = bound(&ns).to_string();
        let (e, is_empty) = match event {
            Event::Start(e) => (e, false),
            Event::Empty(e) => (e, true),
            Event::Text(t) => {
                c.push_str(&t.xml10_content());
                continue;
            }
            Event::CData(t) => {
                c.push_str(&t);
                continue;
            }
            Event::GeneralRef(r) => {
                let ch = if r.is_char_ref() {
                    r.resolve_char_ref().ok().flatten()
                } else {
                    entity(&r)
                };
                if let Some(ch) = ch {
                    c.push_str(ch.encode_utf8(&mut [0; 4]));
                }
                continue;
            }
            Event::End(e) => {
                depth = depth.saturating_sub(1);
                if c.prop.is_some() {
                    if ns == NS_RDF && e.local_name().as_ref() == "li" {
                        if let Some(item) = c.item.take() {
                            c.items.push(item);
                        }
                    } else if depth == c.depth {
                        finish(&mut out, std::mem::take(&mut c));
                    }
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };

        // An opening (or self-closing) element.
        if c.prop.is_none() {
            let local = e.local_name();
            if ns == NS_RDF && local.as_ref() == "Description" {
                read_description_attributes(&reader, &e, &mut out);
            }
            if let Some(p) = prop(&ns, local.as_ref()) {
                c = Collecting {
                    prop: Some(p),
                    depth,
                    ..Default::default()
                };
                if is_empty {
                    finish(&mut out, std::mem::take(&mut c));
                }
            }
        } else if ns == NS_RDF && e.local_name().as_ref() == "li" && !is_empty {
            let is_default = e.attributes().flatten().any(|a| {
                let (ans, local) = reader.resolver().resolve_attribute(a.key);
                bound(&ans) == NS_XML && local.as_ref() == "lang" && a.value.as_ref() == "x-default"
            });
            c.item = Some((is_default, String::new()));
        }
        if !is_empty {
            depth += 1;
        }
    }

    if out.tags.as_ref().is_none_or(|t| t.is_empty())
        && let Some(keywords) = out_keywords(xml)
    {
        out.tags = Some(split_keywords(&keywords));
    }
    out
}

impl Collecting {
    fn push_str(&mut self, s: &str) {
        match &mut self.item {
            Some((_, v)) => v.push_str(s),
            None if self.prop.is_some() => self.text.push_str(s),
            None => {}
        }
    }
}

fn read_description_attributes(reader: &NsReader<&[u8]>, e: &BytesStart, out: &mut PartialMeta) {
    for a in e.attributes().flatten() {
        let (ns, local) = reader.resolver().resolve_attribute(a.key);
        let Some(p) = prop(bound(&ns), local.as_ref()) else {
            continue;
        };
        let value = unescape_attr(&a);
        let value = value.trim().to_string();
        match p {
            Prop::RupertId => out.id = Some(value),
            Prop::RupertDate => out.document_date = Some(value),
            Prop::RupertFavorite => out.favorite = Some(value.eq_ignore_ascii_case("true")),
            _ => {}
        }
    }
}

/// `pdf:Keywords`, in either element or attribute form.
fn out_keywords(xml: &str) -> Option<String> {
    let mut reader = NsReader::from_str(xml);
    let mut inside = false;
    let mut text = String::new();
    loop {
        let Ok((ns, event)) = reader.read_resolved_event() else {
            return None;
        };
        match event {
            Event::Start(e) | Event::Empty(e) => {
                if bound(&ns) == NS_PDF && e.local_name().as_ref() == "Keywords" {
                    inside = true;
                }
                for a in e.attributes().flatten() {
                    let (ans, local) = reader.resolver().resolve_attribute(a.key);
                    if bound(&ans) == NS_PDF && local.as_ref() == "Keywords" {
                        return Some(unescape_attr(&a));
                    }
                }
            }
            Event::Text(t) if inside => text.push_str(&t.xml10_content()),
            Event::GeneralRef(r) if inside => {
                if let Some(ch) = entity(&r) {
                    text.push(ch);
                }
            }
            Event::End(_) if inside => return Some(text.trim().to_string()),
            Event::Eof => return None,
            _ => {}
        }
    }
}

fn finish(out: &mut PartialMeta, c: Collecting) {
    let single = || {
        c.items
            .iter()
            .find(|(d, _)| *d)
            .or(c.items.first())
            .map(|(_, v)| v.trim().to_string())
            .unwrap_or_else(|| c.text.trim().to_string())
    };
    match c.prop {
        Some(Prop::Title) => out.title = Some(single()).filter(|s| !s.is_empty()),
        Some(Prop::Description) => out.notes = Some(single()).filter(|s| !s.is_empty()),
        Some(Prop::Subject) => {
            out.tags = Some(
                c.items
                    .iter()
                    .map(|(_, v)| v.trim().to_string())
                    .filter(|v| !v.is_empty())
                    .collect(),
            )
        }
        Some(Prop::RupertId) => out.id = Some(single()).filter(|s| !s.is_empty()),
        Some(Prop::RupertDate) => out.document_date = Some(single()).filter(|s| !s.is_empty()),
        Some(Prop::RupertFavorite) => out.favorite = Some(single().eq_ignore_ascii_case("true")),
        _ => {}
    }
}

// MARK: - Writing

/// Our `rdf:Description`, declaring every prefix it uses so it is valid
/// wherever it is inserted.
fn description(meta: &DocMeta, now: &str) -> String {
    let mut s = format!(
        "<rdf:Description rdf:about=\"\" xmlns:rdf=\"{NS_RDF}\" xmlns:dc=\"{NS_DC}\" \
         xmlns:pdf=\"{NS_PDF}\" xmlns:xmp=\"{NS_XMP}\" xmlns:rupert=\"{NS_RUPERT}\">\n"
    );
    let alt = |s: &mut String, name: &str, value: &str| {
        s.push_str(&format!(
            "   <{name}><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></{name}>\n",
            escape(value)
        ));
    };
    if let Some(title) = &meta.title {
        alt(&mut s, "dc:title", title);
    }
    if !meta.tags.is_empty() {
        s.push_str("   <dc:subject><rdf:Bag>");
        for tag in &meta.tags {
            s.push_str(&format!("<rdf:li>{}</rdf:li>", escape(tag)));
        }
        s.push_str("</rdf:Bag></dc:subject>\n");
        s.push_str(&format!(
            "   <pdf:Keywords>{}</pdf:Keywords>\n",
            escape(meta.tags.join(", "))
        ));
    }
    if let Some(notes) = &meta.notes {
        alt(&mut s, "dc:description", notes);
    }
    s.push_str(&format!("   <xmp:MetadataDate>{now}</xmp:MetadataDate>\n"));
    if let Some(id) = &meta.id {
        s.push_str(&format!("   <rupert:id>{}</rupert:id>\n", escape(id)));
    }
    if let Some(date) = &meta.document_date {
        s.push_str(&format!(
            "   <rupert:documentDate>{}</rupert:documentDate>\n",
            escape(date)
        ));
    }
    if meta.favorite {
        s.push_str("   <rupert:favorite>True</rupert:favorite>\n");
    }
    s.push_str("  </rdf:Description>\n ");
    s
}

/// A complete packet containing only Rupert's fields.
pub fn fresh_packet(meta: &DocMeta, now: &str) -> String {
    format!(
        "<?xpacket begin=\"\u{FEFF}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
         <x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n \
         <rdf:RDF xmlns:rdf=\"{NS_RDF}\">\n  {}</rdf:RDF>\n\
         </x:xmpmeta>\n\
         <?xpacket end=\"w\"?>",
        description(meta, now)
    )
}

/// Writes Rupert's fields into `existing`: removes every property Rupert
/// manages (wherever it appears) and appends one `rdf:Description` with the
/// new values. Everything else is kept as is. Falls back to a fresh packet
/// if `existing` can't be parsed.
pub fn merge(existing: Option<&str>, meta: &DocMeta, now: &str) -> String {
    existing
        .and_then(|xml| merge_into(xml, meta, now))
        .unwrap_or_else(|| fresh_packet(meta, now))
}

fn merge_into(xml: &str, meta: &DocMeta, now: &str) -> Option<String> {
    let mut reader = NsReader::from_str(xml);
    let mut writer = Writer::new(Vec::new());
    let mut skip_depth: Option<usize> = None;
    let mut depth = 0usize;
    let mut inserted = false;

    loop {
        let (ns, event) = reader.read_resolved_event().ok()?;
        let ns = bound(&ns).to_string();
        match event {
            Event::Eof => break,
            Event::Start(e) => {
                depth += 1;
                if skip_depth.is_some() {
                    continue;
                }
                if prop(&ns, e.local_name().as_ref()).is_some() {
                    skip_depth = Some(depth);
                    continue;
                }
                let e = strip_managed_attributes(&reader, e, &ns);
                writer.write_event(Event::Start(e)).ok()?;
            }
            Event::Empty(e) => {
                if skip_depth.is_some() || prop(&ns, e.local_name().as_ref()).is_some() {
                    continue;
                }
                let e = strip_managed_attributes(&reader, e, &ns);
                writer.write_event(Event::Empty(e)).ok()?;
            }
            Event::End(e) => {
                if let Some(d) = skip_depth {
                    if d == depth {
                        skip_depth = None;
                    }
                    depth -= 1;
                    continue;
                }
                depth = depth.saturating_sub(1);
                if ns == NS_RDF && e.local_name().as_ref() == "RDF" && !inserted {
                    writer
                        .get_mut()
                        .extend_from_slice(description(meta, now).as_bytes());
                    inserted = true;
                }
                writer.write_event(Event::End(e)).ok()?;
            }
            other => {
                if skip_depth.is_none() {
                    writer.write_event(other).ok()?;
                }
            }
        }
    }

    if !inserted {
        return None;
    }
    String::from_utf8(writer.into_inner()).ok()
}

/// Drops managed properties written in attribute form
/// (`<rdf:Description rupert:id="…">`).
fn strip_managed_attributes<'a>(
    reader: &NsReader<&[u8]>,
    e: BytesStart<'a>,
    ns: &str,
) -> BytesStart<'a> {
    if ns != NS_RDF || e.local_name().as_ref() != "Description" {
        return e;
    }
    let keep: Vec<_> = e
        .attributes()
        .flatten()
        .filter(|a| {
            let (ans, local) = reader.resolver().resolve_attribute(a.key);
            prop(bound(&ans), local.as_ref()).is_none()
        })
        .map(|a| (a.key.as_ref().to_string(), a.value.to_string()))
        .collect();
    let mut out = BytesStart::new(e.name().as_ref().to_string());
    for (key, value) in &keep {
        // `value` is still escaped as in the source, so push it verbatim.
        out.push_attribute(quick_xml::events::attributes::Attribute {
            key: quick_xml::name::QName(key),
            value: value.as_str().into(),
        });
    }
    out
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: &str = "2026-11-18T08:00:00Z";

    fn sample() -> DocMeta {
        DocMeta {
            id: Some("abc-123".into()),
            title: Some("PG&E <Bill>".into()),
            tags: vec!["Utilities".into(), "Home & Garden".into()],
            notes: Some("Paid online".into()),
            document_date: Some("2026-11-03".into()),
            favorite: true,
        }
    }

    #[test]
    fn fresh_packet_round_trips() {
        let xml = fresh_packet(&sample(), NOW);
        assert_eq!(read(&xml).into_meta(), sample());
    }

    #[test]
    fn merge_keeps_foreign_properties_and_replaces_ours() {
        let existing = format!(
            r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="{NS_RDF}">
 <rdf:Description rdf:about="" xmlns:pdf="{NS_PDF}" xmlns:rupert="{NS_RUPERT}" pdf:Producer="ScanSnap" rupert:id="old-id"/>
 <rdf:Description rdf:about="" xmlns:dc="{NS_DC}" xmlns:xmp="{NS_XMP}">
  <dc:format>application/pdf</dc:format>
  <dc:title><rdf:Alt><rdf:li xml:lang="x-default">Old title</rdf:li></rdf:Alt></dc:title>
  <xmp:CreateDate>2021-05-19T10:00:00Z</xmp:CreateDate>
 </rdf:Description>
</rdf:RDF></x:xmpmeta>
<?xpacket end="w"?>"#
        );
        let merged = merge(Some(&existing), &sample(), NOW);
        assert!(merged.contains("pdf:Producer=\"ScanSnap\""), "{merged}");
        assert!(merged.contains("<dc:format>application/pdf</dc:format>"));
        assert!(merged.contains("<xmp:CreateDate>2021-05-19T10:00:00Z</xmp:CreateDate>"));
        assert!(!merged.contains("Old title"));
        assert!(!merged.contains("old-id"));
        assert!(merged.trim_end().ends_with("<?xpacket end=\"w\"?>"));
        assert_eq!(read(&merged).into_meta(), sample());

        // Merging twice doesn't duplicate anything.
        let twice = merge(Some(&merged), &sample(), NOW);
        assert_eq!(twice.matches("<dc:title>").count(), 1);
        assert_eq!(read(&twice).into_meta(), sample());
    }

    #[test]
    fn clearing_fields_removes_them() {
        let full = fresh_packet(&sample(), NOW);
        let cleared = DocMeta {
            id: Some("abc-123".into()),
            ..Default::default()
        };
        let merged = merge(Some(&full), &cleared, NOW);
        assert!(!merged.contains("dc:title"));
        assert!(!merged.contains("dc:subject"));
        assert!(!merged.contains("pdf:Keywords"));
        assert_eq!(read(&merged).into_meta(), cleared);
    }

    #[test]
    fn reads_keywords_when_no_subject() {
        let xml = format!(
            r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="{NS_RDF}">
<rdf:Description rdf:about="" xmlns:pdf="{NS_PDF}" pdf:Keywords="tax; 2025, W-2"/>
</rdf:RDF></x:xmpmeta>"#
        );
        assert_eq!(read(&xml).tags.unwrap(), ["tax", "2025", "W-2"]);
    }

    #[test]
    fn unparseable_existing_packet_is_replaced() {
        let merged = merge(Some("<not xml"), &sample(), NOW);
        assert_eq!(read(&merged).into_meta(), sample());
    }
}
