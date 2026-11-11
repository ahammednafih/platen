//! Printing files: pick pages, lay them out on sheets, send one JPEG per sheet over IPP.
use crate::ipp::{PrintOpts, Printer};
use crate::render::{self, SrcPage};
use crate::sheet::{self, Paper, Scale, Subset};
use crate::session;
use anyhow::{Context, Result, bail};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Sheets are rendered at this resolution for printing.
pub const PRINT_DPI: u32 = 300;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrintJob {
    pub files: Vec<PathBuf>,
    /// 1-based page ranges, e.g. `1-3, 6`; empty = all.
    pub ranges: String,
    /// Which sheets to print, for two-sided by hand on a one-sided printer.
    pub subset: Subset,
    pub reverse: bool,
    /// Pages per sheet: 1, 2, 4, 6, 9 or 16.
    pub nup: u32,
    pub scale: Scale,
    /// With several copies: whole sets (1,2,3, 1,2,3) instead of 1,1, 2,2, 3,3.
    pub collate: bool,
    /// With `opts.color` off: whiten the paper and keep ink dark, instead of the printer's plain gray.
    #[serde(default)]
    pub clean: bool,
    pub opts: PrintOpts,
}

/// True when the image has real colour: enough pixels whose channels differ by more than JPEG noise.
fn has_colour(img: &image::RgbImage) -> bool {
    const NOISE: u8 = 18;
    let sampled = img.pixels().step_by(7);
    let total = sampled.clone().count().max(1);
    let tinted = sampled.filter(|p| p.0.iter().max().unwrap() - p.0.iter().min().unwrap() > NOISE).count();
    tinted * 1000 > total
}

/// What a job will print, before sending it.
#[derive(Debug, Serialize)]
pub struct Plan {
    /// Pages in the files.
    pub pages: usize,
    /// Pages chosen by the ranges.
    pub picked: usize,
    /// Sheets the picked pages make; `order` indexes into these.
    pub sheets: usize,
    /// Sheets to send, in order; `None` is a blank back for two-sided.
    pub order: Vec<Option<usize>>,
    #[serde(skip)]
    groups: Vec<Vec<SrcPage>>,
}

impl Plan {
    /// Paper sheets used, copies included.
    pub fn paper(&self, copies: u32) -> usize {
        self.order.len() * copies.max(1) as usize
    }
}

impl PrintJob {
    pub fn paper(&self) -> Paper {
        Paper::from_media(&self.opts.media).unwrap_or(Paper::A4)
    }

    pub fn plan(&self) -> Result<Plan> {
        if self.files.is_empty() {
            bail!("pick something to print");
        }
        let all = render::pages(&self.files)?;
        let picked: Vec<SrcPage> = sheet::pick(&self.ranges, all.len())?.into_iter().map(|i| all[i].clone()).collect();
        let (c, r) = sheet::grid(self.nup);
        let groups: Vec<Vec<SrcPage>> = picked.chunks((c * r) as usize).map(<[SrcPage]>::to_vec).collect();
        let order = sheet::order(groups.len(), self.subset, self.reverse);
        Ok(Plan { pages: all.len(), picked: picked.len(), sheets: groups.len(), order, groups })
    }

    /// One sheet as a JPEG (gray when printing in black and white).
    pub fn sheet(&self, plan: &Plan, sheet: Option<usize>, dpi: u32) -> Result<Vec<u8>> {
        self.render_sheet(plan, sheet, dpi).map(|(jpeg, _)| jpeg)
    }

    /// The sheet as a JPEG, and whether it should print in colour. A sheet with no colour on it is
    /// sent gray even when colour is selected, so the printer doesn't mix colour inks to make black.
    fn render_sheet(&self, plan: &Plan, sheet: Option<usize>, dpi: u32) -> Result<(Vec<u8>, bool)> {
        let pages = match sheet {
            Some(i) => plan.groups.get(i).context("no such sheet")?.iter().map(|p| render::load(p, dpi)).collect::<Result<Vec<_>>>()?,
            None => Vec::new(),
        };
        let img = sheet::compose(&pages, self.nup, self.scale, self.paper(), dpi);
        if self.opts.color && has_colour(&img) {
            return Ok((session::encode_rgb(&img, 90)?, true));
        }
        let mut gray = image::imageops::grayscale(&img);
        if self.clean && !self.opts.color {
            session::whiten(&mut gray);
        }
        Ok((session::encode_gray(&gray, 90)?, false))
    }

