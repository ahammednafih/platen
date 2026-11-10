//! A scanning session: an ordered list of page JPEGs in one folder, plus export.
use crate::pdf::{self, JpegPage};
use anyhow::{Context, Result};
use image::{GrayImage, ImageReader, imageops};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    A4,
    Long,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Page {
    pub file: String,
    pub kind: Kind,
    pub dpi: u32,
    pub w: u32,
    pub h: u32,
    /// Machine time for this page (scans + join), excluding waiting on the user.
    #[serde(default)]
    pub secs: f32,
}

impl Page {
    pub fn mm(&self) -> (u32, u32) {
        let f = 25.4 / self.dpi as f32;
        ((self.w as f32 * f).round() as u32, (self.h as f32 * f).round() as u32)
    }
}

pub struct Session {
    pub name: String,
    pub dir: PathBuf,
    pub pages: Vec<Page>,
    pub dirty: bool,
}

impl Session {
    pub fn open(dir: PathBuf) -> Result<Session> {
        std::fs::create_dir_all(dir.join("pages"))?;
        std::fs::create_dir_all(dir.join("raw"))?;
        let json = dir.join("session.json");
        let pages = match std::fs::read(&json) {
            Ok(b) => serde_json::from_slice(&b).context("session.json is corrupt")?,
            Err(_) => Vec::new(),
        };
        let name = dir.file_name().unwrap_or_default().to_string_lossy().into_owned();
        Ok(Session { name, dir, pages, dirty: false })
    }

