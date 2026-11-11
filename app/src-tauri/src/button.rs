//! The device's Scan button: turn it on (register, point the device at this PC), keep a listener
//! thread running while connected, and scan over the native protocol when a press comes in.
use crate::commands::{blocking, place, scanner, session_dir, web_host, with_password, PageResult};
use crate::{AppState, Res};
use scan_core::button::{self, Saved};
use scan_core::capture;
use scan_core::pushscan::{self, SnmpMode};
use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, State};

#[derive(Serialize)]
pub struct ButtonStatus {
    pub snmp: SnmpMode,
    /// PC the device's Scan button sends to; `None` for USB.
    pub destination: Option<String>,
    /// Name this PC registers under.
    pub name: String,
    pub on: bool,
}

/// Start listening for presses if the button was turned on for this device.
pub fn resume(app: &AppHandle, st: &AppState, host: &str) {
    match button::saved() {
        Some(s) if s.host == host => start(app, st, s),
        _ => stop(st),
    }
}

fn start(app: &AppHandle, st: &AppState, s: Saved) {
    stop(st);
    let flag = Arc::new(AtomicBool::new(false));
    *st.button.lock().unwrap() = Some(flag.clone());
    let app = app.clone();
    std::thread::spawn(move || {
        let name = button::pc_name();
        let res = button::listen(&s.host, &s.community, &name, flag, |p| {
            let _ = app.emit("button-press", p);
        });
        if let Err(e) = res {
            let _ = app.emit("button-error", format!("{e:#}"));
        }
    });
}

fn stop(st: &AppState) {
    if let Some(flag) = st.button.lock().unwrap().take() {
        flag.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
pub async fn button_status(st: State<'_, AppState>, password: Option<String>) -> Res<ButtonStatus> {
    let host = web_host(&st)?;
    let on = st.button.lock().unwrap().is_some();
    blocking(move || {
        with_password(&host, password, |pw| {
            let snmp = pushscan::snmp_access(&host, pw)?.mode;
            let destination = pushscan::destination(&host, pw)?;
            Ok(ButtonStatus { snmp, destination, name: button::pc_name(), on })
        })
    })
    .await
}

/// Needs SNMP write access (errors `needs-snmp`): register, point the device here, start listening.
#[tauri::command]
pub async fn button_enable(app: AppHandle, st: State<'_, AppState>, password: Option<String>) -> Res<()> {
    let host = web_host(&st)?;
    let saved = blocking(move || {
        with_password(&host.clone(), password, |pw| {
            let access = pushscan::snmp_access(&host, pw)?;
            if access.mode != SnmpMode::ReadWrite {
                anyhow::bail!("needs-snmp");
            }
            let name = button::pc_name();
            button::register(&host, &access.set_community, &name)?;
            pushscan::set_destination(&host, pw, &name)?;
            let saved = Saved { host: host.clone(), community: access.set_community };
            button::save(&saved)?;
            Ok(saved)
        })
    })
    .await?;
    start(&app, &st, saved);
    Ok(())
}

#[tauri::command]
pub fn button_disable(st: State<'_, AppState>) {
    stop(&st);
    button::forget();
}

/// The scan a button press asked for, into the open session.
#[tauri::command]
pub async fn scan_button(st: State<'_, AppState>, at: Option<usize>) -> Res<PageResult> {
    let (sc, set, dir) = (scanner(&st)?, *st.settings.lock().unwrap(), session_dir(&st)?);
    let c = blocking(move || capture::scan_button(&sc, set, &dir)).await?;
    place(&st, c, None, at)
}
