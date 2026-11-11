//! `scanimg://localhost/<pages|raw>/<file>?w=<px>` — downscaled JPEGs of the open session's
//! images, cached under `<session>/thumbs/`, so the grid never ships full-size scans to the webview.
use crate::AppState;
use platen_core::session;
use std::path::{Path, PathBuf};
use tauri::http::{Request, Response, StatusCode};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};

pub fn serve<R: Runtime>(ctx: UriSchemeContext<'_, R>, req: Request<Vec<u8>>, responder: UriSchemeResponder) {
    let dir = ctx.app_handle().state::<AppState>().session.lock().unwrap().as_ref().map(|s| s.dir.clone());
    let uri = req.uri().clone();
    std::thread::spawn(move || {
        let res = dir.and_then(|d| resolve(&d, uri.path())).and_then(|src| {
            let w = width(uri.query()).unwrap_or(360);
            thumbnail(&src, w).ok()
        });
        responder.respond(match res {
            Some(bytes) => Response::builder().header("Content-Type", "image/jpeg").body(bytes).unwrap(),
            None => Response::builder().status(StatusCode::NOT_FOUND).body(Vec::new()).unwrap(),
        });
    });
}

/// `scanfile://localhost/?path=<abs>&page=<n>&w=<px>` — a preview JPEG of one page of an exported
/// PDF or image. Only files under the scans folder, or ones the user picked, are served. Rendering a
/// big PDF's page takes seconds, so results are cached by file, modification time, page and width.
pub fn serve_file<R: Runtime>(ctx: UriSchemeContext<'_, R>, req: Request<Vec<u8>>, responder: UriSchemeResponder) {
    let q = query(req.uri().query().unwrap_or(""));
    let path = q.iter().find(|(k, _)| k == "path").map(|(_, v)| PathBuf::from(v));
    let path = path.and_then(|p| allowed(&ctx.app_handle().state::<AppState>(), &p));
    let page: usize = q.iter().find(|(k, _)| k == "page").and_then(|(_, v)| v.parse().ok()).unwrap_or(0);
    let w = width(req.uri().query()).unwrap_or(1600);
    std::thread::spawn(move || {
        let res = path.and_then(|p| {
            let cache = cache_path(&p, page, w);
            if let Some(hit) = cache.as_ref().and_then(|c| std::fs::read(c).ok()) {
                return Some(hit);
            }
            let is_pdf = p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
            let bytes = if is_pdf { platen_core::pdfedit::page_preview(&std::fs::read(&p).ok()?, page, w).ok() } else { resized(&p, w).ok() }?;
            if let Some(c) = cache {
                let _ = std::fs::create_dir_all(c.parent()?).and_then(|_| std::fs::write(&c, &bytes));
            }
            Some(bytes)
        });
        responder.respond(match res {
            Some(bytes) => Response::builder().header("Content-Type", "image/jpeg").body(bytes).unwrap(),
            None => Response::builder().status(StatusCode::NOT_FOUND).body(Vec::new()).unwrap(),
        });
    });
}

/// `<cache>/scan/previews/<hash>.jpg`; a changed file gets a new name, so stale entries are never served.
fn cache_path(path: &Path, page: usize, w: u32) -> Option<PathBuf> {
    use std::hash::{Hash, Hasher};
    let meta = std::fs::metadata(path).ok()?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (path, meta.len(), meta.modified().ok(), page, w).hash(&mut h);
    Some(platen_core::device::cache_dir().join("previews").join(format!("{:016x}.jpg", h.finish())))
}

/// The canonical path if it's inside the scans folder or was picked by the user.
pub fn allowed(st: &AppState, path: &Path) -> Option<PathBuf> {
    let real = path.canonicalize().ok()?;
    let root = platen_core::device::default_root().canonicalize().ok();
    let inside = root.is_some_and(|r| real.starts_with(r));
    let picked = st.picked.lock().unwrap().iter().any(|p| p.canonicalize().ok().as_ref() == Some(&real));
    (inside || picked).then_some(real)
}

fn query(q: &str) -> Vec<(String, String)> {
    q.split('&')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.to_string(), decode(v)))
        .collect()
}

/// Percent-decoding for `encodeURIComponent` output.
fn decode(v: &str) -> String {
    let b = v.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = (b[i] == b'%' && i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit())
            .then(|| u8::from_str_radix(&v[i + 1..i + 3], 16).ok())
            .flatten();
        match hex {
            Some(c) => {
                out.push(c);
                i += 3;
            }
            None => {
                out.push(if b[i] == b'+' { b' ' } else { b[i] });
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn resized(src: &Path, w: u32) -> anyhow::Result<Vec<u8>> {
    let img = session::load_rgb(src)?;
    if img.width() <= w {
        return session::encode_rgb(&img, 85);
    }
    let h = (img.height() as u64 * w as u64 / img.width() as u64) as u32;
    session::encode_rgb(&image::imageops::thumbnail(&img, w, h.max(1)), 85)
}

/// Only plain file names inside `pages/` or `raw/`; nothing else in the session folder.
fn resolve(dir: &Path, path: &str) -> Option<PathBuf> {
    let (sub, file) = path.trim_start_matches('/').split_once('/')?;
    let ok_name = !file.is_empty() && !file.contains(['/', '\\']) && !file.starts_with('.');
    (matches!(sub, "pages" | "raw") && ok_name).then(|| dir.join(sub).join(file))
}

fn width(query: Option<&str>) -> Option<u32> {
    let w: u32 = query?.split('&').find_map(|kv| kv.strip_prefix("w="))?.parse().ok()?;
    Some(w.clamp(64, 2400))
}

fn thumbnail(src: &Path, w: u32) -> anyhow::Result<Vec<u8>> {
    let cache_dir = src.parent().and_then(Path::parent).unwrap_or(src).join("thumbs");
    let cached = cache_dir.join(format!("{w}_{}", src.file_name().unwrap_or_default().to_string_lossy()));
    if let Ok(b) = std::fs::read(&cached) {
        return Ok(b);
    }
    let img = session::load_rgb(src)?;
    let h = (img.height() as u64 * w as u64 / img.width().max(1) as u64) as u32;
    let small = image::imageops::thumbnail(&img, w.min(img.width()), h.min(img.height()).max(1));
    let bytes = session::encode_rgb(&small, 80)?;
    std::fs::create_dir_all(&cache_dir)?;
    std::fs::write(&cached, &bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_only_allows_page_and_raw_files() {
        let d = Path::new("/s");
        assert_eq!(resolve(d, "/pages/1.jpg"), Some(PathBuf::from("/s/pages/1.jpg")));
        assert_eq!(resolve(d, "/raw/1_top.jpg"), Some(PathBuf::from("/s/raw/1_top.jpg")));
        assert_eq!(resolve(d, "/pages/../session.json"), None);
        assert_eq!(resolve(d, "/thumbs/x.jpg"), None);
        assert_eq!(resolve(d, "/pages/.hidden"), None);
    }

    #[test]
    fn decodes_uri_component_paths() {
        assert_eq!(decode("%2FUsers%2Fme%2Fmy%20scan%E2%80%94a.pdf"), "/Users/me/my scan—a.pdf");
        assert_eq!(decode("a%2"), "a%2");
        assert_eq!(decode("%é1"), "%é1");
    }
}
