//! The Dock icon follows the accent colour while the app runs (the bundle keeps the ink icon).
use crate::{Res, err};
use tauri::AppHandle;

fn png(accent: &str) -> &'static [u8] {
    match accent {
        "magenta" => include_bytes!("../icons/accents/magenta.png"),
        "orange" => include_bytes!("../icons/accents/orange.png"),
        "teal" => include_bytes!("../icons/accents/teal.png"),
        "coffee" => include_bytes!("../icons/accents/coffee.png"),
        _ => include_bytes!("../icons/accents/ink.png"),
    }
}

#[tauri::command]
pub fn set_dock_icon(app: AppHandle, accent: String) -> Res<()> {
    let bytes = png(&accent);
    app.run_on_main_thread(move || mac::set(bytes)).map_err(err)
}

#[cfg(target_os = "macos")]
mod mac {
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    pub fn set(bytes: &'static [u8]) {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let data = NSData::with_bytes(bytes);
        if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
            // SAFETY: called on the main thread with a valid image.
            unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image)) };
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod mac {
    pub fn set(_: &'static [u8]) {}
}