    /// Render and send every sheet; `progress(done, total)` runs after each job is accepted.
    pub fn run(&self, printer: &Printer, name: &str, progress: impl FnMut(usize, usize)) -> Result<Vec<i32>> {
        self.run_until(printer, name, &std::sync::atomic::AtomicBool::new(false), progress)
    }

    /// Like `run`, but stops sending sheets once `stop` is set.
    pub fn run_until(&self, printer: &Printer, name: &str, stop: &std::sync::atomic::AtomicBool, mut progress: impl FnMut(usize, usize)) -> Result<Vec<i32>> {
        let plan = self.plan()?;
        if plan.order.is_empty() {
            bail!("nothing to print with these settings");
        }
        let sheets: Vec<(Vec<u8>, bool)> = plan.order.par_iter().map(|s| self.render_sheet(&plan, *s, PRINT_DPI)).collect::<Result<_>>()?;
        let copies = self.opts.copies.max(1);
        // Fit leaves white margins; other scales must keep their size, so let the printer crop.
        let scaling = if self.scale == Scale::Fit { "fit" } else { "fill" };
        let opts = |color: bool, copies: u32| PrintOpts { copies, color, scaling: scaling.into(), ..self.opts.clone() };
        let sends: Vec<(&[u8], PrintOpts)> = if self.collate && copies > 1 && sheets.len() > 1 {
            (0..copies).flat_map(|_| sheets.iter().map(|(s, color)| (s.as_slice(), opts(*color, 1)))).collect()
        } else {
            sheets.iter().map(|(s, color)| (s.as_slice(), opts(*color, copies))).collect()
        };
        let mut jobs = Vec::new();
        for (i, (jpeg, opts)) in sends.iter().enumerate() {
            if stop.load(std::sync::atomic::Ordering::Relaxed) {
                bail!("stopped");
            }
            jobs.push(printer.print_jpeg(jpeg, opts, &format!("{name} {}", i + 1))?);
            progress(i + 1, sends.len());
        }
        Ok(jobs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gray_sheets_are_not_colour_but_a_coloured_stamp_is() {
        let mut img = image::RgbImage::from_fn(200, 200, |x, _| image::Rgb([(x % 256) as u8; 3]));
        assert!(!has_colour(&img));
        for y in 0..20 {
            for x in 0..20 {
                img.put_pixel(x, y, image::Rgb([30, 60, 200]));
            }
        }
        assert!(has_colour(&img));
    }
    use crate::ipp::Quality;

    #[test]
    fn plans_ranges_nup_and_two_sided_backs() {
        let dir = std::env::temp_dir().join(format!("printjob-{}", session::Session::new_stem()));
        std::fs::create_dir_all(&dir).unwrap();
        let files: Vec<PathBuf> = (0..5)
            .map(|i| {
                let f = dir.join(format!("{i}.jpg"));
                std::fs::write(&f, session::encode_rgb(&image::RgbImage::new(20, 30), 80).unwrap()).unwrap();
                f
            })
            .collect();
        let opts = PrintOpts { copies: 2, color: false, media: "iso_a4_210x297mm".into(), scaling: "fit".into(), quality: Quality::Normal };
        let job = PrintJob { files, ranges: "1-5".into(), subset: Subset::Even, reverse: false, nup: 2, scale: Scale::Fit, collate: true, clean: false, opts };

        let plan = job.plan().unwrap();
        let blank = job.sheet(&plan, None, 20).unwrap();
        std::fs::remove_dir_all(dir).unwrap();

        assert_eq!((plan.pages, plan.picked, plan.sheets), (5, 5, 3));
        assert_eq!(plan.order, [Some(1), None]);
        assert_eq!(plan.paper(2), 4);
        assert!(crate::pdf::jpeg_info(&blank).is_some());
    }
}
