//! Join a top scan and a 180°-rotated bottom scan of the same long page.
//!
//! Translation is found by normalised cross-correlation on a high-passed
//! grayscale pyramid (coarse search, then local refinement); a small rotation
//! comes from refining the left and right parts of the overlap separately.
use anyhow::{Result, bail};
use image::{GrayImage, RgbImage, imageops};
use rayon::prelude::*;

pub struct Stitched {
    pub image: RgbImage,
    pub score: f32,
    pub dx: f32,
    pub dy: f32,
    pub angle_deg: f32,
}

struct Plane {
    w: usize,
    h: usize,
    px: Vec<f32>,
}

impl Plane {
    /// Downscale to `w` wide, then subtract a local mean so paper tone and
    /// shading don't dominate the correlation.
    fn from(gray: &GrayImage, w: u32) -> Plane {
        let h = (gray.height() as u64 * w as u64 / gray.width() as u64) as u32;
        let small = imageops::resize(gray, w, h, imageops::FilterType::Triangle);
        let blur = imageops::blur(&small, (w as f32 / 120.0).max(1.5));
        let px = small
            .pixels()
            .zip(blur.pixels())
            .map(|(a, b)| a[0] as f32 - b[0] as f32)
            .collect();
        Plane { w: w as usize, h: h as usize, px }
    }

    /// NCC of `b` placed at (dx, dy) in `a`'s coordinates, over columns
    /// `x0..x1` of `b` (clipped to the overlap). `step` subsamples.
    fn ncc(&self, b: &Plane, dx: i64, dy: i64, x0: usize, x1: usize, step: usize) -> f32 {
        let bx0 = (x0 as i64).max(-dx).max(0) as usize;
        let bx1 = (x1 as i64).min(self.w as i64 - dx).min(b.w as i64);
        let by0 = (-dy).max(0) as usize;
        let by1 = (self.h as i64 - dy).min(b.h as i64);
        if bx1 <= bx0 as i64 + 8 || by1 <= by0 as i64 + 8 {
            return -1.0;
        }
        let (mut sa, mut sb, mut saa, mut sbb, mut sab, mut n) = (0f64, 0f64, 0f64, 0f64, 0f64, 0f64);
        for by in (by0..by1 as usize).step_by(step) {
            let ay = (by as i64 + dy) as usize;
            let arow = &self.px[ay * self.w..];
            let brow = &b.px[by * b.w..];
            for bx in (bx0..bx1 as usize).step_by(step) {
                let va = arow[(bx as i64 + dx) as usize] as f64;
                let vb = brow[bx] as f64;
                sa += va;
                sb += vb;
                saa += va * va;
                sbb += vb * vb;
                sab += va * vb;
                n += 1.0;
            }
        }
        let cov = sab - sa * sb / n;
        let var = (saa - sa * sa / n) * (sbb - sb * sb / n);
        if var <= 0.0 { -1.0 } else { (cov / var.sqrt()) as f32 }
    }

    /// Best (dx, dy) within a window around (cx, cy).
    fn search(&self, b: &Plane, cx: i64, cy: i64, rx: i64, ry: i64, cols: (usize, usize), step: usize) -> (i64, i64, f32) {
        (cy - ry..=cy + ry)
            .into_par_iter()
            .flat_map_iter(|dy| (cx - rx..=cx + rx).map(move |dx| (dx, dy)))
            .map(|(dx, dy)| (dx, dy, self.ncc(b, dx, dy, cols.0, cols.1, step)))
            .reduce(|| (0, 0, -2.0), |p, q| if q.2 > p.2 { q } else { p })
    }
}

