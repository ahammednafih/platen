//! Finding the scanner (cached host, else mDNS) and where sessions live.
use crate::escl::Scanner;
use crate::session::Page;
use anyhow::{Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

const APP_DIR: &str = "Platen";
/// Folder name before the rename; moved over once so settings and presets survive.
const OLD_APP_DIR: &str = "scan";

/// Settings, presets and the cached device host.
pub fn config_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_default();
    let dir = base.join(APP_DIR);
    let old = base.join(OLD_APP_DIR);
    if !dir.exists() && old.is_dir() {
        let _ = std::fs::rename(&old, &dir);
    }
    dir
}

/// Rebuildable files: page previews and shrink drafts.
pub fn cache_dir() -> PathBuf {
    dirs::cache_dir().unwrap_or_default().join(APP_DIR)
}

/// Explicit host wins; else the cached one if it answers; else browse mDNS and cache it.
pub fn connect(host: Option<&str>) -> Result<Scanner> {
    if let Some(h) = host {
        return Ok(Scanner::at(h));
    }
    let cache = config_dir().join("host");
    if let Ok(c) = std::fs::read_to_string(&cache) {
        let mut lines = c.lines();
        let mut s = Scanner::at(lines.next().unwrap_or("").trim());
        if let Some(n) = lines.next() {
            s.name = n.to_string();
        }
        if s.state().is_ok() {
            return Ok(s);
        }
    }
    let s = Scanner::discover(Duration::from_secs(8)).context(
        "scanner not found. Is it on and on the same Wi-Fi? If macOS blocks it, allow this app in \
         System Settings → Privacy & Security → Local Network",
    )?;
    let _ = std::fs::create_dir_all(cache.parent().unwrap());
    let _ = std::fs::write(&cache, format!("{}\n{}", s.base, s.name));
    Ok(s)
}

/// Default parent folder for sessions.
pub fn default_root() -> PathBuf {
    expand("~/Documents/scans")
}

pub fn expand(p: &str) -> PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => dirs::home_dir().unwrap_or_default().join(rest),
        None => PathBuf::from(p),
    }
}

/// Session name from the local time, e.g. 2026-11-10_1432.
pub fn default_name() -> String {
    std::process::Command::new("date")
        .arg("+%Y-%m-%d_%H%M")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "session".into())
}

#[derive(Serialize, Debug)]
pub struct Draft {
    pub name: String,
    pub pages: usize,
    /// Seconds since the epoch.
    pub modified: u64,
}

/// Sessions under `root` that still have working scans, newest first.
pub fn drafts(root: &Path) -> Vec<Draft> {
    let mut out: Vec<Draft> = std::fs::read_dir(root)
        .map(|r| r.flatten().collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|e| {
            let json = e.path().join("session.json");
            let pages: Vec<Page> = serde_json::from_slice(&std::fs::read(&json).ok()?).unwrap_or_default();
            let modified = json.metadata().and_then(|m| m.modified()).ok()?;
            let modified = modified.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
            Some(Draft { name: e.file_name().to_string_lossy().into_owned(), pages: pages.len(), modified })
        })
        .collect();
    out.sort_by_key(|d| std::cmp::Reverse(d.modified));
    out
}

/// Delete a draft's working scans; keep anything exported, and drop the folder if it ends up empty.
pub fn delete_draft(root: &Path, name: &str) -> Result<()> {
    if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
        anyhow::bail!("bad draft name {name:?}");
    }
    let dir = root.join(name);
    if !dir.join("session.json").exists() {
        anyhow::bail!("{name} is not a draft");
    }
    for sub in ["pages", "raw", "thumbs", "text"] {
        let _ = std::fs::remove_dir_all(dir.join(sub));
    }
    std::fs::remove_file(dir.join("session.json"))?;
    let _ = std::fs::remove_dir(&dir); // only succeeds when empty
    Ok(())
}

/// An exported file in a session folder.
#[derive(Serialize, Debug)]
pub struct Saved {
    pub session: String,
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub modified: u64,
}

/// Exported PDFs and images under `root` (top level of each session folder), newest first.
pub fn library(root: &Path) -> Vec<Saved> {
    let mut out: Vec<Saved> = std::fs::read_dir(root)
        .map(|r| r.flatten().collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.path().is_dir())
        .flat_map(|dir| {
            let session = dir.file_name().to_string_lossy().into_owned();
            std::fs::read_dir(dir.path()).into_iter().flatten().flatten().filter_map(move |f| {
                let path = f.path();
                let ext = path.extension()?.to_str()?.to_ascii_lowercase();
                if !matches!(ext.as_str(), "pdf" | "jpg" | "jpeg" | "png") {
                    return None;
                }
                let meta = f.metadata().ok()?;
                let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                let name = f.file_name().to_string_lossy().into_owned();
                Some(Saved { session: session.clone(), name, path, bytes: meta.len(), modified })
            })
        })
        .collect();
    out.sort_by_key(|s| std::cmp::Reverse(s.modified));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{Kind, Session};

    #[test]
    fn drafts_lists_sessions_with_pages_and_skips_other_folders() {
        let root = std::env::temp_dir().join(format!("scan-drafts-{}", Session::new_stem()));
        let mut s = Session::open(root.join("letters")).unwrap();
        s.pages.push(Page { file: "a.jpg".into(), kind: Kind::A4, dpi: 300, w: 1, h: 1, secs: 0.0, color: None });
        s.save().unwrap();
        std::fs::create_dir_all(root.join("exported")).unwrap();

        let d = drafts(&root);

        assert_eq!(d.iter().map(|d| (d.name.as_str(), d.pages)).collect::<Vec<_>>(), [("letters", 1)]);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn delete_draft_keeps_exports() {
        let root = std::env::temp_dir().join(format!("scan-del-{}", Session::new_stem()));
        Session::open(root.join("empty")).unwrap().save().unwrap();
        let kept = Session::open(root.join("kept")).unwrap();
        std::fs::write(kept.dir.join("kept_color.pdf"), b"pdf").unwrap();
        std::fs::write(kept.dir.join("session.json"), b"[]").unwrap();

        delete_draft(&root, "empty").unwrap();
        delete_draft(&root, "kept").unwrap();

        assert!(!root.join("empty").exists());
        assert!(root.join("kept/kept_color.pdf").exists() && !root.join("kept/pages").exists());
        assert!(drafts(&root).is_empty());
        assert!(delete_draft(&root, "../x").is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn library_lists_exports_but_not_working_scans() {
        let root = std::env::temp_dir().join(format!("scan-library-{}", Session::new_stem()));
        let s = Session::open(root.join("tax")).unwrap();
        std::fs::write(s.dir.join("pages/1.jpg"), b"x").unwrap();
        std::fs::write(s.dir.join("tax_color.pdf"), b"pdf").unwrap();

        let l = library(&root);

        assert_eq!(l.iter().map(|s| (s.session.as_str(), s.name.as_str(), s.bytes)).collect::<Vec<_>>(), [("tax", "tax_color.pdf", 3)]);
        std::fs::remove_dir_all(root).unwrap();
    }
}
