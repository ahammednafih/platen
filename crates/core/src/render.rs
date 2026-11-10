//! Printable files as pages: images load as-is, PDF pages are rasterized (CoreGraphics on macOS).
use crate::sheet::Src;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// One page of a printable file (`index` is 0-based; always 0 for images).
#[derive(Clone, Debug, PartialEq)]
pub struct SrcPage {
    pub path: PathBuf,
    pub index: usize,
}

pub fn is_pdf(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// Every page of every file, in order.
pub fn pages(files: &[PathBuf]) -> Result<Vec<SrcPage>> {
    let mut out = Vec::new();
    for f in files {
        let n = if is_pdf(f) { page_count(f)? } else { 1 };
        out.extend((0..n).map(|index| SrcPage { path: f.clone(), index }));
    }
    Ok(out)
}

pub fn page_count(pdf: &Path) -> Result<usize> {
    #[cfg(target_os = "macos")]
    return cg::count(pdf);
    #[cfg(not(target_os = "macos"))]
    return crate::pdfedit::page_count(&std::fs::read(pdf)?);
}

/// Load a page; PDFs render at `dpi`, images keep their pixels and assume 300 dpi.
pub fn load(p: &SrcPage, dpi: u32) -> Result<Src> {
    if !is_pdf(&p.path) {
        let img = crate::session::load_rgb(&p.path).with_context(|| format!("can't open {}", p.path.display()))?;
        return Ok(Src { img, dpi: 300.0 });
    }
    #[cfg(target_os = "macos")]
    return Ok(Src { img: cg::render(&p.path, p.index, dpi)?, dpi: dpi as f32 });
    #[cfg(not(target_os = "macos"))]
    anyhow::bail!("printing PDFs needs macOS");
}

#[cfg(target_os = "macos")]
mod cg {
    use anyhow::{Context, Result, bail};
    use image::RgbImage;
    use objc2_core_foundation::{CFRetained, CFString, CFURL, CFURLPathStyle, CGPoint, CGRect, CGSize};
    use objc2_core_graphics::{CGBitmapContextCreate, CGColorSpace, CGContext, CGImageAlphaInfo, CGPDFBox, CGPDFDocument, CGPDFPage};
    use std::path::Path;

    /// Refuse pages that would need more than ~400 MB of pixels.
    const MAX_PIXELS: usize = 100_000_000;

    fn open(path: &Path) -> Result<CFRetained<CGPDFDocument>> {
        let s = CFString::from_str(path.to_str().context("file path isn't UTF-8")?);
        let url = CFURL::with_file_system_path(None, Some(&s), CFURLPathStyle::CFURLPOSIXPathStyle, false).context("bad file path")?;
        let doc = CGPDFDocument::with_url(Some(&url)).with_context(|| format!("can't open {} as a PDF", path.display()))?;
        if !CGPDFDocument::is_unlocked(Some(&doc)) {
            bail!("{} is password-protected", path.display());
        }
        Ok(doc)
    }

    pub fn count(path: &Path) -> Result<usize> {
        let doc = open(path)?;
        Ok(CGPDFDocument::number_of_pages(Some(&doc)))
    }

    pub fn render(path: &Path, index: usize, dpi: u32) -> Result<RgbImage> {
        let doc = open(path)?;
        let page = CGPDFDocument::page(Some(&doc), index + 1).context("no such page")?;
        let crop = CGPDFPage::box_rect(Some(&page), CGPDFBox::CropBox).size;
        let (wp, hp) = match CGPDFPage::rotation_angle(Some(&page)).rem_euclid(180) {
            0 => (crop.width, crop.height),
            _ => (crop.height, crop.width),
        };
        let k = dpi as f64 / 72.0;
        let (w, h) = ((wp * k).round().max(1.0) as usize, (hp * k).round().max(1.0) as usize);
        if w * h > MAX_PIXELS {
            bail!("page {} is too large to print", index + 1);
        }
        let mut buf = vec![255u8; w * h * 4];
        let space = CGColorSpace::new_device_rgb().context("no RGB colour space")?;
        // SAFETY: `buf` holds w*h RGBX pixels and outlives the context, which is dropped below.
        let ctx = unsafe { CGBitmapContextCreate(buf.as_mut_ptr().cast(), w, h, 8, w * 4, Some(&space), CGImageAlphaInfo::NoneSkipLast.0) }
            .context("can't create a bitmap")?;
        CGContext::scale_ctm(Some(&ctx), k, k);
        let rect = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(wp, hp));
        CGContext::concat_ctm(Some(&ctx), CGPDFPage::drawing_transform(Some(&page), CGPDFBox::CropBox, rect, 0, true));
        CGContext::draw_pdf_page(Some(&ctx), Some(&page));
        drop(ctx);
        let rgb = buf.as_chunks::<4>().0.iter().flat_map(|p| [p[0], p[1], p[2]]).collect();
        RgbImage::from_raw(w as u32, h as u32, rgb).context("bad bitmap")
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use crate::pdf::{self, JpegPage};

    #[test]
    fn renders_pdf_pages_at_the_asked_dpi() {
        let img = image::RgbImage::from_pixel(300, 600, image::Rgb([200, 30, 30]));
        let path = std::env::temp_dir().join(format!("render-{}.pdf", crate::session::Session::new_stem()));
        let jpeg = crate::session::encode_rgb(&img, 90).unwrap();
        pdf::write(&path, &[JpegPage { data: jpeg.clone(), dpi: 100.0 }, JpegPage { data: jpeg, dpi: 100.0 }]).unwrap();

        let pages = pages(std::slice::from_ref(&path)).unwrap();
        let src = load(&pages[1], 50).unwrap();
        std::fs::remove_file(&path).unwrap();

        assert_eq!(pages.len(), 2);
        assert_eq!((src.img.width(), src.img.height()), (150, 300));
        let mid = src.img.get_pixel(75, 150);
        assert!(mid[0] > 150 && mid[1] < 80, "{mid:?}");
    }
}
