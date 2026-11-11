//! Page edits: rotate, straighten, crop away the lid, brightness/contrast.
use crate::capture::{Captured, black_and_white};
use crate::fill::{self, Fill};
use crate::session::{self, Page, Session};
use anyhow::{Context, Result, bail};
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
    /// Trim to the paper, found from the dark lid around it.
    Crop,
    /// Keep a rectangle, in fractions of the page (0..=1) from the top-left.
    CropTo { x: f32, y: f32, w: f32, h: f32 },
    Gray,
    BlackWhite,
    /// Remove small dark marks that stand alone (dust, toner specks, photocopy noise).
    /// Needs the page's dpi to know how small "small" is.
    Despeckle { dpi: u32 },
    /// Back to the colour image a gray or black-and-white page was made from.
    Colour,
    /// Same maths as CSS `brightness(b) contrast(c)`, so the UI preview matches.
    Levels { brightness: f32, contrast: f32 },
}

/// Apply `edit` to a page, writing a new file so thumbnail URLs never go stale.
pub fn edit_page(dir: &Path, page: &Page, edit: Edit) -> Result<Captured> {
    let pages = dir.join("pages");
    let tone = matches!(edit, Edit::Gray | Edit::BlackWhite);
    if edit == Edit::Colour {
        return back_to_colour(&pages, page);
    }
    // Gray and B&W always start from the colour original, so switching between them loses nothing.
    let from = if tone { page.color.as_deref().unwrap_or(&page.file) } else { &page.file };
    let src = image::open(pages.join(from))?;
    let was_gray = matches!(src, DynamicImage::ImageLuma8(_));
    let (img, note) = apply(src.to_rgb8(), edit)?;
    let data = if was_gray || tone { session::encode_gray(&imageops::grayscale(&img), 90)? } else { session::encode_rgb(&img, 92)? };
    let mut out = page.clone();
    out.file = format!("{}.jpg", Session::new_stem());
    (out.w, out.h) = img.dimensions();
    std::fs::write(pages.join(&out.file), data)?;
    out.color = match (&page.color, tone) {
        (_, true) => (!was_gray).then(|| from.to_string()),
        (Some(c), false) => mirror(&pages, c, edit),
        (None, false) => None,
    };
    Ok(Captured { page: out, note })
}

fn back_to_colour(pages: &Path, page: &Page) -> Result<Captured> {
    let file = page.color.clone().context("this page has no colour version")?;
    let (w, h) = image::image_dimensions(pages.join(&file))?;
    Ok(Captured { page: Page { file, w, h, color: None, ..page.clone() }, note: "back to colour".into() })
}

/// Repeat an edit on the kept colour original; if it can't be repeated, stop offering colour.
fn mirror(pages: &Path, color: &str, edit: Edit) -> Option<String> {
    let (img, _) = apply(image::open(pages.join(color)).ok()?.to_rgb8(), edit).ok()?;
    let file = format!("{}c.jpg", Session::new_stem());
    std::fs::write(pages.join(&file), session::encode_rgb(&img, 92).ok()?).ok()?;
    Some(file)
}

