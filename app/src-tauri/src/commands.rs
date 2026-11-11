//! Commands the Svelte frontend invokes. Blocking work (network, image) runs off the main thread.
use crate::{AppState, Res, err};
use scan_core::adjust::{self, Edit};
use scan_core::capture::{self, Captured, Settings, TopHalf};
use scan_core::device::{self, Draft};
use scan_core::escl::{Caps, Scanner};
use scan_core::ipp::{Printer, Status};
use scan_core::presets::{self, Preset};
use scan_core::session::{ExportOpts, Page, Session};
use serde::Serialize;
use std::path::PathBuf;
use tauri::State;

#[derive(Serialize)]
pub struct Device {
    name: String,
    host: String,
}

#[derive(Serialize)]
pub struct DeviceStatus {
    scanner: String,
    printer: Option<Status>,
}

#[derive(Serialize)]
pub struct SessionView {
    name: String,
    dir: PathBuf,
    pages: Vec<Page>,
}

#[derive(Serialize)]
pub struct Exported {
    files: Vec<PathBuf>,
    closed: bool,
}

fn view(s: &Session) -> SessionView {
    SessionView { name: s.name.clone(), dir: s.dir.clone(), pages: s.pages.clone() }
}

pub(crate) async fn blocking<T: Send + 'static>(f: impl FnOnce() -> anyhow::Result<T> + Send + 'static) -> Res<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(err)?.map_err(err)
}

pub(crate) fn scanner(st: &AppState) -> Res<Scanner> {
    st.scanner.lock().unwrap().clone().ok_or_else(|| "Not connected to a scanner".into())
}

fn session_dir(st: &AppState) -> Res<PathBuf> {
    st.session.lock().unwrap().as_ref().map(|s| s.dir.clone()).ok_or_else(|| "No session open".into())
}

/// Lock the open session, apply `f`, persist, and return the new view.
fn edit(st: &AppState, f: impl FnOnce(&mut Vec<Page>) -> Res<()>) -> Res<SessionView> {
    let mut guard = st.session.lock().unwrap();
    let s = guard.as_mut().ok_or("No session open")?;
    f(&mut s.pages)?;
    s.save().map_err(err)?;
    Ok(view(s))
}

#[derive(Serialize)]
pub struct PageResult {
    session: SessionView,
    index: usize,
    note: String,
}

/// Put a captured page at `replace`, else insert at `at`, else append.
fn place(st: &AppState, c: Captured, replace: Option<usize>, at: Option<usize>) -> Res<PageResult> {
    let mut index = 0;
    let session = edit(st, |pages| {
        index = match replace {
            Some(i) if i < pages.len() => {
                pages[i] = c.page;
                i
            }
            _ => {
                let i = at.unwrap_or(pages.len()).min(pages.len());
                pages.insert(i, c.page);
                i
            }
        };
        Ok(())
    })?;
    Ok(PageResult { session, index, note: c.note })
}

#[tauri::command]
pub async fn connect(st: State<'_, AppState>, host: Option<String>) -> Res<Device> {
    let s = blocking(move || device::connect(host.as_deref())).await?;
    let d = Device { name: s.name.clone(), host: s.host().to_string() };
    *st.scanner.lock().unwrap() = Some(s);
    Ok(d)
}

#[tauri::command]
pub async fn status(st: State<'_, AppState>) -> Res<DeviceStatus> {
    let s = scanner(&st)?;
    blocking(move || {
        let scanner = s.state().unwrap_or_else(|_| "Offline".into());
        let printer = Printer::for_host(s.host()).status().ok();
        Ok(DeviceStatus { scanner, printer })
    })
    .await
}

#[tauri::command]
pub async fn capabilities(st: State<'_, AppState>) -> Res<Caps> {
    let s = scanner(&st)?;
    blocking(move || s.capabilities()).await
}

#[tauri::command]
pub fn drafts() -> Vec<Draft> {
    device::drafts(&device::default_root())
}

