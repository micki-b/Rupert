//! QR codes as SVG (#123): the "Scan with your phone" panel shows where to
//! get Rupert on the phone. Black on white in every theme (phone cameras read
//! that best), with the quiet zone as part of the image.

use qrcodegen::{QrCode, QrCodeEcc};

/// An SVG of the QR code for `text`, with a 4-module quiet zone. `None` if
/// the text is too long for a QR code.
pub fn svg(text: &str) -> Option<String> {
    let qr = QrCode::encode_text(text, QrCodeEcc::Medium).ok()?;
    let border = 4;
    let size = qr.size() + border * 2;
    let mut path = String::new();
    for y in 0..qr.size() {
        for x in 0..qr.size() {
            if qr.get_module(x, y) {
                path.push_str(&format!("M{},{}h1v1h-1z", x + border, y + border));
            }
        }
    }
    Some(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {size} {size}\" \
         shape-rendering=\"crispEdges\"><rect width=\"100%\" height=\"100%\" fill=\"#fff\"/>\
         <path d=\"{path}\" fill=\"#000\"/></svg>"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_a_code() {
        let code = svg("https://rupertlibrarian.com/#download").unwrap();
        assert!(code.starts_with("<svg") && code.ends_with("</svg>"));
        // A small version: at most 33 modules plus the quiet zone.
        let side: i32 = code
            .split("viewBox=\"0 0 ")
            .nth(1)
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!((29..=41).contains(&side), "{side}");
        assert!(code.matches('z').count() > 100);
        assert_eq!(svg(&"x".repeat(5000)), None);
    }
}
