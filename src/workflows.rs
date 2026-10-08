use crate::{pdf, renderer};
use anyhow::{Context, Result, ensure};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, dictionary};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Debug)]
pub struct Redaction {
    pub page: u32,
    pub start: [f32; 2],
    pub end: [f32; 2],
}

/// Reconstruct every page from pixels in a brand new document. No original
/// objects, metadata, attachments, form values, or hidden text are copied.
pub fn redacted_copy(source: &Document, redactions: &[Redaction], output: &Path) -> Result<()> {
    ensure!(!redactions.is_empty(), "Mark at least one region to redact");
    let pages = source.get_pages();
    for region in redactions {
        ensure!(
            pages.contains_key(&region.page),
            "Redaction page is out of range"
        );
        ensure!(
            region
                .start
                .iter()
                .chain(region.end.iter())
                .all(|x| x.is_finite() && (0.0..=1.0).contains(x)),
            "Invalid redaction coordinates"
        );
    }
    let temp = tempfile::tempdir()?;
    let input = temp.path().join("source.pdf");
    pdf::atomic_save(source, &input)?;
    let mut result = Document::with_version("1.7");
    let mut order = vec![];
    for (number, id) in pages {
        let mut image =
            image::DynamicImage::ImageRgba8(renderer::render_page(&input, number, 2000)?)
                .into_rgb8();
        for region in redactions.iter().filter(|r| r.page == number) {
            let w = image.width() as f32;
            let h = image.height() as f32;
            let x0 = (region.start[0].min(region.end[0]) * w).floor() as u32;
            let y0 = (region.start[1].min(region.end[1]) * h).floor() as u32;
            let x1 = ((region.start[0].max(region.end[0]) * w).ceil() as u32).min(image.width());
            let y1 = ((region.start[1].max(region.end[1]) * h).ceil() as u32).min(image.height());
            for y in y0.saturating_sub(2)..(y1 + 2).min(image.height()) {
                for x in x0.saturating_sub(2)..(x1 + 2).min(image.width()) {
                    image.put_pixel(x, y, image::Rgb([0, 0, 0]));
                }
            }
        }
        let size = pdf::geometry(source, id)?.display_size();
        order.push(add_raster_page(&mut result, image, size)?);
    }
    finish_pages(&mut result, order)?;
    pdf::atomic_save(&result, output)
}

fn add_raster_page(doc: &mut Document, image: image::RgbImage, size: [f32; 2]) -> Result<ObjectId> {
    let (w, h) = image.dimensions();
    let mut stream = Stream::new(
        dictionary! {"Type"=>"XObject","Subtype"=>"Image","Width"=>w as i64,"Height"=>h as i64,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8},
        image.into_raw(),
    );
    stream.compress()?;
    let image = doc.add_object(stream);
    let content = doc.add_object(Stream::new(
        Dictionary::new(),
        format!("q {} 0 0 {} 0 0 cm /PageImage Do Q", size[0], size[1]).into_bytes(),
    ));
    Ok(doc.add_object(dictionary!{"Type"=>"Page","MediaBox"=>vec![Object::Real(0.),Object::Real(0.),size[0].into(),size[1].into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"PageImage"=>image}},"Contents"=>content}))
}

pub(crate) fn add_jpeg_page(
    doc: &mut Document,
    image: image::RgbImage,
    size: [f32; 2],
    quality: u8,
) -> Result<ObjectId> {
    let (w, h) = image.dimensions();
    let mut jpeg = vec![];
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, quality)
        .encode_image(&image::DynamicImage::ImageRgb8(image))?;
    let image=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>w as i64,"Height"=>h as i64,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8,"Filter"=>"DCTDecode"},jpeg));
    let content = doc.add_object(Stream::new(
        Dictionary::new(),
        format!("q {} 0 0 {} 0 0 cm /PageImage Do Q", size[0], size[1]).into_bytes(),
    ));
    Ok(doc.add_object(dictionary!{"Type"=>"Page","MediaBox"=>vec![Object::Real(0.),Object::Real(0.),size[0].into(),size[1].into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"PageImage"=>image}},"Contents"=>content}))
}

pub(crate) fn finish_pages(doc: &mut Document, order: Vec<ObjectId>) -> Result<()> {
    let pages=doc.add_object(dictionary!{"Type"=>"Pages","Count"=>order.len() as i64,"Kids"=>order.iter().map(|id|Object::Reference(*id)).collect::<Vec<_>>()});
    for id in order {
        doc.get_dictionary_mut(id)?.set("Parent", pages);
    }
    let catalog = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    doc.trailer.set("Root", catalog);
    Ok(())
}

