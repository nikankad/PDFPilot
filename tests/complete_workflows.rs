use anyhow::Result;
use pdfpilot::{
    advanced::{FormKind, FormSpec},
    batch::{Plan, Step},
    forms::{Calculation, FieldProperties},
    inspection,
    pdf::{self, PdfDocument},
    scans,
    security::{self, SecurityOptions},
    service::{Service, verify_audit},
    signatures,
};
use std::{
    io::{Read, Write},
    net::TcpStream,
    process::Command,
};
fn sample() -> Result<PdfDocument> {
    Ok(PdfDocument::from_document(
        pdf::create_text_document("Test", "Document text")?,
        "sample.pdf",
    ))
}
fn field(doc: &mut PdfDocument, kind: FormKind, name: &str, options: Vec<String>) -> Result<()> {
    doc.add_form_field(
        1,
        [0.1, 0.2],
        [0.7, 0.4],
        &FormSpec {
            kind,
            name: name.into(),
            required: false,
            options,
        },
    )
}
#[test]
fn radio_dates_lists_and_signature_fields_roundtrip() -> Result<()> {
    let mut doc = sample()?;
    field(
        &mut doc,
        FormKind::Radio,
        "Choice",
        vec!["Alpha".into(), "Beta".into()],
    )?;
    field(&mut doc, FormKind::Date, "Date", vec![])?;
    field(
        &mut doc,
        FormKind::Listbox,
        "List",
        vec!["One".into(), "Two".into()],
    )?;
    field(&mut doc, FormKind::Signature, "Signature", vec![])?;
    let radio = doc
        .fields()
        .into_iter()
        .find(|f| f.name == "Choice")
        .unwrap();
    assert!(radio.editable);
    assert_eq!(radio.kind, "Radio");
    doc.set_field(&radio, "Beta")?;
    let kids = doc.doc.get_dictionary(radio.id)?.get(b"Kids")?.as_array()?;
    let states = kids
        .iter()
        .map(|o| {
            doc.doc
                .get_dictionary(o.as_reference().unwrap())
                .unwrap()
                .get(b"AS")
                .map(pdf::object_text)
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(states.iter().filter(|s| *s == "Beta").count(), 1);
    assert_eq!(states.iter().filter(|s| *s == "Off").count(), 1);
    assert!(doc.set_field(&radio, "Unknown").is_err());
    let date = doc.fields().into_iter().find(|f| f.name == "Date").unwrap();
    assert!(doc.set_field(&date, "2025-02-29").is_err());
    doc.set_field(&date, "2024-02-29")?;
    doc.order_form_tabs(1)?;
    assert_eq!(
        pdf::object_text(doc.doc.get_dictionary(doc.page_id(1)?)?.get(b"Tabs")?),
        "R"
    );
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("forms.pdf");
    doc.save(&path)?;
    let reopened = PdfDocument::open(&path, "")?;
    assert_eq!(reopened.form_data()["Choice"], "Beta");
    assert_eq!(reopened.form_data()["Date"], "2024-02-29");
    assert!(
        !reopened
            .fields()
            .iter()
            .find(|f| f.name == "Signature")
            .unwrap()
            .editable
    );
    Ok(())
}
#[test]
fn calculations_validation_and_cycles_are_transactional() -> Result<()> {
    let mut doc = sample()?;
    for name in ["Quantity", "Price", "Total"] {
        field(&mut doc, FormKind::Text, name, vec![])?;
    }
    let fields = doc.fields();
    let id = |name: &str| fields.iter().find(|f| f.name == name).unwrap().id;
    doc.configure_field(
        id("Quantity"),
        &FieldProperties {
            required: true,
            numeric_min: Some(0.),
            numeric_max: Some(100.),
            ..Default::default()
        },
    )?;
    assert_eq!(doc.validate_form().len(), 1);
    doc.configure_field(
        id("Total"),
        &FieldProperties {
            calculation: Some((
                Calculation::Product,
                vec!["Quantity".into(), "Price".into()],
            )),
            ..Default::default()
        },
    )?;
    let q = doc
        .fields()
        .into_iter()
        .find(|f| f.name == "Quantity")
        .unwrap();
    let p = doc
        .fields()
        .into_iter()
        .find(|f| f.name == "Price")
        .unwrap();
    doc.set_field(&q, "3")?;
    doc.set_field(&p, "12.5")?;
    assert_eq!(doc.form_data()["Total"], "37.5");
    let revision = doc.revision;
    assert!(doc.set_field(&q, "-2").is_err());
    assert_eq!(doc.revision, revision);
    assert_eq!(doc.form_data()["Quantity"], "3");
    assert!(
        doc.configure_field(
            id("Quantity"),
            &FieldProperties {
                calculation: Some((Calculation::Sum, vec!["Total".into()])),
                ..Default::default()
            }
        )
        .is_err()
    );
    assert_eq!(doc.revision, revision);
    doc.undo();
    assert_eq!(doc.form_data()["Total"], "0");
    Ok(())
}
#[test]
fn owner_authentication_is_required_to_remove_restrictions() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let input = temp.path().join("restricted.pdf");
    let mut options = SecurityOptions {
        owner_password: "owner-secret".into(),
        open_password: "reader-secret".into(),
        print: false,
        edit: false,
        copy: false,
        ..Default::default()
    };
    let doc = sample()?;
    security::protect(&doc.doc, &options, &input)?;
    assert!(PdfDocument::open(&input, "wrong").is_err());
    let opened = PdfDocument::open(&input, "reader-secret")?;
    let report = security::settings(&opened.doc);
    assert!(report.contains("Printing: restricted"));
    assert!(report.contains("Copying: restricted"));
    let output = temp.path().join("plain.pdf");
    assert!(
        opened
            .remove_protection_copy("reader-secret", &output)
            .is_err()
    );
    assert!(!output.exists());
    opened.remove_protection_copy("owner-secret", &output)?;
    assert!(
        PdfDocument::open(&output, "")?
            .doc
            .encryption_state
            .is_none()
    );
    options.owner_password = options.open_password.clone();
    assert!(security::protect(&doc.doc, &options, &input).is_err());
    Ok(())
}
#[test]
fn tags_storage_and_batch_presets_roundtrip() -> Result<()> {
    let mut doc = sample()?;
    doc.tag_page(1, "P", "")?;
    assert!(doc.tag_page(1, "H1", "").is_err());
    let tag = doc.tags()[0].clone();
    doc.edit_tag(tag.id, "Figure", "Overview diagram")?;
    assert_eq!(doc.tags()[0].alternative, "Overview diagram");
    let text = inspection::report(&doc);
    assert!(text.contains("Font objects"));
    assert!(text.contains("Diagnostics only"));
    let temp = tempfile::tempdir()?;
    let input = temp.path().join("source.pdf");
    doc.save(&input)?;
    let plan = Plan {
        steps: vec![
            Step::Header("Header text".into()),
            Step::Footer("Footer text".into()),
            Step::Language("en-US".into()),
            Step::Compress,
        ],
    };
    let preset = temp.path().join("actions.json");
    plan.save(&preset)?;
    assert_eq!(Plan::load(&preset)?, plan);
    let folder = temp.path().join("outputs");
    std::fs::create_dir(&folder)?;
    let outputs = plan.run(std::slice::from_ref(&input), &folder, None)?;
    let output = PdfDocument::open(&outputs[0], "")?;
    assert!(output.text(1)?.contains("Header text"));
    assert!(output.text(1)?.contains("Footer text"));
    assert!(plan.run(&[input], &folder, None).is_err());
    Ok(())
}
#[test]
fn failed_batch_does_not_publish_partial_results() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let input = temp.path().join("first.pdf");
    sample()?.save(&input)?;
    let bad = temp.path().join("broken.pdf");
    std::fs::write(&bad, b"invalid pdf")?;
    let folder = temp.path().join("output");
    std::fs::create_dir(&folder)?;
    assert!(
        Plan {
            steps: vec![Step::Compress]
        }
        .run(&[input, bad], &folder, None)
        .is_err()
    );
    assert_eq!(std::fs::read_dir(folder)?.count(), 0);
    Ok(())
}
#[test]
fn scan_cleanup_removes_isolated_noise() -> Result<()> {
    let mut image = image::RgbImage::from_pixel(20, 20, image::Rgb([255; 3]));
    image.put_pixel(10, 10, image::Rgb([0; 3]));
    let result = scans::enhance(
        image,
        &scans::ScanOptions {
            deskew: false,
            contrast: 0.,
            ..Default::default()
        },
    );
    assert_eq!(result.get_pixel(10, 10).0, [255; 3]);
    Ok(())
}
#[test]
fn detached_certificate_signatures_detect_changes_and_check_trust() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let key = temp.path().join("key.pem");
    let cert = temp.path().join("cert.pem");
    let result = Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-days",
            "1",
            "-subj",
            "/CN=PDFPilot Test",
            "-keyout",
        ])
        .arg(&key)
        .arg("-out")
        .arg(&cert)
        .output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let output = temp.path().join("signed.pdf");
    let mut doc = sample()?;
    doc.set_metadata("9999999997 9999999998 9999999999", "Author", "Subject")?;
    signatures::signed_copy(&doc.doc, &cert, &key, "", &output)?;
    let signed_doc = PdfDocument::open(&output, "")?;
    assert_eq!(
        signed_doc.metadata(b"Title"),
        "9999999997 9999999998 9999999999"
    );
    let widget = signed_doc
        .fields()
        .into_iter()
        .find(|f| f.kind == "Sig")
        .unwrap();
    let appearance = signed_doc
        .doc
        .get_dict_in_dict(signed_doc.doc.get_dictionary(widget.id)?, b"AP")?
        .get(b"N")?
        .as_reference()?;
    let stream = signed_doc.doc.get_object(appearance)?.as_stream()?;
    assert!(
        signed_doc
            .doc
            .get_dict_in_dict(&stream.dict, b"Resources")?
            .get(b"Font")
            .is_ok()
    );
    let reports = signatures::verify(&output, Some(&cert))?;
    assert_eq!(reports.len(), 1);
    assert!(reports[0].integrity);
    assert_eq!(reports[0].trusted, Some(true));
    assert!(reports[0].covers_current_file);
    let mut bytes = std::fs::read(&output)?;
    let position = bytes.windows(9).position(|w| w == b"/Producer").unwrap();
    bytes[position + 8] = b'X';
    let tampered = temp.path().join("tampered.pdf");
    std::fs::write(&tampered, bytes)?;
    assert!(!signatures::verify(&tampered, None)?[0].integrity);
    let signed = PdfDocument::open(&output, "")?;
    assert!(signatures::signed_copy(&signed.doc, &cert, &key, "", &output).is_err());
    Ok(())
}
#[test]
fn independent_review_enforces_signing_order_and_persists_audit() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let service = Service::open(temp.path())?;
    let review = service.publish(
        &sample()?.doc,
        "Review",
        &["First".into(), "Second".into()],
        true,
    )?;
    let id = review["id"].as_str().unwrap();
    let first = review["recipients"][0]["token"].as_str().unwrap();
    let second = review["recipients"][1]["token"].as_str().unwrap();
    let reader = review["review_token"].as_str().unwrap();
    assert!(service.sign(id, reader, "Wrong actor").is_err());
    assert!(service.sign(id, second, "Second").is_err());
    service.comment(id, reader, 1, "Reviewer", "Please check this")?;
    assert!(service.comment(id, "wrong", 1, "Other", "Denied").is_err());
    service.sign(id, first, "First")?;
    assert!(service.sign(id, first, "First").is_err());
    service.remind(id)?;
    service.sign(id, second, "Second")?;
    let state = service.snapshot(id)?;
    verify_audit(&state)?;
    let mut tampered = state.clone();
    tampered["events"][1]["kind"] = serde_json::json!("forged");
    assert!(verify_audit(&tampered).is_err());
    let output = temp.path().join("receipt.pdf");
    service.completed_pdf(id, &output)?;
    let receipt = PdfDocument::open(&output, "")?;
    assert_eq!(receipt.page_count(), 2);
    assert!(receipt.text(2)?.contains("First"));
    let reopened = Service::open(temp.path())?;
    assert_eq!(reopened.snapshot(id)?["recipients"][1]["signed"], true);
    Ok(())
}
#[test]
fn service_http_rejects_unauthenticated_and_oversized_requests() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let service = Service::open(temp.path())?;
    let review = service.publish(&sample()?.doc, "Review", &[], false)?;
    let running = service.start(0)?;
    let address = running.address.strip_prefix("http://").unwrap();
    let request = |text: &str| -> Result<String> {
        let mut stream = TcpStream::connect(address)?;
        stream.set_read_timeout(Some(std::time::Duration::from_secs(4)))?;
        stream.write_all(text.as_bytes())?;
        let mut output = String::new();
        stream.read_to_string(&mut output)?;
        Ok(output)
    };
    let id = review["id"].as_str().unwrap();
    let unauthorized = request(&format!(
        "GET /api/reviews/{id} HTTP/1.1\r\nHost: localhost\r\n\r\n"
    ))?;
    assert!(
        unauthorized.starts_with("HTTP/1.1 401"),
        "Unexpected unauthenticated response: {unauthorized}"
    );
    assert!(
        request(&format!(
            "GET /api/reviews/{id} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {}\r\n\r\n",
            review["review_token"].as_str().unwrap()
        ))?
        .starts_with("HTTP/1.1 200")
    );
    assert!(
        request("POST /api/reviews/x/comments HTTP/1.1\r\nContent-Length: 70000\r\n\r\n")?
            .starts_with("HTTP/1.1 400")
    );
    Ok(())
}

