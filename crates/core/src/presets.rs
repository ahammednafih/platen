//! Named scan settings: a few built-ins plus the user's own, and the last-used settings.
use crate::capture::Settings;
use crate::escl::{ColorMode, SizeId};
use crate::fill::Fill;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Preset {
    pub name: String,
    pub settings: Settings,
    /// Built-ins can't be deleted or overwritten.
    #[serde(default)]
    pub builtin: bool,
}

fn dir() -> PathBuf {
    crate::device::config_dir()
}

fn write(file: &str, bytes: Vec<u8>) {
    let _ = std::fs::create_dir_all(dir());
    let _ = std::fs::write(dir().join(file), bytes);
}

fn builtin() -> Vec<Preset> {
    let p = |name: &str, dpi, mode, fill| Preset {
        name: name.into(),
        settings: Settings { dpi, mode, fill, size: SizeId::A4 },
        builtin: true,
    };
    vec![
        p("Document", 300, ColorMode::Color, Fill::White),
        p("Text", 300, ColorMode::BlackWhite, Fill::White),
        p("Photo", 600, ColorMode::Color, Fill::Off),
    ]
}

fn user() -> Vec<Preset> {
    std::fs::read(dir().join("presets.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Built-ins first, then the user's presets in the order they were saved.
pub fn all() -> Vec<Preset> {
    let mut v = builtin();
    v.extend(user());
    v
}

/// Save (or overwrite) a user preset. Returns the updated list.
pub fn save(name: &str, settings: Settings) -> anyhow::Result<Vec<Preset>> {
    let name = name.trim();
    anyhow::ensure!(!name.is_empty(), "Give the preset a name");
    anyhow::ensure!(!builtin().iter().any(|p| p.name.eq_ignore_ascii_case(name)), "\"{name}\" is a built-in preset");
    let mut list = user();
    list.retain(|p| !p.name.eq_ignore_ascii_case(name));
    list.push(Preset { name: name.into(), settings, builtin: false });
    write("presets.json", serde_json::to_vec_pretty(&list)?);
    Ok(all())
}

pub fn delete(name: &str) -> Vec<Preset> {
    let mut list = user();
    list.retain(|p| p.name != name);
    write("presets.json", serde_json::to_vec_pretty(&list).unwrap_or_default());
    all()
}

/// Settings from the last run, or the Document preset.
pub fn last() -> Settings {
    std::fs::read(dir().join("settings.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(builtin()[0].settings)
}

pub fn remember(s: &Settings) {
    write("settings.json", serde_json::to_vec(s).unwrap_or_default());
}
