//! Print and copy commands. Copies scan into a scratch folder, then print like any file.
use crate::commands::{blocking, scanner};
use crate::{AppState, Res, err};
use scan_core::capture::TopHalf;
use scan_core::copy::{self, Area};
use scan_core::ipp::{PrintCaps, Printer};
use scan_core::printjob::{Plan, PrintJob};
use serde::Serialize;
use std::path::PathBuf;
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, State};

/// Sheet previews render at this resolution.
const PREVIEW_DPI: u32 = 40;

#[derive(Serialize, Clone)]
struct Progress {
    done: usize,
    total: usize,
}

fn printer(st: &AppState) -> Res<Printer> {
    Ok(Printer::for_host(scanner(st)?.host()))
}

/// Remember files the user picked so previews may read them.
fn allow(st: &AppState, files: &[PathBuf]) {
    st.picked.lock().unwrap().extend(files.iter().cloned());
}

#[tauri::command]
pub async fn print_caps(st: State<'_, AppState>) -> Res<PrintCaps> {
    let p = printer(&st)?;
    blocking(move || p.caps()).await
}

#[tauri::command]
pub async fn print_plan(st: State<'_, AppState>, job: PrintJob) -> Res<Plan> {
    allow(&st, &job.files);
    blocking(move || job.plan()).await
}

/// JPEG of the `slot`-th sheet the job would send.
#[tauri::command]
pub async fn print_preview(job: PrintJob, slot: usize) -> Res<Response> {
    blocking(move || {
        let plan = job.plan()?;
        let sheet = *plan.order.get(slot).ok_or_else(|| anyhow::anyhow!("no such sheet"))?;
        Ok(Response::new(job.sheet(&plan, sheet, PREVIEW_DPI)?))
    })
    .await
}

/// Send the job, emitting `print-progress` after each sheet. Returns the number of jobs sent.
#[tauri::command]
pub async fn print(app: AppHandle, st: State<'_, AppState>, job: PrintJob) -> Res<usize> {
    let p = printer(&st)?;
    blocking(move || {
        let jobs = job.run(&p, "scan", |done, total| {
            let _ = app.emit("print-progress", Progress { done, total });
        })?;
        Ok(jobs.len())
    })
    .await
}

fn copy_dir(st: &AppState) -> Res<PathBuf> {
    let mut g = st.copy_dir.lock().unwrap();
    if g.is_none() {
        *g = Some(copy::scratch().map_err(err)?);
    }
    Ok(g.clone().unwrap())
}

/// Throw away scans from the last copy.
#[tauri::command]
pub fn copy_reset(st: State<'_, AppState>) {
    if let Some(d) = st.copy_dir.lock().unwrap().take() {
        let _ = std::fs::remove_dir_all(d);
    }
}

#[tauri::command]
pub async fn copy_scan(st: State<'_, AppState>, area: Area, color: bool) -> Res<PathBuf> {
    let (sc, dir) = (scanner(&st)?, copy_dir(&st)?);
    let path = blocking(move || copy::scan(&sc, area, color, &dir)).await?;
    allow(&st, std::slice::from_ref(&path));
    Ok(path)
}

#[tauri::command]
pub async fn copy_long_top(st: State<'_, AppState>, color: bool) -> Res<TopHalf> {
    let (sc, dir) = (scanner(&st)?, copy_dir(&st)?);
    let top = blocking(move || copy::long_top(&sc, color, &dir)).await?;
    allow(&st, std::slice::from_ref(&top.path));
    Ok(top)
}

#[tauri::command]
pub async fn copy_long_bottom(st: State<'_, AppState>, top: TopHalf, color: bool) -> Res<PathBuf> {
    let (sc, dir) = (scanner(&st)?, copy_dir(&st)?);
    let path = blocking(move || copy::long_bottom(&sc, color, &dir, &top)).await?;
    allow(&st, std::slice::from_ref(&path));
    Ok(path)
}

#[tauri::command]
pub async fn copy_id_card(st: State<'_, AppState>, front: PathBuf, back: PathBuf) -> Res<PathBuf> {
    let dir = copy_dir(&st)?;
    let path = blocking(move || copy::id_card(&front, &back, &dir)).await?;
    allow(&st, std::slice::from_ref(&path));
    Ok(path)
}