#[test]
fn text_blocks_and_image_edits_preserve_other_content() -> Result<()> {
    use pdfpilot::editing::{Alignment, TextBlock};
    let temp = tempfile::tempdir()?;
    let image_path = temp.path().join("blue.png");
    image::RgbImage::from_pixel(100, 80, image::Rgb([0, 0, 255])).save(&image_path)?;
    let mut doc = sample()?;
    doc.add_image(1, [0.1, 0.6], [0.3, 0.8], &image_path)?;
    let block = TextBlock {
        text: "Paragraph line one\nSecond paragraph".into(),
        alignment: Alignment::Center,
        ..Default::default()
    };
    doc.write_text_block(1, None, &block)?;
    let id = doc.text_blocks(1)?[0].id;
    assert_eq!(doc.text_block_spec(1, id)?.text, block.text);
    assert!(doc.text(1)?.contains("Paragraph line one"));
    let placement = doc.image_placements(1)?[0].operation;
    doc.transform_image(1, placement, [0.5, 0.6, 0.7, 0.8], 0., [0., 0., 1., 1.])?;
    assert_eq!(doc.text_blocks(1)?[0].id, id);
    assert!(doc.text(1)?.contains("Document text"));
    let updated = TextBlock {
        text: "Updated paragraph".into(),
        ..block
    };
    doc.write_text_block(1, Some(id), &updated)?;
    assert!(!doc.text(1)?.contains("Second paragraph"));
    assert!(doc.text(1)?.contains("Updated paragraph"));
    let image = doc.image_placements(1)?[0].clone();
    let geo = doc.geometry(1)?;
    let p = pdfpilot::editing::normalized(geo, [image.matrix[4], image.matrix[5]]);
    assert!((p[0] - 0.5).abs() < 0.0001);
    assert!((p[1] - 0.8).abs() < 0.0001);
    let saved = temp.path().join("edited.pdf");
    doc.save(&saved)?;
    let rendered = pdfpilot::renderer::render_page(&saved, 1, 612)?;
    let blue = rendered.get_pixel(360, 550);
    assert!(blue[2] > 200 && blue[0] < 30);
    doc.delete_image_placement(1, image.operation)?;
    assert!(doc.image_placements(1)?.is_empty());
    assert!(doc.text(1)?.contains("Updated paragraph"));
    doc.delete_text_block(1, id)?;
    assert!(!doc.text(1)?.contains("Updated paragraph"));
    assert!(doc.text(1)?.contains("Document text"));
    Ok(())
}
#[test]
fn calibrated_measurements_persist_and_page_boxes_roundtrip() -> Result<()> {
    use pdfpilot::measurements::Mode;
    let mut doc = sample()?;
    let a = [0.1, 0.2];
    let b = [0.1 + 72. / 612., 0.2];
    let value = doc.measure(1, a, b, Mode::Distance, 2., true)?;
    assert!((value - 50.8).abs() < 0.001);
    assert!(doc.comments().iter().any(|c| c.text.contains("50.80 mm")));
    doc.measure(1, [0.1, 0.2], [0.2, 0.3], Mode::Area, 1., true)?;
    assert!(doc.comments().iter().any(|c| c.text.contains("mm2")));
    assert!(doc.measure(1, a, a, Mode::Distance, 1., false).is_err());
    doc.set_page_box(1, "TrimBox", [10., 10., 600., 780.])?;
    assert!(doc.set_page_box(1, "Unknown", [0., 0., 1., 1.]).is_err());
    doc.page_background(1, [0.8, 0.9, 1.])?;
    let temp = tempfile::tempdir()?;
    let file = temp.path().join("measured.pdf");
    doc.save(&file)?;
    let reopened = PdfDocument::open(&file, "")?;
    assert!(
        reopened
            .doc
            .get_dictionary(reopened.page_id(1)?)?
            .get(b"TrimBox")
            .is_ok()
    );
    Ok(())
}

