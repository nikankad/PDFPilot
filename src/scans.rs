use crate::{pdf, renderer, workflows};
use anyhow::{Result, ensure};
use image::{Rgb, RgbImage};
use lopdf::Document;
use std::path::Path;
#[derive(Clone)]
pub struct ScanOptions {
    pub deskew: bool,
    pub grayscale: bool,
    pub despeckle: bool,
    pub contrast: f32,
    pub brightness: i32,
    pub threshold: Option<u8>,
    pub width: u32,
    pub jpeg_quality: u8,
}
impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            deskew: true,
            grayscale: true,
            despeckle: true,
            contrast: 15.,
            brightness: 0,
            threshold: None,
            width: 1800,
            jpeg_quality: 85,
        }
    }
}
fn angle(image: &RgbImage) -> f32 {
    let small = image::imageops::resize(
        image,
        500,
        (image.height() as f32 * 500. / image.width() as f32).max(1.) as u32,
        image::imageops::FilterType::Triangle,
    );
    let (w, h) = small.dimensions();
    let points = small
        .enumerate_pixels()
        .filter(|(_, _, p)| (p[0] as u32 + p[1] as u32 + p[2] as u32) / 3 < 100)
        .map(|(x, y, _)| (x as f32 - w as f32 / 2., y as f32 - h as f32 / 2.))
        .collect::<Vec<_>>();
    if points.len() < 30 {
        return 0.;
    }
    let mut best = (0., 0u64);
    for step in -32..=32 {
        let angle = step as f32 * 0.25;
        let r = angle.to_radians();
        let mut rows = vec![0u32; (h + w + 4) as usize];
        for (x, y) in &points {
            let row = (x * r.sin() + y * r.cos() + (h + w) as f32 / 2.).round() as usize;
            if let Some(bin) = rows.get_mut(row) {
                *bin += 1;
            }
        }
        let score = rows.iter().map(|n| (*n as u64).pow(2)).sum();
        if score > best.1 {
            best = (angle, score);
        }
    }
    if best.0.abs() < 0.5 { 0. } else { best.0 }
}
fn rotate(image: &RgbImage, degrees: f32) -> RgbImage {
    let r = degrees.to_radians();
    let (s, c) = r.sin_cos();
    let (w, h) = image.dimensions();
    let nw = (w as f32 * c.abs() + h as f32 * s.abs()).ceil() as u32;
    let nh = (h as f32 * c.abs() + w as f32 * s.abs()).ceil() as u32;
    let mut out = RgbImage::from_pixel(nw, nh, Rgb([255; 3]));
    for (x, y, p) in out.enumerate_pixels_mut() {
        let dx = x as f32 - nw as f32 / 2.;
        let dy = y as f32 - nh as f32 / 2.;
        let sx = dx * c + dy * s + w as f32 / 2.;
        let sy = -dx * s + dy * c + h as f32 / 2.;
        if sx >= 0. && sy >= 0. && sx < w as f32 && sy < h as f32 {
            *p = *image.get_pixel(sx as u32, sy as u32);
        }
    }
    out
}
pub fn enhance(image: RgbImage, options: &ScanOptions) -> RgbImage {
    let mut image = image::DynamicImage::ImageRgb8(image);
    if options.grayscale {
        image = image.grayscale();
    }
    image = image
        .adjust_contrast(options.contrast)
        .brighten(options.brightness);
    let mut rgb = image.to_rgb8();
    if options.despeckle {
        let source = rgb.clone();
        let (w, h) = source.dimensions();
        for y in 1..h.saturating_sub(1) {
            for x in 1..w.saturating_sub(1) {
                let mut p = [0u8; 9];
                let mut value = [0u8; 3];
                for (channel, result) in value.iter_mut().enumerate() {
                    let mut n = 0;
                    for dy in y - 1..=y + 1 {
                        for dx in x - 1..=x + 1 {
                            p[n] = source.get_pixel(dx, dy)[channel];
                            n += 1;
                        }
                    }
                    p.sort_unstable();
                    *result = p[4];
                }
                rgb.put_pixel(x, y, Rgb(value));
            }
        }
    }
    if let Some(threshold) = options.threshold {
        for pixel in rgb.pixels_mut() {
            let mean = (pixel[0] as u32 + pixel[1] as u32 + pixel[2] as u32) / 3;
            *pixel = Rgb([if mean >= threshold as u32 { 255 } else { 0 }; 3]);
        }
    }
    if options.deskew {
        let a = angle(&rgb);
        if a != 0. {
            rgb = rotate(&rgb, a);
        }
    }
    rgb
}
pub fn optimized_copy(source: &Document, options: &ScanOptions, output: &Path) -> Result<()> {
    ensure!(
        (300..=4000).contains(&options.width) && (10..=100).contains(&options.jpeg_quality),
        "Invalid raster resolution or JPEG quality"
    );
    ensure!(
        options.contrast.is_finite()
            && (-100.0..=100.0).contains(&options.contrast)
            && (-255..=255).contains(&options.brightness),
        "Invalid enhancement settings"
    );
    let temp = tempfile::tempdir()?;
    let input = temp.path().join("source.pdf");
    pdf::atomic_save(source, &input)?;
    let mut doc = Document::with_version("1.7");
    let mut order = vec![];
    for (number, id) in source.get_pages() {
        let original = renderer::render_page(&input, number, options.width)?;
        let (w, h) = original.dimensions();
        let image = enhance(
            image::DynamicImage::ImageRgba8(original).into_rgb8(),
            options,
        );
        let size = pdf::geometry(source, id)?.display_size();
        let size = [
            size[0] * image.width() as f32 / w as f32,
            size[1] * image.height() as f32 / h as f32,
        ];
        order.push(workflows::add_jpeg_page(
            &mut doc,
            image,
            size,
            options.jpeg_quality,
        )?);
    }
    workflows::finish_pages(&mut doc, order)?;
    pdf::atomic_save(&doc, output)
}