/// Open (or resume) a session by name; a fresh dated name when none is given.
#[tauri::command]
pub fn open_session(st: State<'_, AppState>, name: Option<String>) -> Res<SessionView> {
    let name = name.filter(|n| !n.trim().is_empty()).unwrap_or_else(device::default_name);
    if name.contains(['/', '\\']) || name.starts_with('.') {
        return Err("Session names can't contain slashes or start with a dot".into());
    }
    let s = Session::open(device::default_root().join(name.trim())).map_err(err)?;
    let v = view(&s);
    *st.session.lock().unwrap() = Some(s);
    Ok(v)
}

#[tauri::command]
pub fn close_session(st: State<'_, AppState>) {
    *st.session.lock().unwrap() = None;
}

#[tauri::command]
pub fn get_settings(st: State<'_, AppState>) -> Settings {
    *st.settings.lock().unwrap()
}

#[tauri::command]
pub fn set_settings(st: State<'_, AppState>, settings: Settings) {
    *st.settings.lock().unwrap() = settings;
    presets::remember(&settings);
}

#[tauri::command]
pub fn presets() -> Vec<Preset> {
    presets::all()
}

#[tauri::command]
pub fn save_preset(st: State<'_, AppState>, name: String) -> Res<Vec<Preset>> {
    let s = *st.settings.lock().unwrap();
    presets::save(&name, s).map_err(err)
}

#[tauri::command]
pub fn delete_preset(name: String) -> Vec<Preset> {
    presets::delete(&name)
}

#[tauri::command]
pub async fn scan_a4(st: State<'_, AppState>, replace: Option<usize>, at: Option<usize>) -> Res<PageResult> {
    let (sc, set, dir) = (scanner(&st)?, *st.settings.lock().unwrap(), session_dir(&st)?);
    let c = blocking(move || capture::scan_a4(&sc, set, &dir)).await?;
    place(&st, c, replace, at)
}

#[tauri::command]
pub async fn scan_top(st: State<'_, AppState>) -> Res<TopHalf> {
    let (sc, set, dir) = (scanner(&st)?, *st.settings.lock().unwrap(), session_dir(&st)?);
    blocking(move || capture::scan_top(&sc, set, &dir)).await
}

#[tauri::command]
pub async fn scan_bottom(st: State<'_, AppState>, top: TopHalf, replace: Option<usize>, at: Option<usize>) -> Res<PageResult> {
    let (sc, set, dir) = (scanner(&st)?, *st.settings.lock().unwrap(), session_dir(&st)?);
    let c = blocking(move || capture::scan_bottom(&sc, set, &dir, &top)).await?;
    place(&st, c, replace, at)
}

#[tauri::command]
pub async fn fill_page(st: State<'_, AppState>, index: usize) -> Res<PageResult> {
    let dir = session_dir(&st)?;
    let page = st.session.lock().unwrap().as_ref().and_then(|s| s.pages.get(index).cloned()).ok_or("No such page")?;
    let mode = st.settings.lock().unwrap().fill;
    let c = blocking(move || capture::fill_page(&dir, &page, mode)).await?;
    place(&st, c, Some(index), None)
}

#[tauri::command]
pub async fn edit_page(st: State<'_, AppState>, index: usize, edit: Edit) -> Res<PageResult> {
    let dir = session_dir(&st)?;
    let page = st.session.lock().unwrap().as_ref().and_then(|s| s.pages.get(index).cloned()).ok_or("No such page")?;
    let c = blocking(move || adjust::edit_page(&dir, &page, edit)).await?;
    place(&st, c, Some(index), None)
}

/// OCR languages available for searchable PDFs; empty when Tesseract isn't installed.
#[tauri::command]
pub async fn ocr_languages() -> Res<Vec<String>> {
    blocking(|| Ok(scan_core::ocr::languages())).await
}

#[tauri::command]
pub fn delete_page(st: State<'_, AppState>, index: usize) -> Res<SessionView> {
    edit(&st, |pages| {
        if index >= pages.len() {
            return Err("No such page".into());
        }
        pages.remove(index);
        Ok(())
    })
}

