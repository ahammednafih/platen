mod app;

use platen_core::capture::Settings;
use platen_core::device::{self, connect, default_name, expand};
use platen_core::{escl, fill, ipp, pdf, session, stitch};

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

/// Scan pages (including longer-than-A4 ones) from an AirScan scanner.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Session name; pages and PDFs go to <dir>/<name>. Reopening resumes it.
    name: Option<String>,
    /// Parent folder for sessions.
    #[arg(long, default_value = "~/Documents/scans")]
    dir: String,
    /// Scanner address (IP or host[:port]); otherwise auto-discovered.
    #[arg(long)]
    host: Option<String>,
    /// Resolution: 100, 200, 300 or 600.
    #[arg(long, default_value_t = 600)]
    dpi: u32,
    /// Scan in grayscale instead of colour.
    #[arg(long)]
    gray: bool,
    /// Scan in pure black and white.
    #[arg(long, conflicts_with = "gray")]
    bw: bool,
    /// Join two existing scans offline: TOP BOTTOM OUT.jpg
    #[arg(long, num_args = 3, value_names = ["TOP", "BOTTOM", "OUT"])]
    stitch: Option<Vec<PathBuf>>,
    /// Paint torn/cut-away areas: off, white or paper.
    #[arg(long, default_value = "white")]
    fill: fill::Fill,
    /// Apply hole filling to an image offline: IN OUT.jpg
    #[arg(long, num_args = 2, value_names = ["IN", "OUT"])]
    fill_file: Option<Vec<PathBuf>>,
    /// Show printer status and ink levels.
    #[arg(long)]
    status: bool,
    /// Print PDFs and images (pages, in order).
    #[arg(long, num_args = 1.., value_name = "FILE")]
    print: Option<Vec<PathBuf>>,
    /// Copies when printing.
    #[arg(long, default_value_t = 1)]
    copies: u32,
    /// Shrink any PDF to fit under a size limit: IN OUT MB
    #[arg(long, num_args = 3, value_names = ["IN", "OUT", "MB"])]
    shrink: Option<Vec<String>>,
    /// List saved drafts (sessions that can be resumed).
    #[arg(long)]
    list: bool,
    /// Scan once to a JPEG file without the TUI (full glass area).
    #[arg(long, value_name = "OUT")]
    scan_to: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mode = color_mode(&cli);
    if let Some(files) = cli.stitch {
        return stitch_files(&files[0], &files[1], &files[2]);
    }

    if let Some(out) = cli.scan_to {
        let s = connect_noisy(cli.host.as_deref())?;
        let t = std::time::Instant::now();
        let opts = escl::ScanOpts { dpi: cli.dpi, mode, width: escl::GLASS_W, height: escl::GLASS_H };
        let data = s.scan(opts)?;
        std::fs::write(&out, &data)?;
        let (w, h, _) = pdf::jpeg_info(&data).context("bad JPEG")?;
        println!("{w}x{h} from {} in {:.1}s", s.host(), t.elapsed().as_secs_f32());
        return Ok(());
    }

    if let Some(a) = cli.shrink {
        let t = std::time::Instant::now();
        let mb: f32 = a[2].parse().context("MB must be a number, e.g. 1 or 0.5")?;
        let input = std::fs::read(&a[0])?;
        let r = platen_core::pdfedit::shrink(&input, (mb * 1_000_000.0) as u64)?;
        std::fs::write(&a[1], &r.data)?;
        println!(
            "{:.2} MB → {:.2} MB ({}; images at {:.0}% size, quality {}) in {:.1}s",
            input.len() as f32 / 1e6,
            r.data.len() as f32 / 1e6,
            if r.fits { "fits" } else { "still over the limit" },
            r.scale * 100.0,
            r.quality,
            t.elapsed().as_secs_f32()
        );
        return Ok(());
    }
    if let Some(f) = cli.fill_file {
        let t = std::time::Instant::now();
        let mut img = session::load_rgb(&f[0])?;
        let n = fill::fill_holes(&mut img, cli.fill);
        std::fs::write(&f[1], session::encode_rgb(&img, 92)?)?;
        println!("filled {n} px in {:.1}s", t.elapsed().as_secs_f32());
        return Ok(());
    }
    if cli.status || cli.print.is_some() {
        let s = connect_noisy(cli.host.as_deref())?;
        let printer = ipp::Printer::for_host(s.host());
        if let Some(files) = cli.print {
            let opts = ipp::PrintOpts { copies: cli.copies, color: !cli.gray, ..Default::default() };
            let job = platen_core::printjob::PrintJob {
                files,
                ranges: String::new(),
                subset: Default::default(),
                reverse: false,
                nup: 1,
                scale: Default::default(),
                collate: true,
                opts,
            };
            let jobs = job.run(&printer, "scan", |d, n| eprintln!("sent {d}/{n}"))?;
            println!("sent {} job(s): {jobs:?}", jobs.len());
            return Ok(());
        }
        let st = printer.status()?;
        println!("{} — {}{}", st.model, st.state, if st.reasons.is_empty() { String::new() } else { format!(" ({})", st.reasons.join(", ")) });
        for i in st.ink {
            println!("  {:<8} {:>3}%", i.name, i.level);
        }
        if let Ok(c) = printer.caps() {
            println!("paper: {}\ncolour: {}, quality: {:?}, copies up to {}, two-sided: {}", c.media.join(" "), c.color, c.qualities, c.max_copies, c.duplex);
        }
        return Ok(());
    }
    if cli.list {
        return list_drafts(&expand(&cli.dir));
    }

    let name = cli.name.unwrap_or_else(default_name);
    let dir = expand(&cli.dir).join(&name);
    let session = session::Session::open(dir)?;
    let scanner = connect_noisy(cli.host.as_deref())?;
    let settings = Settings { dpi: cli.dpi, mode, fill: cli.fill, size: Default::default() };
    app::run(scanner, session, settings)
}

fn stitch_files(top: &PathBuf, bottom: &PathBuf, out: &PathBuf) -> Result<()> {
    let t = std::time::Instant::now();
    let r = stitch::stitch(&session::load_rgb(top)?, &session::load_rgb(bottom)?)?;
    std::fs::write(out, session::encode_rgb(&r.image, 92)?)?;
    println!(
        "score={:.3} dx={:.0} dy={:.0} angle={:.2}° out={}x{} in {:.1}s",
        r.score,
        r.dx,
        r.dy,
        r.angle_deg,
        r.image.width(),
        r.image.height(),
        t.elapsed().as_secs_f32()
    );
    Ok(())
}

fn list_drafts(root: &std::path::Path) -> Result<()> {
    let drafts = device::drafts(root);
    if drafts.is_empty() {
        println!("No drafts in {}", root.display());
        return Ok(());
    }
    for d in drafts {
        println!("  {:<28} {:>3} pages", d.name, d.pages);
    }
    println!("\nResume with:  scan <name>");
    Ok(())
}

fn connect_noisy(host: Option<&str>) -> Result<escl::Scanner> {
    eprintln!("Connecting to the scanner…");
    connect(host)
}

fn color_mode(cli: &Cli) -> escl::ColorMode {
    match (cli.gray, cli.bw) {
        (_, true) => escl::ColorMode::BlackWhite,
        (true, _) => escl::ColorMode::Gray,
        _ => escl::ColorMode::Color,
    }
}
