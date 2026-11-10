mod app;
mod escl;
mod fill;
mod pdf;
mod session;
mod stitch;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use std::time::Duration;

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
    /// Join two existing scans offline: TOP BOTTOM OUT.jpg
    #[arg(long, num_args = 3, value_names = ["TOP", "BOTTOM", "OUT"])]
    stitch: Option<Vec<PathBuf>>,
    /// Paint torn/cut-away areas: off, white or paper.
    #[arg(long, value_enum, default_value_t = fill::Fill::White)]
    fill: fill::Fill,
    /// Apply hole filling to an image offline: IN OUT.jpg
    #[arg(long, num_args = 2, value_names = ["IN", "OUT"])]
    fill_file: Option<Vec<PathBuf>>,
    /// List saved drafts (sessions that can be resumed).
    #[arg(long)]
    list: bool,
    /// Scan once to a JPEG file without the TUI (full glass area).
    #[arg(long, value_name = "OUT")]
    scan_to: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Some(files) = cli.stitch {
        return stitch_files(&files[0], &files[1], &files[2]);
    }

    if let Some(out) = cli.scan_to {
        let s = connect(cli.host.as_deref())?;
        let t = std::time::Instant::now();
        let opts = escl::ScanOpts { dpi: cli.dpi, color: !cli.gray, width: escl::GLASS_W, height: escl::GLASS_H };
        let data = s.scan(opts)?;
        std::fs::write(&out, &data)?;
        let (w, h, _) = pdf::jpeg_info(&data).context("bad JPEG")?;
        println!("{w}x{h} from {} in {:.1}s", s.host(), t.elapsed().as_secs_f32());
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
    if cli.list {
        return list_drafts(&expand(&cli.dir));
    }

    let name = cli.name.unwrap_or_else(default_name);
    let dir = expand(&cli.dir).join(&name);
    let session = session::Session::open(dir)?;
    let scanner = connect(cli.host.as_deref())?;
    let settings = app::Settings { dpi: cli.dpi, color: !cli.gray, fill: cli.fill };
    app::run(scanner, session, settings)
}

fn connect(host: Option<&str>) -> Result<escl::Scanner> {
    let cache = dirs::config_dir().unwrap_or_default().join("scan").join("host");
    if let Some(h) = host {
        return Ok(escl::Scanner::at(h));
    }
    if let Ok(c) = std::fs::read_to_string(&cache) {
        let mut lines = c.lines();
        let mut s = escl::Scanner::at(lines.next().unwrap_or("").trim());
        if let Some(n) = lines.next() {
            s.name = n.to_string();
        }
        if s.state().is_ok() {
            return Ok(s);
        }
    }
    eprintln!("Looking for the scanner on Wi-Fi…");
    let s = escl::Scanner::discover(Duration::from_secs(8)).context(
        "scanner not found. Is it on and on the same Wi-Fi? If macOS blocks it, allow your terminal in \
         System Settings → Privacy & Security → Local Network",
    )?;
    let _ = std::fs::create_dir_all(cache.parent().unwrap());
    let _ = std::fs::write(&cache, format!("{}\n{}", s.base, s.name));
    Ok(s)
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
    let mut found = 0;
    let mut dirs: Vec<_> = std::fs::read_dir(root).map(|r| r.flatten().collect()).unwrap_or_default();
    dirs.sort_by_key(|e| std::cmp::Reverse(e.metadata().and_then(|m| m.modified()).ok()));
    for e in dirs {
        let Ok(b) = std::fs::read(e.path().join("session.json")) else { continue };
        let pages: Vec<session::Page> = serde_json::from_slice(&b).unwrap_or_default();
        println!("  {:<28} {:>3} pages", e.file_name().to_string_lossy(), pages.len());
        found += 1;
    }
    if found == 0 {
        println!("No drafts in {}", root.display());
    } else {
        println!("\nResume with:  scan <name>");
    }
    Ok(())
}

fn default_name() -> String {
    std::process::Command::new("date")
        .arg("+%Y-%m-%d_%H%M")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "session".into())
}

fn expand(p: &str) -> PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => dirs::home_dir().unwrap_or_default().join(rest),
        None => PathBuf::from(p),
    }
}
