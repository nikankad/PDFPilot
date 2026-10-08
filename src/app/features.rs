use super::*;
use pdfpilot::{
    advanced::{Attachment, FormKind, FormSpec, Shape},
    conversion::{self, ExportFormat},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RunningKind {
    Header,
    Footer,
    Watermark,
    PageNumbers,
    Bates,
}
impl RunningKind {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Header => "Header",
            Self::Footer => "Footer",
            Self::Watermark => "Watermark",
            Self::PageNumbers => "Page numbers",
            Self::Bates => "Bates numbering",
        }
    }
}

pub(super) enum FeatureAction {
    Studio(super::studio::StudioAction),
    Duplicate,
    Split,
    ReplacePage,
    Crop([f32; 2], [f32; 2]),
    ResetCrop,
    RotateRange(i64),
    Reorder(u32, u32),
    Bookmark,
    PageLabels,
    Attach,
    ExportAttachment(Attachment),
    Export,
    ImportForm,
    AddField([f32; 2], [f32; 2]),
    Shape(Shape, [f32; 2], [f32; 2]),
    Link([f32; 2], [f32; 2]),
    UpdateLink(lopdf::ObjectId, String),
    Resolve(lopdf::ObjectId, bool),
    Reply(lopdf::ObjectId, u32),
    StripMetadata,
    Language,
    ImageImport,
    Image([f32; 2], [f32; 2]),
    RunningText,
    Compare,
    ExportReport,
    Measure([f32; 2], [f32; 2]),
    ReadAloud,
    StopReading,
    Print,
    PrintNow,
    Batch,
}