    pub fn save(&mut self) -> Result<()> {
        let tmp = self.dir.join("session.json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&self.pages)?)?;
        std::fs::rename(tmp, self.dir.join("session.json"))?;
        self.dirty = true;
        Ok(())
    }

    pub fn page_path(&self, p: &Page) -> PathBuf {
        self.dir.join("pages").join(&p.file)
    }

    /// Unique file stem for a new scan.
    pub fn new_stem() -> String {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        format!("{}", t.as_millis())
    }

    /// Write the selected outputs; optionally delete the working scans.
    /// Returns the files written.
    pub fn export(&mut self, o: ExportOpts) -> Result<Vec<PathBuf>> {
        let originals: Vec<(Vec<u8>, u32)> = self
            .pages
            .iter()
            .map(|p| Ok((std::fs::read(self.page_path(p))?, p.dpi)))
            .collect::<Result<_>>()?;
        let mut written = Vec::new();
        let out = |suffix: &str| self.dir.join(format!("{}_{suffix}", self.name));

        if o.color {
            let pages: Vec<JpegPage> = originals.iter().map(|(d, dpi)| JpegPage { data: d.clone(), dpi: *dpi as f32 }).collect();
            pdf::write(&out("color.pdf"), &pages)?;
            written.push(out("color.pdf"));
        }
        if o.share || o.print {
            let derived: Vec<(JpegPage, JpegPage)> = originals
                .par_iter()
                .map(|(d, dpi)| derive(d, *dpi, o.share, o.print))
                .collect::<Result<_>>()?;
            let (share, print): (Vec<_>, Vec<_>) = derived.into_iter().unzip();
            if o.share {
                pdf::write(&out("share.pdf"), &share)?;
                written.push(out("share.pdf"));
            }
            if o.print {
                pdf::write(&out("print.pdf"), &print)?;
                written.push(out("print.pdf"));
            }
        }
        if let Some(mb) = o.max_mb {
            for f in written.iter().filter(|f| f.extension().is_some_and(|e| e == "pdf")) {
                fit_pdf(f, mb)?;
            }
        }
        if o.images {
            for (i, (d, _)) in originals.iter().enumerate() {
                let f = out(&format!("page{:02}.jpg", i + 1));
                std::fs::write(&f, d)?;
                written.push(f);
            }
        }
        if o.cleanup {
            let _ = std::fs::remove_dir_all(self.dir.join("raw"));
            let _ = std::fs::remove_dir_all(self.dir.join("pages"));
            let _ = std::fs::remove_file(self.dir.join("session.json"));
        }
        self.dirty = false;
        Ok(written)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct ExportOpts {
    pub color: bool,
    pub share: bool,
    pub print: bool,
    pub images: bool,
    pub cleanup: bool,
    #[serde(default)]
    pub open_pdf: bool,
    #[serde(default)]
    pub open_folder: bool,
    /// Shrink each PDF to fit under this many megabytes (upload limits).
    #[serde(default)]
    pub max_mb: Option<f32>,
}

impl ExportOpts {
    pub const LABELS: [&str; 7] = [
        "Colour PDF (full quality)",
        "Share PDF (colour, 300 dpi, small)",
        "Print PDF (black & white, paper whitened)",
        "Page images (JPEG, full quality)",
        "Delete working scans afterwards (ends session)",
        "Open PDF in Preview when done",
        "Open folder in Finder when done",
    ];

    pub fn get(&self, i: usize) -> bool {
        [self.color, self.share, self.print, self.images, self.cleanup, self.open_pdf, self.open_folder][i]
    }

    pub fn toggle(&mut self, i: usize) {
        let f = match i {
            0 => &mut self.color,
            1 => &mut self.share,
            2 => &mut self.print,
            3 => &mut self.images,
            4 => &mut self.cleanup,
            5 => &mut self.open_pdf,
            _ => &mut self.open_folder,
        };
        *f = !*f;
    }

    pub fn any_output(&self) -> bool {
        self.color || self.share || self.print || self.images
    }

    fn path() -> PathBuf {
        dirs::config_dir().unwrap_or_default().join("scan").join("export.json")
    }

    pub fn load() -> ExportOpts {
        std::fs::read(Self::path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or(ExportOpts { color: true, share: false, print: false, images: false, cleanup: false, open_pdf: true, open_folder: false, max_mb: None })
    }

    pub fn save(&self) {
        let p = Self::path();
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let _ = std::fs::write(p, serde_json::to_vec(self).unwrap_or_default());
    }
}

/// Rewrite `path` in place to fit under `mb` megabytes.
pub fn fit_pdf(path: &Path, mb: f32) -> Result<()> {
    let limit = (mb * 1_000_000.0) as u64;
    let r = crate::pdfedit::shrink(&std::fs::read(path)?, limit)?;
    if !r.fits {
        anyhow::bail!(
            "{} can't get under {mb} MB (smallest is {:.1} MB) — try fewer pages or a lower resolution",
            path.file_name().unwrap_or_default().to_string_lossy(),
            r.data.len() as f32 / 1e6
        );
    }
    std::fs::write(path, r.data)?;
    Ok(())
}

/// 300 dpi colour copy for sharing, and a whitened grayscale copy for printing.
fn derive(data: &[u8], dpi: u32, want_share: bool, want_print: bool) -> Result<(JpegPage, JpegPage)> {
    let img = ImageReader::new(std::io::Cursor::new(data)).with_guessed_format()?.decode()?.to_rgb8();
    let target = 300.min(dpi);
    let img = if dpi > target {
        let (w, h) = (img.width() * target / dpi, img.height() * target / dpi);
        imageops::resize(&img, w, h, imageops::FilterType::Triangle)
    } else {
        img
    };
    let share = if want_share { encode_rgb(&img, 82)? } else { Vec::new() };
    if !want_print {
        return Ok((JpegPage { data: share, dpi: target as f32 }, JpegPage { data: Vec::new(), dpi: target as f32 }));
    }
    let mut gray: GrayImage = imageops::grayscale(&img);
    // Push cream paper to white, keep ink dark (like `-level 25%,76%,0.9`).
    let lut: Vec<u8> = (0..256)
        .map(|v| {
            let t = ((v as f32 / 255.0 - 0.25) / (0.76 - 0.25)).clamp(0.0, 1.0);
            (t.powf(1.0 / 0.9) * 255.0).round() as u8
        })
        .collect();
    gray.pixels_mut().for_each(|p| p[0] = lut[p[0] as usize]);
    let print = encode_gray(&gray, 88)?;
    Ok((JpegPage { data: share, dpi: target as f32 }, JpegPage { data: print, dpi: target as f32 }))
}

pub fn encode_rgb(img: &image::RgbImage, q: u8) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, q).encode_image(img)?;
    Ok(out)
}

pub fn encode_gray(img: &GrayImage, q: u8) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, q).encode_image(img)?;
    Ok(out)
}

pub fn load_rgb(path: &Path) -> Result<image::RgbImage> {
    Ok(ImageReader::open(path)?.with_guessed_format()?.decode()?.to_rgb8())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_writes_selected_outputs_and_cleans_up() {
        let dir = std::env::temp_dir().join(format!("scan-test-{}", Session::new_stem()));
        let mut s = Session::open(dir.clone()).unwrap();
        let img = image::RgbImage::from_pixel(620, 877, image::Rgb([230, 220, 200]));
        std::fs::write(s.dir.join("pages/a.jpg"), encode_rgb(&img, 90).unwrap()).unwrap();
        s.pages.push(Page { file: "a.jpg".into(), kind: Kind::A4, dpi: 75, w: 620, h: 877, secs: 1.0 });
        s.save().unwrap();

        let opts = ExportOpts { color: false, share: true, print: true, images: true, cleanup: true, open_pdf: false, open_folder: false, max_mb: None };
        let files = s.export(opts).unwrap();

        let names: Vec<_> = files.iter().map(|f| f.file_name().unwrap().to_string_lossy().into_owned()).collect();
        let n = &s.name;
        assert_eq!(names, [format!("{n}_share.pdf"), format!("{n}_print.pdf"), format!("{n}_page01.jpg")]);
        assert!(files.iter().all(|f| f.exists()));
        assert!(!dir.join("pages").exists() && !dir.join("raw").exists() && !dir.join("session.json").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
