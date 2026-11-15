//! Copying: scan to a scratch folder, then print the scans as a `PrintJob`.
//!
//! Normal = one scan of the original's size; 2-in-1 = two scans on one sheet; ID card = front and back, each on
//! the top half of the glass, stacked at actual size; long = the two-pass stitched page; poster = one
//! scan enlarged across a grid of sheets.
use crate::capture::{self, Settings, TopHalf};
use crate::escl::{self, ColorMode, ScanOpts, Scanner, Size, SizeId};
use crate::fill::Fill;
use crate::session::{self, Session};
use anyhow::{Context, Result};
use image::imageops;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const COPY_DPI: u32 = 300;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Area {
    Size(SizeId),
    /// Top half of an A4 area, for ID cards.
    HalfTop,
}

/// Fresh scratch folder (with `pages/` and `raw/` for the long-page pipeline).
pub fn scratch() -> Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("scan-copy-{}", Session::new_stem()));
    std::fs::create_dir_all(dir.join("pages"))?;
    std::fs::create_dir_all(dir.join("raw"))?;
    Ok(dir)
}

fn settings(color: bool) -> Settings {
    let mode = if color { ColorMode::Color } else { ColorMode::Gray };
    Settings { dpi: COPY_DPI, mode, fill: Fill::Off, size: SizeId::A4 }
}

/// Scan one area into `dir`, returning the JPEG path.
pub fn scan(scanner: &Scanner, area: Area, color: bool, dir: &Path) -> Result<PathBuf> {
    let (width, height) = match area {
        Area::Size(id) => Size::get(id).region(),
        Area::HalfTop => (escl::A4_W, escl::A4_H / 2),
    };
    let opts = ScanOpts { dpi: COPY_DPI, mode: settings(color).mode, width, height };
    let path = dir.join(format!("{}.jpg", Session::new_stem()));
    std::fs::write(&path, scanner.scan(opts)?)?;
    Ok(path)
}

pub fn long_top(scanner: &Scanner, color: bool, dir: &Path) -> Result<TopHalf> {
    capture::scan_top(scanner, settings(color), dir)
}

pub fn long_bottom(scanner: &Scanner, color: bool, dir: &Path, top: &TopHalf) -> Result<PathBuf> {
    let c = capture::scan_bottom(scanner, settings(color), dir, top)?;
    Ok(dir.join("pages").join(c.page.file))
}

/// Front above back on one image the size of the two scans together (A4 for half-glass scans).
pub fn id_card(front: &Path, back: &Path, dir: &Path) -> Result<PathBuf> {
    let (f, b) = (session::load_rgb(front)?, session::load_rgb(back)?);
    let w = f.width().max(b.width());
    let mut out = image::RgbImage::from_pixel(w, f.height() + b.height(), image::Rgb([255, 255, 255]));
    imageops::replace(&mut out, &f, 0, 0);
    imageops::replace(&mut out, &b, 0, f.height() as i64);
    let path = dir.join(format!("{}_id.jpg", Session::new_stem()));
    std::fs::write(&path, session::encode_rgb(&out, 92)?).context("can't save the ID card sheet")?;
    Ok(path)
}

/// One scan enlarged across `n`×`n` sheets of `paper`, centred and as large as fits. Returns the
/// tiles row by row. Each tile is cut and scaled from the scan on its own, so the whole poster is
/// never held in memory.
pub fn poster(src: &Path, n: u32, paper: crate::sheet::Paper, dir: &Path) -> Result<Vec<PathBuf>> {
    let img = session::load_rgb(src)?;
    let n = n.clamp(2, 5);
    let (pw, ph) = paper.px(COPY_DPI);
    let (cw, ch) = ((pw * n) as f32, (ph * n) as f32);
    let scale = (cw / img.width() as f32).min(ch / img.height() as f32);
    let (sw, sh) = (img.width() as f32 * scale, img.height() as f32 * scale);
    let (ox, oy) = ((cw - sw) / 2.0, (ch - sh) / 2.0);
    let stem = Session::new_stem();
    let mut out = Vec::new();
    for row in 0..n {
        for col in 0..n {
            let (tx, ty) = ((col * pw) as f32, (row * ph) as f32);
            let mut tile = image::RgbImage::from_pixel(pw, ph, image::Rgb([255, 255, 255]));
            let (x0, y0) = (tx.max(ox), ty.max(oy));
            let (x1, y1) = ((tx + pw as f32).min(ox + sw), (ty + ph as f32).min(oy + sh));
            if x1 - x0 >= 1.0 && y1 - y0 >= 1.0 {
                let (sx, sy) = (((x0 - ox) / scale) as u32, ((y0 - oy) / scale) as u32);
                let sw_src = (((x1 - x0) / scale).ceil() as u32).clamp(1, img.width() - sx);
                let sh_src = (((y1 - y0) / scale).ceil() as u32).clamp(1, img.height() - sy);
                let part = imageops::crop_imm(&img, sx, sy, sw_src, sh_src).to_image();
                let part = imageops::resize(&part, (x1 - x0) as u32, (y1 - y0) as u32, imageops::FilterType::Triangle);
                imageops::replace(&mut tile, &part, (x0 - tx) as i64, (y0 - ty) as i64);
            }
            let path = dir.join(format!("{stem}_poster_{}_{}.jpg", row + 1, col + 1));
            std::fs::write(&path, session::encode_rgb(&tile, 90)?).context("can't save a poster tile")?;
            out.push(path);
        }
    }
    Ok(out)
}