pub fn find_tesseract() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("PDFPILOT_TESSERACT") {
        let p = PathBuf::from(path);
        return p.is_file().then_some(p);
    }
    let mut candidates = vec![
        PathBuf::from("/opt/homebrew/bin/tesseract"),
        PathBuf::from("/usr/local/bin/tesseract"),
    ];
    if let Some(paths) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&paths).map(|p| {
            p.join(if cfg!(windows) {
                "tesseract.exe"
            } else {
                "tesseract"
            })
        }));
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// Tesseract builds searchable pages with an image and an invisible text layer.
#[derive(Clone)]
pub struct OcrOptions {
    pub languages: String,
    pub segmentation: u8,
    pub width: u32,
}
impl Default for OcrOptions {
    fn default() -> Self {
        Self {
            languages: "eng".into(),
            segmentation: 3,
            width: 2400,
        }
    }
}
pub fn searchable_copy(source: &Document, output: &Path) -> Result<()> {
    searchable_copy_with_options(source, &OcrOptions::default(), output)
}
pub fn searchable_copy_with_options(
    source: &Document,
    options: &OcrOptions,
    output: &Path,
) -> Result<()> {
    ensure!(!options.languages.is_empty() && options.languages.len()<=100 && options.languages.split('+').all(|s|!s.is_empty() && s.bytes().all(|c|c.is_ascii_alphanumeric() || c==b'_')),"Invalid OCR languages");
    ensure!(
        options.segmentation <= 13 && (600..=4000).contains(&options.width),
        "Invalid OCR settings"
    );
    let binary=find_tesseract().context("OCR requires Tesseract. Install it with brew install tesseract or apt install tesseract-ocr, or set PDFPILOT_TESSERACT.")?;
    let temp = tempfile::tempdir()?;
    let input = temp.path().join("source.pdf");
    pdf::atomic_save(source, &input)?;
    let mut result = Document::with_version("1.7");
    let mut order = vec![];
    for (number, id) in source.get_pages() {
        let image_path = temp.path().join(format!("page-{number}.png"));
        let image = renderer::render_page(&input, number, options.width)?;
        image.save(&image_path)?;
        let prefix = temp.path().join(format!("ocr-{number}"));
        let status = Command::new(&binary)
            .arg(&image_path)
            .arg(&prefix)
            .args([
                "-l",
                &options.languages,
                "--dpi",
                "300",
                "--psm",
                &options.segmentation.to_string(),
                "pdf",
            ])
            .output()
            .context("Could not start Tesseract")?;
        ensure!(
            status.status.success(),
            "OCR failed: {}",
            String::from_utf8_lossy(&status.stderr).trim()
        );
        let mut page = Document::load(prefix.with_extension("pdf"))?;
        page.renumber_objects_with(result.max_id + 1);
        let page_id = *page
            .get_pages()
            .values()
            .next()
            .context("OCR produced no page")?;
        // Tesseract's explicit DPI sets a physical page size. Scale its complete
        // content to match the source's displayed CropBox and rotation.
        let size = pdf::geometry(source, id)?.display_size();
        let original = pdf::geometry(&page, page_id)?.display_size();
        let content = page.get_page_content_with_limit(page_id, pdf::TEXT_LIMIT)?;
        let mut wrapped = format!(
            "q {} 0 0 {} 0 0 cm\n",
            size[0] / original[0],
            size[1] / original[1]
        )
        .into_bytes();
        wrapped.extend(content);
        wrapped.extend(b"\nQ");
        page.change_page_content(page_id, wrapped)?;
        page.get_dictionary_mut(page_id)?.set(
            "MediaBox",
            vec![0.into(), 0.into(), size[0].into(), size[1].into()],
        );
        result.max_id = page.max_id;
        result.objects.extend(page.objects);
        order.push(page_id);
    }
    finish_pages(&mut result, order)?;
    result.prune_objects();
    pdf::atomic_save(&result, output)
}

pub struct ComparisonReport {
    pub summary: String,
    pub html: String,
}

