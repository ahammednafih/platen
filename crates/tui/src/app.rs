//! The TUI: page list, key handling, and background scan jobs.
use scan_core::capture::{self, Settings, TopHalf, dur};
use scan_core::escl::Scanner;
use scan_core::session::{self, ExportOpts, Kind, Page, Session};
use anyhow::{Context, Result};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph, Wrap};
use image::DynamicImage;
use ratatui_image::{StatefulImage, picker::Picker, protocol::StatefulProtocol};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

enum Mode {
    Idle,
    /// Optional image to keep previewing while the job runs.
    Busy(String, Instant, Option<PathBuf>),
    /// Top half scanned; waiting for the user to turn the page.
    Flip { top: TopHalf, replace: Option<usize> },
    /// Choosing what to export.
    Export { cursor: usize, then_quit: bool },
}

enum Msg {
    State(String),
    Ink(String),
    Page { page: Page, replace: Option<usize>, note: String },
    Top { top: TopHalf, replace: Option<usize> },
    Exported { files: Vec<PathBuf>, quit: bool },
    Failed(String),
    Thumb(String, Option<DynamicImage>),
}

struct App {
    scanner: Scanner,
    session: Session,
    settings: Settings,
    mode: Mode,
    list: ListState,
    state: String,
    ink: String,
    log: Vec<(Color, String)>,
    tx: Sender<Msg>,
    quit_after_export: bool,
    started: Instant,
    export_opts: ExportOpts,
    exit_note: Option<String>,
    last_export: Vec<PathBuf>,
    picker: Option<Picker>,
    thumbs: HashMap<String, DynamicImage>,
    loading: HashSet<String>,
    preview: Option<(String, StatefulProtocol)>,
}

pub fn run(scanner: Scanner, session: Session, settings: Settings) -> Result<()> {
    let (tx, rx) = channel();
    spawn_status(scanner.clone(), tx.clone());
    let mut app = App {
        list: ListState::default(),
        state: "…".into(),
        ink: String::new(),
        log: vec![(Color::DarkGray, format!("Saving to {}", session.dir.display()))],
        mode: Mode::Idle,
        quit_after_export: false,
        started: Instant::now(),
        export_opts: ExportOpts::load(),
        exit_note: None,
        last_export: Vec::new(),
        picker: None,
        thumbs: HashMap::new(),
        loading: HashSet::new(),
        preview: None,
        scanner,
        session,
        settings,
        tx,
    };
    if !app.session.pages.is_empty() {
        app.list.select(Some(app.session.pages.len() - 1));
        app.info(format!("Resumed with {} pages", app.session.pages.len()));
    }
    let mut term = ratatui::init();
    app.picker = Some(Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks()));
    let res = app.event_loop(&mut term, rx);
    ratatui::restore();
    if let Some(note) = app.exit_note {
        println!("{note}");
    }
    res
}

fn spawn_status(scanner: Scanner, tx: Sender<Msg>) {
    let printer = scan_core::ipp::Printer::for_host(scanner.host());
    let tx2 = tx.clone();
    std::thread::spawn(move || loop {
        if let Ok(st) = printer.status() {
            let ink = st.ink.iter().map(|i| format!("{}{}%", &i.name[..1], i.level)).collect::<Vec<_>>().join(" ");
            if tx2.send(Msg::Ink(ink)).is_err() {
                return;
            }
        }
        std::thread::sleep(Duration::from_secs(60));
    });
    std::thread::spawn(move || loop {
        let s = scanner.state().unwrap_or_else(|_| "Offline".into());
        if tx.send(Msg::State(s)).is_err() {
            return;
        }
        std::thread::sleep(Duration::from_secs(3));
    });
}