/// Density/background-removed copy of a scan; the original is returned untouched when both are off.
pub fn tone(src: &Path, density: i32, background: u8, dir: &Path) -> Result<PathBuf> {
    if density == 0 && background == 0 {
        return Ok(src.to_path_buf());
    }
    let out = crate::adjust::tone(session::load_rgb(src)?, density, background);
    let path = dir.join(format!("{}_tone.jpg", Session::new_stem()));
    std::fs::write(&path, session::encode_rgb(&out, 92)?).context("can't save the toned copy")?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_card_stacks_front_over_back() {
        let dir = scratch().unwrap();
        let (f, b) = (dir.join("f.jpg"), dir.join("b.jpg"));
        std::fs::write(&f, session::encode_rgb(&image::RgbImage::from_pixel(40, 20, image::Rgb([0, 0, 0])), 95).unwrap()).unwrap();
        std::fs::write(&b, session::encode_rgb(&image::RgbImage::from_pixel(40, 20, image::Rgb([255, 255, 255])), 95).unwrap()).unwrap();

        let out = session::load_rgb(&id_card(&f, &b, &dir).unwrap()).unwrap();
        std::fs::remove_dir_all(dir).unwrap();

        assert_eq!((out.width(), out.height()), (40, 40));
        assert!(out.get_pixel(20, 5)[0] < 30 && out.get_pixel(20, 35)[0] > 225);
    }

    #[test]
    fn poster_splits_the_scan_into_paper_sized_tiles() {
        let dir = std::env::temp_dir().join(format!("scan-poster-{}", Session::new_stem()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("s.jpg");
        let img = image::RgbImage::from_fn(200, 280, |x, y| if (x < 100) == (y < 140) { image::Rgb([0, 0, 0]) } else { image::Rgb([255, 255, 255]) });
        std::fs::write(&src, session::encode_rgb(&img, 95).unwrap()).unwrap();
        let paper = crate::sheet::Paper { w_mm: 50.0, h_mm: 70.0 };

        let tiles = poster(&src, 2, paper, &dir).unwrap();
        let first = session::load_rgb(&tiles[0]).unwrap();
        let second = session::load_rgb(&tiles[1]).unwrap();
        std::fs::remove_dir_all(dir).unwrap();

        assert_eq!(tiles.len(), 4);
        assert_eq!((first.width(), first.height()), paper.px(COPY_DPI));
        let (w, h) = paper.px(COPY_DPI);
        assert!(first.get_pixel(w / 2, h / 2)[0] < 40, "top-left quarter of the scan is black");
        assert!(second.get_pixel(w / 2, h / 2)[0] > 215, "top-right quarter is white");
    }

    #[test]
    fn tone_whitens_tinted_paper_and_darkens_midtones() {
        let img = image::RgbImage::from_fn(2, 1, |x, _| if x == 0 { image::Rgb([220, 215, 200]) } else { image::Rgb([128; 3]) });
        let plain = crate::adjust::tone(img.clone(), 0, 0);
        let clean = crate::adjust::tone(img.clone(), 0, 3);
        let dark = crate::adjust::tone(img, 2, 0);

        assert_eq!(plain.get_pixel(0, 0), &image::Rgb([220, 215, 200]));
        assert_eq!(clean.get_pixel(0, 0), &image::Rgb([255; 3]));
        assert!(dark.get_pixel(1, 0)[0] < 128);
    }
}
