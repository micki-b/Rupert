//! Network scanning over eSCL ("AirScan", "Apple AirPrint scanning"),
//! which most networked scanners and all-in-ones made since ~2014 speak,
//! HP included. Plain HTTP + XML, so one implementation covers macOS,
//! Windows and Linux.
//!
//! - Scanners announce themselves over Bonjour as `_uscan._tcp`; one can
//!   also be added by address.
//! - [`capabilities`] reads what it can do (flatbed/feeder, duplex, color
//!   modes, resolutions).
//! - [`scan`] starts a job, fetches the pages one by one as JPEG and
//!   assembles them into a PDF (written under a hidden name, then renamed,
//!   so the inbox never sees a half-written file).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use lopdf::{Dictionary, Document, Object, Stream};
use quick_xml::Reader;
use quick_xml::events::Event;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

const SERVICE: &str = "_uscan._tcp.local.";
/// Longest a page may take to arrive (feeding, warming up).
const PAGE_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_PAGE_BYTES: u64 = 200 * 1024 * 1024;

// MARK: - Scanners

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scanner {
    pub name: String,
    /// The eSCL root, e.g. `http://192.168.1.20:80/eSCL`.
    pub url: String,
}

/// Scanners announcing eSCL on the local network, found within `wait`.
pub fn discover(wait: Duration) -> Result<Vec<Scanner>> {
    let daemon = mdns_sd::ServiceDaemon::new().map_err(net_err)?;
    let events = daemon.browse(SERVICE).map_err(net_err)?;
    let deadline = Instant::now() + wait;
    let mut found: Vec<Scanner> = Vec::new();
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        let Ok(event) = events.recv_timeout(left) else {
            break;
        };
        if let mdns_sd::ServiceEvent::ServiceResolved(info) = event {
            let Some(addr) = info.get_addresses_v4().into_iter().next() else {
                continue;
            };
            let root = info
                .get_property_val_str("rs")
                .unwrap_or("eSCL")
                .trim_matches('/');
            let name = info
                .get_property_val_str("ty")
                .map(str::to_string)
                .unwrap_or_else(|| {
                    let full = mdns_sd::instance_name_unescaped(info.get_fullname());
                    full.strip_suffix(&format!(".{SERVICE}"))
                        .unwrap_or(&full)
                        .to_string()
                });
            let url = format!("http://{addr}:{}/{root}", info.get_port());
            if !found.iter().any(|s| s.url == url) {
                found.push(Scanner { name, url });
            }
        }
    }
    let _ = daemon.shutdown();
    Ok(found)
}

/// A scanner typed in by address: `192.168.1.20`, `scanner.local:8080`,
/// or a full `http://…/eSCL` URL.
pub fn scanner_at(address: &str) -> Scanner {
    let a = address.trim().trim_end_matches('/');
    let url = if a.starts_with("http://") || a.starts_with("https://") {
        if a.to_ascii_lowercase().ends_with("/escl") {
            a.to_string()
        } else {
            format!("{a}/eSCL")
        }
    } else {
        format!("http://{a}/eSCL")
    };
    Scanner {
        name: a.to_string(),
        url,
    }
}

fn net_err(e: impl std::fmt::Display) -> Error {
    Error::InvalidValue(format!("scanner: {e}"))
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .http_status_as_error(false)
        .build()
        .into()
}

