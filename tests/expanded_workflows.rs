use anyhow::Result;
use lopdf::{Object, dictionary};
use pdfpilot::{
    advanced::{FormKind, FormSpec, Shape},
    conversion::{self, ExportFormat},
    pdf::{self, PdfDocument, TextStyle},
    renderer, workflows,
};
use std::collections::BTreeMap;

fn sample() -> Result<PdfDocument> {
    Ok(PdfDocument::from_document(
        pdf::create_text_document("Example", "First page\nData A\tData B")?,
        "sample.pdf",
    ))
}

#[test]
fn page_tools_preserve_content_and_navigation() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let mut doc = sample()?;
    doc.add_outline(1, "Overview")?;
    doc.duplicate_page(1)?;
    assert_eq!(doc.page_count(), 2);
    assert_eq!(doc.outlines()[0].page, Some(1));
    doc.replace_text(2, "First page", "Second page")?;
    assert!(doc.text(1)?.contains("First page"));
    assert!(doc.text(2)?.contains("Second page"));
    doc.move_page(1, 2)?;
    assert_eq!(doc.outlines()[0].page, Some(2));
    doc.rotate_range(&[1, 2], 90)?;
    assert_eq!(doc.geometry(2)?.rotation, 90);
    doc.crop_page(1, [0.1, 0.1], [0.9, 0.9])?;
    let cropped = doc.geometry(1)?;
    assert!(cropped.width() < 612.);
    doc.reset_crop(1)?;
    assert_eq!(doc.geometry(1)?.width(), 612.);
    doc.set_page_labels("A-", 7)?;
    assert_eq!(doc.page_labels(), vec!["A-7", "A-8"]);
    assert_eq!(doc.split_to_folder(1, temp.path())?, 2);
    assert!(doc.split_to_folder(1, temp.path()).is_err());
    let second = temp.path().join("document-part-002.pdf");
    doc.replace_page(1, &second, 1)?;
    assert!(doc.text(1)?.contains("First page"));
    Ok(())
}

#[test]
fn bookmarks_links_and_embedded_files_round_trip() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let mut doc = sample()?;
    doc.add_outline(1, "Introduction")?;
    doc.add_outline(1, "Details")?;
    doc.add_link(1, [0.1, 0.2], [0.5, 0.25], "https://example.com")?;
    let link = doc.links()[0].clone();
    doc.update_link(link.id, "https://example.org/manual")?;
    assert!(
        doc.add_link(1, [0.1, 0.2], [0.5, 0.25], "javascript:alert(1)")
            .is_err()
    );
    let attachment = temp.path().join("notes.txt");
    std::fs::write(&attachment, b"Embedded content")?;
    doc.attach_file(&attachment)?;
    let output = temp.path().join("document.pdf");
    doc.save(&output)?;
    let loaded = PdfDocument::open(&output, "")?;
    assert_eq!(loaded.outlines().len(), 2);
    assert_eq!(loaded.links()[0].uri, "https://example.org/manual");
    let attachments = loaded.attachments();
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].name, "notes.txt");
    let saved = temp.path().join("extracted.txt");
    loaded.export_attachment(&attachments[0], &saved)?;
    assert_eq!(std::fs::read(saved)?, b"Embedded content");
    Ok(())
}

#[test]
fn authored_forms_save_values_and_import_atomically() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let mut doc = sample()?;
    for (n, kind, name) in [
        (0, FormKind::Text, "Name"),
        (1, FormKind::Checkbox, "Consent"),
        (2, FormKind::Dropdown, "Plan"),
    ] {
        doc.add_form_field(
            1,
            [0.1, 0.3 + n as f32 * 0.1],
            [0.6, 0.36 + n as f32 * 0.1],
            &FormSpec {
                kind,
                name: name.into(),
                required: true,
                options: vec!["Basic".into(), "Premium".into()],
            },
        )?;
    }
    let data = BTreeMap::from([
        ("Name".into(), "Alice".into()),
        ("Consent".into(), "Yes".into()),
        ("Plan".into(), "Premium".into()),
    ]);
    doc.import_form_data(&data)?;
    assert_eq!(doc.form_data(), data);
    let bad = BTreeMap::from([
        ("Name".into(), "Bob".into()),
        ("Plan".into(), "Unsupported".into()),
    ]);
    let revision = doc.revision;
    assert!(doc.import_form_data(&bad).is_err());
    assert_eq!(doc.revision, revision);
    assert_eq!(doc.form_data(), data);
    let path = temp.path().join("form.pdf");
    doc.save(&path)?;
    let loaded = PdfDocument::open(&path, "")?;
    assert_eq!(loaded.form_data(), data);
    for field in loaded.fields() {
        assert!(loaded.doc.get_dictionary(field.id)?.has(b"AP"));
    }
    assert!(doc.duplicate_page(1).is_err());
    Ok(())
}

