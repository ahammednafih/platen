//! Tauri shell over scan-core: shared state, commands, and the `scanimg://` thumbnail protocol.
mod commands;
mod thumbs;

use scan_core::capture::Settings;
use scan_core::escl::{ColorMode, Scanner};
use scan_core::fill::Fill;
use scan_core::session::Session;
use std::sync::Mutex;

pub struct AppState {
    scanner: Mutex<Option<Scanner>>,
    session: Mutex<Option<Session>>,
    settings: Mutex<Settings>,
}

type Res<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    format!("{e:#}")
}

pub fn run() {
    let state = AppState {
        scanner: Mutex::new(None),
        session: Mutex::new(None),
        settings: Mutex::new(Settings { dpi: 300, mode: ColorMode::Color, fill: Fill::White }),
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .register_asynchronous_uri_scheme_protocol("scanimg", thumbs::serve)
        .invoke_handler(tauri::generate_handler![
            commands::connect,
            commands::status,
            commands::capabilities,
            commands::drafts,
            commands::open_session,
            commands::close_session,
            commands::get_settings,
            commands::set_settings,
            commands::scan_a4,
            commands::scan_top,
            commands::scan_bottom,
            commands::fill_page,
            commands::delete_page,
            commands::move_page,
            commands::get_export_opts,
            commands::export,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the app");
}
