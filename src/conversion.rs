use crate::{pdf, renderer};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use lopdf::Document;
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};
use zip::{ZipWriter, write::FileOptions};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Text,
    Html,
    Docx,
    Xlsx,
    Pptx,
    Png,
    Jpeg,
    Tiff,
    Comments,
    FormData,
}
impl ExportFormat {
    pub fn label(self) -> &'static str {
        match self {
            Self::Text => "Plain text",
            Self::Html => "HTML (page images)",
            Self::Docx => "Word (editable text)",
            Self::Xlsx => "Excel (text rows)",
            Self::Pptx => "PowerPoint (page images)",
            Self::Png => "PNG image",
            Self::Jpeg => "JPEG image",
            Self::Tiff => "TIFF image",
            Self::Comments => "Comment summary",
            Self::FormData => "Form data (JSON)",
        }
    }
    pub fn extension(self) -> &'static str {
        match self {
            Self::Text | Self::Comments => "txt",
            Self::Html => "html",
            Self::Docx => "docx",
            Self::Xlsx => "xlsx",
            Self::Pptx => "pptx",
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Tiff => "tiff",
            Self::FormData => "json",
        }
    }
}

pub fn export(doc: &Document, pages: &[u32], format: ExportFormat, path: &Path) -> Result<()> {
    ensure!(!pages.is_empty(), "Select at least one page");
    let all = doc.get_pages();
    ensure!(
        pages.iter().all(|n| all.contains_key(n)),
        "Page is out of range"
    );
    match format {
        ExportFormat::Text => {
            std::fs::write(path, doc.extract_text_with_limit(pages, pdf::TEXT_LIMIT)?)?
        }
        ExportFormat::Docx => export_docx(doc, pages, path)?,
        ExportFormat::Xlsx => export_xlsx(doc, pages, path)?,
        ExportFormat::Pptx => export_pptx(doc, pages, path)?,
        ExportFormat::Html => export_html(doc, pages, path)?,
        ExportFormat::Png | ExportFormat::Jpeg | ExportFormat::Tiff => {
            ensure!(pages.len() == 1, "Image export accepts one selected page");
            let temp = tempfile::tempdir()?;
            let input = temp.path().join("source.pdf");
            pdf::atomic_save(doc, &input)?;
            let image = renderer::render_page(&input, pages[0], 2400)?;
            match format {
                ExportFormat::Jpeg => image::DynamicImage::ImageRgba8(image)
                    .to_rgb8()
                    .save_with_format(path, image::ImageFormat::Jpeg)?,
                ExportFormat::Tiff => image.save_with_format(path, image::ImageFormat::Tiff)?,
                _ => image.save_with_format(path, image::ImageFormat::Png)?,
            }
        }
        ExportFormat::Comments => {
            let view = pdf::PdfDocument::from_document(doc.clone(), "summary");
            let text = view
                .comments()
                .into_iter()
                .filter(|c| pages.contains(&c.page))
                .map(|c| {
                    format!(
                        "Page {} / {}{}\n{}\n",
                        c.page,
                        c.kind,
                        if view.comment_resolved(c.id) {
                            " / Resolved"
                        } else {
                            ""
                        },
                        c.text
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            std::fs::write(path, text)?;
        }
        ExportFormat::FormData => {
            let view = pdf::PdfDocument::from_document(doc.clone(), "form");
            std::fs::write(path, serde_json::to_vec_pretty(&view.form_data())?)?;
        }
    }
    Ok(())
}

fn xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn package(
    path: &Path,
    write: impl FnOnce(&mut ZipWriter<&mut std::fs::File>) -> Result<()>,
) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    {
        let mut zip = ZipWriter::new(temp.as_file_mut());
        write(&mut zip)?;
        zip.finish()?;
    }
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}
fn part(zip: &mut ZipWriter<&mut std::fs::File>, name: &str, bytes: &[u8]) -> Result<()> {
    zip.start_file(
        name,
        FileOptions::default().compression_method(zip::CompressionMethod::Deflated),
    )?;
    zip.write_all(bytes)?;
    Ok(())
}
const ROOT_RELS: &str = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="{target}"/></Relationships>"#;

fn export_docx(doc: &Document, pages: &[u32], path: &Path) -> Result<()> {
    let mut body = String::new();
    for (index, page) in pages.iter().enumerate() {
        if index > 0 {
            body.push_str(r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#);
        }
        for line in doc
            .extract_text_with_limit(&[*page], pdf::TEXT_LIMIT)?
            .lines()
        {
            body.push_str(&format!(
                r#"<w:p><w:r><w:t xml:space="preserve">{}</w:t></w:r></w:p>"#,
                xml(line)
            ));
        }
    }
    let main = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1080" w:right="1080" w:bottom="1080" w:left="1080"/></w:sectPr></w:body></w:document>"#
    );
    package(path, |zip| {
        part(zip,"[Content_Types].xml",br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#)?;
        part(
            zip,
            "_rels/.rels",
            ROOT_RELS
                .replace("{target}", "word/document.xml")
                .as_bytes(),
        )?;
        part(zip, "word/document.xml", main.as_bytes())
    })
}

fn export_xlsx(doc: &Document, pages: &[u32], path: &Path) -> Result<()> {
    let mut sheets = vec![];
    for page in pages {
        let mut data = String::new();
        for (row, line) in doc
            .extract_text_with_limit(&[*page], pdf::TEXT_LIMIT)?
            .lines()
            .enumerate()
        {
            let cells = if line.contains('\t') {
                line.split('\t').map(str::trim).collect::<Vec<_>>()
            } else {
                vec![line]
            };
            let cells = cells
                .iter()
                .enumerate()
                .map(|(column, value)| {
                    format!(
                        r#"<c r="{}{}" t="inlineStr"><is><t xml:space="preserve">{}</t></is></c>"#,
                        column_name(column + 1),
                        row + 1,
                        xml(value)
                    )
                })
                .collect::<String>();
            data.push_str(&format!(r#"<row r="{}">{cells}</row>"#, row + 1));
        }
        sheets.push(format!(r#"<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><cols><col min="1" max="20" width="28" customWidth="1"/></cols><sheetData>{data}</sheetData></worksheet>"#));
    }
    package(path, |zip| {
        let overrides=(1..=sheets.len()).map(|n|format!(r#"<Override PartName="/xl/worksheets/sheet{n}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#)).collect::<String>();
        part(zip,"[Content_Types].xml",format!(r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>{overrides}</Types>"#).as_bytes())?;
        part(
            zip,
            "_rels/.rels",
            ROOT_RELS.replace("{target}", "xl/workbook.xml").as_bytes(),
        )?;
        let names = pages
            .iter()
            .enumerate()
            .map(|(i, page)| {
                format!(
                    r#"<sheet name="Page {page}" sheetId="{}" r:id="rId{}"/>"#,
                    i + 1,
                    i + 1
                )
            })
            .collect::<String>();
        part(zip,"xl/workbook.xml",format!(r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>{names}</sheets></workbook>"#).as_bytes())?;
        let rels=(1..=sheets.len()).map(|n|format!(r#"<Relationship Id="rId{n}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{n}.xml"/>"#)).collect::<String>();
        part(zip,"xl/_rels/workbook.xml.rels",format!(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rels}</Relationships>"#).as_bytes())?;
        for (index, sheet) in sheets.iter().enumerate() {
            part(
                zip,
                &format!("xl/worksheets/sheet{}.xml", index + 1),
                sheet.as_bytes(),
            )?;
        }
        Ok(())
    })
}
fn column_name(mut index: usize) -> String {
    let mut result = String::new();
    while index > 0 {
        index -= 1;
        result.insert(0, (b'A' + (index % 26) as u8) as char);
        index /= 26;
    }
    result
}

fn export_html(doc: &Document, pages: &[u32], path: &Path) -> Result<()> {
    let temp = tempfile::tempdir()?;
    let input = temp.path().join("source.pdf");
    pdf::atomic_save(doc, &input)?;
    let mut html = String::from(
        "<!doctype html><html lang=\"en\"><meta charset=\"UTF-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>PDFPilot document</title><style>body{background:#edf0f6;margin:32px;font:14px system-ui}main{max-width:960px;margin:auto}figure{margin:0 0 24px}img{width:100%;box-shadow:0 4px 24px #0002}figcaption{padding:12px;text-align:center;color:#667}</style><main>",
    );
    for page in pages {
        let image = renderer::render_page(&input, *page, 1600)?;
        let mut png = std::io::Cursor::new(vec![]);
        image::DynamicImage::ImageRgba8(image).write_to(&mut png, image::ImageFormat::Png)?;
        html.push_str(&format!("<figure><img alt=\"PDF page {page}\" src=\"data:image/png;base64,{}\"><figcaption>Page {page}</figcaption></figure>",STANDARD.encode(png.into_inner())));
    }
    html.push_str("</main></html>");
    std::fs::write(path, html)?;
    Ok(())
}

fn export_pptx(doc: &Document, pages: &[u32], path: &Path) -> Result<()> {
    let temp = tempfile::tempdir()?;
    let input = temp.path().join("source.pdf");
    pdf::atomic_save(doc, &input)?;
    let first = *doc.get_pages().get(&pages[0]).context("Missing page")?;
    let size = pdf::geometry(doc, first)?.display_size();
    let width = (size[0] * 12700.) as i64;
    let height = (size[1] * 12700.) as i64;
    package(path, |zip| {
        let overrides=(1..=pages.len()).map(|n|format!(r#"<Override PartName="/ppt/slides/slide{n}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#)).collect::<String>();
        part(zip,"[Content_Types].xml",format!(r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>{overrides}</Types>"#).as_bytes())?;
        part(
            zip,
            "_rels/.rels",
            ROOT_RELS
                .replace("{target}", "ppt/presentation.xml")
                .as_bytes(),
        )?;
        let slide_ids = (1..=pages.len())
            .map(|n| format!(r#"<p:sldId id="{}" r:id="rId{n}"/>"#, n + 255))
            .collect::<String>();
        part(zip,"ppt/presentation.xml",format!(r#"<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:sldIdLst>{slide_ids}</p:sldIdLst><p:sldSz cx="{width}" cy="{height}"/><p:notesSz cx="6858000" cy="9144000"/></p:presentation>"#).as_bytes())?;
        let relationships=(1..=pages.len()).map(|n|format!(r#"<Relationship Id="rId{n}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{n}.xml"/>"#)).collect::<String>();
        part(zip,"ppt/_rels/presentation.xml.rels",format!(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{relationships}</Relationships>"#).as_bytes())?;
        for (index, page) in pages.iter().enumerate() {
            let n = index + 1;
            let image = renderer::render_page(&input, *page, 1800)?;
            let image_size = image.dimensions();
            let mut png = std::io::Cursor::new(vec![]);
            image::DynamicImage::ImageRgba8(image).write_to(&mut png, image::ImageFormat::Png)?;
            part(zip, &format!("ppt/media/page{n}.png"), &png.into_inner())?;
            let ratio =
                (width as f64 / image_size.0 as f64).min(height as f64 / image_size.1 as f64);
            let w = (image_size.0 as f64 * ratio) as i64;
            let h = (image_size.1 as f64 * ratio) as i64;
            let x = (width - w) / 2;
            let y = (height - h) / 2;
            let slide = format!(
                r#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr><p:pic><p:nvPicPr><p:cNvPr id="2" name="Page {page}"/><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="rId1"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{w}" cy="{h}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#
            );
            part(zip, &format!("ppt/slides/slide{n}.xml"), slide.as_bytes())?;
            part(zip,&format!("ppt/slides/_rels/slide{n}.xml.rels"),format!(r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/page{n}.png"/></Relationships>"#).as_bytes())?;
        }
        Ok(())
    })
}

pub fn find_office() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("PDFPILOT_SOFFICE") {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }
    let mut candidates = vec![PathBuf::from(
        "/Applications/LibreOffice.app/Contents/MacOS/soffice",
    )];
    if let Some(paths) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&paths).map(|p| {
            p.join(if cfg!(windows) {
                "soffice.exe"
            } else {
                "soffice"
            })
        }));
    }
    candidates.into_iter().find(|p| p.is_file())
}
pub fn office_to_document(path: &Path) -> Result<Document> {
    let binary=find_office().context("Office import requires LibreOffice. Install LibreOffice or set PDFPILOT_SOFFICE to its executable path.")?;
    let temp = tempfile::tempdir()?;
    let profile = temp.path().join("profile");
    let profile_url = format!(
        "file://{}",
        profile
            .to_string_lossy()
            .replace('%', "%25")
            .replace(' ', "%20")
    );
    let result = Command::new(binary)
        .arg(format!("-env:UserInstallation={profile_url}"))
        .args(["--headless", "--nologo", "--convert-to", "pdf", "--outdir"])
        .arg(temp.path())
        .arg(path)
        .output()
        .context("Could not start LibreOffice")?;
    ensure!(
        result.status.success(),
        "Office conversion failed ({}): {}",
        result.status,
        String::from_utf8_lossy(&result.stderr)
    );
    let output = temp
        .path()
        .join(path.file_stem().context("Missing file name")?)
        .with_extension("pdf");
    Document::load(&output).context("LibreOffice did not produce a PDF")
}
