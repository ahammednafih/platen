//! Searchable PDFs: Tesseract writes an invisible text-only page per scan, which is
//! laid over the matching image page.
use anyhow::{Context, Result, bail};
use image::imageops;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Languages used when installed, in Tesseract's naming.
const WANTED: [&str; 2] = ["eng", "mal"];
/// Tesseract is most accurate around 300 dpi; more only costs time.
const OCR_DPI: u32 = 300;

pub struct Ocr {
    exe: PathBuf,
    langs: Vec<String>,
}

impl Ocr {
    pub fn find() -> Result<Ocr> {
        let exe = executable().context("Searchable PDF needs Tesseract — install it with `brew install tesseract`")?;
        let langs = installed(&exe);
        if langs.is_empty() {
            bail!("Tesseract has no English or Malayalam data installed");
        }
        Ok(Ocr { exe, langs })
    }

    pub fn langs(&self) -> &[String] {
        &self.langs
    }

    /// One text-only PDF per page (JPEG bytes, dpi), OCR'd in parallel inside `tmp`.
    pub fn layers(&self, pages: &[(Vec<u8>, u32)], tmp: &Path) -> Result<Vec<Vec<u8>>> {
        std::fs::create_dir_all(tmp)?;
        let out = pages.par_iter().enumerate().map(|(i, (d, dpi))| self.layer(d, *dpi, &tmp.join(format!("p{i}")))).collect();
        let _ = std::fs::remove_dir_all(tmp);
        out
    }

    fn layer(&self, jpeg: &[u8], dpi: u32, stem: &Path) -> Result<Vec<u8>> {
        let img = image::load_from_memory(jpeg)?.to_luma8();
        let dpi_out = dpi.min(OCR_DPI);
        let img = if dpi > dpi_out {
            let (w, h) = (img.width() * dpi_out / dpi, img.height() * dpi_out / dpi);
            imageops::resize(&img, w, h, imageops::FilterType::Triangle)
        } else {
            img
        };
        let png = stem.with_extension("png");
        img.save(&png)?;
        let status = Command::new(&self.exe)
            .arg(&png)
            .arg(stem)
            .args(["-l", &self.langs.join("+"), "--dpi", &dpi_out.to_string(), "-c", "textonly_pdf=1", "pdf"])
            .env("OMP_THREAD_LIMIT", "1") // pages already run in parallel
            .output()?;
        if !status.status.success() {
            bail!("Tesseract failed: {}", String::from_utf8_lossy(&status.stderr).trim());
        }
        Ok(std::fs::read(stem.with_extension("pdf"))?)
    }
}

/// Languages from `WANTED` that this machine's Tesseract has; empty when it isn't installed.
pub fn languages() -> Vec<String> {
    executable().map(|e| installed(&e)).unwrap_or_default()
}

/// Apps launched from Finder don't get the shell PATH, so also look in Homebrew's prefixes.
fn executable() -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .chain(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from))
        .map(|d| d.join("tesseract"))
        .find(|p| p.is_file())
}

fn installed(exe: &Path) -> Vec<String> {
    let out = Command::new(exe).arg("--list-langs").output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let have: Vec<&str> = out.lines().map(str::trim).collect();
    WANTED.iter().filter(|l| have.contains(l)).map(|l| l.to_string()).collect()
}

/// Lay each text-only PDF over the matching page of `pdf`, scaled to its MediaBox.
pub fn overlay(pdf: &[u8], layers: &[Vec<u8>]) -> Result<Vec<u8>> {
    let mut doc = Document::load_mem(pdf)?;
    let pages: Vec<ObjectId> = doc.get_pages().into_values().collect();
    if pages.len() != layers.len() {
        bail!("{} pages but {} text layers", pages.len(), layers.len());
    }
    for (pid, layer) in pages.into_iter().zip(layers) {
        add_layer(&mut doc, pid, layer)?;
    }
    doc.prune_objects();
    let mut out = Vec::new();
    doc.save_to(&mut out)?;
    Ok(out)
}

