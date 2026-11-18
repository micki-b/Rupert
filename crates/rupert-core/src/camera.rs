//! Scans from a phone camera (the iOS document scanner): the cropped and
//! straightened JPEG pages become one PDF, which goes to the inbox folder.
//! OCR and filing suggestions happen later, on a computer running Rupert.

use crate::error::Result;
use crate::escl;

/// Camera pages carry no real resolution. Sizing each page so its longer
/// side is 11 inches makes a photographed Letter or A4 sheet come out at
/// its real size and keeps receipts in proportion.
const LONG_SIDE_INCHES: f64 = 11.0;

// MARK: - PDF

/// One page per JPEG, embedded as is.
pub fn pages_to_pdf(pages: &[Vec<u8>]) -> Result<Vec<u8>> {
    if pages.is_empty() {
        return Err(crate::Error::InvalidValue("the scan has no pages".into()));
    }
    escl::jpegs_to_pdf(pages, |w, h| w.max(h).max(1) as f64 / LONG_SIDE_INCHES)
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{Document, Object};

    #[test]
    fn long_side_is_eleven_inches() {
        let jpeg =
            std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/page.jpg")).unwrap();
        let pdf = pages_to_pdf(&[jpeg.clone(), jpeg]).unwrap();
        let doc = Document::load_mem(&pdf).unwrap();
        let pages = doc.get_pages();
        assert_eq!(pages.len(), 2);
        let page = doc.get_dictionary(pages[&1]).unwrap();
        let mb = page.get(b"MediaBox").unwrap().as_array().unwrap();
        let num = |o: &Object| o.as_float().unwrap();
        // The test page is 64×80 pixels: portrait, so 792 pt tall.
        assert!((num(&mb[3]) - 792.0).abs() < 0.01);
        assert!((num(&mb[2]) - 792.0 * 64.0 / 80.0).abs() < 0.01);
    }

    #[test]
    fn rejects_empty_scans_and_non_jpegs() {
        assert!(pages_to_pdf(&[]).is_err());
        assert!(pages_to_pdf(&[b"not a jpeg".to_vec()]).is_err());
    }
}
