//! Platen for phones: the app state and commands the Android and iOS apps share, bound with UniFFI.
//! Platform work (finding the printer, rendering PDF pages, storing the password) stays in the apps.
mod types;

use platen_core::capture::{self, Captured};
use platen_core::escl::Scanner;
use platen_core::history::History;
use platen_core::ipp::Printer;
use platen_core::session::{self, Session};
use platen_core::{adjust, copy, device, pdfedit, presets, printjob, render, webui};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
pub use types::*;

uniffi::setup_scaffolding!();

#[derive(Debug, uniffi::Error)]
#[uniffi(flat_error)]
pub enum PlatenError {
    Failed(String),
}

impl std::fmt::Display for PlatenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let PlatenError::Failed(m) = self;
        f.write_str(m)
    }
}

impl From<anyhow::Error> for PlatenError {
    fn from(e: anyhow::Error) -> Self {
        if e.is::<webui::WrongPassword>() {
            return fail("wrong-password");
        }
        PlatenError::Failed(format!("{e:#}"))
    }
}

impl From<std::io::Error> for PlatenError {
    fn from(e: std::io::Error) -> Self {
        PlatenError::Failed(e.to_string())
    }
}

impl From<uniffi::UnexpectedUniFFICallbackError> for PlatenError {
    fn from(e: uniffi::UnexpectedUniFFICallbackError) -> Self {
        PlatenError::Failed(e.reason)
    }
}

fn fail(m: &str) -> PlatenError {
    PlatenError::Failed(m.into())
}

type Result<T> = std::result::Result<T, PlatenError>;

#[uniffi::export(with_foreign)]
pub trait PdfRenderer: Send + Sync {
    fn render(&self, path: String, index: u32, dpi: u32) -> Result<RenderedPage>;
}

#[uniffi::export(with_foreign)]
pub trait PrintProgress: Send + Sync {
    fn progress(&self, done: u32, total: u32);
}

#[derive(uniffi::Object)]
pub struct Platen {
    scanner: Mutex<Option<Scanner>>,
    session: Mutex<Option<Session>>,
    settings: Mutex<capture::Settings>,
    history: Mutex<History>,
    copy_dir: Mutex<Option<PathBuf>>,
    stop: Arc<AtomicBool>,
}

#[uniffi::export]
impl Platen {
    /// `config` keeps settings, `cache` rebuildable files, `scans` the drafts and their exports.
    #[uniffi::constructor]
    pub fn new(config: String, cache: String, scans: String, renderer: Arc<dyn PdfRenderer>) -> Arc<Self> {
        let tmp = Path::new(&cache).join("tmp");
        let _ = std::fs::create_dir_all(&tmp);
        // Safety: set once at startup, before this library starts any thread that reads the environment.
        unsafe { std::env::set_var("TMPDIR", &tmp) };
        device::set_dirs(device::Dirs { config: config.into(), cache: cache.into(), scans: scans.into() });
        render::set_renderer(Box::new(move |path, index, dpi| {
            let p = renderer.render(path.to_string_lossy().into_owned(), index as u32, dpi).map_err(|e| anyhow::anyhow!("{e}"))?;
            image::RgbImage::from_raw(p.width, p.height, p.rgb).ok_or_else(|| anyhow::anyhow!("the rendered page has the wrong size"))
        }));
        Arc::new(Platen {
            scanner: Mutex::new(None),
            session: Mutex::new(None),
            settings: Mutex::new(presets::last()),
            history: Mutex::new(History::default()),
            copy_dir: Mutex::new(None),
            stop: Arc::default(),
        })
    }

    /// Use the scanner at `base` (an eSCL URL from discovery, or a host) and check it answers.
    pub fn connect(&self, base: String, name: String) -> Result<Device> {
        let mut s = Scanner::at(&base);
        s.state()?;
        s.name = name;
        let d = Device { name: s.name.clone(), host: s.host().to_string() };
        *self.scanner.lock().unwrap() = Some(s);
        Ok(d)
    }

    pub fn status(&self) -> Result<DeviceStatus> {
        let s = self.scanner()?;
        let scanner = s.state().unwrap_or_else(|_| "Offline".into());
        let printer = Printer::for_host(s.host()).status().ok().map(Into::into);
        Ok(DeviceStatus { scanner, printer })
    }

    pub fn capabilities(&self) -> Result<Caps> {
        Ok(self.scanner()?.capabilities()?.into())
    }

    pub fn drafts(&self) -> Vec<Draft> {
        device::drafts(&device::default_root()).into_iter().map(Into::into).collect()
    }

    pub fn delete_draft(&self, name: String) -> Result<Vec<Draft>> {
        if self.session.lock().unwrap().as_ref().is_some_and(|s| s.name == name) {
            return Err(fail("Close this scan before deleting it"));
        }
        device::delete_draft(&device::default_root(), &name)?;
        Ok(self.drafts())
    }

