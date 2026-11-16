//! Records and enums the phone apps see, converted from the core's types.
use platen_core::{adjust, capture, copy, device, escl, fill, ipp, maintenance, printjob, session, sheet, webui};
use std::path::PathBuf;

macro_rules! mirror_enum {
    ($name:ident, $core:path, [$($v:ident),+]) => {
        #[derive(Clone, Copy, PartialEq, Eq, Debug, uniffi::Enum)]
        pub enum $name { $($v),+ }

        impl From<$core> for $name {
            fn from(c: $core) -> Self {
                use $core as C;
                match c { $(C::$v => $name::$v),+ }
            }
        }

        impl From<$name> for $core {
            fn from(m: $name) -> Self {
                use $core as C;
                match m { $($name::$v => C::$v),+ }
            }
        }
    };
}

mirror_enum!(ColorMode, escl::ColorMode, [Color, Gray, BlackWhite]);
mirror_enum!(Fill, fill::Fill, [Off, White, Paper]);
mirror_enum!(Kind, session::Kind, [A4, Long]);
mirror_enum!(Quality, ipp::Quality, [Draft, Normal, High]);
mirror_enum!(Subset, sheet::Subset, [All, Odd, Even]);
mirror_enum!(Inks, maintenance::Inks, [Black, Colour, All]);
mirror_enum!(Strength, maintenance::Strength, [Normal, Strong, Strongest, Special]);
mirror_enum!(
    SizeId,
    escl::SizeId,
    [A4, Letter, B5, Executive, A5, HalfLetter, Photo148x200, Photo5x8, A6, Photo5x7, Photo4x6, Photo3_5x5, Card, Glass]
);

#[derive(uniffi::Record)]
pub struct Device {
    pub name: String,
    pub host: String,
}

#[derive(uniffi::Record)]
pub struct Ink {
    pub name: String,
    pub level: i32,
}

#[derive(uniffi::Record)]
pub struct PrinterStatus {
    pub model: String,
    pub state: String,
    pub reasons: Vec<String>,
    pub ink: Vec<Ink>,
}

impl From<ipp::Ink> for Ink {
    fn from(i: ipp::Ink) -> Self {
        Ink { name: i.name, level: i.level }
    }
}

impl From<ipp::Status> for PrinterStatus {
    fn from(s: ipp::Status) -> Self {
        PrinterStatus { model: s.model, state: s.state, reasons: s.reasons, ink: s.ink.into_iter().map(Into::into).collect() }
    }
}

#[derive(uniffi::Record)]
pub struct DeviceStatus {
    pub scanner: String,
    pub printer: Option<PrinterStatus>,
}

#[derive(uniffi::Record)]
pub struct PaperSize {
    pub id: SizeId,
    pub name: String,
    pub w_mm: f32,
    pub h_mm: f32,
}

impl From<escl::Size> for PaperSize {
    fn from(s: escl::Size) -> Self {
        PaperSize { id: s.id.into(), name: s.name.into(), w_mm: s.w_mm, h_mm: s.h_mm }
    }
}

#[derive(uniffi::Record)]
pub struct Caps {
    pub resolutions: Vec<u32>,
    pub modes: Vec<ColorMode>,
    pub sizes: Vec<PaperSize>,
}