/// Where the paper is on a page, as fractions `(x, y, w, h)`; `None` when no dark lid shows.
pub fn paper_bounds(dir: &Path, page: &Page) -> Result<Option<(f32, f32, f32, f32)>> {
    let img = image::open(dir.join("pages").join(&page.file))?.to_rgb8();
    let (iw, ih) = (img.width() as f32, img.height() as f32);
    Ok(lid_bounds(&img).map(|(x, y, w, h)| (x as f32 / iw, y as f32 / ih, w as f32 / iw, h as f32 / ih)))
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
        Edit::CropTo { x, y, w, h } => {
            let (iw, ih) = img.dimensions();
            let px = |v: f32, full: u32| (v.clamp(0.0, 1.0) * full as f32).round() as u32;
            let (x, y) = (px(x, iw).min(iw - 1), px(y, ih).min(ih - 1));
            let (w, h) = (px(w, iw).min(iw - x), px(h, ih).min(ih - y));
            if w < 8 || h < 8 {
                bail!("that crop is too small");
            }
            (imageops::crop_imm(&img, x, y, w, h).to_image(), "cropped".into())
        }
        Edit::Despeckle { dpi } => {
            let mut img = img;
            let n = despeckle(&mut img, dpi);
            if n == 0 {
                bail!("no stray specks found");
            }
            (img, format!("removed {n} {}", if n == 1 { "speck" } else { "specks" }))
        }
        Edit::Colour => bail!("this page has no colour version"),
        Edit::Gray => (DynamicImage::ImageLuma8(imageops::grayscale(&img)).to_rgb8(), "made gray".into()),
        Edit::BlackWhite => (DynamicImage::ImageLuma8(crate::capture::black_and_white(&img)).to_rgb8(), "made black and white".into()),
        Edit::Levels { brightness, contrast } => (levels(img, brightness, contrast), "adjusted brightness and contrast".into()),
    })
}

/// A speck is at most this wide, and has nothing else dark within this distance. The second rule is
/// what keeps full stops, dots on letters and diacritics: those always sit close to other ink.
const SPECK_MM: f32 = 0.6;
const SPECK_ALONE_MM: f32 = 1.5;

/// Paint over small isolated dark marks with the paper around them; returns how many.
/// Marks are found on a copy reduced to at most 300 dpi, which keeps large scans quick.
pub fn despeckle(img: &mut RgbImage, dpi: u32) -> usize {
    let k = (dpi / 300).max(1);
    let (sw, sh) = (img.width() / k, img.height() / k);
    if sw < 32 || sh < 32 {
        return 0;
    }
    let small = imageops::grayscale(&imageops::resize(img, sw, sh, imageops::FilterType::Triangle));
    // Same split as the B&W conversion: at or below the Otsu level is ink.
    let ink_level = crate::capture::otsu(&small);
    let dark = |x: u32, y: u32| small.get_pixel(x, y)[0] <= ink_level;
    // Summed-area table of dark pixels, to count ink in a rectangle in constant time.
    let stride = (sw + 1) as usize;
    let mut sum = vec![0u32; stride * (sh + 1) as usize];
    for y in 0..sh {
        let mut row = 0;
        for x in 0..sw {
            row += dark(x, y) as u32;
            sum[(y + 1) as usize * stride + (x + 1) as usize] = sum[y as usize * stride + (x + 1) as usize] + row;
        }
    }
    let ink = |x0: u32, y0: u32, x1: u32, y1: u32| {
        let at = |x: u32, y: u32| sum[y as usize * stride + x as usize];
        at(x1, y1) + at(x0, y0) - at(x1, y0) - at(x0, y1)
    };
    let px = |mm: f32| ((dpi / k) as f32 / 25.4 * mm).ceil() as u32;
    let (max_side, alone) = (px(SPECK_MM), px(SPECK_ALONE_MM));
    let mut seen = vec![false; (sw * sh) as usize];
    let mut stack = Vec::new();
    let mut removed = 0;
    for y in 0..sh {
        for x in 0..sw {
            if seen[(y * sw + x) as usize] || !dark(x, y) {
                continue;
            }
            // Flood the mark, tracking its box and size.
            let (mut x0, mut y0, mut x1, mut y1, mut count) = (x, y, x, y, 0u32);
            seen[(y * sw + x) as usize] = true;
            stack.push((x, y));
            while let Some((cx, cy)) = stack.pop() {
                count += 1;
                (x0, y0, x1, y1) = (x0.min(cx), y0.min(cy), x1.max(cx), y1.max(cy));
                for (nx, ny) in [(cx.wrapping_sub(1), cy), (cx + 1, cy), (cx, cy.wrapping_sub(1)), (cx, cy + 1), (cx.wrapping_sub(1), cy.wrapping_sub(1)), (cx + 1, cy + 1), (cx.wrapping_sub(1), cy + 1), (cx + 1, cy.wrapping_sub(1))] {
                    if nx < sw && ny < sh && !seen[(ny * sw + nx) as usize] && dark(nx, ny) {
                        seen[(ny * sw + nx) as usize] = true;
                        stack.push((nx, ny));
                    }
                }
            }
            if x1 - x0 >= max_side || y1 - y0 >= max_side {
                continue;
            }
            let (ax0, ay0) = (x0.saturating_sub(alone), y0.saturating_sub(alone));
            let (ax1, ay1) = ((x1 + 1 + alone).min(sw), (y1 + 1 + alone).min(sh));
            if ink(ax0, ay0, ax1, ay1) > count {
                continue;
            }
            paint_over(img, (x0.saturating_sub(1) * k, y0.saturating_sub(1) * k), ((x1 + 2).min(sw) * k, (y1 + 2).min(sh) * k));
            removed += 1;
        }
    }
    removed
}

