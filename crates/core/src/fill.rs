//! Paint torn / cut-away areas (where the dark scanner lid shows through)
//! white or paper-coloured.
//!
//! Holes are dark regions connected to the image border. Thin ink strokes
//! touching the edge are excluded by eroding the dark mask before the flood
//! fill, so only wide areas count.
use image::{RgbImage, imageops};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum Fill {
    Off,
    White,
    Paper,
}

impl std::str::FromStr for Fill {
    type Err = String;
    fn from_str(s: &str) -> Result<Fill, String> {
        match s {
            "off" => Ok(Fill::Off),
            "white" => Ok(Fill::White),
            "paper" => Ok(Fill::Paper),
            _ => Err(format!("expected off, white or paper, got {s:?}")),
        }
    }
}

impl Fill {
    pub fn next(self) -> Fill {
        match self {
            Fill::Off => Fill::White,
            Fill::White => Fill::Paper,
            Fill::Paper => Fill::Off,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Fill::Off => "off",
            Fill::White => "white",
            Fill::Paper => "paper colour",
        }
    }
}

fn luma(p: [u8; 3]) -> u8 {
    ((p[0] as u32 * 77 + p[1] as u32 * 150 + p[2] as u32 * 29) >> 8) as u8
}

/// Returns the number of pixels painted.
pub fn fill_holes(img: &mut RgbImage, mode: Fill) -> usize {
    if mode == Fill::Off {
        return 0;
    }
    let (w, h) = (img.width(), img.height());
    let k = (w / 600).max(1);
    let (sw, sh) = (w / k, h / k);
    let small = imageops::resize(img, sw, sh, imageops::FilterType::Triangle);
    let lum: Vec<u8> = small.pixels().map(|p| luma(p.0)).collect();

    let mut sorted = lum.clone();
    sorted.sort_unstable();
    let paper = sorted[sorted.len() * 3 / 4] as f32; // most of the page is paper
    let dark_thr = (paper * 0.62) as u8;
    let edge_thr = (paper * 0.85) as u8;

    let (sw, sh) = (sw as usize, sh as usize);
    let dark: Vec<bool> = lum.iter().map(|&l| l < dark_thr).collect();
    let r = 3;
    let core = morph(&dark, sw, sh, r, false);
    let mut region = flood_from_border(&core, sw, sh);
    if region.iter().filter(|&&b| b).count() < sw * sh / 2000 {
        return 0;
    }
    region = morph(&region, sw, sh, r + 2, true);

    let color = match mode {
        Fill::Paper => paper_color(&small, &lum, paper),
        _ => [255, 255, 255],
    };
    let width = w as usize;
    let k = k as usize;
    img.par_chunks_mut(width * 3)
        .enumerate()
        .map(|(y, row)| {
            let sy = (y / k).min(sh - 1);
            let mut n = 0;
            for x in 0..width {
                let sx = (x / k).min(sw - 1);
                if !region[sy * sw + sx] {
                    continue;
                }
                let px = &mut row[x * 3..x * 3 + 3];
                if luma([px[0], px[1], px[2]]) < edge_thr {
                    px.copy_from_slice(&color);
                    n += 1;
                }
            }
            n
        })
        .sum()
}

/// Average colour of pixels near the paper brightness.
fn paper_color(small: &RgbImage, lum: &[u8], paper: f32) -> [u8; 3] {
    let (mut s, mut n) = ([0u64; 3], 0u64);
    for (p, &l) in small.pixels().zip(lum) {
        if (l as f32 - paper).abs() < 12.0 {
            (0..3).for_each(|i| s[i] += p[i] as u64);
            n += 1;
        }
    }
    if n == 0 { [255; 3] } else { s.map(|v| (v / n) as u8) }
}

/// Square erosion (dilate = false) or dilation (dilate = true), separable.
fn morph(m: &[bool], w: usize, h: usize, r: usize, dilate: bool) -> Vec<bool> {
    let pass = |src: &[bool], horizontal: bool| -> Vec<bool> {
        let mut out = vec![false; src.len()];
        for y in 0..h {
            for x in 0..w {
                let (lo, hi) = if horizontal { (x.saturating_sub(r), (x + r).min(w - 1)) } else { (y.saturating_sub(r), (y + r).min(h - 1)) };
                let at = |i: usize| if horizontal { src[y * w + i] } else { src[i * w + x] };
                out[y * w + x] = if dilate { (lo..=hi).any(at) } else { (lo..=hi).all(at) };
            }
        }
        out
    };
    pass(&pass(m, true), false)
}

fn flood_from_border(m: &[bool], w: usize, h: usize) -> Vec<bool> {
    let mut seen = vec![false; m.len()];
    let mut q = VecDeque::new();
    let push = |x: usize, y: usize, seen: &mut Vec<bool>, q: &mut VecDeque<(usize, usize)>| {
        let i = y * w + x;
        if m[i] && !seen[i] {
            seen[i] = true;
            q.push_back((x, y));
        }
    };
    for x in 0..w {
        push(x, 0, &mut seen, &mut q);
        push(x, h - 1, &mut seen, &mut q);
    }
    for y in 0..h {
        push(0, y, &mut seen, &mut q);
        push(w - 1, y, &mut seen, &mut q);
    }
    while let Some((x, y)) = q.pop_front() {
        if x > 0 { push(x - 1, y, &mut seen, &mut q); }
        if x + 1 < w { push(x + 1, y, &mut seen, &mut q); }
        if y > 0 { push(x, y - 1, &mut seen, &mut q); }
        if y + 1 < h { push(x, y + 1, &mut seen, &mut q); }
    }
    seen
}
