//! Laying pages onto paper: page ranges, scaling, N-up and odd/even sheets for manual two-sided.
use anyhow::{Result, bail};
use image::{Rgb, RgbImage, imageops};
use serde::{Deserialize, Serialize};

/// Paper size in millimetres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paper {
    pub w_mm: f32,
    pub h_mm: f32,
}

impl Paper {
    pub const A4: Paper = Paper { w_mm: 210.0, h_mm: 297.0 };

    /// From an IPP media keyword, e.g. `iso_a4_210x297mm` or `na_letter_8.5x11in`.
    pub fn from_media(kw: &str) -> Option<Paper> {
        let dims = kw.rsplit('_').next()?;
        let (dims, unit) = match dims.strip_suffix("mm") {
            Some(d) => (d, 1.0),
            None => (dims.strip_suffix("in")?, 25.4),
        };
        let (w, h) = dims.split_once('x')?;
        Some(Paper { w_mm: w.parse::<f32>().ok()? * unit, h_mm: h.parse::<f32>().ok()? * unit })
    }

    pub fn px(&self, dpi: u32) -> (u32, u32) {
        let f = dpi as f32 / 25.4;
        ((self.w_mm * f).round() as u32, (self.h_mm * f).round() as u32)
    }
}

/// A page image and the resolution its pixels were made at (for actual-size placement).
pub struct Src {
    pub img: RgbImage,
    pub dpi: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Scale {
    /// Shrink or grow to fill the space, rotating if that fits better.
    #[default]
    Fit,
    /// Original size, from the top-left corner (cropped if larger).
    Actual,
    /// Percent of original size, from the top-left corner.
    Percent(u32),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Subset {
    #[default]
    All,
    Odd,
    Even,
}

/// 0-based page indices for `ranges` like `1-3, 5, 8-` (1-based; empty = every page).
pub fn pick(ranges: &str, n: usize) -> Result<Vec<usize>> {
    let parts: Vec<&str> = ranges.split(',').map(str::trim).filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return Ok((0..n).collect());
    }
    let num = |s: &str, default: usize| -> Result<usize> {
        let s = s.trim();
        if s.is_empty() { Ok(default) } else { s.parse().map_err(|_| anyhow::anyhow!("“{s}” isn't a page number")) }
    };
    let mut out = Vec::new();
    for part in parts {
        let (a, b) = match part.split_once('-') {
            Some((a, b)) => (num(a, 1)?, num(b, n)?),
            None => (num(part, 0)?, num(part, 0)?),
        };
        if a == 0 || a > b || a > n {
            bail!("pages “{part}” aren't in 1–{n}");
        }
        out.extend(a - 1..b.min(n));
    }
    Ok(out)
}

/// Sheet order after odd/even and reverse. For even sheets of an odd count, `None` is a blank
/// back so the reloaded stack lines up with the fronts.
pub fn order(sheets: usize, subset: Subset, reverse: bool) -> Vec<Option<usize>> {
    let mut out: Vec<Option<usize>> = (0..sheets)
        .filter(|i| match subset {
            Subset::All => true,
            Subset::Odd => i % 2 == 0,
            Subset::Even => i % 2 == 1,
        })
        .map(Some)
        .collect();
    if subset == Subset::Even && sheets % 2 == 1 {
        out.push(None);
    }
    if reverse {
        out.reverse();
    }
    out
}

/// Columns × rows for `n` pages per sheet.
pub fn grid(n: u32) -> (u32, u32) {
    match n {
        2 => (1, 2),
        4 => (2, 2),
        6 => (2, 3),
        9 => (3, 3),
        16 => (4, 4),
        _ => (1, 1),
    }
}

/// Lay `pages` onto one white sheet, row by row in an N-up grid. N-up always fits each cell.
pub fn compose(pages: &[Src], n: u32, scale: Scale, paper: Paper, dpi: u32) -> RgbImage {
    let (w, h) = paper.px(dpi);
    let mut sheet = RgbImage::from_pixel(w, h, Rgb([255, 255, 255]));
    let (cols, rows) = grid(n);
    let gap = if cols * rows > 1 { (4.0 * dpi as f32 / 25.4) as u32 } else { 0 };
    let (cw, ch) = ((w - gap * (cols + 1)) / cols, (h - gap * (rows + 1)) / rows);
    let scale = if cols * rows > 1 { Scale::Fit } else { scale };
    for (i, p) in pages.iter().take((cols * rows) as usize).enumerate() {
        let (c, r) = (i as u32 % cols, i as u32 / cols);
        let (x, y) = (gap + c * (cw + gap), gap + r * (ch + gap));
        let img = place(p, scale, cw, ch, dpi);
        let (ox, oy) = match scale {
            Scale::Fit => ((cw - img.width()) / 2, (ch - img.height()) / 2),
            _ => (0, 0),
        };
        imageops::replace(&mut sheet, &img, (x + ox) as i64, (y + oy) as i64);
    }
    sheet
}

/// The page resized for a `cw`×`ch` cell; never larger than the cell.
fn place(p: &Src, scale: Scale, cw: u32, ch: u32, dpi: u32) -> RgbImage {
    let (pw, ph) = (p.img.width() as f32, p.img.height() as f32);
    if scale == Scale::Fit {
        let k = |w: f32, h: f32| (cw as f32 / w).min(ch as f32 / h);
        let rotate = k(ph, pw) > k(pw, ph) * 1.02;
        let img = if rotate { imageops::rotate270(&p.img) } else { p.img.clone() };
        let f = k(img.width() as f32, img.height() as f32);
        let size = ((img.width() as f32 * f) as u32).clamp(1, cw);
        let tall = ((img.height() as f32 * f) as u32).clamp(1, ch);
        return imageops::resize(&img, size, tall, imageops::FilterType::Triangle);
    }
    let pct = if let Scale::Percent(v) = scale { v.clamp(10, 400) } else { 100 } as f32 / 100.0;
    let f = dpi as f32 / p.dpi * pct;
    let (w, h) = (((pw * f) as u32).max(1), ((ph * f) as u32).max(1));
    let img = if (w, h) == (p.img.width(), p.img.height()) { p.img.clone() } else { imageops::resize(&p.img, w, h, imageops::FilterType::Triangle) };
    imageops::crop_imm(&img, 0, 0, w.min(cw), h.min(ch)).to_image()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(w: u32, h: u32, v: u8) -> Src {
        Src { img: RgbImage::from_pixel(w, h, Rgb([v, v, v])), dpi: 100.0 }
    }

    #[test]
    fn picks_ranges_and_open_ends() {
        assert_eq!(pick("", 3).unwrap(), [0, 1, 2]);
        assert_eq!(pick("1-2, 5, 7-", 8).unwrap(), [0, 1, 4, 6, 7]);
        assert_eq!(pick("-2", 5).unwrap(), [0, 1]);
        assert!(pick("4", 3).is_err());
        assert!(pick("x", 3).is_err());
    }

    #[test]
    fn orders_odd_and_even_sheets_for_two_sided() {
        assert_eq!(order(5, Subset::Odd, false), [Some(0), Some(2), Some(4)]);
        assert_eq!(order(5, Subset::Even, true), [None, Some(3), Some(1)]);
        assert_eq!(order(2, Subset::All, true), [Some(1), Some(0)]);
    }

    #[test]
    fn reads_media_keywords() {
        assert_eq!(Paper::from_media("iso_a4_210x297mm"), Some(Paper::A4));
        let letter = Paper::from_media("na_letter_8.5x11in").unwrap();
        assert!((letter.w_mm - 215.9).abs() < 0.1);
    }

    #[test]
    fn two_up_rotates_portrait_pages_into_landscape_cells() {
        let s = compose(&[page(200, 300, 0), page(200, 300, 0)], 2, Scale::Fit, Paper::A4, 50);
        let (w, h) = Paper::A4.px(50);
        assert_eq!((s.width(), s.height()), (w, h));
        // Both halves have ink in their middle, the gutter between them stays white.
        assert_eq!(s.get_pixel(w / 2, h / 4)[0], 0);
        assert_eq!(s.get_pixel(w / 2, h * 3 / 4)[0], 0);
        assert_eq!(s.get_pixel(w / 2, h / 2)[0], 255);
    }

    #[test]
    fn actual_size_keeps_physical_size_from_the_corner() {
        // 100 px at 100 dpi = 1 inch = 50 px on a 50 dpi sheet.
        let s = compose(&[page(100, 100, 0)], 1, Scale::Actual, Paper::A4, 50);
        assert_eq!(s.get_pixel(49, 49)[0], 0);
        assert_eq!(s.get_pixel(51, 51)[0], 255);
        let half = compose(&[page(100, 100, 0)], 1, Scale::Percent(50), Paper::A4, 50);
        assert_eq!(half.get_pixel(26, 26)[0], 255);
    }
}