#[test]
fn vector_form_detection_and_ink_annotations_are_editable() -> Result<()> {
    let mut doc = sample()?;
    let id = doc.page_id(1)?;
    let stream = doc.doc.add_object(lopdf::Stream::new(
        lopdf::dictionary! {},
        b"q 0 0 0 RG 100 300 200 30 re S 100 250 12 12 re S Q".to_vec(),
    ));
    let mut contents = doc
        .doc
        .get_page_contents(id)
        .into_iter()
        .map(lopdf::Object::Reference)
        .collect::<Vec<_>>();
    contents.push(stream.into());
    doc.doc.get_dictionary_mut(id)?.set("Contents", contents);
    let candidates = doc.detect_field_candidates(1)?;
    assert_eq!(candidates.len(), 2);
    doc.create_candidate_fields(1, &candidates)?;
    assert_eq!(doc.fields().len(), 2);
    assert!(doc.detect_field_candidates(1)?.is_empty());
    let original = doc.text(1)?;
    doc.add_ink(1, &[[0.1, 0.6], [0.2, 0.65], [0.3, 0.6]], 2.)?;
    let ink = doc
        .comments()
        .into_iter()
        .find(|c| c.kind == "Ink")
        .unwrap();
    assert!(doc.doc.get_dictionary(ink.id)?.get(b"InkList").is_ok());
    assert_eq!(doc.text(1)?, original);
    doc.remove_comment(&ink)?;
    assert!(!doc.comments().iter().any(|c| c.kind == "Ink"));
    Ok(())
}
#[test]
fn reader_permissions_block_edits_but_owner_can_edit() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let input = temp.path().join("restricted.pdf");
    security::protect(
        &sample()?.doc,
        &SecurityOptions {
            owner_password: "owner".into(),
            open_password: "reader".into(),
            edit: false,
            copy: false,
            annotate: false,
            assemble: false,
            ..Default::default()
        },
        &input,
    )?;
    let mut reader = PdfDocument::open(&input, "reader")?;
    assert!(!reader.owner_authenticated());
    assert!(!reader.permits(lopdf::Permissions::COPYABLE));
    assert!(
        reader
            .extract(&[1], &temp.path().join("extracted.pdf"))
            .is_err()
    );
    assert!(reader.add_text(1, [0.1, 0.1], "Forbidden", 12.).is_err());
    assert!(reader.add_ink(1, &[[0.1, 0.1], [0.2, 0.2]], 1.).is_err());
    let mut owner = PdfDocument::open(&input, "owner")?;
    assert!(owner.owner_authenticated());
    owner.add_text(1, [0.1, 0.1], "Allowed", 12.)?;
    Ok(())
}
#[test]
fn booklet_imposes_padded_pages_in_duplex_order() -> Result<()> {
    use pdfpilot::printing::{self, Paper};
    assert_eq!(
        printing::booklet_order(&[1, 2, 3]),
        vec![[None, Some(1)], [Some(2), Some(3)]]
    );
    assert_eq!(
        printing::booklet_order(&[1, 2, 3, 4, 5, 6, 7, 8]),
        vec![
            [Some(8), Some(1)],
            [Some(2), Some(7)],
            [Some(6), Some(3)],
            [Some(4), Some(5)]
        ]
    );
    let mut doc = sample()?;
    doc.duplicate_page(1)?;
    doc.duplicate_page(1)?;
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("booklet.pdf");
    printing::booklet(&doc.doc, &[1, 2, 3], Paper::Letter, true, &path)?;
    let reopened = PdfDocument::open(&path, "")?;
    assert_eq!(reopened.page_count(), 2);
    assert_eq!(reopened.geometry(1)?.display_size(), [792., 612.]);
    let page = pdfpilot::renderer::render_page(&path, 1, 792)?;
    assert!(page.width() > page.height());
    Ok(())
}

