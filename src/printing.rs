use crate::{pdf, renderer, workflows};
use anyhow::{Result, ensure};
use image::{Rgb, RgbImage};
use lopdf::Document;
use std::path::Path;
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Paper {
    Letter,
    A4,
    Legal,
}
impl Paper {
    pub fn label(self) -> &'static str {
        match self {
            Self::Letter => "Letter",
            Self::A4 => "A4",
            Self::Legal => "Legal",
        }
    }
    pub fn points(self) -> [f32; 2] {
        match self {
            Self::Letter => [612., 792.],
            Self::A4 => [595.276, 841.89],
            Self::Legal => [612., 1008.],
        }
    }
}
/// Two-up booklet sheets; backs require short-edge duplex printing.
pub fn booklet_order(pages: &[u32]) -> Vec<[Option<u32>; 2]> {
    let padded = pages.len().div_ceil(4) * 4;
    let get = |index: usize| pages.get(index).copied();
    let mut order = vec![];
    for sheet in 0..padded / 4 {
        let left = sheet * 2;
        let right = padded - 1 - sheet * 2;
        order.push([get(right), get(left)]);
        order.push([get(left + 1), get(right - 1)]);
    }
    order
}
pub fn booklet(
    source: &Document,
    pages: &[u32],
    paper: Paper,
    marks: bool,
    output: &Path,
) -> Result<()> {
    ensure!(
        !pages.is_empty() && pages.iter().all(|p| source.get_pages().contains_key(p)),
        "Choose valid booklet pages"
    );
    let temp = tempfile::tempdir()?;
    let input = temp.path().join("source.pdf");
    pdf::atomic_save(source, &input)?;
    let [height, width] = paper.points();
    let pixels = 1600;
    let rows = (pixels as f32 * height / width).round() as u32;
    let mut doc = Document::with_version("1.7");
    let mut order = vec![];
    for pair in booklet_order(pages) {
        let mut sheet = RgbImage::from_pixel(pixels, rows, Rgb([255; 3]));
        for (half, page) in pair.iter().enumerate() {
            if let Some(page) = page {
                let image =
                    image::DynamicImage::ImageRgba8(renderer::render_page(&input, *page, 1200)?)
                        .into_rgb8();
                let (w, h) = image.dimensions();
                let available_w = pixels / 2 - 48;
                let available_h = rows - 48;
                let scale = (available_w as f32 / w as f32).min(available_h as f32 / h as f32);
                let new_w = (w as f32 * scale).round().max(1.) as u32;
                let new_h = (h as f32 * scale).round().max(1.) as u32;
                let image = image::imageops::resize(
                    &image,
                    new_w,
                    new_h,
                    image::imageops::FilterType::Lanczos3,
                );
                let x = half as u32 * pixels / 2 + (pixels / 2 - new_w) / 2;
                let y = (rows - new_h) / 2;
                image::imageops::overlay(&mut sheet, &image, x as i64, y as i64);
            }
        }
        if marks {
            for y in 4..20 {
                sheet.put_pixel(pixels / 2, y, Rgb([0; 3]));
                sheet.put_pixel(pixels / 2, rows - 1 - y, Rgb([0; 3]));
            }
        }
        order.push(workflows::add_jpeg_page(
            &mut doc,
            sheet,
            [width, height],
            92,
        )?);
    }
    workflows::finish_pages(&mut doc, order)?;
    pdf::atomic_save(&doc, output)
}