fn add_layer(doc: &mut Document, pid: ObjectId, layer: &[u8]) -> Result<()> {
    let mut src = Document::load_mem(layer)?;
    src.renumber_objects_with(doc.max_id + 1);
    let sid = *src.get_pages().values().next().context("empty text layer")?;
    let sp = src.get_dictionary(sid)?;
    let fonts = deref(&src, deref(&src, sp.get(b"Resources")?)?.as_dict()?.get(b"Font")?)?.clone();
    let text = sp.get(b"Contents")?.clone();
    let ((sw, sh), (dw, dh)) = (media(sp)?, media(doc.get_dictionary(pid)?)?);
    doc.max_id = doc.max_id.max(src.max_id);
    doc.objects.extend(src.objects); // the layer's own catalog and page get pruned later

    let pre = doc.add_object(Stream::new(Dictionary::new(), format!("q {:.5} 0 0 {:.5} 0 0 cm\n", dw / sw, dh / sh).into_bytes()));
    let post = doc.add_object(Stream::new(Dictionary::new(), b"\nQ".to_vec()));
    let page = doc.get_dictionary_mut(pid)?;
    let mut contents = list(page.get(b"Contents")?);
    contents.push(pre.into());
    contents.extend(list(&text));
    contents.push(post.into());
    page.set("Contents", contents);
    page.get_mut(b"Resources")?.as_dict_mut()?.set("Font", fonts);
    Ok(())
}

fn deref<'a>(doc: &'a Document, o: &'a Object) -> Result<&'a Object> {
    Ok(match o {
        Object::Reference(id) => doc.get_object(*id)?,
        o => o,
    })
}

fn list(o: &Object) -> Vec<Object> {
    match o {
        Object::Array(a) => a.clone(),
        o => vec![o.clone()],
    }
}

fn media(page: &Dictionary) -> Result<(f32, f32)> {
    let b = page.get(b"MediaBox")?.as_array()?;
    let v: Vec<f32> = b.iter().map(|o| o.as_float()).collect::<Result<_, _>>()?;
    Ok((v[2] - v[0], v[3] - v[1]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::{self, JpegPage};
    use image::{Rgb, RgbImage};

    /// Big block letters "SCAN" drawn from rectangles, so the test needs no fonts.
    fn word() -> RgbImage {
        let mut img = RgbImage::from_pixel(1240, 600, Rgb([255, 255, 255]));
        let mut rect = |x: u32, y: u32, w: u32, h: u32| {
            for yy in y..y + h {
                for xx in x..x + w {
                    img.put_pixel(xx, yy, Rgb([0, 0, 0]));
                }
            }
        };
        let (y, s) = (200, 24); // stroke
        // S
        rect(100, y, 120, s); rect(100, y, s, 100); rect(100, y + 88, 120, s); rect(196, y + 88, s, 112); rect(100, y + 176, 120, s);
        // C
        rect(280, y, 120, s); rect(280, y, s, 200); rect(280, y + 176, 120, s);
        // A
        rect(460, y, s, 200); rect(556, y, s, 200); rect(460, y, 120, s); rect(460, y + 88, 120, s);
        // N
        rect(640, y, s, 200); rect(736, y, s, 200);
        for i in 0..200 { rect(640 + i * 96 / 200, y + i, s, 1); }
        img
    }

    #[test]
    fn overlays_a_text_layer_that_keeps_the_page() {
        let Ok(ocr) = Ocr::find() else { return eprintln!("tesseract not installed; skipping") };
        let dir = std::env::temp_dir().join(format!("scan-ocr-{}", crate::session::Session::new_stem()));
        let jpeg = crate::session::encode_rgb(&word(), 90).unwrap();
        let layers = ocr.layers(&[(jpeg.clone(), 150)], &dir.join("tmp")).unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        pdf::write(&dir.join("a.pdf"), &[JpegPage { data: jpeg, dpi: 150.0 }]).unwrap();
        let out = overlay(&std::fs::read(dir.join("a.pdf")).unwrap(), &layers).unwrap();

        let doc = Document::load_mem(&out).unwrap();
        let pid = *doc.get_pages().values().next().unwrap();
        let page = doc.get_dictionary(pid).unwrap();
        let res = page.get(b"Resources").unwrap().as_dict().unwrap();
        assert!(res.get(b"XObject").is_ok() && res.get(b"Font").is_ok());
        // Accuracy is Tesseract's business; this checks the layer survived the merge.
        assert!(doc.extract_text(&[1]).unwrap().trim().len() >= 3);
        // Shrinking to a size limit re-encodes images but must keep the text.
        let small = crate::pdfedit::shrink(&out, out.len() as u64 / 2).unwrap();
        assert!(Document::load_mem(&small.data).unwrap().extract_text(&[1]).unwrap().trim().len() >= 3);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
