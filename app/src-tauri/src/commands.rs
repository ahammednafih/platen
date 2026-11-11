//! Commands the Svelte frontend invokes. Blocking work (network, image) runs off the main thread.
use crate::{AppState, Res, err};
use scan_core::adjust::{self, Edit};
use scan_core::capture::{self, Captured, Settings, TopHalf};
use scan_core::device::{self, Draft};
use scan_core::escl::{Caps, Scanner};
use scan_core::ipp::{Printer, Status};
use scan_core::presets::{self, Preset};
use scan_core::session::{ExportOpts, Page, Session};
use scan_core::{pushscan, webui};
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

pub(crate) fn session_dir(st: &AppState) -> Res<PathBuf> {
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
pub(crate) fn place(st: &AppState, c: Captured, replace: Option<usize>, at: Option<usize>) -> Res<PageResult> {
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
pub async fn connect(app: tauri::AppHandle, st: State<'_, AppState>, host: Option<String>) -> Res<Device> {
    let s = blocking(move || device::connect(host.as_deref())).await?;
    let d = Device { name: s.name.clone(), host: s.host().to_string() };
    *st.scanner.lock().unwrap() = Some(s);
    crate::button::resume(&app, &st, d.host.split(':').next().unwrap_or_default());
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
    scan_core::session::check_name(name.trim()).map_err(err)?;
    let s = Session::open(device::default_root().join(name.trim())).map_err(err)?;
    let v = view(&s);
    *st.session.lock().unwrap() = Some(s);
    Ok(v)
}

/// Rename the open scan (its folder and exports carrying the old name).
#[tauri::command]
pub fn rename_session(st: State<'_, AppState>, name: String) -> Res<SessionView> {
    let mut guard = st.session.lock().unwrap();
    let s = guard.as_mut().ok_or("No session open")?;
    s.rename(&name).map_err(err)?;
    Ok(view(s))
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

/// Device web UI host: the scanner's host without the port.
pub(crate) fn web_host(st: &AppState) -> Res<String> {
    Ok(scanner(st)?.host().split(':').next().unwrap_or_default().to_string())
}

/// Counters, refills, error history and Wi-Fi status. A typed password is saved to the Keychain once it
/// works; errors `needs-password` / `wrong-password` ask the UI for one.
#[tauri::command]
pub async fn device_details(st: State<'_, AppState>, password: Option<String>) -> Res<Vec<webui::Group>> {
    let host = web_host(&st)?;
    blocking(move || with_password(&host, password, |pw| webui::details(&host, pw))).await
}

/// Runs `f` with the device password: the typed one (saved to the Keychain once it works) or the saved one.
pub(crate) fn with_password<T>(host: &str, password: Option<String>, f: impl FnOnce(&str) -> anyhow::Result<T>) -> anyhow::Result<T> {
    let typed = password.is_some();
    let pw = password.or_else(|| webui::saved_password(host)).ok_or_else(|| anyhow::anyhow!("needs-password"))?;
    match f(&pw) {
        Err(e) if e.is::<webui::WrongPassword>() => Err(anyhow::anyhow!("wrong-password")),
        Ok(v) if typed => webui::save_password(host, &pw).map(|_| v),
        other => other,
    }
}

/// Switch the device's SNMP v1/v2c to read-write through its web UI.
#[tauri::command]
pub async fn enable_snmp_write(st: State<'_, AppState>, password: Option<String>) -> Res<()> {
    let host = web_host(&st)?;
    blocking(move || with_password(&host, password, |pw| pushscan::enable_snmp_write(&host, pw))).await
}

/// Words on a session page for selecting and copying text; cached per page file (edits make new files).
#[tauri::command]
pub async fn page_text(st: State<'_, AppState>, file: String) -> Res<scan_core::ocr::PageText> {
    if file.contains(['/', '\\']) {
        return Err("bad page name".into());
    }
    let dir = session_dir(&st)?;
    let dpi = st.session.lock().unwrap().as_ref().and_then(|s| s.pages.iter().find(|p| p.file == file).map(|p| p.dpi)).ok_or("page not found")?;
    blocking(move || {
        let cache = dir.join("text").join(format!("{file}.json"));
        if let Some(t) = std::fs::read(&cache).ok().and_then(|b| serde_json::from_slice(&b).ok()) {
            return Ok(t);
        }
        let text = scan_core::ocr::Ocr::find()?.words(&dir.join("pages").join(&file), dpi, &dir.join("text"))?;
        std::fs::write(&cache, serde_json::to_vec(&text)?)?;
        Ok(text)
    })
    .await
}
