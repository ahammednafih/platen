//! A scanning session: an ordered list of page JPEGs in one folder, plus export.
use crate::pdf::{self, JpegPage};
use anyhow::{Context, Result, bail};
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

/// Scan names become folder names.
pub fn check_name(name: &str) -> Result<()> {
    if name.is_empty() || name.contains(['/', '\\', ':']) || name.starts_with('.') {
        bail!("Scan names can't be empty, contain / or :, or start with a dot");
    }
    Ok(())
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

    /// Rename the scan: moves its folder and renames exports that carry the old name (`<old>_print.pdf`).
    pub fn rename(&mut self, name: &str) -> Result<()> {
        let name = name.trim();
        if name == self.name {
            return Ok(());
        }
        check_name(name)?;
        let to = self.dir.parent().context("scan has no parent folder")?.join(name);
        if to.exists() {
            bail!("There's already a scan called “{name}”");
        }
        std::fs::rename(&self.dir, &to)?;
        let old = format!("{}_", self.name);
        for entry in std::fs::read_dir(&to)?.flatten() {
            let file = entry.file_name().to_string_lossy().into_owned();
            if let Some(rest) = file.strip_prefix(&old) {
                let _ = std::fs::rename(entry.path(), to.join(format!("{name}_{rest}")));
            }
        }
        self.dir = to;
        self.name = name.to_string();
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

    /// The pages as they are now, in one PDF at `out`, for printing without exporting first.
    pub fn write_pdf(&self, out: &Path) -> Result<()> {
        let pages: Vec<JpegPage> = self.pages.iter().map(|p| Ok(JpegPage { data: std::fs::read(self.page_path(p))?, dpi: p.dpi as f32 })).collect::<Result<_>>()?;
        if pages.is_empty() {
            bail!("Nothing to print yet");
        }
        std::fs::create_dir_all(out.parent().context("bad output path")?)?;
        pdf::write(out, &pages)
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
        if o.searchable {
            let pdfs: Vec<&PathBuf> = written.iter().collect();
            if !pdfs.is_empty() {
                let ocr = crate::ocr::Ocr::find()?;
                let layers = ocr.layers(&originals, &self.dir.join("ocr"))?;
                for f in pdfs {
                    std::fs::write(f, crate::ocr::overlay(&std::fs::read(f)?, &layers)?)?;
                }
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
    /// Add an invisible OCR text layer to each PDF.
    #[serde(default)]
    pub searchable: bool,
}

impl ExportOpts {
    pub const LABELS: [&str; 8] = [
        "Colour PDF (full quality)",
        "Share PDF (colour, 300 dpi, small)",
        "Print PDF (black & white, paper whitened)",
        "Page images (JPEG, full quality)",
        "Delete working scans afterwards (ends session)",
        "Open PDF in Preview when done",
        "Open folder in Finder when done",
        "Searchable PDFs (OCR text layer)",
    ];

    pub fn get(&self, i: usize) -> bool {
        [self.color, self.share, self.print, self.images, self.cleanup, self.open_pdf, self.open_folder, self.searchable][i]
    }

    pub fn toggle(&mut self, i: usize) {
        let f = match i {
            0 => &mut self.color,
            1 => &mut self.share,
            2 => &mut self.print,
            3 => &mut self.images,
            4 => &mut self.cleanup,
            5 => &mut self.open_pdf,
            6 => &mut self.open_folder,
            _ => &mut self.searchable,
        };
        *f = !*f;
    }

    pub fn any_output(&self) -> bool {
        self.color || self.share || self.print || self.images
    }

    fn path() -> PathBuf {
        crate::device::config_dir().join("export.json")
    }

    pub fn load() -> ExportOpts {
        std::fs::read(Self::path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or(ExportOpts { color: true, share: false, print: false, images: false, cleanup: false, open_pdf: true, open_folder: false, max_mb: None, searchable: false })
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
    whiten(&mut gray);
    let print = encode_gray(&gray, 88)?;
    Ok((JpegPage { data: share, dpi: target as f32 }, JpegPage { data: print, dpi: target as f32 }))
}

/// The print treatment: paper to white, ink kept dark.
///
/// Fixed cut-offs wash out faded documents, so this adapts to the page: the paper's brightness is
/// estimated locally (which also flattens stains and shadows), each pixel is taken relative to it, and
/// only what is close to paper goes white. Mid-tones are darkened so faint strokes survive.
pub fn whiten(gray: &mut GrayImage) {
    let (w, h) = gray.dimensions();
    if w < 16 || h < 16 {
        return;
    }
    let paper = paper_map(gray);
    let lut: Vec<u8> = (0..256)
        .map(|v| {
            let t = ((v as f32 / 255.0 - WHITEN_BLACK) / (WHITEN_WHITE - WHITEN_BLACK)).clamp(0.0, 1.0);
            (t.powf(WHITEN_GAMMA) * 255.0).round() as u8
        })
        .collect();
    for (p, bg) in gray.pixels_mut().zip(paper.pixels()) {
        let rel = (p[0] as f32 / bg[0] as f32).min(1.0);
        p[0] = lut[(rel * 255.0) as usize];
    }
}

/// Relative brightness (pixel / local paper) that becomes pure black and pure white, and the curve
/// between them; a gamma above 1 darkens faint ink.
const WHITEN_BLACK: f32 = 0.30;
const WHITEN_WHITE: f32 = 0.90;
const WHITEN_GAMMA: f32 = 1.5;

/// Brightness of the paper around each pixel: shrink the page, spread the bright values over ink
/// (a max filter), blur, and scale back up. Large dark areas (stamps, photos) are floored against
/// the page's overall paper level so they stay dark instead of being "corrected" to white.
fn paper_map(gray: &GrayImage) -> GrayImage {
    let (w, h) = gray.dimensions();
    // ~72 px across whatever the resolution, so the 5-px spread covers about 7% of the page width:
    // wide enough to see past tape marks and fold shadows, at preview size and at 600 dpi alike.
    let sw = 72.min(w / 2).max(8);
    let sh = (h as u64 * sw as u64 / w as u64).max(8) as u32;
    let small = imageops::resize(gray, sw, sh, imageops::FilterType::Triangle);
    let mut levels: Vec<u8> = small.pixels().map(|p| p[0]).collect();
    levels.sort_unstable();
    let floor = (levels[levels.len() * 9 / 10] as f32 * 0.6).max(40.0) as u8;
    let mut spread = small.clone();
    for (x, y, p) in spread.enumerate_pixels_mut() {
        let mut max = floor;
        for dy in -2i32..=2 {
            for dx in -2i32..=2 {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx >= 0 && ny >= 0 && (nx as u32) < sw && (ny as u32) < sh {
                    max = max.max(small.get_pixel(nx as u32, ny as u32)[0]);
                }
            }
        }
        p[0] = max;
    }
    imageops::resize(&imageops::blur(&spread, 1.5), w, h, imageops::FilterType::Triangle)
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
    fn rename_moves_the_folder_and_its_exports() {
        let root = std::env::temp_dir().join(format!("scan-rename-{}", Session::new_stem()));
        let mut s = Session::open(root.join("2026-11-11_1301")).unwrap();
        std::fs::write(s.dir.join("2026-11-11_1301_print.pdf"), b"x").unwrap();
        std::fs::create_dir_all(root.join("taken")).unwrap();
        assert!(s.rename("taken").is_err());
        assert!(s.rename("a/b").is_err());
        s.rename(" Deed copy ").unwrap();
        assert_eq!(s.name, "Deed copy");
        assert!(root.join("Deed copy/Deed copy_print.pdf").exists());
        assert!(!root.join("2026-11-11_1301").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn export_writes_selected_outputs_and_cleans_up() {
        let dir = std::env::temp_dir().join(format!("scan-test-{}", Session::new_stem()));
        let mut s = Session::open(dir.clone()).unwrap();
        let img = image::RgbImage::from_pixel(620, 877, image::Rgb([230, 220, 200]));
        std::fs::write(s.dir.join("pages/a.jpg"), encode_rgb(&img, 90).unwrap()).unwrap();
        s.pages.push(Page { file: "a.jpg".into(), kind: Kind::A4, dpi: 75, w: 620, h: 877, secs: 1.0 });
        s.save().unwrap();

        let opts = ExportOpts { color: false, share: true, print: true, images: true, cleanup: true, open_pdf: false, open_folder: false, max_mb: None, searchable: false };
        let files = s.export(opts).unwrap();

        let names: Vec<_> = files.iter().map(|f| f.file_name().unwrap().to_string_lossy().into_owned()).collect();
        let n = &s.name;
        assert_eq!(names, [format!("{n}_share.pdf"), format!("{n}_print.pdf"), format!("{n}_page01.jpg")]);
        assert!(files.iter().all(|f| f.exists()));
        assert!(!dir.join("pages").exists() && !dir.join("raw").exists() && !dir.join("session.json").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