    /// Open (or resume) a scan by name; a fresh dated name when none is given.
    pub fn open_session(&self, name: Option<String>) -> Result<SessionView> {
        let name = name.filter(|n| !n.trim().is_empty()).unwrap_or_else(device::default_name);
        session::check_name(name.trim())?;
        let s = Session::open(device::default_root().join(name.trim()))?;
        self.history.lock().unwrap().clear();
        let v = SessionView::of(&s, (false, false));
        *self.session.lock().unwrap() = Some(s);
        Ok(v)
    }

    pub fn session(&self) -> Option<SessionView> {
        self.session.lock().unwrap().as_ref().map(|s| SessionView::of(s, self.history.lock().unwrap().can()))
    }

    pub fn rename_session(&self, name: String) -> Result<SessionView> {
        let mut g = self.session.lock().unwrap();
        let s = g.as_mut().ok_or_else(|| fail("No scan open"))?;
        s.rename(&name)?;
        Ok(SessionView::of(s, self.history.lock().unwrap().can()))
    }

    pub fn close_session(&self) {
        *self.session.lock().unwrap() = None;
        self.history.lock().unwrap().clear();
    }

    pub fn settings(&self) -> Settings {
        (*self.settings.lock().unwrap()).into()
    }

    pub fn set_settings(&self, settings: Settings) {
        let s = settings.into();
        *self.settings.lock().unwrap() = s;
        presets::remember(&s);
    }

    pub fn presets(&self) -> Vec<Preset> {
        types::presets(presets::all())
    }

    pub fn save_preset(&self, name: String) -> Result<Vec<Preset>> {
        let s = *self.settings.lock().unwrap();
        Ok(types::presets(presets::save(&name, s)?))
    }

    pub fn delete_preset(&self, name: String) -> Vec<Preset> {
        types::presets(presets::delete(&name))
    }

    pub fn scan_page(&self, replace: Option<u32>, at: Option<u32>) -> Result<PageResult> {
        let (sc, set, dir) = (self.scanner()?, *self.settings.lock().unwrap(), self.session_dir()?);
        let c = capture::scan_a4(&sc, set, &dir)?;
        self.place(c, replace, at)
    }

    pub fn scan_top(&self) -> Result<TopHalf> {
        let (sc, set, dir) = (self.scanner()?, *self.settings.lock().unwrap(), self.session_dir()?);
        Ok(capture::scan_top(&sc, set, &dir)?.into())
    }

    pub fn scan_bottom(&self, top: TopHalf, replace: Option<u32>, at: Option<u32>) -> Result<PageResult> {
        let (sc, set, dir) = (self.scanner()?, *self.settings.lock().unwrap(), self.session_dir()?);
        let c = capture::scan_bottom(&sc, set, &dir, &top.into())?;
        self.place(c, replace, at)
    }

    pub fn cancel_scan(&self) {
        if let Ok(s) = self.scanner() {
            s.cancel_scan();
        }
    }

    pub fn fill_page(&self, index: u32) -> Result<PageResult> {
        let (dir, page) = (self.session_dir()?, self.page(index)?);
        let mode = self.settings.lock().unwrap().fill;
        let c = capture::fill_page(&dir, &page, mode)?;
        self.place(c, Some(index), None)
    }

    pub fn edit_page(&self, index: u32, edit: Edit) -> Result<PageResult> {
        let (dir, page) = (self.session_dir()?, self.page(index)?);
        let c = adjust::edit_page(&dir, &page, edit.into())?;
        self.place(c, Some(index), None)
    }

    /// The paper's rectangle on a page, as fractions, for the crop tool's Auto.
    pub fn paper_bounds(&self, index: u32) -> Result<Option<Bounds>> {
        let (dir, page) = (self.session_dir()?, self.page(index)?);
        Ok(adjust::paper_bounds(&dir, &page)?.map(|(x, y, w, h)| Bounds { x, y, w, h }))
    }

    pub fn delete_page(&self, index: u32) -> Result<SessionView> {
        self.edit(|pages| {
            let i = index as usize;
            if i >= pages.len() {
                return Err(fail("No such page"));
            }
            pages.remove(i);
            Ok(())
        })
    }

    pub fn move_page(&self, from: u32, to: u32) -> Result<SessionView> {
        self.edit(|pages| {
            let (from, to) = (from as usize, to as usize);
            if from >= pages.len() || to >= pages.len() {
                return Err(fail("No such page"));
            }
            let p = pages.remove(from);
            pages.insert(to, p);
            Ok(())
        })
    }

    pub fn undo(&self) -> Result<SessionView> {
        self.step(false)
    }

