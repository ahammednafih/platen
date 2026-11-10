//! Tiny PDF writer: one JPEG per page, embedded as-is (DCTDecode).
use anyhow::{Context, Result};
use std::io::Write;

pub struct JpegPage {
    pub data: Vec<u8>,
    pub dpi: f32,
}

pub fn write(path: &std::path::Path, pages: &[JpegPage]) -> Result<()> {
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = vec![0usize; 3 + pages.len() * 3];
    let n = pages.len();
    // Object ids: 1 catalog, 2 pages, then per page: page, image, content.
    let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 3 + i * 3)).collect();

    let mut obj = |out: &mut Vec<u8>, id: usize, body: &[u8]| {
        offsets[id] = out.len();
        write!(out, "{id} 0 obj\n").unwrap();
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };
    obj(&mut out, 1, b"<< /Type /Catalog /Pages 2 0 R >>");
    obj(&mut out, 2, format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")).as_bytes());

    for (i, p) in pages.iter().enumerate() {
        let (w, h, comps) = jpeg_info(&p.data).context("bad JPEG")?;
        let (pw, ph) = (w as f32 * 72.0 / p.dpi, h as f32 * 72.0 / p.dpi);
        let (pid, iid, cid) = (3 + i * 3, 4 + i * 3, 5 + i * 3);
        obj(&mut out, pid, format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {pw:.2} {ph:.2}] /Resources << /XObject << /Im0 {iid} 0 R >> >> /Contents {cid} 0 R >>"
        ).as_bytes());
        let cs = if comps == 1 { "/DeviceGray" } else { "/DeviceRGB" };
        let mut img = format!(
            "<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace {cs} /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
            p.data.len()
        ).into_bytes();
        img.extend_from_slice(&p.data);
        img.extend_from_slice(b"\nendstream");
        obj(&mut out, iid, &img);
        let content = format!("q {pw:.2} 0 0 {ph:.2} 0 0 cm /Im0 Do Q");
        obj(&mut out, cid, format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()).as_bytes());
    }

    let xref = out.len();
    write!(out, "xref\n0 {}\n0000000000 65535 f \n", offsets.len())?;
    for off in &offsets[1..] {
        write!(out, "{off:010} 00000 n \n")?;
    }
    write!(out, "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", offsets.len())?;
    std::fs::write(path, out)?;
    Ok(())
}

/// (width, height, components) from the JPEG SOF marker.
pub fn jpeg_info(d: &[u8]) -> Option<(u32, u32, u8)> {
    let mut i = 2;
    while i + 9 < d.len() {
        if d[i] != 0xFF {
            i += 1;
            continue;
        }
        let m = d[i + 1];
        let len = u16::from_be_bytes([d[i + 2], d[i + 3]]) as usize;
        if matches!(m, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF) {
            let h = u16::from_be_bytes([d[i + 5], d[i + 6]]) as u32;
            let w = u16::from_be_bytes([d[i + 7], d[i + 8]]) as u32;
            return Some((w, h, d[i + 9]));
        }
        i += 2 + len;
    }
    None
}
