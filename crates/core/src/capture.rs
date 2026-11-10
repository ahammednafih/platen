//! Scan pipelines shared by the TUI and the app: A4 page, two-pass long page, hole fill.
use crate::escl::{self, ScanOpts, Scanner};
use crate::fill::{self, Fill};
use crate::pdf::jpeg_info;
use crate::session::{self, Kind, Page, Session};
use crate::stitch;
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Clone, Copy, Serialize, serde::Deserialize, Debug)]
pub struct Settings {
    pub dpi: u32,
    pub color: bool,
    pub fill: Fill,
}

impl Settings {
    pub fn opts(&self, kind: Kind) -> ScanOpts {
        let (width, height) = match kind {
            Kind::A4 => (escl::A4_W, escl::A4_H),
            Kind::Long => (escl::GLASS_W, escl::GLASS_H),
        };
        ScanOpts { dpi: self.dpi, color: self.color, width, height }
    }
}

/// A finished page plus a one-line summary for the user.
#[derive(Serialize, Debug)]
pub struct Captured {
    pub page: Page,
    pub note: String,
}

/// Top half of a long page, waiting for the user to turn the sheet.
#[derive(Serialize, serde::Deserialize, Clone, Debug)]
pub struct TopHalf {
    pub path: PathBuf,
    pub stem: String,
    pub secs: f32,
}

pub fn scan_a4(scanner: &Scanner, s: Settings, dir: &Path) -> Result<Captured> {
    let opts = s.opts(Kind::A4);
    let t0 = Instant::now();
    let data = scanner.scan(opts)?;
    let secs = t0.elapsed().as_secs_f32();
    let data = apply_fill(data, s.fill)?;
    let (w, h, _) = jpeg_info(&data).context("scanner sent a bad image")?;
    let file = format!("{}.jpg", Session::new_stem());
    std::fs::write(dir.join("pages").join(&file), &data)?;
    let page = Page { file, kind: Kind::A4, dpi: opts.dpi, w, h, secs };
    let (mw, mh) = page.mm();
    Ok(Captured { note: format!("A4 page {mw}×{mh} mm in {}", dur(secs)), page })
}

pub fn scan_top(scanner: &Scanner, s: Settings, dir: &Path) -> Result<TopHalf> {
    let stem = Session::new_stem();
    let path = dir.join("raw").join(format!("{stem}_top.jpg"));
    let t0 = Instant::now();
    std::fs::write(&path, scanner.scan(s.opts(Kind::Long))?)?;
    Ok(TopHalf { path, stem, secs: t0.elapsed().as_secs_f32() })
}

pub fn scan_bottom(scanner: &Scanner, s: Settings, dir: &Path, top: &TopHalf) -> Result<Captured> {
    let opts = s.opts(Kind::Long);
    let bottom = dir.join("raw").join(format!("{}_bottom.jpg", top.stem));
    let t0 = Instant::now();
    std::fs::write(&bottom, scanner.scan(opts)?)?;
    let bottom_secs = t0.elapsed().as_secs_f32();
    let t1 = Instant::now();
    let mut r = stitch::stitch(&session::load_rgb(&top.path)?, &session::load_rgb(&bottom)?)?;
    if s.fill != Fill::Off {
        fill::fill_holes(&mut r.image, s.fill);
    }
    let file = format!("{}.jpg", top.stem);
    std::fs::write(dir.join("pages").join(&file), session::encode_rgb(&r.image, 92)?)?;
    let join_secs = t1.elapsed().as_secs_f32();
    let secs = top.secs + bottom_secs + join_secs;
    let page = Page { file, kind: Kind::Long, dpi: opts.dpi, w: r.image.width(), h: r.image.height(), secs };
    let (mw, mh) = page.mm();
    let warn = if r.score < 0.2 { " — low match, check the preview" } else { "" };
    let note = format!(
        "Long page {mw}×{mh} mm in {} (top {} · bottom {} · join {}, match {:.2}){warn}",
        dur(secs),
        dur(top.secs),
        dur(bottom_secs),
        dur(join_secs),
        r.score
    );
    Ok(Captured { page, note })
}

/// Paint torn areas on an existing page into a new file; the caller swaps it in.
pub fn fill_page(dir: &Path, page: &Page, mode: Fill) -> Result<Captured> {
    let mode = if mode == Fill::Off { Fill::White } else { mode };
    let mut img = session::load_rgb(&dir.join("pages").join(&page.file))?;
    let n = fill::fill_holes(&mut img, mode);
    if n == 0 {
        bail!("no torn or cut areas found");
    }
    let mut page = page.clone();
    page.file = format!("{}.jpg", Session::new_stem());
    std::fs::write(dir.join("pages").join(&page.file), session::encode_rgb(&img, 92)?)?;
    let pct = n as f32 * 100.0 / (img.width() * img.height()) as f32;
    Ok(Captured { page, note: format!("filled torn areas {} ({pct:.1}% of page)", mode.label()) })
}

/// Fill holes in a scanner JPEG; re-encode only if something was painted.
fn apply_fill(data: Vec<u8>, mode: Fill) -> Result<Vec<u8>> {
    if mode == Fill::Off {
        return Ok(data);
    }
    let mut img = image::load_from_memory(&data)?.to_rgb8();
    if fill::fill_holes(&mut img, mode) == 0 {
        return Ok(data);
    }
    session::encode_rgb(&img, 92)
}

pub fn dur(secs: f32) -> String {
    let s = secs.round() as u64;
    match s {
        0..60 => format!("{s}s"),
        60..3600 => format!("{}m {:02}s", s / 60, s % 60),
        _ => format!("{}h {:02}m", s / 3600, s % 3600 / 60),
    }
}