impl App {
    fn event_loop(&mut self, term: &mut ratatui::DefaultTerminal, rx: Receiver<Msg>) -> Result<()> {
        loop {
            while let Ok(m) = rx.try_recv() {
                if self.on_msg(m) {
                    return Ok(());
                }
            }
            term.draw(|f| self.draw(f))?;
            if !event::poll(Duration::from_millis(150))? {
                continue;
            }
            if let Event::Key(k) = event::read()? {
                if k.kind != KeyEventKind::Press {
                    continue;
                }
                if k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL) {
                    return Ok(());
                }
                if self.on_key(k.code, k.modifiers) {
                    return Ok(());
                }
            }
        }
    }

    // ---- input -------------------------------------------------------------

    /// Returns true to quit.
    fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) -> bool {
        if let Mode::Flip { .. } = self.mode {
            match code {
                KeyCode::Enter | KeyCode::Char(' ') => self.scan_bottom(),
                KeyCode::Esc => {
                    self.mode = Mode::Idle;
                    self.warn("Long page cancelled".into());
                }
                KeyCode::Char('q') => {
                    self.mode = Mode::Idle;
                    self.warn("Long page cancelled".into());
                    return self.quit();
                }
                KeyCode::Char('Q') => return self.quit_draft(),
                _ => {}
            }
            return false;
        }
        if let Mode::Export { cursor, then_quit } = self.mode {
            return self.on_export_key(code, cursor, then_quit);
        }
        let busy = matches!(self.mode, Mode::Busy(..));
        let shift = mods.contains(KeyModifiers::SHIFT);
        match code {
            KeyCode::Char('q') if !busy => return self.quit(),
            KeyCode::Char('Q') if !busy => return self.quit_draft(),
            KeyCode::Up | KeyCode::Char('k') if !shift => self.select(-1),
            KeyCode::Down | KeyCode::Char('j') if !shift => self.select(1),
            KeyCode::Char('K') | KeyCode::Up => self.move_page(-1),
            KeyCode::Char('J') | KeyCode::Down => self.move_page(1),
            KeyCode::Char('d') => {
                self.settings.dpi = if self.settings.dpi == 600 { 300 } else { 600 };
            }
            KeyCode::Char('c') => self.settings.mode = self.settings.mode.next(),
            KeyCode::Char('f') => self.settings.fill = self.settings.fill.next(),
            KeyCode::Char('v') => self.preview(),
            KeyCode::Char('o') => open(&self.session.dir),
            KeyCode::Char('p') => match best_pdf(&self.last_export) {
                Some(f) => open(&f),
                None => self.warn("Export first (e)".into()),
            },
            _ if busy => {}
            KeyCode::Char('a') | KeyCode::Enter => self.scan_a4(None),
            KeyCode::Char('l') => self.scan_top(None),
            KeyCode::Char('r') => self.retake(),
            KeyCode::Char('F') => self.fill_selected(),
            KeyCode::Char('x') | KeyCode::Delete | KeyCode::Backspace => self.delete(),
            KeyCode::Char('e') if !self.session.pages.is_empty() => self.mode = Mode::Export { cursor: 0, then_quit: false },
            _ => {}
        }
        false
    }

    fn select(&mut self, d: i64) {
        let n = self.session.pages.len() as i64;
        if n == 0 {
            return;
        }
        let i = self.list.selected().unwrap_or(0) as i64 + d;
        self.list.select(Some(i.clamp(0, n - 1) as usize));
    }

    fn move_page(&mut self, d: i64) {
        let Some(i) = self.list.selected() else { return };
        let j = i as i64 + d;
        if j < 0 || j >= self.session.pages.len() as i64 {
            return;
        }
        self.session.pages.swap(i, j as usize);
        self.list.select(Some(j as usize));
        self.persist();
    }

    fn delete(&mut self) {
        let Some(i) = self.list.selected() else { return };
        let p = self.session.pages.remove(i);
        self.warn(format!("Removed page {} (file kept in pages/{})", i + 1, p.file));
        let n = self.session.pages.len();
        self.list.select(if n == 0 { None } else { Some(i.min(n - 1)) });
        self.persist();
    }

    fn retake(&mut self) {
        let Some(i) = self.list.selected() else { return };
        match self.session.pages[i].kind {
            Kind::A4 => self.scan_a4(Some(i)),
            Kind::Long => self.scan_top(Some(i)),
        }
    }

    fn preview(&self) {
        if let Some(p) = self.list.selected().and_then(|i| self.session.pages.get(i)) {
            open(&self.session.page_path(p));
        }
    }

    fn quit(&mut self) -> bool {
        if self.session.dirty && !self.session.pages.is_empty() {
            self.mode = Mode::Export { cursor: 0, then_quit: true };
            return false;
        }
        true
    }

    /// Quit keeping the working scans so the session can be resumed.
    fn quit_draft(&mut self) -> bool {
        if !self.session.pages.is_empty() {
            self.exit_note = Some(format!(
                "Draft saved: {} pages in {}\nResume with:  scan {}",
                self.session.pages.len(),
                self.session.dir.display(),
                self.session.name
            ));
        }
        true
    }

    /// Returns true to quit.
    fn on_export_key(&mut self, code: KeyCode, cursor: usize, then_quit: bool) -> bool {
        let n = ExportOpts::LABELS.len();
        match code {
            KeyCode::Up | KeyCode::Char('k') => self.mode = Mode::Export { cursor: (cursor + n - 1) % n, then_quit },
            KeyCode::Down | KeyCode::Char('j') => self.mode = Mode::Export { cursor: (cursor + 1) % n, then_quit },
            KeyCode::Char(' ') | KeyCode::Char('x') => self.export_opts.toggle(cursor),
            KeyCode::Char(c @ '1'..='7') => self.export_opts.toggle(c as usize - '1' as usize),
            KeyCode::Enter => {
                if !self.export_opts.any_output() {
                    self.warn("Pick at least one output".into());
                } else {
                    self.export_opts.save();
                    self.export(then_quit || self.export_opts.cleanup);
                }
            }
            KeyCode::Char('d') | KeyCode::Char('Q') if then_quit => return self.quit_draft(),
            KeyCode::Esc | KeyCode::Char('q') => self.mode = Mode::Idle,
            _ => {}
        }
        false
    }

    // ---- jobs --------------------------------------------------------------

    fn busy(&mut self, what: &str) {
        self.mode = Mode::Busy(what.into(), Instant::now(), None);
    }

    fn scan_a4(&mut self, replace: Option<usize>) {
        self.busy("Scanning A4 page");
        let (scanner, settings, dir) = (self.scanner.clone(), self.settings, self.session.dir.clone());
        job(self.tx.clone(), move || {
            let c = capture::scan_a4(&scanner, settings, &dir)?;
            Ok(Msg::Page { page: c.page, replace, note: c.note })
        });
    }

    fn fill_selected(&mut self) {
        let Some(i) = self.list.selected() else { return };
        let (page, mode, dir) = (self.session.pages[i].clone(), self.settings.fill, self.session.dir.clone());
        self.busy("Filling torn areas");
        job(self.tx.clone(), move || {
            let c = capture::fill_page(&dir, &page, mode).with_context(|| format!("page {}", i + 1))?;
            Ok(Msg::Page { page: c.page, replace: Some(i), note: c.note })
        });
    }

    fn scan_top(&mut self, replace: Option<usize>) {
        self.busy("Scanning top half");
        let (scanner, settings, dir) = (self.scanner.clone(), self.settings, self.session.dir.clone());
        job(self.tx.clone(), move || Ok(Msg::Top { top: capture::scan_top(&scanner, settings, &dir)?, replace }));
    }

    fn scan_bottom(&mut self) {
        let Mode::Flip { top, replace } = std::mem::replace(&mut self.mode, Mode::Idle) else { return };
        self.mode = Mode::Busy("Scanning bottom half, then joining".into(), Instant::now(), Some(top.path.clone()));
        let (scanner, settings, dir) = (self.scanner.clone(), self.settings, self.session.dir.clone());
        job(self.tx.clone(), move || {
            let c = capture::scan_bottom(&scanner, settings, &dir, &top)?;
            Ok(Msg::Page { page: c.page, replace, note: c.note })
        });
    }

    fn export(&mut self, then_quit: bool) {
        if self.session.pages.is_empty() {
            self.warn("Nothing to export yet".into());
            return;
        }
        self.busy("Exporting");
        self.quit_after_export = then_quit;
        let (tx, opts) = (self.tx.clone(), self.export_opts);
        let mut s = Session { name: self.session.name.clone(), dir: self.session.dir.clone(), pages: self.session.pages.clone(), dirty: true };
        job(tx, move || Ok(Msg::Exported { files: s.export(opts)?, quit: then_quit }));
    }

    /// Returns true to quit.
    fn on_msg(&mut self, m: Msg) -> bool {
        match m {
            Msg::State(s) => self.state = s,
            Msg::Ink(s) => self.ink = s,
            Msg::Thumb(key, img) => {
                self.loading.remove(&key);
                if let Some(img) = img {
                    self.thumbs.insert(key, img);
                }
            }
            Msg::Top { top, replace } => {
                self.mode = Mode::Flip { top, replace };
                bell();
            }
            Msg::Page { page, replace, note } => {
                let i = match replace {
                    Some(i) if i < self.session.pages.len() => {
                        self.session.pages[i] = page;
                        i
                    }
                    _ => {
                        let at = self.list.selected().map_or(self.session.pages.len(), |i| i + 1);
                        self.session.pages.insert(at, page);
                        at
                    }
                };
                self.list.select(Some(i));
                self.mode = Mode::Idle;
                self.info(format!("✓ Page {}: {note}", i + 1));
                self.persist();
                bell();
            }
            Msg::Exported { files, quit } => {
                self.session.dirty = false;
                self.mode = Mode::Idle;
                let names: Vec<String> = files.iter().filter_map(|f| f.file_name()).map(|f| f.to_string_lossy().into_owned()).collect();
                let summary = format!(
                    "✓ Saved in {}: {}\n  {} pages · {} open · {} scanning",
                    self.session.dir.display(),
                    names.join(", "),
                    self.session.pages.len(),
                    dur(self.started.elapsed().as_secs_f32()),
                    dur(self.session.pages.iter().map(|p| p.secs).sum())
                );
                self.info(summary.clone());
                if self.export_opts.open_pdf {
                    if let Some(f) = best_pdf(&files) {
                        open(&f);
                    }
                }
                if self.export_opts.open_folder {
                    open(&self.session.dir);
                }
                self.last_export = files;
                if quit {
                    self.exit_note = Some(summary);
                }
                return quit;
            }
            Msg::Failed(e) => {
                self.mode = Mode::Idle;
                self.quit_after_export = false;
                self.error(e);
                bell();
            }
        }
        false
    }

    fn persist(&mut self) {
        if let Err(e) = self.session.save() {
            self.error(format!("Couldn't save session: {e}"));
        }
    }

    fn info(&mut self, s: String) {
        self.log.push((Color::Green, s));
    }
    fn warn(&mut self, s: String) {
        self.log.push((Color::Yellow, s));
    }
    fn error(&mut self, s: String) {
        self.log.push((Color::Red, format!("✗ {s}")));
    }

    // ---- drawing -----------------------------------------------------------

    /// What the preview pane should show: the top half while waiting to flip,
    /// otherwise the selected page.
    fn preview_target(&self) -> Option<(String, PathBuf, String)> {
        match &self.mode {
            Mode::Flip { top, .. } => {
                return Some((top.path.display().to_string(), top.path.clone(), " Top half (check before turning) ".into()));
            }
            Mode::Busy(_, _, Some(top)) => {
                return Some((top.display().to_string(), top.clone(), " Top half (joining…) ".into()));
            }
            _ => {}
        }
        let i = self.list.selected()?;
        let p = self.session.pages.get(i)?;
        let (w, h) = p.mm();
        Some((p.file.clone(), self.session.page_path(p), format!(" Page {}  {w}×{h} mm ", i + 1)))
    }

    fn draw_export(&self, f: &mut ratatui::Frame, area: ratatui::layout::Rect, cursor: usize, then_quit: bool) {
        let mut lines = vec![Line::raw(""), Line::raw(" Keep which outputs?"), Line::raw("")];
        for (i, label) in ExportOpts::LABELS.iter().enumerate() {
            let mark = if self.export_opts.get(i) { "[x]" } else { "[ ]" };
            let style = if i == cursor { Style::new().add_modifier(Modifier::REVERSED) } else { Style::new() };
            let style = if i == 4 && self.export_opts.cleanup { style.fg(Color::Yellow) } else { style };
            lines.push(Line::styled(format!(" {mark} {} {label}", i + 1), style));
        }
        lines.push(Line::raw(""));
        lines.push(Line::styled(" ↑↓ move · Space toggle · Enter export · Esc back", Style::new().fg(Color::DarkGray)));
        if then_quit {
            lines.push(Line::raw(""));
            lines.push(Line::styled(" d  Save as draft & quit (resume later)", Style::new().fg(Color::Green).bold()));
        }
        let title = if then_quit { " Export before quitting " } else { " Export " };
        f.render_widget(Paragraph::new(lines).block(Block::bordered().title(title).border_style(Style::new().fg(Color::Cyan))), area);
    }

    fn draw_preview(&mut self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        if let Mode::Export { cursor, then_quit } = self.mode {
            self.preview = None;
            return self.draw_export(f, area, cursor, then_quit);
        }
        let Some((key, path, title)) = self.preview_target() else {
            f.render_widget(Paragraph::new(" No pages yet").block(Block::bordered().title(" Preview ")), area);
            return;
        };
        let block = Block::bordered().title(title);
        let inner = block.inner(area);
        f.render_widget(block, area);
        if self.preview.as_ref().map(|(k, _)| k) != Some(&key) {
            self.preview = None;
            if let (Some(img), Some(picker)) = (self.thumbs.get(&key), &self.picker) {
                self.preview = Some((key.clone(), picker.new_resize_protocol(img.clone())));
            } else if self.loading.insert(key.clone()) {
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let img = session::load_rgb(&path).ok().map(|i| {
                        let h = 1400u32.min(i.height());
                        let w = (i.width() as u64 * h as u64 / i.height() as u64) as u32;
                        DynamicImage::ImageRgb8(image::imageops::resize(&i, w, h, image::imageops::FilterType::Triangle))
                    });
                    let _ = tx.send(Msg::Thumb(key, img));
                });
            }
        }
        match &mut self.preview {
            Some((_, proto)) => f.render_stateful_widget(StatefulImage::default(), inner, proto),
            None => f.render_widget(Paragraph::new(" loading…").style(Style::new().fg(Color::DarkGray)), inner),
        }
    }

    fn draw(&mut self, f: &mut ratatui::Frame) {
        let [head, body, prompt, keys] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(3),
            Constraint::Length(2),
        ])
        .areas(f.area());
        let [left, view] = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(body);
        let [pages, logs] = Layout::vertical([Constraint::Min(5), Constraint::Length(9)]).areas(left);
        self.draw_preview(f, view);

        let state_color = match self.state.as_str() {
            "Idle" => Color::Green,
            "Offline" => Color::Red,
            _ => Color::Yellow,
        };
        let header = Line::from(vec![
            Span::raw(format!(" {} ({}) ", self.scanner.name, self.scanner.host())),
            Span::styled(format!("● {}", self.state), Style::new().fg(state_color)),
            Span::raw("   "),
            Span::styled(format!("{} dpi", self.settings.dpi), Style::new().bold()),
            Span::raw(" · "),
            Span::styled(self.settings.mode.label(), Style::new().bold()),
            Span::raw(" · fill "),
            Span::styled(self.settings.fill.label(), Style::new().bold()),
            Span::raw(format!("   session: {}", self.session.name)),
            Span::raw(format!(
                "   ink {}   ⏱ {} open · {} scanning",
                self.ink,
                dur(self.started.elapsed().as_secs_f32()),
                dur(self.session.pages.iter().map(|p| p.secs).sum())
            )),
        ]);
        f.render_widget(Paragraph::new(header).block(Block::bordered().title(" scan ")), head);

        let items: Vec<ListItem> = self
            .session
            .pages
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let (w, h) = p.mm();
                let kind = match p.kind {
                    Kind::A4 => "A4  ",
                    Kind::Long => "long",
                };
                ListItem::new(format!("{:>3}  {kind}  {w}×{h} mm  {} dpi  {:>6}", i + 1, p.dpi, if p.secs > 0.0 { dur(p.secs) } else { String::new() }))
            })
            .collect();
        let list = List::new(items)
            .block(Block::bordered().title(format!(" Pages ({}) ", self.session.pages.len())))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol("▶ ");
        f.render_stateful_widget(list, pages, &mut self.list);

        let h = logs.height.saturating_sub(2) as usize;
        let lines: Vec<Line> = self.log.iter().rev().take(h).rev().map(|(c, s)| Line::styled(s.clone(), Style::new().fg(*c))).collect();
        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).block(Block::bordered().title(" Log ")), logs);

        let (text, style) = match &self.mode {
            Mode::Idle => ("Ready. Put a page face down in the top-left corner.".to_string(), Style::new()),
            Mode::Busy(what, t, _) => {
                let spin = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"][(t.elapsed().as_millis() / 100 % 10) as usize];
                (format!("{spin} {what}… {}s", t.elapsed().as_secs()), Style::new().fg(Color::Cyan))
            }
            Mode::Export { .. } => ("Choose outputs on the right, then Enter.".to_string(), Style::new().fg(Color::Cyan)),
            Mode::Flip { .. } => (
                "↻ Turn the page 180° (bottom edge into the same corner), then press Enter.  Esc cancel · q quit".to_string(),
                Style::new().fg(Color::Black).bg(Color::Yellow).bold(),
            ),
        };
        f.render_widget(Paragraph::new(text).style(style).block(Block::bordered()), prompt);

        let k = |key: &'static str, label: &'static str| {
            [Span::styled(key, Style::new().fg(Color::Cyan).bold()), Span::raw(format!(" {label}  "))]
        };
        let row1: Vec<Span> = [k("a/⏎", "A4 page"), k("l", "long page"), k("r", "retake"), k("x", "delete"), k("J/K", "move"), k("v", "view")]
            .into_iter()
            .flatten()
            .collect();
        let row2: Vec<Span> = [k("d", "dpi"), k("c", "colour"), k("f/F", "fill gaps / fix page"), k("e", "export"), k("p", "open PDF"), k("o", "open folder"), k("q", "export & quit"), k("Q", "save draft & quit")]
            .into_iter()
            .flatten()
            .collect();
        f.render_widget(Paragraph::new(vec![Line::from(row1), Line::from(row2)]), keys);
    }
}

/// Print PDF if exported, else share, else colour, else the first file.
fn best_pdf(files: &[PathBuf]) -> Option<PathBuf> {
    ["_print.pdf", "_share.pdf", "_color.pdf"]
        .iter()
        .find_map(|s| files.iter().find(|f| f.to_string_lossy().ends_with(s)))
        .or(files.first())
        .cloned()
}

fn job(tx: Sender<Msg>, f: impl FnOnce() -> Result<Msg> + Send + 'static) {
    std::thread::spawn(move || {
        let msg = f().unwrap_or_else(|e| Msg::Failed(format!("{e:#}")));
        let _ = tx.send(msg);
    });
}

fn open(path: &std::path::Path) {
    let _ = std::process::Command::new("open")
        .arg(path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

fn bell() {
    use std::io::Write;
    let _ = std::io::stdout().write_all(b"\x07");
    let _ = std::io::stdout().flush();
}