#[test]
fn shapes_replies_and_resolution_preserve_direct_annotations() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let mut doc = sample()?;
    let page = doc.page_id(1)?;
    doc.doc.get_dictionary_mut(page)?.set("Annots",vec![Object::Dictionary(dictionary!{"Subtype"=>"Text","Rect"=>vec![0.into(),0.into(),10.into(),10.into()],"Contents"=>Object::string_literal("Direct annotation")})]);
    for (n, shape) in [
        Shape::Strikeout,
        Shape::Line,
        Shape::Arrow,
        Shape::Circle,
        Shape::Stamp,
    ]
    .into_iter()
    .enumerate()
    {
        doc.add_shape(
            1,
            shape,
            [0.1, 0.2 + n as f32 * 0.1],
            [0.7, 0.26 + n as f32 * 0.1],
            "Reviewed",
        )?;
    }
    assert_eq!(
        doc.doc
            .get_dictionary(page)?
            .get(b"Annots")?
            .as_array()?
            .len(),
        6
    );
    let parent = doc.comments()[0].clone();
    doc.reply_comment(parent.id, 1, "Follow up")?;
    doc.resolve_comment(parent.id, true)?;
    assert!(doc.comment_resolved(parent.id));
    let path = temp.path().join("review.pdf");
    doc.save(&path)?;
    let loaded = PdfDocument::open(&path, "")?;
    assert!(loaded.comment_resolved(parent.id));
    assert!(loaded.comments().iter().any(|c| c.text == "Follow up"));
    if renderer::find_poppler().is_some() {
        let image = renderer::render_page(&path, 1, 600)?;
        assert!(image.pixels().any(|p| p[2] as u16 > p[0] as u16 + 30));
    }
    Ok(())
}

#[test]
fn styled_text_images_numbering_and_metadata() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let mut doc = sample()?;
    doc.duplicate_page(1)?;
    doc.add_running_text("CASE-", [0.1, 0.9], &TextStyle::default(), true, 42, 6)?;
    assert!(doc.text(1)?.contains("CASE-000042"));
    assert!(doc.text(2)?.contains("CASE-000043"));
    let path = temp.path().join("image.png");
    image::RgbaImage::from_pixel(20, 20, image::Rgba([255, 0, 0, 128])).save(&path)?;
    doc.rotate(2, 90)?;
    doc.add_image(2, [0.1, 0.5], [0.3, 0.7], &path)?;
    doc.add_styled_text(
        2,
        [0.5, 0.6],
        "New text",
        &TextStyle {
            font: "Courier".into(),
            size: 20.,
            color: [0.8, 0.1, 0.1],
        },
    )?;
    doc.set_metadata("Secret", "Author", "Subject")?;
    doc.strip_metadata()?;
    assert_eq!(doc.metadata(b"Title"), "");
    doc.set_language("en-US")?;
    assert_eq!(pdf::object_text(doc.doc.catalog()?.get(b"Lang")?), "en-US");
    let output = temp.path().join("styled.pdf");
    doc.save(&output)?;
    if renderer::find_poppler().is_some() {
        let image = renderer::render_page(&output, 2, 600)?;
        let pixel = image.get_pixel(
            (image.width() as f32 * 0.2) as u32,
            (image.height() as f32 * 0.6) as u32,
        );
        assert!(
            pixel[0] > 200 && pixel[1] < 170,
            "Rotated image placement: {pixel:?}"
        );
    }
    Ok(())
}

#[test]
fn office_exports_reopen_as_pdf_with_real_libreoffice() -> Result<()> {
    if conversion::find_office().is_none() {
        return Ok(());
    }
    let temp = tempfile::tempdir()?;
    let doc = sample()?;
    for format in [ExportFormat::Docx, ExportFormat::Xlsx, ExportFormat::Pptx] {
        if format == ExportFormat::Pptx && renderer::find_poppler().is_none() {
            continue;
        }
        let path = temp.path().join(format!("export.{}", format.extension()));
        conversion::export(&doc.doc, &[1], format, &path)?;
        let output = conversion::office_to_document(&path)?;
        assert!(!output.get_pages().is_empty(), "Failed {format:?}");
        if format != ExportFormat::Pptx {
            assert!(
                output.extract_text(&[1])?.contains("First page"),
                "Lost text in {format:?}"
            );
        }
    }
    Ok(())
}

#[test]
fn image_html_and_summary_exports_are_usable() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let mut doc = sample()?;
    doc.add_shape(1, Shape::Circle, [0.1, 0.3], [0.5, 0.4], "Review note")?;
    for format in [
        ExportFormat::Text,
        ExportFormat::Comments,
        ExportFormat::FormData,
    ] {
        let path = temp.path().join(format!("export.{}", format.extension()));
        conversion::export(&doc.doc, &[1], format, &path)?;
        let content = std::fs::read_to_string(path)?;
        if format == ExportFormat::FormData {
            let _: serde_json::Value = serde_json::from_str(&content)?;
        } else {
            assert!(!content.is_empty());
        }
    }
    if renderer::find_poppler().is_some() {
        for format in [
            ExportFormat::Png,
            ExportFormat::Jpeg,
            ExportFormat::Tiff,
            ExportFormat::Html,
        ] {
            let path = temp.path().join(format!("export.{}", format.extension()));
            conversion::export(&doc.doc, &[1], format, &path)?;
            if format == ExportFormat::Html {
                assert!(std::fs::read_to_string(path)?.contains("data:image/png;base64,"));
            } else {
                let image = image::open(path)?;
                assert_eq!(image.width(), 2400);
            }
        }
    }
    Ok(())
}

#[test]
fn comparison_detects_visual_text_and_added_pages() -> Result<()> {
    if renderer::find_poppler().is_none() {
        return Ok(());
    }
    let temp = tempfile::tempdir()?;
    let original = sample()?;
    let mut changed = sample()?;
    let path = temp.path().join("changed.pdf");
    changed.save(&path)?;
    let same = workflows::compare_documents(&original.doc, &path)?;
    assert!(same.summary.contains("0 changed pages"));
    changed.replace_text(1, "First page", "Changed text")?;
    changed.duplicate_page(1)?;
    changed.save(&path)?;
    let report = workflows::compare_documents(&original.doc, &path)?;
    assert!(report.summary.contains("text changed"));
    assert!(report.summary.contains("Page 2: Added"));
    assert!(report.summary.contains("2 changed pages"));
    assert!(report.html.contains("data:image/png;base64,"));
    Ok(())
}