pub(super) struct Features {
    pub studio: super::studio::Studio,
    pub export_format: ExportFormat,
    pub export_all: bool,
    pub export_range: String,
    pub split_size: u32,
    pub source_page: u32,
    pub bookmark: String,
    pub prefix: String,
    pub label_start: i64,
    pub field_kind: FormKind,
    pub field_name: String,
    pub field_required: bool,
    pub field_options: String,
    pub url: String,
    pub link_values: HashMap<lopdf::ObjectId, String>,
    pub style: pdf::TextStyle,
    pub image: Option<PathBuf>,
    pub running_kind: RunningKind,
    pub running_text: String,
    pub number_start: u32,
    pub number_padding: usize,
    pub language: String,
    pub comment_filter: String,
    pub show_resolved: bool,
    pub measurement_scale: f32,
    pub report: String,
    pub report_html: String,
    pub speech: Option<std::process::Child>,
    pub print_range: String,
    pub print_all: bool,
    pub print_nup: u32,
    pub print_gray: bool,
    pub print_landscape: bool,
    pub print_fit: bool,
    pub print_paper: pdfpilot::printing::Paper,
    pub print_scale: u32,
    pub print_booklet: bool,
    pub print_marks: bool,
    pub ocr: pdfpilot::workflows::OcrOptions,
    pub batch_watermark: bool,
}
impl Default for Features {
    fn default() -> Self {
        Self {
            studio: Default::default(),
            export_format: ExportFormat::Docx,
            export_all: true,
            export_range: "1".into(),
            split_size: 1,
            source_page: 1,
            bookmark: String::new(),
            prefix: String::new(),
            label_start: 1,
            field_kind: FormKind::Text,
            field_name: "Field 1".into(),
            field_required: false,
            field_options: "Option 1, Option 2".into(),
            url: "https://".into(),
            link_values: HashMap::new(),
            style: Default::default(),
            image: None,
            running_kind: RunningKind::PageNumbers,
            running_text: String::new(),
            number_start: 1,
            number_padding: 6,
            language: "en-US".into(),
            comment_filter: String::new(),
            show_resolved: true,
            measurement_scale: 1.,
            report: String::new(),
            report_html: String::new(),
            speech: None,
            print_range: "1".into(),
            print_all: true,
            print_nup: 1,
            print_gray: false,
            print_landscape: false,
            print_fit: true,
            print_paper: pdfpilot::printing::Paper::Letter,
            print_scale: 100,
            print_booklet: false,
            print_marks: false,
            ocr: Default::default(),
            batch_watermark: false,
        }
    }
}
impl Drop for Features {
    fn drop(&mut self) {
        if let Some(child) = self.speech.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl PdfApp {
    pub(super) fn pages_panel(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        section(ui, "ORGANIZE PAGES");
        let s = self.current().unwrap();
        let page = s.page;
        let count = s.pdf.page_count();
        egui::Grid::new("page_actions")
            .num_columns(3)
            .spacing([8., 8.])
            .show(ui, |ui| {
                if icons::tile(
                    ui,
                    Icon::RotateLeft,
                    "Rotate current page left",
                    "Left",
                    true,
                )
                .clicked()
                {
                    *action = Some(Action::Rotate(-90));
                }
                if icons::tile(
                    ui,
                    Icon::RotateRight,
                    "Rotate current page right",
                    "Right",
                    true,
                )
                .clicked()
                {
                    *action = Some(Action::Rotate(90));
                }
                if icons::tile(ui, Icon::New, "Insert blank page", "Insert", true).clicked() {
                    *action = Some(Action::Insert);
                }
                ui.end_row();
                if icons::tile(ui, Icon::Left, "Move page earlier", "Earlier", page > 1).clicked() {
                    *action = Some(Action::Move(page - 1));
                }
                if icons::tile(ui, Icon::Right, "Move page later", "Later", page < count).clicked()
                {
                    *action = Some(Action::Move(page + 1));
                }
                if icons::tile(
                    ui,
                    Icon::Duplicate,
                    "Duplicate current page",
                    "Duplicate",
                    true,
                )
                .clicked()
                {
                    feature(action, FeatureAction::Duplicate);
                }
                ui.end_row();
                if icons::tile(ui, Icon::Merge, "Combine with another PDF", "Combine", true)
                    .clicked()
                {
                    *action = Some(Action::Merge);
                }
            });
        ui.add_space(12.);
        ui.label("Page selection");
        ui.add(
            egui::TextEdit::singleline(&mut self.page_range)
                .hint_text("1, 3-5")
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            if icons::tile(ui, Icon::Export, "Extract selected pages", "Extract", true).clicked() {
                *action = Some(Action::Extract);
            }
            if icons::tile(ui, Icon::Delete, "Delete selected pages", "Delete", true).clicked() {
                *action = Some(Action::Delete);
            }
            if icons::tile(
                ui,
                Icon::RotateRight,
                "Rotate selection clockwise",
                "Rotate",
                true,
            )
            .clicked()
            {
                feature(action, FeatureAction::RotateRange(90));
            }
        });
        ui.separator();
        ui.add_space(8.);
        section(ui, "CROP & REPLACE");
        if wide_button(ui, Icon::Crop, "Draw a page crop").clicked() {
            self.tool = Tool::Crop;
        }
        if wide_button(ui, Icon::Reset, "Reset current page crop").clicked() {
            feature(action, FeatureAction::ResetCrop);
        }
        ui.horizontal(|ui| {
            ui.label("Source page");
            ui.add(egui::DragValue::new(&mut self.features.source_page).range(1..=100_000));
        });
        if wide_button(ui, Icon::Replace, "Replace from another PDF").clicked() {
            feature(action, FeatureAction::ReplacePage);
        }
        ui.add_space(12.);
        section(ui, "SPLIT DOCUMENT");
        ui.horizontal(|ui| {
            ui.label("Pages per file");
            ui.add(egui::DragValue::new(&mut self.features.split_size).range(1..=100_000));
        });
        if wide_button(ui, Icon::Split, "Split into a folder").clicked() {
            feature(action, FeatureAction::Split);
        }
        ui.add_space(12.);
        section(ui, "PAGE LABELS");
        ui.add(
            egui::TextEdit::singleline(&mut self.features.prefix)
                .hint_text("Label prefix")
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            ui.label("Start at");
            ui.add(egui::DragValue::new(&mut self.features.label_start).range(1..=1_000_000));
        });
        if wide_button(ui, Icon::Labels, "Apply page labels").clicked() {
            feature(action, FeatureAction::PageLabels);
        }
    }