/// Fill a box with the average of the pixels just outside its corners (paper, since the mark is alone).
fn paint_over(img: &mut RgbImage, (x0, y0): (u32, u32), (x1, y1): (u32, u32)) {
    let (w, h) = img.dimensions();
    let corners = [(x0.saturating_sub(2), y0.saturating_sub(2)), ((x1 + 1).min(w - 1), y0.saturating_sub(2)), (x0.saturating_sub(2), (y1 + 1).min(h - 1)), ((x1 + 1).min(w - 1), (y1 + 1).min(h - 1))];
    let mut mean = [0u32; 3];
    for (x, y) in corners {
        let p = img.get_pixel(x, y);
        (0..3).for_each(|c| mean[c] += p[c] as u32);
    }
    let fill = Rgb(mean.map(|v| (v / 4) as u8));
    for y in y0..y1.min(h) {
        for x in x0..x1.min(w) {
            img.put_pixel(x, y, fill);
        }
    }
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

    #[test]
    fn despeckle_removes_lone_marks_but_keeps_dots_next_to_text() {
        let mut img = lines();
        let blob = |img: &mut RgbImage, x: u32, y: u32| (0..3).for_each(|d| (0..3).for_each(|e| img.put_pixel(x + d, y + e, Rgb([10, 10, 10]))));
        blob(&mut img, 500, 60); // alone in the top margin
        blob(&mut img, 40, 700); // alone in the left margin
        blob(&mut img, 905, 101); // a full stop right after a line of text
        assert_eq!(despeckle(&mut img, 300), 2);
        assert_eq!(*img.get_pixel(501, 61), WHITE);
        assert_eq!(*img.get_pixel(906, 102), Rgb([10, 10, 10]));
        assert_eq!(despeckle(&mut img, 300), 0);
    }

    #[test]
    fn a_black_and_white_page_goes_back_to_colour_with_its_later_edits() {
        let dir = std::env::temp_dir().join(format!("platen-tone-{}", Session::new_stem()));
        std::fs::create_dir_all(dir.join("pages")).unwrap();
        let img = RgbImage::from_fn(60, 40, |x, _| if x < 30 { Rgb([200, 40, 40]) } else { WHITE });
        std::fs::write(dir.join("pages/a.jpg"), session::encode_rgb(&img, 92).unwrap()).unwrap();
        let page = Page { file: "a.jpg".into(), kind: session::Kind::A4, dpi: 300, w: 60, h: 40, secs: 0.0, color: None };

        let bw = edit_page(&dir, &page, Edit::BlackWhite).unwrap().page;
        assert_eq!(bw.color.as_deref(), Some("a.jpg"));
        let turned = edit_page(&dir, &bw, Edit::RotateRight).unwrap().page;
        let back = edit_page(&dir, &turned, Edit::Colour).unwrap().page;

        assert_eq!((back.w, back.h, back.color), (40, 60, None));
        let red = image::open(dir.join("pages").join(&back.file)).unwrap().to_rgb8();
        let p = red.get_pixel(20, 10);
        assert!(p[0] > p[1] + 80, "colour survived the round trip: {p:?}");
        assert!(edit_page(&dir, &page, Edit::Colour).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

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
