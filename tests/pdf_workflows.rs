use anyhow::Result;
use lopdf::{Document, Object, Stream, dictionary};
use pdfpilot::{
    pdf::{self, Geometry, Mark, PdfDocument},
    renderer,
    workflows::{self, Redaction},
};

fn sample() -> PdfDocument {
    PdfDocument::from_document(
        pdf::create_text_document("Example", "Original text\nSecond line").unwrap(),
        "sample.pdf",
    )
}

#[test]
fn creates_text_and_round_trips() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("sample.pdf");
    let mut doc = sample();
    doc.save(&path)?;
    let loaded = PdfDocument::open(&path, "")?;
    assert_eq!(loaded.page_count(), 1);
    assert!(loaded.text(1)?.contains("Original text"));
    assert!(!doc.dirty);
    Ok(())
}

#[test]
fn text_flows_across_pages() -> Result<()> {
    let body = (0..100)
        .map(|n| format!("Line {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    let doc = PdfDocument::from_document(
        pdf::create_text_document("Long document", &body)?,
        "long.pdf",
    );
    assert_eq!(doc.page_count(), 3);
    assert!(doc.text(3)?.contains("Line 99"));
    Ok(())
}

#[test]
fn edits_undo_and_redo_without_losing_content() -> Result<()> {
    let mut doc = sample();
    doc.rotate(1, 90)?;
    doc.add_text(1, [0.3, 0.4], "Added text", 14.)?;
    assert!(doc.text(1)?.contains("Added text"));
    assert_eq!(doc.geometry(1)?.rotation, 90);
    doc.undo();
    assert!(!doc.text(1)?.contains("Added text"));
    doc.undo();
    assert_eq!(doc.geometry(1)?.rotation, 0);
    doc.redo();
    doc.redo();
    assert!(doc.text(1)?.contains("Added text"));
    Ok(())
}

#[test]
fn failed_edits_do_not_commit() {
    let mut doc = sample();
    let count = doc.doc.objects.len();
    assert!(
        doc.add_text(1, [0.1, 0.2], "Unsupported: \u{1f600}", 12.)
            .is_err()
    );
    assert_eq!(doc.revision, 0);
    assert_eq!(doc.doc.objects.len(), count);
    assert!(!doc.can_undo());
    assert!(doc.delete(&[1]).is_err());
    assert!(doc.delete(&[0]).is_err());
    assert_eq!(doc.page_count(), 1);
}

#[test]
fn annotation_comments_round_trip_and_remove() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("reviewed.pdf");
    let mut doc = sample();
    for mark in [
        Mark::Highlight,
        Mark::Underline,
        Mark::Rectangle,
        Mark::Note,
    ] {
        doc.add_mark(1, mark, [0.1, 0.2], [0.6, 0.3], "Review this: \u{03bb}")?;
    }
    doc.save(&path)?;
    let mut loaded = PdfDocument::open(&path, "")?;
    let comments = loaded.comments();
    assert_eq!(comments.len(), 4);
    assert_eq!(comments[0].text, "Review this: \u{03bb}");
    loaded.remove_comment(&comments[0])?;
    assert_eq!(loaded.comments().len(), 3);
    loaded.undo();
    assert_eq!(loaded.comments().len(), 4);
    Ok(())
}

#[test]
fn page_reorder_extract_merge_and_delete() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let selected = temp.path().join("selected.pdf");
    let mut doc = sample();
    doc.insert_blank(1)?;
    doc.add_text(2, [0.1, 0.2], "Page Two", 16.)?;
    doc.insert_blank(2)?;
    doc.add_text(3, [0.1, 0.2], "Page Three", 16.)?;
    doc.move_page(3, 1)?;
    assert!(doc.text(1)?.contains("Page Three"));
    assert!(doc.text(2)?.contains("Original text"));
    doc.extract(&[3, 1], &selected)?;
    let subset = PdfDocument::open(&selected, "")?;
    assert_eq!(subset.page_count(), 2);
    assert!(subset.text(1)?.contains("Page Two"));
    doc.merge(&selected)?;
    assert_eq!(doc.page_count(), 5);
    assert!(doc.text(5)?.contains("Page Three"));
    doc.delete(&[2, 3])?;
    assert_eq!(doc.page_count(), 3);
    assert!(doc.text(1)?.contains("Page Three"));
    Ok(())
}

#[test]
fn inherited_resources_survive_reordering() -> Result<()> {
    let mut doc = sample();
    let page = doc.page_id(1)?;
    let parent = doc
        .doc
        .get_dictionary(page)?
        .get(b"Parent")?
        .as_reference()?;
    let resources = doc
        .doc
        .get_dictionary_mut(page)?
        .remove(b"Resources")
        .unwrap();
    let box_value = doc
        .doc
        .get_dictionary_mut(page)?
        .remove(b"MediaBox")
        .unwrap();
    doc.doc
        .get_dictionary_mut(parent)?
        .set("Resources", resources);
    doc.doc
        .get_dictionary_mut(parent)?
        .set("MediaBox", box_value);
    doc.doc.get_dictionary_mut(parent)?.set("Rotate", 90);
    doc.insert_blank(1)?;
    doc.move_page(1, 2)?;
    assert!(doc.text(2)?.contains("Original text"));
    assert_eq!(doc.geometry(2)?.rotation, 90);
    Ok(())
}