/// Page-by-page text and rendered pixel comparison; reports include independent
/// images of both documents and a difference mask, with no remote resources.
pub fn compare_documents(source: &Document, other: &Path) -> Result<ComparisonReport> {
    use base64::Engine;
    use std::io::Cursor;
    let other = pdf::PdfDocument::open(other, "")?;
    let temp = tempfile::tempdir()?;
    let left = temp.path().join("left.pdf");
    let right = temp.path().join("right.pdf");
    pdf::atomic_save(source, &left)?;
    pdf::atomic_save(&other.doc, &right)?;
    let a = source.get_pages().len() as u32;
    let b = other.page_count();
    let mut summary = format!("Original: {a} pages\nCompared: {b} pages\n\n");
    let mut html = String::from(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>PDFPilot comparison</title><style>body{font:15px system-ui;background:#101724;color:#e6ecf8;margin:40px}section{padding:20px;margin:24px 0;background:#1a2333;border-radius:12px}figure{margin:0;flex:1;min-width:0}img{width:100%;background:white}article{display:flex;gap:16px}pre{white-space:pre-wrap}figcaption{padding:8px 0}</style><h1>Document comparison</h1><p>Pages are compared by position. Red pixels mark visual differences; text extraction can omit scanned or unsupported fonts.</p>",
    );
    let mut changed = 0;
    let data_url = |image: &image::RgbaImage| -> Result<String> {
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png)?;
        Ok(format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes.into_inner())
        ))
    };
    for page in 1..=a.max(b) {
        if page > a || page > b {
            changed += 1;
            let description = if page > a { "Added" } else { "Removed" };
            summary.push_str(&format!("Page {page}: {description}\n"));
            let image = renderer::render_page(if page > a { &right } else { &left }, page, 900)?;
            html.push_str(&format!("<section><h2>Page {page}: {description}</h2><img alt=\"{description} page {page}\" src=\"{}\"></section>", data_url(&image)?));
            continue;
        }
        let before = source
            .extract_text_with_limit(&[page], pdf::TEXT_LIMIT)
            .unwrap_or_default();
        let after = other
            .doc
            .extract_text_with_limit(&[page], pdf::TEXT_LIMIT)
            .unwrap_or_default();
        let first = renderer::render_page(&left, page, 900)?;
        let second = renderer::render_page(&right, page, 900)?;
        let dims = (
            first.width().max(second.width()),
            first.height().max(second.height()),
        );
        let mut mask =
            image::RgbaImage::from_pixel(dims.0, dims.1, image::Rgba([255, 255, 255, 255]));
        let mut pixels = 0u64;
        for (x, y, pixel) in mask.enumerate_pixels_mut() {
            let old = first.get_pixel_checked(x, y);
            let new = second.get_pixel_checked(x, y);
            let difference = match (old, new) {
                (Some(old), Some(new)) => old.0[..3]
                    .iter()
                    .zip(&new.0[..3])
                    .any(|(a, b)| a.abs_diff(*b) > 20),
                _ => true,
            };
            if difference {
                pixels += 1;
                *pixel = image::Rgba([233, 61, 82, 255]);
            } else if let Some(old) = old {
                *pixel = image::Rgba([old[0] / 4 + 190, old[1] / 4 + 190, old[2] / 4 + 190, 255]);
            }
        }
        let text_changed = before != after;
        if pixels == 0 && !text_changed {
            continue;
        }
        changed += 1;
        let percent = pixels as f64 * 100. / (dims.0 as f64 * dims.1 as f64);
        let description = format!(
            "{percent:.2}% visual difference; text {}",
            if text_changed { "changed" } else { "unchanged" }
        );
        summary.push_str(&format!("Page {page}: {description}\n"));
        html.push_str(&format!(
            "<section><h2>Page {page}</h2><p>{description}</p><article>"
        ));
        for (label, img) in [
            ("Original", &first),
            ("Compared", &second),
            ("Differences", &mask),
        ] {
            html.push_str(&format!("<figure><figcaption>{label}</figcaption><img alt=\"{label} page {page}\" src=\"{}\"></figure>",data_url(img)?));
        }
        html.push_str("</article>");
        if text_changed {
            for (label, text) in [("Original text", &before), ("Compared text", &after)] {
                html.push_str(&format!(
                    "<details><summary>{label}</summary><pre>{}</pre></details>",
                    text.replace('&', "&amp;")
                        .replace('<', "&lt;")
                        .replace('>', "&gt;")
                ));
            }
        }
        html.push_str("</section>");
    }
    summary.push_str(&format!("\n{changed} changed pages.\n"));
    html.push_str(&format!("<p>{changed} changed pages.</p></html>"));
    Ok(ComparisonReport { summary, html })
}
