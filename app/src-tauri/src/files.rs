//! Saved-file actions from the library: share (macOS share sheet), move to Trash, rename.
use crate::thumbs::allowed;
use crate::{AppState, Res, err};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, State, WebviewWindow};

/// Where the clicked button is, in page coordinates, so the share sheet opens next to it.
#[derive(serde::Deserialize)]
pub struct Anchor {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

fn checked(st: &AppState, path: &Path) -> Res<PathBuf> {
    allowed(st, path).ok_or_else(|| "That file can't be changed here".into())
}

#[tauri::command]
pub fn share_file(app: AppHandle, window: WebviewWindow, st: State<'_, AppState>, path: PathBuf, anchor: Anchor) -> Res<()> {
    let path = checked(&st, &path)?;
    let view = window.ns_view().map_err(err)? as usize;
    app.run_on_main_thread(move || mac::share(view, &path, anchor)).map_err(err)
}

/// Into the Trash, so it can be put back from Finder.
#[tauri::command]
pub fn trash_file(st: State<'_, AppState>, path: PathBuf) -> Res<()> {
    mac::trash(&checked(&st, &path)?)
}

/// Rename in place, keeping the extension. Returns the new path.
#[tauri::command]
pub fn rename_file(st: State<'_, AppState>, path: PathBuf, name: String) -> Res<PathBuf> {
    let path = checked(&st, &path)?;
    let stem = name.trim().trim_end_matches(".pdf").trim_end_matches(".PDF");
    if stem.is_empty() || stem.contains(['/', '\\', ':']) || stem.starts_with('.') {
        return Err("Names can't be empty, contain / or :, or start with a dot".into());
    }
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let to = path.with_file_name(format!("{stem}{ext}"));
    if to == path {
        return Ok(path);
    }
    if to.exists() {
        return Err(format!("There's already a file called “{stem}{ext}”"));
    }
    std::fs::rename(&path, &to).map_err(err)?;
    Ok(to)
}

#[cfg(target_os = "macos")]
mod mac {
    use super::Anchor;
    use crate::Res;
    use objc2::AnyThread;
    use objc2::rc::Retained;
    use objc2_app_kit::{NSSharingServicePicker, NSView};
    use objc2_foundation::{NSArray, NSFileManager, NSPoint, NSRect, NSRectEdge, NSSize, NSString, NSURL};
    use std::path::Path;

    fn url(path: &Path) -> Retained<NSURL> {
        NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()))
    }

    pub fn share(view: usize, path: &Path, a: Anchor) {
        // SAFETY: `view` is the window's content view pointer from Tauri, used on the main thread.
        let view: &NSView = unsafe { &*(view as *const NSView) };
        let items = NSArray::from_retained_slice(&[Retained::into_super(Retained::into_super(url(path)))]);
        let picker = unsafe { NSSharingServicePicker::initWithItems(NSSharingServicePicker::alloc(), &items) };
        let flipped = view.isFlipped();
        let y = if flipped { a.y } else { view.frame().size.height - a.y - a.h };
        let rect = NSRect::new(NSPoint::new(a.x, y), NSSize::new(a.w, a.h));
        picker.showRelativeToRect_ofView_preferredEdge(rect, view, if flipped { NSRectEdge::MaxY } else { NSRectEdge::MinY });
        // The picker must outlive its popover; one small object per share.
        std::mem::forget(picker);
    }

    pub fn trash(path: &Path) -> Res<()> {
        NSFileManager::defaultManager().trashItemAtURL_resultingItemURL_error(&url(path), None).map_err(|e| e.localizedDescription().to_string())
    }
}

#[cfg(not(target_os = "macos"))]
mod mac {
    use super::Anchor;
    use crate::Res;
    use std::path::Path;

    pub fn share(_: usize, _: &Path, _: Anchor) {}

    pub fn trash(path: &Path) -> Res<()> {
        std::fs::remove_file(path).map_err(crate::err)
    }
}