pub fn stitch(top: &RgbImage, bottom_raw: &RgbImage) -> Result<Stitched> {
    let bottom = imageops::rotate180(bottom_raw);
    let (w, h) = (top.width() as usize, top.height() as usize);
    let ga = imageops::grayscale(top);
    let gb = imageops::grayscale(&bottom);

    // Coarse: ~160 px wide, bottom must sit below with ≥30% overlap.
    let cw = 160u32;
    let (a0, b0) = (Plane::from(&ga, cw), Plane::from(&gb, cw));
    let ry = (a0.h as f32 * 0.35) as i64;
    let rx = (a0.w as f32 * 0.10) as i64;
    let (mut dx, mut dy, _) = a0.search(&b0, 0, ry, rx, ry, (0, b0.w), 1);
    let mut scale = cw as f64;

    // Refine through finer levels.
    for lw in [640u32, 1600] {
        let lw = lw.min(w as u32);
        let (a, b) = (Plane::from(&ga, lw), Plane::from(&gb, lw));
        let k = lw as f64 / scale;
        let r = (k.ceil() as i64) + 2;
        let (x, y, _) = a.search(&b, (dx as f64 * k) as i64, (dy as f64 * k) as i64, r, r, (0, b.w), 1);
        dx = x;
        dy = y;
        scale = lw as f64;
    }

    // Rotation: refine left and right thirds separately at this level.
    let lw = scale as u32;
    let (a, b) = (Plane::from(&ga, lw), Plane::from(&gb, lw));
    let third = b.w / 3;
    let (_, yl, _) = a.search(&b, dx, dy, 1, 4, (0, third), 1);
    let (_, yr, _) = a.search(&b, dx, dy, 1, 4, (2 * third, b.w), 1);
    let score = a.ncc(&b, dx, dy, 0, b.w, 1);
    if score < 0.05 {
        bail!("couldn't line up the two halves (score {score:.2}); retake with more overlap");
    }

    let to_full = w as f64 / scale;
    let xl = third as f64 / 2.0 * to_full;
    let xr = (2.5 * third as f64) * to_full;
    let theta = (yr - yl) as f64 * to_full / (xr - xl);
    let dxf = dx as f64 * to_full;
    let dyf = dy as f64 * to_full;
    // dy measured at the centre column; express as translation at x = 0.
    let ty = dyf - (w as f64 / 2.0) * theta;
    let yc = (h as f64 - dyf) / 2.0;
    let tx = dxf + yc * theta;

    let image = compose(top, &bottom, tx, ty, theta);
    Ok(Stitched {
        image,
        score,
        dx: dxf as f32,
        dy: dyf as f32,
        angle_deg: theta.to_degrees() as f32,
    })
}

fn compose(a: &RgbImage, b: &RgbImage, tx: f64, ty: f64, theta: f64) -> RgbImage {
    let (w, ha) = (a.width() as usize, a.height() as usize);
    let (bw, bh) = (b.width() as f64, b.height() as f64);
    let hh = (ty + bh).max(ha as f64).ceil() as usize;
    let seam = ((ty.max(0.0) + ha as f64) / 2.0) as i64;
    let feather = 60i64;
    let (s, c) = theta.sin_cos();

    let mut out = vec![255u8; w * hh * 3];
    out.par_chunks_mut(w * 3).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let pa = (y < ha).then(|| a.get_pixel(x as u32, y as u32).0);
            let alpha = ((seam + feather - y as i64) as f64 / (2 * feather) as f64).clamp(0.0, 1.0);
            let pb = if alpha < 1.0 {
                let (xx, yy) = (x as f64 - tx, y as f64 - ty);
                sample(b, xx * c + yy * s, -xx * s + yy * c, bw, bh)
            } else {
                None
            };
            let px = match (pa, pb) {
                (Some(p), Some(q)) => [0, 1, 2].map(|i| (p[i] as f64 * alpha + q[i] as f64 * (1.0 - alpha)) as u8),
                (Some(p), None) => p,
                (None, Some(q)) => q,
                (None, None) => [255; 3],
            };
            row[x * 3..x * 3 + 3].copy_from_slice(&px);
        }
    });
    let img = RgbImage::from_raw(w as u32, hh as u32, out).expect("buffer size");

    // Trim the side strip the shifted bottom half doesn't cover.
    let pad = (tx.abs() + theta.abs() * hh as f64).ceil() as u32 + 8;
    let x0 = if tx > 0.0 { pad } else { 0 };
    imageops::crop_imm(&img, x0, 0, w as u32 - pad, hh as u32).to_image()
}

fn sample(b: &RgbImage, x: f64, y: f64, bw: f64, bh: f64) -> Option<[u8; 3]> {
    if x < 0.0 || y < 0.0 || x >= bw - 1.0 || y >= bh - 1.0 {
        return None;
    }
    let (x0, y0) = (x as u32, y as u32);
    let (fx, fy) = (x - x0 as f64, y - y0 as f64);
    let p = |xx, yy| b.get_pixel(xx, yy).0;
    let (p00, p10, p01, p11) = (p(x0, y0), p(x0 + 1, y0), p(x0, y0 + 1), p(x0 + 1, y0 + 1));
    Some([0, 1, 2].map(|i| {
        let top = p00[i] as f64 * (1.0 - fx) + p10[i] as f64 * fx;
        let bot = p01[i] as f64 * (1.0 - fx) + p11[i] as f64 * fx;
        (top * (1.0 - fy) + bot * fy).round() as u8
    }))
}
