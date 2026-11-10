//! Work on existing PDFs: shrink to a size limit, and preview pages (largest image per page).
//!
//! Shrinking re-encodes embedded raster images (lower resolution / JPEG quality) and leaves
//! text and vector content alone, so it works for scans and for most other PDFs.
use anyhow::{Context, Result, bail};
use image::{DynamicImage, imageops::FilterType};
use lopdf::{Document, Object, ObjectId};
use rayon::prelude::*;
use std::collections::HashSet;

/// Mildest first. Steps are tried by binary search, assuming size falls along the ladder.
const LADDER: [(f32, u8); 15] = [
    (1.0, 85),
    (1.0, 75),
    (1.0, 65),
    (0.85, 65),
    (0.7, 65),
    (0.7, 55),
    (0.55, 55),
    (0.45, 50),
    (0.38, 50),
    (0.3, 45),
    (0.24, 45),
    (0.19, 40),
    (0.15, 40),
    (0.12, 35),
    (0.09, 30),
];

pub struct Shrunk {
    pub data: Vec<u8>,
    /// False when even the strongest step stays above the limit (`data` is then the smallest).
    pub fits: bool,
    /// Resolution kept relative to the original, 1.0 = untouched.
    pub scale: f32,
    pub quality: u8,
}

struct Img {
    id: ObjectId,
    img: DynamicImage,
    original_len: usize,
}

pub fn shrink(pdf: &[u8], limit: u64) -> Result<Shrunk> {
    if pdf.len() as u64 <= limit {
        return Ok(Shrunk { data: pdf.to_vec(), fits: true, scale: 1.0, quality: 100 });
    }
    let doc = Document::load_mem(pdf).context("couldn't read the PDF")?;
    let images = images(&doc);
    if images.is_empty() {
        bail!("this PDF has no images to compress, so it can't be made smaller");
    }
    let (mut lo, mut hi) = (0, LADDER.len() - 1);
    let mut best: Option<(usize, Vec<u8>)> = None;
    // Strongest step first: if that doesn't fit, nothing will.
    let strongest = render(&doc, &images, LADDER[hi])?;
    if strongest.len() as u64 > limit {
        let (scale, quality) = LADDER[hi];
        return Ok(Shrunk { data: strongest, fits: false, scale, quality });
    }
    best = best.or(Some((hi, strongest)));
    while lo < hi {
        let mid = (lo + hi) / 2;
        let data = render(&doc, &images, LADDER[mid])?;
        if data.len() as u64 <= limit {
            hi = mid;
            best = Some((mid, data));
        } else {
            lo = mid + 1;
        }
    }
    let (i, data) = best.unwrap();
    let (scale, quality) = LADDER[i];
    Ok(Shrunk { data, fits: true, scale, quality })
}

/// Raster images we can decode and safely replace (8-bit RGB/gray, JPEG or Flate).
fn images(doc: &Document) -> Vec<Img> {
    let masks: HashSet<ObjectId> = doc
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok()?.dict.get(b"SMask").ok()?.as_reference().ok())
        .collect();
    let candidates: Vec<(ObjectId, &lopdf::Stream)> = doc
        .objects
        .iter()
        .filter(|(id, _)| !masks.contains(id))
        .filter_map(|(id, o)| Some((*id, o.as_stream().ok()?)))
        .filter(|(_, s)| s.dict.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Image"))
        .filter(|(_, s)| !s.dict.get(b"ImageMask").and_then(Object::as_bool).unwrap_or(false))
        .collect();
    candidates.into_par_iter().filter_map(|(id, s)| Some(Img { id, img: decode(s)?, original_len: s.content.len() })).collect()
}

fn decode(s: &lopdf::Stream) -> Option<DynamicImage> {
    let filters = s.filters().unwrap_or_default();
    if filters == [b"DCTDecode".as_slice()] {
        return image::load_from_memory_with_format(&s.content, image::ImageFormat::Jpeg).ok();
    }
    if !(filters.is_empty() || filters == [b"FlateDecode".as_slice()]) {
        return None;
    }
    let int = |k: &[u8]| s.dict.get(k).and_then(Object::as_i64).ok();
    if int(b"BitsPerComponent")? != 8 {
        return None;
    }
    let (w, h) = (int(b"Width")? as u32, int(b"Height")? as u32);
    let raw = s.get_plain_content().ok()?;
    match s.dict.get(b"ColorSpace").and_then(Object::as_name).ok()? {
        b"DeviceRGB" => image::RgbImage::from_raw(w, h, raw).map(DynamicImage::ImageRgb8),
        b"DeviceGray" => image::GrayImage::from_raw(w, h, raw).map(DynamicImage::ImageLuma8),
        _ => None,
    }
}