impl From<escl::Caps> for Caps {
    fn from(c: escl::Caps) -> Self {
        Caps {
            resolutions: c.resolutions,
            modes: c.modes.into_iter().map(Into::into).collect(),
            sizes: c.sizes.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Copy, uniffi::Record)]
pub struct Settings {
    pub dpi: u32,
    pub mode: ColorMode,
    pub fill: Fill,
    pub size: SizeId,
}

impl From<capture::Settings> for Settings {
    fn from(s: capture::Settings) -> Self {
        Settings { dpi: s.dpi, mode: s.mode.into(), fill: s.fill.into(), size: s.size.into() }
    }
}

impl From<Settings> for capture::Settings {
    fn from(s: Settings) -> Self {
        capture::Settings { dpi: s.dpi, mode: s.mode.into(), fill: s.fill.into(), size: s.size.into() }
    }
}

#[derive(uniffi::Record)]
pub struct Preset {
    pub name: String,
    pub settings: Settings,
    pub builtin: bool,
}

pub fn presets(p: Vec<platen_core::presets::Preset>) -> Vec<Preset> {
    p.into_iter().map(|p| Preset { name: p.name, settings: p.settings.into(), builtin: p.builtin }).collect()
}

#[derive(Clone, uniffi::Record)]
pub struct Page {
    pub file: String,
    pub path: String,
    pub kind: Kind,
    pub dpi: u32,
    pub w: u32,
    pub h: u32,
    pub secs: f32,
    pub color: Option<String>,
}

#[derive(uniffi::Record)]
pub struct SessionView {
    pub name: String,
    pub dir: String,
    pub pages: Vec<Page>,
    pub can_undo: bool,
    pub can_redo: bool,
}

impl SessionView {
    pub fn of(s: &session::Session, (can_undo, can_redo): (bool, bool)) -> Self {
        let pages = s
            .pages
            .iter()
            .map(|p| Page {
                file: p.file.clone(),
                path: s.page_path(p).to_string_lossy().into_owned(),
                kind: p.kind.into(),
                dpi: p.dpi,
                w: p.w,
                h: p.h,
                secs: p.secs,
                color: p.color.clone(),
            })
            .collect();
        SessionView { name: s.name.clone(), dir: s.dir.to_string_lossy().into_owned(), pages, can_undo, can_redo }
    }
}

#[derive(uniffi::Record)]
pub struct PageResult {
    pub session: SessionView,
    pub index: u32,
    pub note: String,
}

#[derive(Clone, uniffi::Record)]
pub struct TopHalf {
    pub path: String,
    pub stem: String,
    pub secs: f32,
}

impl From<capture::TopHalf> for TopHalf {
    fn from(t: capture::TopHalf) -> Self {
        TopHalf { path: t.path.to_string_lossy().into_owned(), stem: t.stem, secs: t.secs }
    }
}

impl From<TopHalf> for capture::TopHalf {
    fn from(t: TopHalf) -> Self {
        capture::TopHalf { path: PathBuf::from(t.path), stem: t.stem, secs: t.secs }
    }
}

#[derive(uniffi::Record)]
pub struct Draft {
    pub name: String,
    pub pages: u32,
    pub modified: u64,
}

impl From<device::Draft> for Draft {
    fn from(d: device::Draft) -> Self {
        Draft { name: d.name, pages: d.pages as u32, modified: d.modified }
    }
}

#[derive(Clone, Copy, uniffi::Record)]
pub struct ExportOpts {
    pub color: bool,
    pub share: bool,
    pub print: bool,
    pub images: bool,
    pub cleanup: bool,
    pub max_mb: Option<f32>,
}

impl From<session::ExportOpts> for ExportOpts {
    fn from(o: session::ExportOpts) -> Self {
        ExportOpts { color: o.color, share: o.share, print: o.print, images: o.images, cleanup: o.cleanup, max_mb: o.max_mb }
    }
}

impl ExportOpts {
    pub fn into_core(self, saved: session::ExportOpts) -> session::ExportOpts {
        session::ExportOpts {
            color: self.color,
            share: self.share,
            print: self.print,
            images: self.images,
            cleanup: self.cleanup,
            max_mb: self.max_mb,
            searchable: false,
            ..saved
        }
    }
}

#[derive(uniffi::Record)]
pub struct Exported {
    pub files: Vec<String>,
    pub closed: bool,
}

#[derive(uniffi::Record)]
pub struct ShrinkResult {
    pub out: String,
    pub before: u64,
    pub after: u64,
    pub fits: bool,
    pub pages: u32,
}

#[derive(Clone, Copy, uniffi::Enum)]
pub enum Edit {
    RotateLeft,
    RotateRight,
    Rotate180,
    Straighten,
    Crop,
    CropTo { x: f32, y: f32, w: f32, h: f32 },
    Gray,
    BlackWhite,
    Despeckle { dpi: u32 },
    Colour,
    Levels { brightness: f32, contrast: f32 },
}

impl From<Edit> for adjust::Edit {
    fn from(e: Edit) -> Self {
        use adjust::Edit as E;
        match e {
            Edit::RotateLeft => E::RotateLeft,
            Edit::RotateRight => E::RotateRight,
            Edit::Rotate180 => E::Rotate180,
            Edit::Straighten => E::Straighten,
            Edit::Crop => E::Crop,
            Edit::CropTo { x, y, w, h } => E::CropTo { x, y, w, h },
            Edit::Gray => E::Gray,
            Edit::BlackWhite => E::BlackWhite,
            Edit::Despeckle { dpi } => E::Despeckle { dpi },
            Edit::Colour => E::Colour,
            Edit::Levels { brightness, contrast } => E::Levels { brightness, contrast },
        }
    }
}

#[derive(uniffi::Record)]
pub struct Bounds {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Copy, uniffi::Enum)]
pub enum Scale {
    Fit,
    Actual,
    Percent { percent: u32 },
}

