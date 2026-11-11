//! Tauri shell over scan-core: shared state, commands, and the `scanimg://` thumbnail protocol.
mod button;
mod commands;
mod printing;
mod thumbs;

use scan_core::capture::Settings;
use scan_core::escl::Scanner;
use scan_core::session::Session;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct AppState {
    scanner: Mutex<Option<Scanner>>,
    session: Mutex<Option<Session>>,
    settings: Mutex<Settings>,
    /// Files outside the scans folder the user picked (e.g. a PDF to shrink) and may preview.
    picked: Mutex<HashSet<PathBuf>>,
    /// Scratch folder for the copy in progress.
    copy_dir: Mutex<Option<PathBuf>>,
    /// Set by Stop; copy scans and print jobs check it.
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Stop flag of the Scan-button listener, while one runs.
    button: Mutex<Option<std::sync::Arc<std::sync::atomic::AtomicBool>>>,
}

type Res<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    format!("{e:#}")
}

pub fn run() {
    let state = AppState {
        scanner: Mutex::new(None),
        session: Mutex::new(None),
        settings: Mutex::new(scan_core::presets::last()),
        picked: Mutex::new(HashSet::new()),
        copy_dir: Mutex::new(None),
        stop: Default::default(),
        button: Mutex::new(None),
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .register_asynchronous_uri_scheme_protocol("scanimg", thumbs::serve)
        .register_asynchronous_uri_scheme_protocol("scanfile", thumbs::serve_file)
        .invoke_handler(tauri::generate_handler![
            commands::connect,
            commands::status,
            commands::capabilities,
            commands::drafts,
            commands::open_session,
            commands::close_session,
            commands::rename_session,
            commands::get_settings,
            commands::set_settings,
            commands::presets,
            commands::save_preset,
            commands::delete_preset,
            commands::scan_a4,
            commands::scan_top,
            commands::scan_bottom,
            commands::fill_page,
            commands::edit_page,
            commands::ocr_languages,
            commands::delete_page,
            commands::move_page,
            commands::get_export_opts,
            commands::export,
            commands::library,
            commands::delete_draft,
            commands::file_pages,
            commands::shrink_pdf,
            printing::stage_file,
            printing::print_caps,
            printing::print_plan,
            printing::print_preview,
            printing::print,
            printing::copy_reset,
            printing::copy_scan,
            printing::copy_long_top,
            printing::copy_long_bottom,
            printing::copy_id_card,
            printing::copy_tone,
            printing::stop_job,
            commands::device_details,
            commands::enable_snmp_write,
            button::button_status,
            button::button_enable,
            button::button_disable,
            button::scan_button,
            commands::page_text,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the app");
}