#[tauri::command]
pub fn move_page(st: State<'_, AppState>, from: usize, to: usize) -> Res<SessionView> {
    edit(&st, |pages| {
        if from >= pages.len() || to >= pages.len() {
            return Err("No such page".into());
        }
        let p = pages.remove(from);
        pages.insert(to, p);
        Ok(())
    })
}

#[tauri::command]
pub fn get_export_opts() -> ExportOpts {
    ExportOpts::load()
}

#[tauri::command]
pub async fn export(st: State<'_, AppState>, opts: ExportOpts) -> Res<Exported> {
    opts.save();
    let mut s = {
        let g = st.session.lock().unwrap();
        let s = g.as_ref().ok_or("No session open")?;
        if s.pages.is_empty() {
            return Err("Nothing to export yet".into());
        }
        Session { name: s.name.clone(), dir: s.dir.clone(), pages: s.pages.clone(), dirty: true }
    };
    let dir = s.dir.clone();
    let files = blocking(move || s.export(opts)).await?;
    if opts.open_pdf {
        if let Some(f) = best_pdf(&files) {
            let _ = tauri_plugin_opener::open_path(f, None::<&str>);
        }
    }
    if opts.open_folder {
        let _ = tauri_plugin_opener::open_path(&dir, None::<&str>);
    }
    if opts.cleanup {
        *st.session.lock().unwrap() = None;
    }
    Ok(Exported { files, closed: opts.cleanup })
}

/// Print PDF if exported, else share, else colour, else the first file.
fn best_pdf(files: &[PathBuf]) -> Option<&PathBuf> {
    ["_print.pdf", "_share.pdf", "_color.pdf"]
        .iter()
        .find_map(|s| files.iter().find(|f| f.to_string_lossy().ends_with(s)))
        .or(files.first())
}

#[tauri::command]
pub fn delete_draft(st: State<'_, AppState>, name: String) -> Res<Vec<Draft>> {
    if st.session.lock().unwrap().as_ref().is_some_and(|s| s.name == name) {
        return Err("Close this session before deleting it".into());
    }
    let root = device::default_root();
    device::delete_draft(&root, &name).map_err(err)?;
    Ok(device::drafts(&root))
}

#[tauri::command]
pub fn library() -> Vec<device::Saved> {
    device::library(&device::default_root())
}

/// Page count for previewing; images count as one page.
#[tauri::command]
pub async fn file_pages(st: State<'_, AppState>, path: PathBuf) -> Res<usize> {
    let path = crate::thumbs::allowed(&st, &path).ok_or("That file can't be opened here")?;
    if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
        return Ok(1);
    }
    blocking(move || scan_core::pdfedit::page_count(&std::fs::read(path)?)).await
}

#[derive(Serialize)]
pub struct ShrinkResult {
    out: PathBuf,
    before: u64,
    after: u64,
}

/// Write `<name>_under<N>MB.pdf` next to `path`. `path` may be any PDF the user picked.
#[tauri::command]
pub async fn shrink_pdf(st: State<'_, AppState>, path: PathBuf, max_mb: f32) -> Res<ShrinkResult> {
    if !(0.05..=1000.0).contains(&max_mb) {
        return Err("Pick a size between 0.05 and 1000 MB".into());
    }
    st.picked.lock().unwrap().insert(path.clone());
    let out = blocking(move || {
        let input = std::fs::read(&path)?;
        let r = scan_core::pdfedit::shrink(&input, (max_mb * 1_000_000.0) as u64)?;
        if !r.fits {
            anyhow::bail!("Can't get this PDF under {max_mb} MB — the smallest possible is {:.2} MB", r.data.len() as f32 / 1e6);
        }
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let out = path.with_file_name(format!("{stem}_under{max_mb}MB.pdf"));
        std::fs::write(&out, &r.data)?;
        Ok(ShrinkResult { out, before: input.len() as u64, after: r.data.len() as u64 })
    })
    .await?;
    st.picked.lock().unwrap().insert(out.out.clone());
    Ok(out)
}