    pub fn redo(&self) -> Result<SessionView> {
        self.step(true)
    }

    pub fn export_opts(&self) -> ExportOpts {
        session::ExportOpts::load().into()
    }

    /// Write the chosen PDFs and images into the scan's folder; the app then copies them where the user keeps scans.
    pub fn export(&self, opts: ExportOpts) -> Result<Exported> {
        let opts = opts.into_core(session::ExportOpts::load());
        opts.save();
        let mut s = self.snapshot()?;
        if s.pages.is_empty() {
            return Err(fail("Nothing to export yet"));
        }
        let files = s.export(opts)?;
        if opts.cleanup {
            self.close_session();
        }
        Ok(Exported { files: files.iter().map(|f| f.to_string_lossy().into_owned()).collect(), closed: opts.cleanup })
    }

    /// A PDF of the open scan in the cache, for printing it.
    pub fn scan_to_pdf(&self) -> Result<String> {
        let s = self.snapshot()?;
        let out = device::cache_dir().join("print").join(format!("{}.pdf", s.name));
        s.write_pdf(&out)?;
        Ok(out.to_string_lossy().into_owned())
    }

    pub fn page_count(&self, path: String) -> Result<u32> {
        if !render::is_pdf(Path::new(&path)) {
            return Ok(1);
        }
        Ok(pdfedit::page_count(&std::fs::read(path)?)? as u32)
    }

    /// Shrink `path` under `max_mb` (or only leave out pages), writing a draft into the cache.
    pub fn shrink_pdf(&self, path: String, max_mb: Option<f32>, skip: Vec<u32>) -> Result<ShrinkResult> {
        if max_mb.is_some_and(|mb| !(0.05..=1000.0).contains(&mb)) {
            return Err(fail("Pick a size between 0.05 and 1000 MB"));
        }
        let input = std::fs::read(&path)?;
        let kept = pdfedit::drop_pages(&input, &skip)?;
        let r = pdfedit::shrink(&kept, max_mb.map_or(u64::MAX, |mb| (mb * 1_000_000.0) as u64))?;
        let pages = pdfedit::page_count(&r.data)?;
        let stem = Path::new(&path).file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let dir = device::cache_dir().join("shrink");
        std::fs::create_dir_all(&dir)?;
        let out = dir.join(match max_mb {
            Some(mb) => format!("{stem}_under{mb}MB.pdf"),
            None => format!("{stem}_{pages}pages.pdf"),
        });
        std::fs::write(&out, &r.data)?;
        Ok(ShrinkResult { out: out.to_string_lossy().into_owned(), before: input.len() as u64, after: r.data.len() as u64, fits: r.fits, pages: pages as u32 })
    }

    pub fn print_caps(&self) -> Result<PrintCaps> {
        Ok(self.printer()?.caps()?.into())
    }

    pub fn print_plan(&self, job: PrintJob) -> Result<Plan> {
        let job: printjob::PrintJob = job.into();
        Ok((&job.plan()?).into())
    }

    /// JPEG of the `slot`-th sheet the job would send.
    pub fn print_preview(&self, job: PrintJob, slot: u32, dpi: u32) -> Result<Vec<u8>> {
        let job: printjob::PrintJob = job.into();
        let plan = job.plan()?;
        let sheet = *plan.order.get(slot as usize).ok_or_else(|| fail("No such sheet"))?;
        Ok(job.sheet(&plan, sheet, dpi)?)
    }

    /// Send the job, reporting each sheet. Returns how many jobs went to the printer.
    pub fn print(&self, job: PrintJob, progress: Arc<dyn PrintProgress>) -> Result<u32> {
        let p = self.printer()?;
        let job: printjob::PrintJob = job.into();
        self.stop.store(false, Ordering::Relaxed);
        let res = job.run_until(&p, "scan", &self.stop, |done, total| progress.progress(done as u32, total as u32));
        Ok(self.stopped(res)?.len() as u32)
    }

    /// Stop the copy or print in progress: no more sheets, the scan aborts, queued jobs are cancelled.
    pub fn stop_job(&self) -> Result<u32> {
        self.stop.store(true, Ordering::Relaxed);
        self.scanner()?.cancel_scan();
        Ok(self.printer()?.cancel_active()? as u32)
    }