#[test]
fn page_ranges_validate_and_preserve_requested_order() -> Result<()> {
    assert_eq!(pdf::parse_pages("3, 1-2, 3", 4)?, vec![3, 1, 2]);
    for invalid in ["", "0", "-1", "4-2", "1-9", "1,,2", "1-2-3", "hello"] {
        assert!(pdf::parse_pages(invalid, 4).is_err(), "{invalid}");
    }
    Ok(())
}

#[test]
fn geometry_maps_all_page_rotations_and_crop_offsets() {
    let g = Geometry {
        bounds: [10., 20., 210., 420.],
        rotation: 0,
    };
    assert_eq!(g.point([0., 0.]), [10., 420.]);
    assert_eq!(g.point([1., 1.]), [210., 20.]);
    let g = Geometry { rotation: 90, ..g };
    assert_eq!(g.point([0., 0.]), [10., 20.]);
    assert_eq!(g.point([1., 1.]), [210., 420.]);
    assert_eq!(g.display_size(), [400., 200.]);
    let g = Geometry { rotation: 180, ..g };
    assert_eq!(g.point([0., 0.]), [210., 20.]);
    let g = Geometry { rotation: 270, ..g };
    assert_eq!(g.point([0., 0.]), [210., 420.]);
}

#[test]
fn unicode_metadata_round_trips() -> Result<()> {
    let mut doc = sample();
    doc.set_metadata("Title \u{03bb}", "Ren\u{00e9}", "\u{65e5}\u{672c}")?;
    assert_eq!(doc.metadata(b"Title"), "Title \u{03bb}");
    assert_eq!(doc.metadata(b"Subject"), "\u{65e5}\u{672c}");
    Ok(())
}

#[test]
fn protected_copies_require_password_and_keep_protection_on_save() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let protected = temp.path().join("protected.pdf");
    let saved = temp.path().join("saved.pdf");
    let doc = sample();
    pdf::protected_copy(&doc.doc, "correct horse battery staple", &protected)?;
    assert!(PdfDocument::open(&protected, "wrong").is_err());
    let mut opened = PdfDocument::open(&protected, "correct horse battery staple")?;
    assert!(opened.text(1)?.contains("Original text"));
    opened.add_text(1, [0.1, 0.5], "Encrypted edit", 12.)?;
    opened.save(&saved)?;
    assert!(PdfDocument::open(&saved, "").is_err());
    assert!(
        PdfDocument::open(&saved, "correct horse battery staple")?
            .text(1)?
            .contains("Encrypted edit")
    );
    assert!(pdf::protected_copy(&doc.doc, "", &protected).is_err());
    Ok(())
}

#[test]
fn save_copy_does_not_change_original_path_or_dirty_state() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let original = temp.path().join("original.pdf");
    let copy = temp.path().join("copy.pdf");
    let mut doc = sample();
    doc.save(&original)?;
    doc.rotate(1, 90)?;
    doc.save_copy(&copy)?;
    assert_eq!(doc.path.as_ref(), Some(&original));
    assert!(doc.dirty);
    assert_eq!(PdfDocument::open(&original, "")?.geometry(1)?.rotation, 0);
    assert_eq!(PdfDocument::open(&copy, "")?.geometry(1)?.rotation, 90);
    Ok(())
}

#[test]
fn stream_text_replacement_changes_content_and_can_be_undone() -> Result<()> {
    let mut doc = sample();
    doc.replace_text(1, "Original", "Replaced")?;
    assert!(doc.text(1)?.contains("Replaced text"));
    assert!(!doc.text(1)?.contains("Original"));
    doc.undo();
    assert!(doc.text(1)?.contains("Original"));
    assert!(doc.replace_text(1, "missing", "new").is_err());
    Ok(())
}

#[test]
fn fills_text_forms_and_saves_appearance() -> Result<()> {
    let mut doc = sample();
    let page = doc.page_id(1)?;
    let widget=doc.doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","T"=>pdf::pdf_string("Full name"),"V"=>pdf::pdf_string(""),"Rect"=>vec![50.into(),500.into(),300.into(),525.into()],"P"=>page,"F"=>4});
    doc.doc
        .get_dictionary_mut(page)?
        .set("Annots", vec![Object::Reference(widget)]);
    let form = doc
        .doc
        .add_object(dictionary! {"Fields"=>vec![Object::Reference(widget)]});
    doc.doc.catalog_mut()?.set("AcroForm", form);
    let field = doc.fields().remove(0);
    assert!(field.editable);
    doc.set_field(&field, "Jane Doe")?;
    assert_eq!(doc.fields()[0].value, "Jane Doe");
    assert!(doc.doc.get_dictionary(widget)?.get(b"AP").is_ok());
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("filled.pdf");
    doc.save(&path)?;
    assert_eq!(PdfDocument::open(&path, "")?.fields()[0].value, "Jane Doe");
    Ok(())
}

