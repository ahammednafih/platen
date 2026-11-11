//! Page edits: rotate, straighten, crop away the lid, brightness/contrast.
use crate::capture::{Captured, black_and_white};
use crate::fill::{self, Fill};
use crate::session::{self, Page, Session};
use anyhow::{Result, bail};
use image::{DynamicImage, GrayImage, Rgb, RgbImage, imageops};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum Edit {
    RotateLeft,
    RotateRight,
    Rotate180,
    Straighten,
    Crop,
    /// Same maths as CSS `brightness(b) contrast(c)`, so the UI preview matches.
    Levels { brightness: f32, contrast: f32 },
}

/// Apply `edit` to a page, writing a new file so thumbnail URLs never go stale.
pub fn edit_page(dir: &Path, page: &Page, edit: Edit) -> Result<Captured> {
    let src = image::open(dir.join("pages").join(&page.file))?;
    let gray = matches!(src, DynamicImage::ImageLuma8(_));
    let (img, note) = apply(src.to_rgb8(), edit)?;
    let data = if gray { session::encode_gray(&imageops::grayscale(&img), 90)? } else { session::encode_rgb(&img, 92)? };
    let mut page = page.clone();
    page.file = format!("{}.jpg", Session::new_stem());
    (page.w, page.h) = img.dimensions();
    std::fs::write(dir.join("pages").join(&page.file), data)?;
    Ok(Captured { page, note })
}

pub fn apply(img: RgbImage, edit: Edit) -> Result<(RgbImage, String)> {
    Ok(match edit {
        Edit::RotateLeft => (imageops::rotate270(&img), "rotated left".into()),
        Edit::RotateRight => (imageops::rotate90(&img), "rotated right".into()),
        Edit::Rotate180 => (imageops::rotate180(&img), "turned upside down".into()),
        Edit::Straighten => {
            let deg = skew(&img);
            if deg.abs() < 0.05 {
                bail!("already straight");
            }
            let bg = paper(&img);
            (rotate(&img, -deg, bg), format!("straightened by {:.1}°", -deg))
        }
        Edit::Crop => {
            let (x, y, w, h) = lid_bounds(&img).ok_or_else(|| anyhow::anyhow!("no dark border to crop"))?;
            (imageops::crop_imm(&img, x, y, w, h).to_image(), "cropped to the paper".into())
        }
        Edit::Levels { brightness, contrast } => (levels(img, brightness, contrast), "adjusted brightness and contrast".into()),
    })
}