fn render(doc: &Document, images: &[Img], (scale, quality): (f32, u8)) -> Result<Vec<u8>> {
    let encoded: Vec<(ObjectId, Option<(Vec<u8>, u32, u32, bool)>)> =
        images.par_iter().map(|i| (i.id, encode(i, scale, quality))).collect();
    let mut doc = doc.clone();
    for (id, enc) in encoded {
        let Some((data, w, h, gray)) = enc else { continue };
        let Ok(Object::Stream(s)) = doc.get_object_mut(id) else { continue };
        s.dict.set("Filter", Object::Name(b"DCTDecode".to_vec()));
        s.dict.remove(b"DecodeParms");
        s.dict.remove(b"Decode");
        s.dict.set("Width", w as i64);
        s.dict.set("Height", h as i64);
        s.dict.set("BitsPerComponent", 8);
        s.dict.set("ColorSpace", Object::Name(if gray { b"DeviceGray".to_vec() } else { b"DeviceRGB".to_vec() }));
        s.allows_compression = false;
        s.set_content(data);
    }
    doc.compress();
    let mut out = Vec::new();
    doc.save_to(&mut out)?;
    Ok(out)
}

/// None keeps the original stream (re-encoding wouldn't make it smaller).
fn encode(i: &Img, scale: f32, quality: u8) -> Option<(Vec<u8>, u32, u32, bool)> {
    let (w, h) = (i.img.width(), i.img.height());
    let (nw, nh) = (((w as f32 * scale) as u32).max(16), ((h as f32 * scale) as u32).max(16));
    let img = if nw < w { i.img.resize_exact(nw, nh, FilterType::Triangle) } else { i.img.clone() };
    let gray = matches!(img, DynamicImage::ImageLuma8(_));
    let mut out = Vec::new();
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
    let ok = if gray { img.to_luma8().write_with_encoder(enc) } else { img.to_rgb8().write_with_encoder(enc) };
    (ok.is_ok() && out.len() < i.original_len).then(|| (out, img.width(), img.height(), gray))
}

pub fn page_count(pdf: &[u8]) -> Result<usize> {
    Ok(Document::load_mem(pdf)?.get_pages().len())
}

/// The largest decodable image on page `index` (0-based), as a JPEG at most `max_w` wide.
pub fn page_preview(pdf: &[u8], index: usize, max_w: u32) -> Result<Vec<u8>> {
    let doc = Document::load_mem(pdf)?;
    let page = *doc.get_pages().values().nth(index).context("no such page")?;
    let best = doc
        .get_page_images(page)?
        .into_iter()
        .max_by_key(|i| i.width * i.height)
        .context("this page has no image to preview")?;
    let s = doc.get_object(best.id)?.as_stream()?;
    let img = decode(s).context("can't preview this page's image")?;
    let img = if img.width() > max_w { img.resize(max_w, u32::MAX, FilterType::Triangle) } else { img };
    crate::session::encode_rgb(&img.to_rgb8(), 82)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::{self, JpegPage};

    fn noisy_pdf(tag: &str) -> Vec<u8> {
        let img = image::RgbImage::from_fn(1200, 1700, |x, y| image::Rgb([(x * 7 ^ y * 13) as u8, (x * y) as u8, (x + y) as u8]));
        let jpeg = crate::session::encode_rgb(&img, 95).unwrap();
        let path = std::env::temp_dir().join(format!("shrink-{tag}-{}.pdf", crate::session::Session::new_stem()));
        pdf::write(&path, &[JpegPage { data: jpeg.clone(), dpi: 150.0 }, JpegPage { data: jpeg, dpi: 150.0 }]).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        bytes
    }

    #[test]
    fn shrinks_under_the_limit_and_keeps_pages() {
        let pdf = noisy_pdf("fits");
        let limit = pdf.len() as u64 / 4;

        let s = shrink(&pdf, limit).unwrap();

        assert!(s.fits && s.data.len() as u64 <= limit, "{} > {limit}", s.data.len());
        assert_eq!(page_count(&s.data).unwrap(), 2);
        assert!(page_preview(&s.data, 1, 300).is_ok());
    }

    #[test]
    fn reports_when_the_limit_is_out_of_reach() {
        let s = shrink(&noisy_pdf("reach"), 1000).unwrap();
        assert!(!s.fits);
    }
}