#[test]
fn fills_checkbox_using_its_actual_on_state() -> Result<()> {
    let mut doc = sample();
    let page = doc.page_id(1)?;
    let off=doc.doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),20.into(),20.into()]},vec![]));
    let on=doc.doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),20.into(),20.into()]},b"0 0 20 20 re f".to_vec()));
    let widget=doc.doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Btn","T"=>pdf::pdf_string("Accepted"),"V"=>"Off","AS"=>"Off","Rect"=>vec![50.into(),400.into(),70.into(),420.into()],"P"=>page,"AP"=>dictionary!{"N"=>dictionary!{"Off"=>off,"Checked"=>on}}});
    let form = doc
        .doc
        .add_object(dictionary! {"Fields"=>vec![Object::Reference(widget)]});
    doc.doc.catalog_mut()?.set("AcroForm", form);
    let field = doc.fields().remove(0);
    doc.set_field(&field, "Yes")?;
    assert_eq!(
        doc.doc.get_dictionary(widget)?.get(b"V")?.as_name()?,
        b"Checked"
    );
    doc.set_field(&field, "Off")?;
    assert_eq!(doc.fields()[0].value, "Off");
    Ok(())
}

#[test]
fn images_create_pdf_pages() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let image = temp.path().join("photo.png");
    image::RgbImage::from_pixel(50, 30, image::Rgb([40, 120, 220])).save(&image)?;
    let doc = pdf::images_to_document(&[image.clone(), image])?;
    assert_eq!(doc.get_pages().len(), 2);
    Ok(())
}

#[test]
fn renderer_draws_added_text_and_rotated_annotations() -> Result<()> {
    if renderer::find_poppler().is_none() {
        return Ok(());
    }
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("render.pdf");
    let mut doc = sample();
    for rotation in [0, 90, 180, 270] {
        if rotation > 0 {
            doc.rotate(1, 90)?;
        }
        doc.add_mark(1, Mark::Highlight, [0.3, 0.45], [0.6, 0.55], "")?;
        doc.save(&path)?;
        let image = renderer::render_page(&path, 1, 600)?;
        let pixel = image.get_pixel(
            (image.width() as f32 * 0.45) as u32,
            (image.height() as f32 * 0.5) as u32,
        );
        assert!(
            pixel[0] > 220 && pixel[2] < 220,
            "rotation={rotation}, pixel={pixel:?}"
        );
    }
    Ok(())
}

#[test]
fn redaction_export_removes_original_content_and_metadata() -> Result<()> {
    if renderer::find_poppler().is_none() {
        return Ok(());
    }
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("redacted.pdf");
    let mut doc = sample();
    doc.set_metadata("Secret title", "Private author", "Hidden subject")?;
    workflows::redacted_copy(
        &doc.doc,
        &[Redaction {
            page: 1,
            start: [0.1, 0.1],
            end: [0.8, 0.3],
        }],
        &path,
    )?;
    let redacted = Document::load(&path)?;
    assert_eq!(redacted.get_pages().len(), 1);
    assert!(redacted.extract_text(&[1])?.trim().is_empty());
    assert!(redacted.trailer.get(b"Info").is_err());
    let image = renderer::render_page(&path, 1, 600)?;
    let pixel = image.get_pixel(300, (image.height() as f32 * 0.2) as u32);
    assert!(pixel[0] < 5 && pixel[1] < 5 && pixel[2] < 5);
    assert!(doc.text(1)?.contains("Original text"));
    Ok(())
}

#[test]
fn ocr_produces_searchable_pdf_with_matching_page_dimensions() -> Result<()> {
    if renderer::find_poppler().is_none() || workflows::find_tesseract().is_none() {
        return Ok(());
    }
    let temp = tempfile::tempdir()?;
    let output = temp.path().join("searchable.pdf");
    let doc = sample();
    workflows::searchable_copy(&doc.doc, &output)?;
    let loaded = PdfDocument::open(&output, "")?;
    assert_eq!(loaded.page_count(), 1);
    assert!(
        loaded
            .text(1)?
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .contains("Original text"),
        "{}",
        loaded.text(1)?
    );
    assert_eq!(loaded.geometry(1)?.display_size(), [612., 792.]);
    Ok(())
}

#[test]
fn history_is_bounded_and_a_new_edit_clears_redo() -> Result<()> {
    let mut doc = sample();
    for _ in 0..25 {
        doc.rotate(1, 90)?;
    }
    for _ in 0..pdf::HISTORY_LIMIT {
        assert!(doc.can_undo());
        doc.undo();
    }
    assert!(!doc.can_undo());
    assert!(doc.can_redo());
    doc.rotate(1, 90)?;
    assert!(!doc.can_redo());
    Ok(())
}