#[test]
fn correction_and_attachment_comments_preserve_embedded_files() -> Result<()> {
    let mut doc = sample()?;
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("note.txt");
    std::fs::write(&source, b"Review evidence")?;
    doc.add_correction(1, [0.2, 0.2], "Insert missing word")?;
    doc.attach_comment(1, [0.3, 0.3], &source, "Supporting note")?;
    let saved = temp.path().join("review.pdf");
    doc.save(&saved)?;
    let loaded = PdfDocument::open(&saved, "")?;
    assert!(loaded.comments().iter().any(|c| c.kind == "Caret"));
    let comment = loaded
        .comments()
        .into_iter()
        .find(|c| c.kind == "FileAttachment")
        .unwrap();
    assert!(loaded.doc.get_dictionary(comment.id)?.get(b"FS").is_ok());
    let output = temp.path().join("extracted.txt");
    loaded.export_attachment(&loaded.attachments()[0], &output)?;
    assert_eq!(std::fs::read(output)?, b"Review evidence");
    Ok(())
}

#[test]
fn removed_calculations_clear_actions_and_saved_order() -> Result<()> {
    let mut doc = sample()?;
    field(&mut doc, FormKind::Text, "Input", vec![])?;
    field(&mut doc, FormKind::Text, "Result", vec![])?;
    let result = doc
        .fields()
        .into_iter()
        .find(|f| f.name == "Result")
        .unwrap()
        .id;
    doc.configure_field(
        result,
        &FieldProperties {
            calculation: Some((Calculation::Sum, vec!["Input".into()])),
            ..Default::default()
        },
    )?;
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("calculated.pdf");
    doc.save(&path)?;
    let mut saved = PdfDocument::open(&path, "")?;
    let form = saved
        .doc
        .get_dict_in_dict(saved.doc.catalog()?, b"AcroForm")?;
    assert_eq!(form.get(b"CO")?.as_array()?.len(), 1);
    assert!(
        saved
            .doc
            .get_dict_in_dict(saved.doc.get_dictionary(result)?, b"AA")?
            .get(b"C")
            .is_ok()
    );
    saved.configure_field(result, &FieldProperties::default())?;
    saved.save(&path)?;
    let reopened = PdfDocument::open(&path, "")?;
    assert!(reopened.doc.get_dictionary(result)?.get(b"AA").is_err());
    assert!(
        reopened
            .doc
            .get_dict_in_dict(reopened.doc.catalog()?, b"AcroForm")?
            .get(b"CO")?
            .as_array()?
            .is_empty()
    );
    assert!(
        reopened
            .fields()
            .iter()
            .find(|f| f.id == result)
            .unwrap()
            .editable
    );
    Ok(())
}
#[test]
fn unicode_signing_records_preserve_names_without_access_tokens() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let service = Service::open(temp.path())?;
    let review = service.publish(&sample()?.doc, "合同", &["李明".into()], false)?;
    let id = review["id"].as_str().unwrap();
    let token = review["recipients"][0]["token"].as_str().unwrap();
    assert!(service.sign(id, token, "Name\nInjected line").is_err());
    service.sign(id, token, "李明")?;
    let output = temp.path().join("receipt.pdf");
    service.completed_pdf(id, &output)?;
    let receipt = PdfDocument::open(&output, "")?;
    let evidence = receipt
        .attachments()
        .into_iter()
        .find(|a| a.name == "signing-record.json")
        .unwrap();
    let file = temp.path().join("record.json");
    receipt.export_attachment(&evidence, &file)?;
    let bytes = std::fs::read(&file)?;
    let record: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(record["recipients"][0]["typed_signature"], "李明");
    assert!(!String::from_utf8(bytes)?.contains(token));
    assert!(record.get("review_token").is_none());
    verify_audit(&record)?;
    Ok(())
}
#[test]
fn restricted_annotation_passwords_must_differ() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let options = SecurityOptions {
        open_password: "same".into(),
        owner_password: "same".into(),
        annotate: false,
        ..Default::default()
    };
    assert!(
        security::protect(
            &sample()?.doc,
            &options,
            &temp.path().join("restricted.pdf")
        )
        .is_err()
    );
    Ok(())
}
#[test]
fn scan_optimization_exports_renderable_flattened_copy() -> Result<()> {
    let mut doc = sample()?;
    field(&mut doc, FormKind::Text, "PrivateField", vec![])?;
    let temp = tempfile::tempdir()?;
    let output = temp.path().join("scan.pdf");
    scans::optimized_copy(
        &doc.doc,
        &scans::ScanOptions {
            width: 600,
            deskew: false,
            ..Default::default()
        },
        &output,
    )?;
    let scan = PdfDocument::open(&output, "")?;
    assert_eq!(scan.page_count(), 1);
    assert!(scan.fields().is_empty());
    assert!(scan.text(1)?.trim().is_empty());
    let image = pdfpilot::renderer::render_page(&output, 1, 600)?;
    assert!(image.pixels().any(|p| p.0[0] < 200));
    assert_eq!(doc.fields().len(), 1);
    Ok(())
}
