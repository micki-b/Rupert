//! Scans from a phone camera: the cropped and straightened pages become
//! one PDF, which goes to the inbox folder. OCR and filing suggestions
//! happen later, on a computer running Rupert.
//!
//! Color and grayscale pages arrive as JPEGs and are embedded as is. Black
//! and white pages arrive as PNGs and become 1-bit images.

use crate::error::{Error, Result};
use crate::escl::{self, PageImage};

/// Camera pages carry no real resolution. Sizing each page so its longer
/// side is 11 inches makes a photographed Letter or A4 sheet come out at
/// its real size and keeps receipts in proportion.
const LONG_SIDE_INCHES: f64 = 11.0;

// MARK: - PDF

/// One page per image file (JPEG, or PNG for black and white).
pub fn pages_to_pdf(pages: &[Vec<u8>]) -> Result<Vec<u8>> {
    if pages.is_empty() {
        return Err(Error::InvalidValue("the scan has no pages".into()));
    }
    let images = pages
        .iter()
        .map(|data| {
            if data.starts_with(b"\x89PNG") {
                bilevel_from_png(data)
            } else {
                Ok(PageImage::Jpeg(data.clone()))
            }
        })
        .collect::<Result<Vec<_>>>()?;
    escl::images_to_pdf(&images, |w, h| w.max(h).max(1) as f64 / LONG_SIDE_INCHES)
}

/// A black-and-white page from a PNG (already thresholded on the phone;
/// anything below mid-gray counts as black).
fn bilevel_from_png(data: &[u8]) -> Result<PageImage> {
    let bad =
        |e: png::DecodingError| Error::InvalidValue(format!("a scanned page is unreadable: {e}"));
    let mut decoder = png::Decoder::new(std::io::Cursor::new(data));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(bad)?;
    let mut buf = vec![0; reader.output_buffer_size().unwrap_or(0)];
    let frame = reader.next_frame(&mut buf).map_err(bad)?;
    let (width, height) = (frame.width, frame.height);
    let channels = frame.color_type.samples();
    let row_bytes = (width as usize).div_ceil(8);
    let mut bits = vec![0u8; row_bytes * height as usize];
    for y in 0..height as usize {
        let row = &buf[y * frame.line_size..];
        for x in 0..width as usize {
            let px = &row[x * channels..x * channels + channels];
            let luma = match channels {
                1 | 2 => px[0] as u32,
                _ => (px[0] as u32 * 299 + px[1] as u32 * 587 + px[2] as u32 * 114) / 1000,
            };
            if luma >= 128 {
                bits[y * row_bytes + x / 8] |= 0x80 >> (x % 8);
            }
        }
    }
    Ok(PageImage::Bilevel {
        width,
        height,
        bits,
    })
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
    fn black_and_white_pages_become_one_bit_images() {
        // A 10×4 gray PNG: left half black, right half white.
        let mut png_bytes = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut png_bytes, 10, 4);
            enc.set_color(png::ColorType::Grayscale);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().unwrap();
            let row: Vec<u8> = (0..10).map(|x| if x < 5 { 20 } else { 240 }).collect();
            w.write_image_data(&row.repeat(4)).unwrap();
        }
        let PageImage::Bilevel {
            width,
            height,
            bits,
        } = bilevel_from_png(&png_bytes).unwrap()
        else {
            panic!("not bilevel");
        };
        assert_eq!((width, height), (10, 4));
        assert_eq!(&bits[..2], &[0b0000_0111, 0b1100_0000]);

        let pdf = pages_to_pdf(&[png_bytes]).unwrap();
        let doc = Document::load_mem(&pdf).unwrap();
        let page = doc.get_dictionary(doc.get_pages()[&1]).unwrap();
        let mb = page.get(b"MediaBox").unwrap().as_array().unwrap();
        assert!(
            (mb[2].as_float().unwrap() - 792.0).abs() < 0.01,
            "long side 11 in"
        );
        let image = doc
            .objects
            .values()
            .find_map(|o| {
                o.as_stream()
                    .ok()
                    .filter(|s| s.dict.get(b"BitsPerComponent").is_ok())
            })
            .unwrap();
        assert_eq!(
            image
                .dict
                .get(b"BitsPerComponent")
                .unwrap()
                .as_i64()
                .unwrap(),
            1
        );
        assert_eq!(image.decompressed_content().unwrap(), bits);
    }

    #[test]
    fn rejects_empty_scans_and_non_jpegs() {
        assert!(pages_to_pdf(&[]).is_err());
        assert!(pages_to_pdf(&[b"not a jpeg".to_vec()]).is_err());
    }
}