// MARK: - Capabilities

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InputSource {
    /// The flatbed glass.
    Platen,
    /// The document feeder.
    Feeder,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceCaps {
    pub color_modes: Vec<String>,
    pub resolutions: Vec<u32>,
    pub formats: Vec<String>,
    /// In 1/300 inch.
    pub max_width: u32,
    pub max_height: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub make_and_model: Option<String>,
    pub platen: Option<SourceCaps>,
    pub feeder: Option<SourceCaps>,
    pub duplex: bool,
}

impl Capabilities {
    pub fn source(&self, s: InputSource) -> Option<&SourceCaps> {
        match s {
            InputSource::Platen => self.platen.as_ref(),
            InputSource::Feeder => self.feeder.as_ref(),
        }
    }
}

pub fn capabilities(scanner: &Scanner) -> Result<Capabilities> {
    let mut resp = agent(Duration::from_secs(8))
        .get(&format!("{}/ScannerCapabilities", scanner.url))
        .call()
        .map_err(|e| net_err(format!("{} isn't reachable ({e})", scanner.name)))?;
    if resp.status() != 200 {
        return Err(net_err(format!(
            "{} answered {} (is it an eSCL scanner?)",
            scanner.name,
            resp.status()
        )));
    }
    let xml = resp.body_mut().read_to_string().map_err(net_err)?;
    parse_capabilities(&xml)
}

/// Element names without namespace prefixes, so `scan:` vs `escl:` and
/// `pwg:` spellings all work.
fn local(name: &str) -> String {
    name.rsplit(':').next().unwrap_or(name).to_string()
}

fn parse_capabilities(xml: &str) -> Result<Capabilities> {
    let mut reader = Reader::from_str(xml);
    let mut caps = Capabilities::default();
    let mut path: Vec<String> = Vec::new();
    let mut current: Option<SourceCaps> = None;
    let mut text = String::new();
    loop {
        match reader
            .read_event()
            .map_err(|e| net_err(format!("bad capabilities: {e}")))?
        {
            Event::Start(e) => {
                let name = local(e.name().as_ref());
                if matches!(name.as_str(), "PlatenInputCaps" | "AdfSimplexInputCaps") {
                    current = Some(SourceCaps::default());
                }
                if name == "AdfDuplexInputCaps" {
                    caps.duplex = true;
                }
                path.push(name);
                text.clear();
            }
            Event::Text(t) => text.push_str(&t.xml10_content()),
            Event::End(_) => {
                let name = path.pop().unwrap_or_default();
                let value = text.trim().to_string();
                text.clear();
                let parent = path.last().map(String::as_str).unwrap_or("");
                match name.as_str() {
                    "MakeAndModel" if caps.make_and_model.is_none() && !value.is_empty() => {
                        caps.make_and_model = Some(value)
                    }
                    "PlatenInputCaps" => caps.platen = current.take(),
                    "AdfSimplexInputCaps" => caps.feeder = current.take(),
                    _ => {
                        if let Some(c) = current.as_mut() {
                            match name.as_str() {
                                "ColorMode" if !c.color_modes.contains(&value) => {
                                    c.color_modes.push(value)
                                }
                                "XResolution" if parent == "DiscreteResolution" => {
                                    if let Ok(r) = value.parse::<u32>()
                                        && !c.resolutions.contains(&r)
                                    {
                                        c.resolutions.push(r);
                                    }
                                }
                                "DocumentFormat" | "DocumentFormatExt"
                                    if !c.formats.contains(&value) =>
                                {
                                    c.formats.push(value)
                                }
                                "MaxWidth" => c.max_width = value.parse().unwrap_or(0),
                                "MaxHeight" => c.max_height = value.parse().unwrap_or(0),
                                _ => {}
                            }
                        }
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    for c in [caps.platen.as_mut(), caps.feeder.as_mut()]
        .into_iter()
        .flatten()
    {
        c.resolutions.sort_unstable();
    }
    if caps.platen.is_none() && caps.feeder.is_none() {
        return Err(net_err("the scanner reported no flatbed or feeder"));
    }
    Ok(caps)
}

// MARK: - Scanning

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ColorMode {
    Color,
    Gray,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PageSize {
    Letter,
    Legal,
    A4,
    /// Everything the scanner can take.
    Max,
}

impl PageSize {
    /// Width and height in 1/300 inch.
    fn units(self) -> (u32, u32) {
        match self {
            PageSize::Letter => (2550, 3300),
            PageSize::Legal => (2550, 4200),
            PageSize::A4 => (2480, 3508),
            PageSize::Max => (u32::MAX, u32::MAX),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRequest {
    pub source: InputSource,
    pub duplex: bool,
    pub color: ColorMode,
    pub resolution: u32,
    pub page_size: PageSize,
}

/// Scans with `req` and saves the pages as one PDF in `dest_dir` (named
/// `stem.pdf`, or `stem (2).pdf` if taken). `on_page(n)` is called as each
/// page arrives.
pub fn scan(
    scanner: &Scanner,
    caps: &Capabilities,
    req: &ScanRequest,
    dest_dir: &Path,
    stem: &str,
    mut on_page: impl FnMut(usize),
) -> Result<PathBuf> {
    let source = caps
        .source(req.source)
        .ok_or_else(|| net_err("this scanner doesn't have that input"))?;
    let color = match req.color {
        ColorMode::Color if source.color_modes.iter().any(|m| m == "RGB24") => "RGB24",
        _ => "Grayscale8",
    };
    let resolution = nearest(&source.resolutions, req.resolution).unwrap_or(req.resolution);
    let (w, h) = req.page_size.units();
    let (w, h) = (
        w.min(source.max_width.max(1)),
        h.min(source.max_height.max(1)),
    );
    let settings = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<scan:ScanSettings xmlns:scan="http://schemas.hp.com/imaging/escl/2011/05/03" xmlns:pwg="http://www.pwg.org/schemas/2010/12/sm">
  <pwg:Version>2.0</pwg:Version>
  <pwg:ScanRegions>
    <pwg:ScanRegion>
      <pwg:Height>{h}</pwg:Height>
      <pwg:Width>{w}</pwg:Width>
      <pwg:XOffset>0</pwg:XOffset>
      <pwg:YOffset>0</pwg:YOffset>
      <pwg:ContentRegionUnits>escl:ThreeHundredthsOfInches</pwg:ContentRegionUnits>
    </pwg:ScanRegion>
  </pwg:ScanRegions>
  <pwg:InputSource>{input}</pwg:InputSource>
  <scan:Duplex>{duplex}</scan:Duplex>
  <scan:ColorMode>{color}</scan:ColorMode>
  <scan:XResolution>{resolution}</scan:XResolution>
  <scan:YResolution>{resolution}</scan:YResolution>
  <pwg:DocumentFormat>image/jpeg</pwg:DocumentFormat>
  <scan:DocumentFormatExt>image/jpeg</scan:DocumentFormatExt>
</scan:ScanSettings>"#,
        input = match req.source {
            InputSource::Platen => "Platen",
            InputSource::Feeder => "Feeder",
        },
        duplex = req.duplex && req.source == InputSource::Feeder && caps.duplex,
    );

    let http = agent(PAGE_TIMEOUT);
    let resp = http
        .post(&format!("{}/ScanJobs", scanner.url))
        .header("Content-Type", "text/xml")
        .send(settings.as_bytes())
        .map_err(|e| net_err(format!("couldn't start the scan ({e})")))?;
    match resp.status().as_u16() {
        201 => {}
        409 | 503 => return Err(net_err("the scanner is busy, or the feeder is empty")),
        s => return Err(net_err(format!("the scanner refused the scan ({s})"))),
    }
    let job = resp
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .map(|loc| absolute(&scanner.url, loc))
        .ok_or_else(|| net_err("the scanner didn't say where the scan is"))?;

    // Fetch pages until the scanner says there are no more.
    let mut pages: Vec<Vec<u8>> = Vec::new();
    let result = (|| -> Result<()> {
        loop {
            let started = Instant::now();
            let mut resp = loop {
                let resp = http
                    .get(&format!("{job}/NextDocument"))
                    .call()
                    .map_err(|e| net_err(format!("lost the scanner ({e})")))?;
                if resp.status() == 503 && started.elapsed() < PAGE_TIMEOUT {
                    std::thread::sleep(Duration::from_millis(700));
                    continue;
                }
                break resp;
            };
            match resp.status().as_u16() {
                200 => {
                    let bytes = resp
                        .body_mut()
                        .with_config()
                        .limit(MAX_PAGE_BYTES)
                        .read_to_vec()
                        .map_err(net_err)?;
                    pages.push(bytes);
                    on_page(pages.len());
                    if req.source == InputSource::Platen {
                        return Ok(()); // one page from the glass
                    }
                }
                404 | 410 => return Ok(()), // done
                s => return Err(net_err(format!("the scan failed ({s})"))),
            }
        }
    })();
    let _ = http.delete(&job).call(); // tidy up; not all scanners need it
    result?;
    if pages.is_empty() {
        return Err(net_err("no pages were scanned (is the feeder empty?)"));
    }

    let pdf = if pages.len() == 1 && pages[0].starts_with(b"%PDF") {
        pages.remove(0) // some scanners send PDF regardless
    } else {
        jpegs_to_pdf(&pages, resolution as f64)?
    };
    write_new(dest_dir, stem, &pdf)
}

fn nearest(options: &[u32], want: u32) -> Option<u32> {
    options
        .iter()
        .copied()
        .min_by_key(|r| (*r as i64 - want as i64).abs())
}

/// Resolves a `Location` header against the scanner's URL.
fn absolute(base: &str, location: &str) -> String {
    let location = location.trim_end_matches('/');
    if location.starts_with("http://") || location.starts_with("https://") {
        return location.to_string();
    }
    // Scheme + host[:port] of the base.
    let origin_end = base
        .find("://")
        .and_then(|i| base[i + 3..].find('/').map(|j| i + 3 + j))
        .unwrap_or(base.len());
    let origin = &base[..origin_end];
    if location.starts_with('/') {
        format!("{origin}{location}")
    } else {
        format!("{}/{location}", base.trim_end_matches('/'))
    }
}

/// Writes under a hidden temporary name, then renames to a free name.
fn write_new(dir: &Path, stem: &str, bytes: &[u8]) -> Result<PathBuf> {
    let tmp = dir.join(format!(".{stem}.pdf.part"));
    fs::write(&tmp, bytes)?;
    let mut target = dir.join(format!("{stem}.pdf"));
    let mut n = 2;
    while target.exists() {
        target = dir.join(format!("{stem} ({n}).pdf"));
        n += 1;
    }
    fs::rename(&tmp, &target)?;
    Ok(target)
}

// MARK: - JPEG pages → PDF

struct JpegInfo {
    width: u32,
    height: u32,
    components: u8,
}

/// Size and color components from a JPEG's start-of-frame marker.
fn jpeg_info(data: &[u8]) -> Option<JpegInfo> {
    if !data.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut i = 2;
    while i + 4 <= data.len() {
        if data[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = data[i + 1];
        if marker == 0xFF {
            i += 1;
            continue;
        }
        let len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
        let is_sof = (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC);
        if is_sof && i + 9 < data.len() {
            return Some(JpegInfo {
                height: u16::from_be_bytes([data[i + 5], data[i + 6]]) as u32,
                width: u16::from_be_bytes([data[i + 7], data[i + 8]]) as u32,
                components: data[i + 9],
            });
        }
        i += 2 + len;
    }
    None
}

/// One page per JPEG, each the scanned size at `dpi`. The JPEG data is
/// embedded as is (no re-encoding).
fn jpegs_to_pdf(pages: &[Vec<u8>], dpi: f64) -> Result<Vec<u8>> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut kids = Vec::new();
    for data in pages {
        let info =
            jpeg_info(data).ok_or_else(|| net_err("the scanner sent a page that isn't a JPEG"))?;
        let color_space = match info.components {
            1 => "DeviceGray",
            3 => "DeviceRGB",
            n => {
                return Err(net_err(format!(
                    "unsupported JPEG with {n} color components"
                )));
            }
        };
        let image = doc.add_object(Stream::new(
            Dictionary::from_iter([
                ("Type", Object::Name(b"XObject".to_vec())),
                ("Subtype", Object::Name(b"Image".to_vec())),
                ("Width", Object::Integer(info.width as i64)),
                ("Height", Object::Integer(info.height as i64)),
                ("ColorSpace", Object::Name(color_space.as_bytes().to_vec())),
                ("BitsPerComponent", Object::Integer(8)),
                ("Filter", Object::Name(b"DCTDecode".to_vec())),
            ]),
            data.clone(),
        ));
        let (w, h) = (
            info.width as f64 * 72.0 / dpi,
            info.height as f64 * 72.0 / dpi,
        );
        let content = doc.add_object(Stream::new(
            Dictionary::new(),
            format!("q {w:.3} 0 0 {h:.3} 0 0 cm /Im0 Do Q\n").into_bytes(),
        ));
        let page = doc.add_object(Dictionary::from_iter([
            ("Type", Object::Name(b"Page".to_vec())),
            ("Parent", Object::Reference(pages_id)),
            (
                "MediaBox",
                Object::Array(vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Real(w as f32),
                    Object::Real(h as f32),
                ]),
            ),
            (
                "Resources",
                Object::Dictionary(Dictionary::from_iter([(
                    "XObject",
                    Object::Dictionary(Dictionary::from_iter([("Im0", Object::Reference(image))])),
                )])),
            ),
            ("Contents", Object::Reference(content)),
        ]));
        kids.push(Object::Reference(page));
    }
    doc.objects.insert(
        pages_id,
        Object::Dictionary(Dictionary::from_iter([
            ("Type", Object::Name(b"Pages".to_vec())),
            ("Count", Object::Integer(kids.len() as i64)),
            ("Kids", Object::Array(kids)),
        ])),
    );
    let catalog = doc.add_object(Dictionary::from_iter([
        ("Type", Object::Name(b"Catalog".to_vec())),
        ("Pages", Object::Reference(pages_id)),
    ]));
    doc.trailer.set("Root", catalog);
    let mut out = Vec::new();
    doc.save_to(&mut out)
        .map_err(|e| Error::Pdf(e.to_string()))?;
    Ok(out)
}

// MARK: - Simulated scanner

/// A minimal eSCL scanner over HTTP, for tests and for trying the scan
/// screen without hardware. Serves `pages` (JPEGs) through the feeder.
#[doc(hidden)]
pub mod mock {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};

    pub struct MockScanner {
        pub url: String,
        /// Scan settings received, as XML.
        pub jobs: Arc<Mutex<Vec<String>>>,
    }

    pub const CAPABILITIES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<scan:ScannerCapabilities xmlns:scan="http://schemas.hp.com/imaging/escl/2011/05/03" xmlns:pwg="http://www.pwg.org/schemas/2010/12/sm">
  <pwg:Version>2.63</pwg:Version>
  <pwg:MakeAndModel>Rupert Test Scanner</pwg:MakeAndModel>
  <scan:Platen><scan:PlatenInputCaps>
    <scan:MinWidth>8</scan:MinWidth><scan:MaxWidth>2550</scan:MaxWidth>
    <scan:MinHeight>8</scan:MinHeight><scan:MaxHeight>3508</scan:MaxHeight>
    <scan:SettingProfiles><scan:SettingProfile>
      <scan:ColorModes><scan:ColorMode>BlackAndWhite1</scan:ColorMode><scan:ColorMode>Grayscale8</scan:ColorMode><scan:ColorMode>RGB24</scan:ColorMode></scan:ColorModes>
      <scan:DocumentFormats><pwg:DocumentFormat>image/jpeg</pwg:DocumentFormat><pwg:DocumentFormat>application/pdf</pwg:DocumentFormat></scan:DocumentFormats>
      <scan:SupportedResolutions><scan:DiscreteResolutions>
        <scan:DiscreteResolution><scan:XResolution>300</scan:XResolution><scan:YResolution>300</scan:YResolution></scan:DiscreteResolution>
        <scan:DiscreteResolution><scan:XResolution>150</scan:XResolution><scan:YResolution>150</scan:YResolution></scan:DiscreteResolution>
      </scan:DiscreteResolutions></scan:SupportedResolutions>
    </scan:SettingProfile></scan:SettingProfiles>
  </scan:PlatenInputCaps></scan:Platen>
  <scan:Adf>
    <scan:AdfSimplexInputCaps>
      <scan:MaxWidth>2550</scan:MaxWidth><scan:MaxHeight>4200</scan:MaxHeight>
      <scan:SettingProfiles><scan:SettingProfile>
        <scan:ColorModes><scan:ColorMode>Grayscale8</scan:ColorMode><scan:ColorMode>RGB24</scan:ColorMode></scan:ColorModes>
        <scan:DocumentFormats><pwg:DocumentFormat>image/jpeg</pwg:DocumentFormat></scan:DocumentFormats>
        <scan:SupportedResolutions><scan:DiscreteResolutions>
          <scan:DiscreteResolution><scan:XResolution>150</scan:XResolution><scan:YResolution>150</scan:YResolution></scan:DiscreteResolution>
          <scan:DiscreteResolution><scan:XResolution>300</scan:XResolution><scan:YResolution>300</scan:YResolution></scan:DiscreteResolution>
        </scan:DiscreteResolutions></scan:SupportedResolutions>
      </scan:SettingProfile></scan:SettingProfiles>
    </scan:AdfSimplexInputCaps>
    <scan:AdfDuplexInputCaps><scan:MaxWidth>2550</scan:MaxWidth></scan:AdfDuplexInputCaps>
  </scan:Adf>
</scan:ScannerCapabilities>"#;

    impl MockScanner {
        /// Listens on `addr` (e.g. `127.0.0.1:0`) on a background thread.
        pub fn start(addr: &str, pages: Vec<Vec<u8>>) -> std::io::Result<MockScanner> {
            let listener = TcpListener::bind(addr)?;
            let url = format!("http://{}/eSCL", listener.local_addr()?);
            let jobs = Arc::new(Mutex::new(Vec::new()));
            let state = Arc::new(Mutex::new((pages, 0usize, 0usize))); // pages, next page, job count
            let jobs2 = Arc::clone(&jobs);
            std::thread::spawn(move || {
                for stream in listener.incoming().flatten() {
                    let (state, jobs) = (Arc::clone(&state), Arc::clone(&jobs2));
                    std::thread::spawn(move || {
                        let _ = handle(stream, &state, &jobs);
                    });
                }
            });
            Ok(MockScanner { url, jobs })
        }
    }

    type State = Mutex<(Vec<Vec<u8>>, usize, usize)>;

    fn handle(stream: TcpStream, state: &State, jobs: &Mutex<Vec<String>>) -> std::io::Result<()> {
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut request_line = String::new();
        reader.read_line(&mut request_line)?;
        let mut length = 0usize;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            if line.trim().is_empty() {
                break;
            }
            if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                length = v.trim().parse().unwrap_or(0);
            }
        }
        let mut body = vec![0u8; length];
        reader.read_exact(&mut body)?;
        let mut parts = request_line.split_whitespace();
        let (method, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        let mut out = stream;
        let respond = |out: &mut TcpStream, status: &str, ctype: &str, extra: &str, body: &[u8]| {
            let head = format!(
                "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n",
                body.len()
            );
            out.write_all(head.as_bytes())?;
            out.write_all(body)
        };
        match (method, path) {
            ("GET", "/eSCL/ScannerCapabilities") => {
                respond(&mut out, "200 OK", "text/xml", "", CAPABILITIES.as_bytes())
            }
            ("POST", "/eSCL/ScanJobs") => {
                jobs.lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&body).into_owned());
                let mut s = state.lock().unwrap();
                s.1 = 0;
                s.2 += 1;
                let location = format!("Location: /eSCL/ScanJobs/job{}\r\n", s.2);
                respond(&mut out, "201 Created", "text/plain", &location, b"")
            }
            ("GET", p) if p.ends_with("/NextDocument") => {
                let mut s = state.lock().unwrap();
                let platen = jobs
                    .lock()
                    .unwrap()
                    .last()
                    .is_some_and(|j| j.contains(">Platen<"));
                let limit = if platen { 1 } else { s.0.len() };
                if s.1 < limit.min(s.0.len()) {
                    let page = s.0[s.1].clone();
                    s.1 += 1;
                    drop(s);
                    respond(&mut out, "200 OK", "image/jpeg", "", &page)
                } else {
                    respond(&mut out, "404 Not Found", "text/plain", "", b"")
                }
            }
            ("DELETE", _) => respond(&mut out, "200 OK", "text/plain", "", b""),
            _ => respond(&mut out, "404 Not Found", "text/plain", "", b""),
        }
    }
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    /// A small grayscale JPEG (64 × 80 px).
    const PAGE: &[u8] = include_bytes!("../tests/data/page.jpg");

    #[test]
    fn parses_capabilities() {
        let caps = parse_capabilities(mock::CAPABILITIES).unwrap();
        assert_eq!(caps.make_and_model.as_deref(), Some("Rupert Test Scanner"));
        let platen = caps.platen.as_ref().unwrap();
        assert_eq!(platen.resolutions, [150, 300]);
        assert_eq!(
            platen.color_modes,
            ["BlackAndWhite1", "Grayscale8", "RGB24"]
        );
        assert_eq!((platen.max_width, platen.max_height), (2550, 3508));
        let feeder = caps.feeder.as_ref().unwrap();
        assert_eq!(feeder.max_height, 4200);
        assert!(caps.duplex);
    }

    #[test]
    fn reads_jpeg_size() {
        let info = jpeg_info(PAGE).unwrap();
        assert_eq!((info.width, info.height, info.components), (64, 80, 1));
        assert!(jpeg_info(b"not a jpeg").is_none());
    }

    #[test]
    fn resolves_job_locations() {
        let base = "http://10.0.0.5:8080/eSCL";
        assert_eq!(
            absolute(base, "/eSCL/ScanJobs/7"),
            "http://10.0.0.5:8080/eSCL/ScanJobs/7"
        );
        assert_eq!(
            absolute(base, "http://10.0.0.5/eSCL/ScanJobs/7/"),
            "http://10.0.0.5/eSCL/ScanJobs/7"
        );
        assert_eq!(scanner_at("192.168.1.20").url, "http://192.168.1.20/eSCL");
        assert_eq!(
            scanner_at("http://hp.local:8080/").url,
            "http://hp.local:8080/eSCL"
        );
    }

    #[test]
    fn scans_from_the_feeder_and_the_glass() {
        let mock = mock::MockScanner::start("127.0.0.1:0", vec![PAGE.to_vec(); 3]).unwrap();
        let scanner = Scanner {
            name: "Mock".into(),
            url: mock.url.clone(),
        };
        let caps = capabilities(&scanner).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut seen = 0;
        let req = ScanRequest {
            source: InputSource::Feeder,
            duplex: true,
            color: ColorMode::Gray,
            resolution: 200,
            page_size: PageSize::Letter,
        };
        let path = scan(&scanner, &caps, &req, dir.path(), "Scan 1", |n| seen = n).unwrap();
        assert_eq!(seen, 3);
        assert_eq!(path, dir.path().join("Scan 1.pdf"));
        let doc = Document::load(&path).unwrap();
        assert_eq!(doc.get_pages().len(), 3);
        let settings = mock.jobs.lock().unwrap()[0].clone();
        assert!(settings.contains("<pwg:InputSource>Feeder</pwg:InputSource>"));
        assert!(settings.contains("<scan:Duplex>true</scan:Duplex>"));
        assert!(
            settings.contains("<scan:XResolution>150</scan:XResolution>"),
            "nearest offered"
        );
        assert!(settings.contains("<pwg:Height>3300</pwg:Height>"), "letter");

        // The glass gives one page; a second scan gets a new name.
        let glass = ScanRequest {
            source: InputSource::Platen,
            ..req
        };
        let path = scan(&scanner, &caps, &glass, dir.path(), "Scan 1", |_| {}).unwrap();
        assert_eq!(path, dir.path().join("Scan 1 (2).pdf"));
        assert_eq!(Document::load(&path).unwrap().get_pages().len(), 1);
        assert!(
            !fs::read_dir(dir.path()).unwrap().any(|e| e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".part")),
            "no temporary files left"
        );
    }

    #[test]
    fn unreachable_scanner_is_an_error() {
        let scanner = scanner_at("127.0.0.1:9");
        assert!(capabilities(&scanner).is_err());
    }
}