impl From<Scale> for sheet::Scale {
    fn from(s: Scale) -> Self {
        match s {
            Scale::Fit => sheet::Scale::Fit,
            Scale::Actual => sheet::Scale::Actual,
            Scale::Percent { percent } => sheet::Scale::Percent(percent),
        }
    }
}

#[derive(Clone, Copy, uniffi::Enum)]
pub enum Area {
    Size { size: SizeId },
    HalfTop,
}

impl From<Area> for copy::Area {
    fn from(a: Area) -> Self {
        match a {
            Area::Size { size } => copy::Area::Size(size.into()),
            Area::HalfTop => copy::Area::HalfTop,
        }
    }
}

#[derive(uniffi::Record)]
pub struct PrintCaps {
    pub media: Vec<String>,
    pub media_default: String,
    pub color: bool,
    pub qualities: Vec<Quality>,
    pub max_copies: i32,
    pub duplex: bool,
    pub media_types: Vec<String>,
    pub borderless: bool,
}

impl From<ipp::PrintCaps> for PrintCaps {
    fn from(c: ipp::PrintCaps) -> Self {
        PrintCaps {
            media: c.media,
            media_default: c.media_default,
            color: c.color,
            qualities: c.qualities.into_iter().map(Into::into).collect(),
            max_copies: c.max_copies,
            duplex: c.duplex,
            media_types: c.media_types,
            borderless: c.borderless,
        }
    }
}

#[derive(Clone, uniffi::Record)]
pub struct PrintOpts {
    pub copies: u32,
    pub color: bool,
    pub media: String,
    pub scaling: String,
    pub quality: Quality,
    pub media_type: Option<String>,
    pub borderless: bool,
}

#[derive(Clone, uniffi::Record)]
pub struct PrintJob {
    pub files: Vec<String>,
    pub ranges: String,
    pub subset: Subset,
    pub reverse: bool,
    pub nup: u32,
    pub scale: Scale,
    pub collate: bool,
    pub clean: bool,
    pub opts: PrintOpts,
}

impl From<PrintJob> for printjob::PrintJob {
    fn from(j: PrintJob) -> Self {
        printjob::PrintJob {
            files: j.files.into_iter().map(PathBuf::from).collect(),
            ranges: j.ranges,
            subset: j.subset.into(),
            reverse: j.reverse,
            nup: j.nup,
            scale: j.scale.into(),
            collate: j.collate,
            clean: j.clean,
            opts: ipp::PrintOpts {
                copies: j.opts.copies,
                color: j.opts.color,
                media: j.opts.media,
                scaling: j.opts.scaling,
                quality: j.opts.quality.into(),
                media_type: j.opts.media_type,
                borderless: j.opts.borderless,
            },
        }
    }
}

#[derive(uniffi::Record)]
pub struct Plan {
    pub pages: u32,
    pub picked: u32,
    pub sheets: u32,
    pub order: Vec<Option<u32>>,
}

impl From<&printjob::Plan> for Plan {
    fn from(p: &printjob::Plan) -> Self {
        Plan { pages: p.pages as u32, picked: p.picked as u32, sheets: p.sheets as u32, order: p.order.iter().map(|o| o.map(|i| i as u32)).collect() }
    }
}

#[derive(uniffi::Record)]
pub struct DetailRow {
    pub label: String,
    pub value: String,
    pub sub: bool,
}

#[derive(uniffi::Record)]
pub struct DetailGroup {
    pub title: String,
    pub rows: Vec<DetailRow>,
}

impl From<webui::Group> for DetailGroup {
    fn from(g: webui::Group) -> Self {
        DetailGroup { title: g.title, rows: g.rows.into_iter().map(|r| DetailRow { label: r.label, value: r.value, sub: r.sub }).collect() }
    }
}

#[derive(uniffi::Record)]
pub struct RenderedPage {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}