    pub(super) fn export_panel(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        section(ui, "EXPORT YOUR DOCUMENT");
        egui::ComboBox::from_id_salt("export_format")
            .selected_text(self.features.export_format.label())
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for format in [
                    ExportFormat::Docx,
                    ExportFormat::Xlsx,
                    ExportFormat::Pptx,
                    ExportFormat::Html,
                    ExportFormat::Text,
                    ExportFormat::Png,
                    ExportFormat::Jpeg,
                    ExportFormat::Tiff,
                    ExportFormat::Comments,
                    ExportFormat::FormData,
                ] {
                    ui.selectable_value(&mut self.features.export_format, format, format.label());
                }
            });
        let image = matches!(
            self.features.export_format,
            ExportFormat::Png | ExportFormat::Jpeg | ExportFormat::Tiff
        );
        if image {
            ui.label(
                RichText::new("Exports the current page.")
                    .size(12.)
                    .color(palette(ui.ctx()).muted),
            );
        } else {
            ui.checkbox(&mut self.features.export_all, "All pages");
            if !self.features.export_all {
                ui.add(
                    egui::TextEdit::singleline(&mut self.features.export_range)
                        .hint_text("1, 3-5")
                        .desired_width(f32::INFINITY),
                );
            }
        }
        if primary(ui, Icon::Export, "Export file").clicked() {
            feature(action, FeatureAction::Export);
        }
        ui.add_space(16.);
        section(ui, "SCANS & SECURITY");
        egui::Grid::new("security_actions")
            .num_columns(3)
            .spacing([8., 8.])
            .show(ui, |ui| {
                if icons::tile(ui, Icon::Scan, "OCR to searchable PDF", "OCR", true).clicked() {
                    *action = Some(Action::Ocr);
                }
                if icons::tile(ui, Icon::Redact, "Redact a region", "Redact", true).clicked() {
                    self.tool = Tool::Redact;
                    self.panel = Panel::Tools;
                }
                if icons::tile(
                    ui,
                    Icon::Lock,
                    "Save password-protected copy",
                    "Protect",
                    true,
                )
                .clicked()
                {
                    *action = Some(Action::Protect);
                }
                ui.end_row();
                if icons::tile(ui, Icon::Compress, "Compress PDF streams", "Compress", true)
                    .clicked()
                {
                    *action = Some(Action::Compress);
                }
                if icons::tile(
                    ui,
                    Icon::Delete,
                    "Remove document metadata",
                    "Metadata",
                    true,
                )
                .clicked()
                {
                    feature(action, FeatureAction::StripMetadata);
                }
            });
        ui.add_space(16.);
        section(ui, "COMPARE & PRINT");
        ui.horizontal(|ui| {
            if icons::tile(
                ui,
                Icon::Compare,
                "Compare with another PDF",
                "Compare",
                true,
            )
            .clicked()
            {
                feature(action, FeatureAction::Compare);
            }
            if icons::tile(ui, Icon::Print, "Print settings", "Print", true).clicked() {
                feature(action, FeatureAction::Print);
            }
        });
        if !self.features.report.is_empty()
            && wide_button(ui, Icon::Compare, "Open comparison report").clicked()
        {
            self.modal = Some(Modal::Report);
        }
        ui.add_space(16.);
        self.export_settings(ui, action);
        section(ui, "QUICK BATCH");
        ui.checkbox(
            &mut self.features.batch_watermark,
            "Also apply watermark text",
        );
        if self.features.batch_watermark {
            ui.add(
                egui::TextEdit::singleline(&mut self.features.running_text)
                    .hint_text("Watermark text")
                    .desired_width(f32::INFINITY),
            );
        }
        if wide_button(ui, Icon::Batch, "Compress multiple PDFs").clicked() {
            feature(action, FeatureAction::Batch);
        }
    }

    pub(super) fn files_panel(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        section(ui, "BOOKMARKS");
        for outline in self.current().unwrap().pdf.outlines() {
            let title = format!("{}{}", "  ".repeat(outline.depth.min(12)), outline.title);
            if ui
                .add_enabled(outline.page.is_some(), egui::Button::new(title))
                .clicked()
            {
                *action = Some(Action::Page(outline.page.unwrap()));
            }
        }
        ui.add(
            egui::TextEdit::singleline(&mut self.features.bookmark)
                .hint_text("Bookmark title")
                .desired_width(f32::INFINITY),
        );
        if wide_button(ui, Icon::Bookmark, "Bookmark current page").clicked() {
            feature(action, FeatureAction::Bookmark);
        }
        ui.add_space(16.);
        section(ui, "ATTACHMENTS");
        let attachments = self.current().unwrap().pdf.attachments();
        if attachments.is_empty() {
            ui.label(RichText::new("No embedded files.").color(palette(ui.ctx()).muted));
        }
        for attachment in attachments {
            ui.label(&attachment.name);
            if let Some(size) = attachment.size {
                ui.label(
                    RichText::new(format!("{:.1} KB", size as f64 / 1024.))
                        .size(11.)
                        .color(palette(ui.ctx()).muted),
                );
            }
            if ui.small_button("Save attachment").clicked() {
                feature(action, FeatureAction::ExportAttachment(attachment));
            }
        }
        if wide_button(ui, Icon::Files, "Attach a local file").clicked() {
            feature(action, FeatureAction::Attach);
        }
        ui.add_space(16.);
        section(ui, "DOCUMENT LINKS");
        for link in self.current().unwrap().pdf.links() {
            if ui.link(format!("Page {}", link.page)).clicked() {
                *action = Some(Action::Page(link.page));
            }
            ui.hyperlink_to("Open link", &link.uri);
            let value = self.features.link_values.entry(link.id).or_insert(link.uri);
            ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY));
            if ui.small_button("Update URL").clicked() {
                feature(action, FeatureAction::UpdateLink(link.id, value.clone()));
            }
        }
        ui.add(
            egui::TextEdit::singleline(&mut self.features.url)
                .hint_text("https://...")
                .desired_width(f32::INFINITY),
        );
        if wide_button(ui, Icon::Link, "Draw a new link area").clicked() {
            self.tool = Tool::Link;
        }
    }

    pub(super) fn form_author_panel(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        section(ui, "CREATE A FORM FIELD");
        egui::ComboBox::from_id_salt("field_kind")
            .selected_text(self.features.field_kind.label())
            .show_ui(ui, |ui| {
                for kind in [
                    FormKind::Text,
                    FormKind::Checkbox,
                    FormKind::Dropdown,
                    FormKind::Radio,
                    FormKind::Listbox,
                    FormKind::Date,
                    FormKind::Signature,
                ] {
                    ui.selectable_value(&mut self.features.field_kind, kind, kind.label());
                }
            });
        ui.add(
            egui::TextEdit::singleline(&mut self.features.field_name)
                .hint_text("Unique field name")
                .desired_width(f32::INFINITY),
        );
        ui.checkbox(&mut self.features.field_required, "Required field");
        if matches!(
            self.features.field_kind,
            FormKind::Dropdown | FormKind::Listbox | FormKind::Radio
        ) {
            ui.add(
                egui::TextEdit::singleline(&mut self.features.field_options)
                    .hint_text("Options separated by commas")
                    .desired_width(f32::INFINITY),
            );
        }
        if primary(ui, Icon::Form, "Draw field area").clicked() {
            self.tool = Tool::Form;
        }
        if self.tool == Tool::Form {
            ui.label(
                RichText::new("Drag on the page to place the field.")
                    .color(palette(ui.ctx()).accent),
            );
        }
        self.form_settings(ui, action);
        ui.horizontal(|ui| {
            if ui.button("Import data").clicked() {
                feature(action, FeatureAction::ImportForm);
            }
            if ui.button("Export data").clicked() {
                self.features.export_format = ExportFormat::FormData;
                feature(action, FeatureAction::Export);
            }
        });
        ui.separator();
        ui.add_space(10.);
    }

    pub(super) fn execute_feature(&mut self, action: FeatureAction) {
        if let Some(session) = self.current() {
            let permission = match &action {
                FeatureAction::Export | FeatureAction::ExportAttachment(_) => {
                    Some(lopdf::Permissions::COPYABLE)
                }
                FeatureAction::PrintNow => Some(lopdf::Permissions::PRINTABLE),
                _ => None,
            };
            if let Some(permission) = permission
                && !session.pdf.permits(permission)
            {
                self.error = Some(
                    "Document permissions restrict this operation. Reopen with the owner password."
                        .into(),
                );
                return;
            }
        }
        if let FeatureAction::Studio(action) = action {
            self.execute_studio(action);
            return;
        }
        let Some(s) = self.current() else {
            return;
        };
        let page = s.page;
        let result = match action {
            FeatureAction::Export => {
                let format = self.features.export_format;
                let pages = if matches!(
                    format,
                    ExportFormat::Png | ExportFormat::Jpeg | ExportFormat::Tiff
                ) {
                    Ok(vec![page])
                } else if self.features.export_all {
                    Ok((1..=s.pdf.page_count()).collect())
                } else {
                    pdf::parse_pages(&self.features.export_range, s.pdf.page_count())
                };
                let pages = match pages {
                    Ok(pages) => pages,
                    Err(e) => {
                        self.run(Err(e), "");
                        return;
                    }
                };
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(format.label(), &[format.extension()])
                    .set_file_name(format!("document.{}", format.extension()))
                    .save_file()
                {
                    let doc = s.pdf.doc.clone();
                    self.background(move || {
                        conversion::export(&doc, &pages, format, &path)?;
                        Ok(format!("Exported {}", path.display()))
                    });
                }
                return;
            }
            FeatureAction::Split => {
                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                    let doc = s.pdf.doc.clone();
                    let size = self.features.split_size;
                    self.background(move || {
                        let doc = PdfDocument::from_document(doc, "split.pdf");
                        let count = doc.split_to_folder(size, &folder)?;
                        Ok(format!("Created {count} PDFs in {}", folder.display()))
                    });
                }
                return;
            }
            FeatureAction::ExportAttachment(attachment) => {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name(
                        PathBuf::from(&attachment.name)
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy(),
                    )
                    .save_file()
                {
                    let result = s.pdf.export_attachment(&attachment, &path);
                    self.run(result, "Attachment saved");
                }
                return;
            }
            FeatureAction::ExportReport => {
                let html = !self.features.report_html.is_empty();
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(
                        if html { "HTML" } else { "Text" },
                        &[if html { "html" } else { "txt" }],
                    )
                    .set_file_name(if html {
                        "comparison.html"
                    } else {
                        "report.txt"
                    })
                    .save_file()
                {
                    let result = std::fs::write(
                        path,
                        if html {
                            &self.features.report_html
                        } else {
                            &self.features.report
                        },
                    )
                    .map_err(Into::into);
                    self.run(result, "Report saved");
                }
                return;
            }
            FeatureAction::Compare => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("PDF", &["pdf"])
                    .pick_file()
                {
                    let doc = s.pdf.doc.clone();
                    let tx = self.sender.clone();
                    self.jobs += 1;
                    self.status = "Comparing documents...".into();
                    std::thread::spawn(move || {
                        let result = pdfpilot::workflows::compare_documents(&doc, &path);
                        let _ = tx.send(Event::Report(result));
                    });
                }
                return;
            }
            FeatureAction::Print => {
                self.features.print_range = page.to_string();
                self.modal = Some(Modal::Print);
                return;
            }
            FeatureAction::PrintNow => {
                let pages = if self.features.print_all {
                    Ok((1..=s.pdf.page_count()).collect())
                } else {
                    pdf::parse_pages(&self.features.print_range, s.pdf.page_count())
                };
                let pages = match pages {
                    Ok(p) => p,
                    Err(e) => {
                        self.run(Err(e), "");
                        return;
                    }
                };
                let doc = s.pdf.doc.clone();
                let paper = self.features.print_paper;
                let percentage = self.features.print_scale;
                let booklet = self.features.print_booklet;
                let marks = self.features.print_marks;
                let nup = self.features.print_nup;
                let gray = self.features.print_gray;
                let landscape = self.features.print_landscape;
                let fit = self.features.print_fit;
                self.background(move || {
                    let temp = tempfile::tempdir()?;
                    let path = temp.path().join("print.pdf");
                    if booklet {
                        pdfpilot::printing::booklet(&doc, &pages, paper, marks, &path)?;
                    } else {
                        pdf::atomic_save(&doc, &path)?;
                    }
                    let ranges = if booklet {
                        (1..=(pages.len().div_ceil(4) * 2) as u32).collect::<Vec<_>>()
                    } else {
                        pages
                    }
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                    let output = std::process::Command::new("lp")
                        .args([
                            "-o",
                            &format!("page-ranges={ranges}"),
                            "-o",
                            &format!("number-up={}", if booklet { 1 } else { nup }),
                            "-o",
                            &format!("media={}", paper.label()),
                            "-o",
                            &format!("scaling={percentage}"),
                            "-o",
                            if booklet {
                                "sides=two-sided-short-edge"
                            } else {
                                "sides=one-sided"
                            },
                            "-o",
                            if gray {
                                "print-color-mode=monochrome"
                            } else {
                                "print-color-mode=color"
                            },
                            "-o",
                            if landscape || booklet {
                                "orientation-requested=4"
                            } else {
                                "orientation-requested=3"
                            },
                            "-o",
                            if fit || booklet {
                                "fit-to-page=true"
                            } else {
                                "fit-to-page=false"
                            },
                        ])
                        .arg(path)
                        .output()
                        .context("Printing requires CUPS lp and a default printer")?;
                    anyhow::ensure!(
                        output.status.success(),
                        "{}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    Ok(String::from_utf8_lossy(&output.stdout).trim().into())
                });
                return;
            }
            FeatureAction::ReadAloud => {
                use std::io::Write;
                let text = s.pdf.text(page);
                if let Some(mut old) = self.features.speech.take() {
                    let _ = old.kill();
                    let _ = old.wait();
                }
                text.and_then(|text| {
                    let mut child = std::process::Command::new("say")
                        .stdin(std::process::Stdio::piped())
                        .spawn()
                        .context("Read aloud currently requires macOS speech")?;
                    if let Some(mut stdin) = child.stdin.take() {
                        stdin.write_all(text.as_bytes())?;
                    }
                    self.features.speech = Some(child);
                    Ok(())
                })
            }
            FeatureAction::StopReading => {
                if let Some(mut child) = self.features.speech.take() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                Ok(())
            }
            FeatureAction::Batch => {
                if let Some(paths) = rfd::FileDialog::new()
                    .add_filter("PDF", &["pdf"])
                    .pick_files()
                    && let Some(folder) = rfd::FileDialog::new().pick_folder()
                {
                    let watermark = self.features.batch_watermark;
                    let text = self.features.running_text.clone();
                    self.background(move || {
                        let mut steps = vec![];
                        if watermark {
                            steps.push(pdfpilot::batch::Step::Watermark(text));
                        }
                        steps.push(pdfpilot::batch::Step::Compress);
                        let count = pdfpilot::batch::Plan { steps }
                            .run(&paths, &folder, None)?
                            .len();
                        Ok(format!("Processed {count} PDFs"))
                    });
                }
                return;
            }
            FeatureAction::ImageImport => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Images", &["png", "jpg", "jpeg"])
                    .pick_file()
                {
                    self.features.image = Some(path);
                    self.tool = Tool::Image;
                }
                return;
            }
            FeatureAction::Measure(a, b) => {
                let mode = self.features.studio.measure_mode;
                let persist = self.features.studio.persist_measurement;
                let scale = self.features.measurement_scale;
                let value = self
                    .current_mut()
                    .unwrap()
                    .pdf
                    .measure(page, a, b, mode, scale, persist);
                match value {
                    Ok(value) => {
                        if mode == pdfpilot::measurements::Mode::Calibrate {
                            self.features.measurement_scale *=
                                self.features.studio.known_mm / value;
                            self.status = format!(
                                "Calibrated scale 1:{:.4}",
                                self.features.measurement_scale
                            );
                        } else {
                            self.status = format!(
                                "{}: {value:.2} {}",
                                mode.label(),
                                if mode == pdfpilot::measurements::Mode::Area {
                                    "mm2"
                                } else {
                                    "mm"
                                }
                            );
                        }
                    }
                    Err(e) => self.error = Some(e.to_string()),
                }
                return;
            }
            other => {
                let bookmark = self.features.bookmark.clone();
                let prefix = self.features.prefix.clone();
                let label_start = self.features.label_start;
                let kind = self.features.field_kind;
                let name = self.features.field_name.clone();
                let required = self.features.field_required;
                let options = self
                    .features
                    .field_options
                    .split(',')
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>();
                let url = self.features.url.clone();
                let image = self.features.image.clone();
                let running_kind = self.features.running_kind;
                let running_text = self.features.running_text.clone();
                let number_start = self.features.number_start;
                let padding = self.features.number_padding;
                let language = self.features.language.clone();
                let note = self.note.clone();
                let range = self.page_range.clone();
                let source_page = self.features.source_page;
                let s = self.current_mut().unwrap();
                match other {
                    FeatureAction::Duplicate => {
                        s.pdf.duplicate_page(page).map(|_| s.page = page + 1)
                    }
                    FeatureAction::Crop(a, b) => s.pdf.crop_page(page, a, b),
                    FeatureAction::ResetCrop => s.pdf.reset_crop(page),
                    FeatureAction::RotateRange(degrees) => {
                        pdf::parse_pages(&range, s.pdf.page_count())
                            .and_then(|p| s.pdf.rotate_range(&p, degrees))
                    }
                    FeatureAction::Reorder(from, to) => {
                        s.pdf.move_page(from, to).map(|_| s.page = to)
                    }
                    FeatureAction::ReplacePage => {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("PDF", &["pdf"])
                            .pick_file()
                        {
                            s.pdf.replace_page(page, &path, source_page)
                        } else {
                            return;
                        }
                    }
                    FeatureAction::Bookmark => s.pdf.add_outline(page, &bookmark),
                    FeatureAction::PageLabels => s.pdf.set_page_labels(&prefix, label_start),
                    FeatureAction::Attach => {
                        if let Some(path) = rfd::FileDialog::new().pick_file() {
                            s.pdf.attach_file(&path)
                        } else {
                            return;
                        }
                    }
                    FeatureAction::Shape(shape, a, b) => s.pdf.add_shape(page, shape, a, b, &note),
                    FeatureAction::Link(a, b) => s.pdf.add_link(page, a, b, &url),
                    FeatureAction::UpdateLink(id, url) => s.pdf.update_link(id, &url),
                    FeatureAction::Resolve(id, resolved) => s.pdf.resolve_comment(id, resolved),
                    FeatureAction::Reply(id, page) => s.pdf.reply_comment(id, page, &note),
                    FeatureAction::StripMetadata => s.pdf.strip_metadata(),
                    FeatureAction::Language => s.pdf.set_language(&language),
                    FeatureAction::AddField(a, b) => s.pdf.add_form_field(
                        page,
                        a,
                        b,
                        &FormSpec {
                            kind,
                            name,
                            required,
                            options,
                        },
                    ),
                    FeatureAction::Image(a, b) => image
                        .context("Choose an image first")
                        .and_then(|path| s.pdf.add_image(page, a, b, &path)),
                    FeatureAction::ImportForm => {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .pick_file()
                        {
                            std::fs::read(path)
                                .map_err(Into::into)
                                .and_then(|bytes| Ok(serde_json::from_slice(&bytes)?))
                                .and_then(|data| s.pdf.import_form_data(&data))
                        } else {
                            return;
                        }
                    }
                    FeatureAction::RunningText => {
                        let position = match running_kind {
                            RunningKind::Header => [0.08, 0.06],
                            RunningKind::Footer | RunningKind::PageNumbers | RunningKind::Bates => {
                                [0.08, 0.95]
                            }
                            RunningKind::Watermark => [0.2, 0.5],
                        };
                        let style = pdf::TextStyle {
                            size: if running_kind == RunningKind::Watermark {
                                36.
                            } else {
                                11.
                            },
                            color: if running_kind == RunningKind::Watermark {
                                [0.55, 0.58, 0.64]
                            } else {
                                [0.12, 0.16, 0.24]
                            },
                            ..Default::default()
                        };
                        s.pdf.add_running_text(
                            &running_text,
                            position,
                            &style,
                            matches!(running_kind, RunningKind::PageNumbers | RunningKind::Bates),
                            number_start,
                            if running_kind == RunningKind::Bates {
                                padding
                            } else {
                                0
                            },
                        )
                    }
                    _ => Ok(()),
                }
            }
        };
        self.run(result, "Document updated");
    }
}

pub(super) fn feature(action: &mut Option<Action>, feature: FeatureAction) {
    *action = Some(Action::Feature(feature));
}
