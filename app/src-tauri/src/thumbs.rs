//! `scanimg://localhost/<pages|raw>/<file>?w=<px>` — downscaled JPEGs of the open session's
//! images, cached under `<session>/thumbs/`, so the grid never ships full-size scans to the webview.
use crate::AppState;
use scan_core::session;
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
}