fn levels(mut img: RgbImage, b: f32, c: f32) -> RgbImage {
    let lut: Vec<u8> = (0..256).map(|v| ((((v as f32 / 255.0) * b - 0.5) * c + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8).collect();
    img.pixels_mut().for_each(|p| p.0 = p.0.map(|v| lut[v as usize]));
    img
}

/// Copy tone: `density` -3..=3 darkens (+) or lightens (-) the midtones, keeping paper white;
/// `background` 0..=3 pushes tinted or grey paper to white, stronger at each step.
pub fn tone(mut img: RgbImage, density: i32, background: u8) -> RgbImage {
    let white = [1.0, 0.9, 0.82, 0.74][background.min(3) as usize];
    let gamma = 1.0 + 0.15 * density.clamp(-3, 3) as f32;
    let lut: Vec<u8> = (0..256).map(|v| ((v as f32 / 255.0 / white).min(1.0).powf(gamma) * 255.0).round() as u8).collect();
    img.pixels_mut().for_each(|p| p.0 = p.0.map(|v| lut[v as usize]));
    img
}

/// Downscaled copy (longest side ≤ `max`) and the factor back to full size.
fn small(img: &RgbImage, max: u32) -> (RgbImage, f32) {
    let k = (img.width().max(img.height()) as f32 / max as f32).max(1.0);
    let (w, h) = ((img.width() as f32 / k) as u32, (img.height() as f32 / k) as u32);
    (imageops::resize(img, w.max(1), h.max(1), imageops::FilterType::Triangle), k)
}

/// Clockwise skew of the text lines in degrees, from the projection profile of ink pixels.
pub fn skew(img: &RgbImage) -> f32 {
    let (mut s, _) = small(img, 1200);
    fill::fill_holes(&mut s, Fill::White); // the dark lid would swamp the text
    let bw: GrayImage = black_and_white(&s);
    let ink: Vec<(f32, f32)> = bw.enumerate_pixels().filter(|(_, _, p)| p[0] < 128).map(|(x, y, _)| (x as f32, y as f32)).collect();
    if ink.len() < 200 {
        return 0.0;
    }
    let score = |deg: f32| {
        let (sin, cos) = deg.to_radians().sin_cos();
        let mut rows = vec![0u32; (s.width() + s.height()) as usize * 2];
        let off = s.width() as f32;
        for &(x, y) in &ink {
            rows[(y * cos - x * sin + off) as usize] += 1;
        }
        rows.iter().map(|&n| (n as u64).pow(2)).sum::<u64>()
    };
    // Thick strokes score the same over a small range of angles; prefer the smallest.
    let best = |angles: Vec<f32>| {
        let scored: Vec<(u64, f32)> = angles.into_par_iter().map(|a| (score(a), a)).collect();
        scored.into_iter().max_by(|x, y| x.0.cmp(&y.0).then(y.1.abs().total_cmp(&x.1.abs()))).map_or(0.0, |(_, a)| a)
    };
    let coarse = best((-24..=24).map(|i| i as f32 * 0.25).collect());
    best((-15..=15).map(|i| coarse + i as f32 * 0.02).collect())
}

/// Rotate clockwise by `deg` about the centre, same size, corners painted `bg`.
pub fn rotate(img: &RgbImage, deg: f32, bg: Rgb<u8>) -> RgbImage {
    let (w, h) = img.dimensions();
    let (sin, cos) = deg.to_radians().sin_cos();
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let mut out = RgbImage::new(w, h);
    out.par_chunks_mut(w as usize * 3).enumerate().for_each(|(y, row)| {
        for x in 0..w as usize {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            let (sx, sy) = (dx * cos + dy * sin + cx - 0.5, -dx * sin + dy * cos + cy - 0.5);
            row[x * 3..x * 3 + 3].copy_from_slice(&bilinear(img, sx, sy).unwrap_or(bg).0);
        }
    });
    out
}

fn bilinear(img: &RgbImage, x: f32, y: f32) -> Option<Rgb<u8>> {
    if x < 0.0 || y < 0.0 || x > (img.width() - 1) as f32 || y > (img.height() - 1) as f32 {
        return None;
    }
    let (x0, y0) = (x as u32, y as u32);
    let (x1, y1) = ((x0 + 1).min(img.width() - 1), (y0 + 1).min(img.height() - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let (a, b, c, d) = (img.get_pixel(x0, y0), img.get_pixel(x1, y0), img.get_pixel(x0, y1), img.get_pixel(x1, y1));
    Some(Rgb(std::array::from_fn(|i| {
        let top = a[i] as f32 * (1.0 - fx) + b[i] as f32 * fx;
        let bot = c[i] as f32 * (1.0 - fx) + d[i] as f32 * fx;
        (top * (1.0 - fy) + bot * fy).round() as u8
    })))
}

/// Typical colour of the brightest third of the page.
fn paper(img: &RgbImage) -> Rgb<u8> {
    let (s, _) = small(img, 300);
    let mut px: Vec<_> = s.pixels().map(|p| (p.0.iter().map(|&v| v as u32).sum::<u32>(), p.0)).collect();
    px.sort_unstable_by_key(|&(l, _)| l);
    Rgb(px[px.len() * 5 / 6].1)
}

/// Bounds of the paper inside the dark lid: trim edge rows/columns that are almost all dark.
/// None when there is nothing worth trimming.
fn lid_bounds(img: &RgbImage) -> Option<(u32, u32, u32, u32)> {
    let (s, k) = small(img, 800);
    let g = imageops::grayscale(&s);
    let t = crate::capture::otsu(&g);
    let (w, h) = g.dimensions();
    let bright = |x: u32, y: u32| g.get_pixel(x, y)[0] > t;
    let row = |y: u32| (0..w).filter(|&x| bright(x, y)).count() as f32 / w as f32 > 0.1;
    let col = |x: u32| (0..h).filter(|&y| bright(x, y)).count() as f32 / h as f32 > 0.1;
    let top = (0..h).find(|&y| row(y))?;
    let bottom = (0..h).rev().find(|&y| row(y))? + 1;
    let left = (0..w).find(|&x| col(x))?;
    let right = (0..w).rev().find(|&x| col(x))? + 1;
    if (bottom - top) as f32 > h as f32 * 0.99 && (right - left) as f32 > w as f32 * 0.99 {
        return None;
    }
    let f = |v: u32| (v as f32 * k).round() as u32;
    let (x, y) = (f(left), f(top));
    Some((x, y, (f(right) - x).min(img.width() - x), (f(bottom) - y).min(img.height() - y)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: Rgb<u8> = Rgb([250, 248, 240]);

    fn lines() -> RgbImage {
        let mut img = RgbImage::from_pixel(1000, 1400, WHITE);
        for y in (100..1300).step_by(40) {
            for x in 100..900 {
                for dy in 0..6 {
                    img.put_pixel(x, y + dy, Rgb([20, 20, 20]));
                }
            }
        }
        img
    }

    #[test]
    fn straightens_a_skewed_page() {
        let skewed = rotate(&lines(), 2.0, WHITE);
        assert!((skew(&skewed) - 2.0).abs() < 0.1, "{}", skew(&skewed));
        let (fixed, _) = apply(skewed, Edit::Straighten).unwrap();
        assert!(skew(&fixed).abs() < 0.1);
        assert!(skew(&lines()).abs() < 0.05, "{}", skew(&lines()));
    }

    #[test]
    fn crops_the_dark_lid_around_the_paper() {
        let mut img = RgbImage::from_pixel(1200, 1600, Rgb([30, 30, 35]));
        imageops::replace(&mut img, &lines(), 50, 120);
        let (out, _) = apply(img, Edit::Crop).unwrap();
        let (w, h) = out.dimensions();
        assert!(w.abs_diff(1000) < 6 && h.abs_diff(1400) < 6, "{w}x{h}");
        assert!(apply(lines(), Edit::Crop).is_err());
    }

    #[test]
    fn rotation_swaps_dimensions_and_levels_match_css() {
        let (r, _) = apply(lines(), Edit::RotateRight).unwrap();
        assert_eq!(r.dimensions(), (1400, 1000));
        let img = RgbImage::from_pixel(1, 1, Rgb([100, 128, 200]));
        let (out, _) = apply(img, Edit::Levels { brightness: 1.2, contrast: 1.5 }).unwrap();
        // CSS: ((v/255 * 1.2) - 0.5) * 1.5 + 0.5
        assert_eq!(out.get_pixel(0, 0).0, [css(100), css(128), 255]);
    }

    fn css(v: u8) -> u8 {
        ((((v as f32 / 255.0) * 1.2 - 0.5) * 1.5 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8
    }
}