    pub fn copy_reset(&self) {
        if let Some(d) = self.copy_dir.lock().unwrap().take() {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    pub fn copy_scan(&self, area: Area, color: bool) -> Result<String> {
        let (sc, dir) = (self.scanner()?, self.copy_dir()?);
        self.stop.store(false, Ordering::Relaxed);
        Ok(self.stopped(copy::scan(&sc, area.into(), color, &dir))?.to_string_lossy().into_owned())
    }

    pub fn copy_long_top(&self, color: bool) -> Result<TopHalf> {
        let (sc, dir) = (self.scanner()?, self.copy_dir()?);
        Ok(copy::long_top(&sc, color, &dir)?.into())
    }

    pub fn copy_long_bottom(&self, top: TopHalf, color: bool) -> Result<String> {
        let (sc, dir) = (self.scanner()?, self.copy_dir()?);
        Ok(copy::long_bottom(&sc, color, &dir, &top.into())?.to_string_lossy().into_owned())
    }

    pub fn copy_id_card(&self, front: String, back: String) -> Result<String> {
        let dir = self.copy_dir()?;
        Ok(copy::id_card(Path::new(&front), Path::new(&back), &dir)?.to_string_lossy().into_owned())
    }

    pub fn copy_tone(&self, files: Vec<String>, density: i32, background: u8) -> Result<Vec<String>> {
        let dir = self.copy_dir()?;
        files.iter().map(|f| Ok(copy::tone(Path::new(f), density, background, &dir)?.to_string_lossy().into_owned())).collect()
    }

    /// Counters, refills, error history and Wi-Fi status from the device's web page. Fails with
    /// `wrong-password` when the password doesn't work; the app keeps the password.
    pub fn device_details(&self, password: String) -> Result<Vec<DetailGroup>> {
        let host = self.web_host()?;
        Ok(webui::details(&host, &password)?.into_iter().map(Into::into).collect())
    }
}

impl Platen {
    fn scanner(&self) -> Result<Scanner> {
        self.scanner.lock().unwrap().clone().ok_or_else(|| fail("Not connected to a scanner"))
    }

    fn printer(&self) -> Result<Printer> {
        Ok(Printer::for_host(self.scanner()?.host()))
    }

    fn web_host(&self) -> Result<String> {
        Ok(self.scanner()?.host().split(':').next().unwrap_or_default().to_string())
    }

    fn session_dir(&self) -> Result<PathBuf> {
        self.session.lock().unwrap().as_ref().map(|s| s.dir.clone()).ok_or_else(|| fail("No scan open"))
    }

    fn page(&self, index: u32) -> Result<session::Page> {
        self.session.lock().unwrap().as_ref().and_then(|s| s.pages.get(index as usize).cloned()).ok_or_else(|| fail("No such page"))
    }

    fn snapshot(&self) -> Result<Session> {
        let g = self.session.lock().unwrap();
        let s = g.as_ref().ok_or_else(|| fail("No scan open"))?;
        Ok(Session { name: s.name.clone(), dir: s.dir.clone(), pages: s.pages.clone(), dirty: true })
    }

    fn copy_dir(&self) -> Result<PathBuf> {
        let mut g = self.copy_dir.lock().unwrap();
        if g.is_none() {
            *g = Some(copy::scratch()?);
        }
        Ok(g.clone().unwrap())
    }

    fn stopped<T>(&self, res: anyhow::Result<T>) -> Result<T> {
        match res {
            Err(_) if self.stop.load(Ordering::Relaxed) => Err(fail("stopped")),
            other => Ok(other?),
        }
    }

    /// Change the open scan's pages, save, and remember the step for undo.
    fn edit(&self, f: impl FnOnce(&mut Vec<session::Page>) -> Result<()>) -> Result<SessionView> {
        let mut g = self.session.lock().unwrap();
        let s = g.as_mut().ok_or_else(|| fail("No scan open"))?;
        let before = s.pages.clone();
        f(&mut s.pages)?;
        s.save()?;
        let mut h = self.history.lock().unwrap();
        h.record(before);
        Ok(SessionView::of(s, h.can()))
    }

    fn step(&self, forward: bool) -> Result<SessionView> {
        let mut g = self.session.lock().unwrap();
        let s = g.as_mut().ok_or_else(|| fail("No scan open"))?;
        let mut h = self.history.lock().unwrap();
        let now = s.pages.clone();
        if let Some(p) = if forward { h.redo(now) } else { h.undo(now) } {
            s.pages = p;
            s.save()?;
        }
        Ok(SessionView::of(s, h.can()))
    }

    /// Put a captured page at `replace`, else insert at `at`, else append.
    fn place(&self, c: Captured, replace: Option<u32>, at: Option<u32>) -> Result<PageResult> {
        let mut index = 0;
        let session = self.edit(|pages| {
            index = match replace.map(|r| r as usize) {
                Some(i) if i < pages.len() => {
                    pages[i] = c.page;
                    i
                }
                _ => {
                    let i = at.map_or(pages.len(), |a| a as usize).min(pages.len());
                    pages.insert(i, c.page);
                    i
                }
            };
            Ok(())
        })?;
        Ok(PageResult { session, index: index as u32, note: c.note })
    }
}
