use super::*;
use pdfpilot::{
    batch::{Plan, Step},
    forms::{Calculation, FieldProperties},
    inspection,
    scans::ScanOptions,
    security::SecurityOptions,
    service::{Running, Service},
    signatures,
};

pub(super) enum StudioAction {
    ChooseCommentAttachment,
    AttachComment([f32; 2]),
    Correction([f32; 2]),
    BookletPreview,
    DetectFields,
    CreateFields,
    WriteBlock,
    DeleteBlock,
    TransformImage,
    DeleteImage,
    Background,
    PageBox,
    StageRedaction([f32; 2], [f32; 2]),
    ExportRedactions,
    ConfigureField,
    ValidateForm,
    OrderTabs,
    Inspect,
    TagPage,
    EditTag(lopdf::ObjectId),
    ScanCopy,
    Sign,
    Verify,
    RemoveProtection,
    SavePreset,
    LoadPreset,
    RunPlan,
    StartService,
    StopService,
    PublishReview,
    RefreshReview,
    RemindReview,
    DownloadSigned,
}
pub(super) struct Studio {
    pub security: SecurityOptions,
    comment_attachment: Option<PathBuf>,
    block: pdfpilot::editing::TextBlock,
    block_id: Option<lopdf::ObjectId>,
    image_op: Option<usize>,
    image_rect: [f32; 4],
    image_crop: [f32; 4],
    image_rotation: f32,
    background: [f32; 3],
    box_name: String,
    box_rect: [f32; 4],
    pub measure_mode: pdfpilot::measurements::Mode,
    pub persist_measurement: bool,
    pub known_mm: f32,
    pub redactions: Vec<pdfpilot::workflows::Redaction>,
    pub redaction_document: Option<u64>,
    pub redaction_revision: Option<u64>,
    batch_protect: bool,
    pub field: Option<lopdf::ObjectId>,
    candidates: Vec<(bool, pdfpilot::forms::Candidate)>,
    properties: FieldProperties,
    min_enabled: bool,
    max_enabled: bool,
    min: f64,
    max: f64,
    calculated: bool,
    calculation: Calculation,
    inputs: String,
    tag_role: String,
    tag_alt: String,
    scan: ScanOptions,
    threshold_enabled: bool,
    threshold: u8,
    cert: Option<PathBuf>,
    key: Option<PathBuf>,
    ca: Option<PathBuf>,
    key_password: String,
    plan: Plan,
    next_step: usize,
    step_text: String,
    service: Option<Service>,
    running: Option<Running>,
    port: u16,
    recipients: String,
    ordered: bool,
    review: Option<serde_json::Value>,
    links: Vec<(String, String)>,
    activity: String,
    last_poll: std::time::Instant,
    polling: bool,
}
impl Default for Studio {
    fn default() -> Self {
        Self {
            security: Default::default(),
            comment_attachment: None,
            block: Default::default(),
            block_id: None,
            image_op: None,
            image_rect: [0.1, 0.1, 0.6, 0.6],
            image_crop: [0., 0., 1., 1.],
            image_rotation: 0.,
            background: [1., 1., 1.],
            box_name: "CropBox".into(),
            box_rect: [0., 0., 612., 792.],
            measure_mode: pdfpilot::measurements::Mode::Distance,
            persist_measurement: false,
            known_mm: 100.,
            redactions: vec![],
            redaction_document: None,
            redaction_revision: None,
            batch_protect: false,
            field: None,
            candidates: vec![],
            properties: Default::default(),
            min_enabled: false,
            max_enabled: false,
            min: 0.,
            max: 100.,
            calculated: false,
            calculation: Calculation::Sum,
            inputs: String::new(),
            tag_role: "P".into(),
            tag_alt: String::new(),
            scan: Default::default(),
            threshold_enabled: false,
            threshold: 170,
            cert: None,
            key: None,
            ca: None,
            key_password: String::new(),
            plan: Plan {
                steps: vec![Step::Compress],
            },
            next_step: 0,
            step_text: String::new(),
            service: None,
            running: None,
            port: 8787,
            recipients: String::new(),
            ordered: true,
            review: None,
            links: vec![],
            activity: String::new(),
            last_poll: std::time::Instant::now(),
            polling: false,
        }
    }
}
fn command(
    ui: &mut egui::Ui,
    action: &mut Option<Action>,
    icon: Icon,
    label: &str,
    next: StudioAction,
) {
    if wide_button(ui, icon, label).clicked() {
        feature(action, FeatureAction::Studio(next));
    }
}
fn choose(ui: &mut egui::Ui, value: &mut Option<PathBuf>, label: &str) {
    ui.horizontal_wrapped(|ui| {
        if ui.button(label).clicked() {
            *value = rfd::FileDialog::new()
                .add_filter("PEM certificate / key", &["pem", "crt", "key"])
                .pick_file();
        }
        if let Some(path) = value {
            ui.label(path.file_name().unwrap_or_default().to_string_lossy());
        }
    });
}
impl PdfApp {
    pub(super) fn form_settings(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        ui.collapsing("Detect form fields",|ui|{command(ui,action,Icon::Scan,"Find rectangular field candidates",StudioAction::DetectFields);ui.label(RichText::new("Review suggestions before adding. Detects vector rectangles; scan/image field detection is not supported.").size(11.).color(palette(ui.ctx()).muted));for (index,(selected,candidate)) in self.features.studio.candidates.iter_mut().enumerate(){ui.checkbox(selected,format!("{}: {}",index+1,candidate.kind.label()));}if !self.features.studio.candidates.is_empty(){command(ui,action,Icon::Form,"Create selected fields",StudioAction::CreateFields);}});
        ui.collapsing("Validation & calculations",|ui|{
            let fields=self.current().unwrap().pdf.fields();let studio=&mut self.features.studio;
            let previous=studio.field;
            egui::ComboBox::from_id_salt("property_field").selected_text(fields.iter().find(|f|Some(f.id)==studio.field).map(|f|f.name.as_str()).unwrap_or("Choose field")).show_ui(ui,|ui|{for field in &fields{ui.selectable_value(&mut studio.field,Some(field.id),&field.name);}});
            if studio.field!=previous && let Some(id)=studio.field {let d=self.current().unwrap().pdf.doc.get_dictionary(id).cloned().unwrap_or_default();let studio=&mut self.features.studio;studio.properties=FieldProperties {tooltip:d.get(b"TU").map(pdf::object_text).unwrap_or_default(),required:d.get(b"Ff").and_then(lopdf::Object::as_i64).unwrap_or(0)&2!=0,read_only:d.get(b"Ff").and_then(lopdf::Object::as_i64).unwrap_or(0)&1!=0,max_length:d.get(b"MaxLen").and_then(lopdf::Object::as_i64).unwrap_or(0).max(0) as usize,..Default::default()};let rules=d.get(b"PilotValidation").and_then(lopdf::Object::as_dict).ok();let min=rules.and_then(|d|d.get(b"Min").ok()).and_then(|o|o.as_float().ok());let max=rules.and_then(|d|d.get(b"Max").ok()).and_then(|o|o.as_float().ok());studio.min_enabled=min.is_some();studio.max_enabled=max.is_some();studio.min=min.unwrap_or(0.) as f64;studio.max=max.unwrap_or(100.) as f64;let calc=d.get(b"PilotCalculation").and_then(lopdf::Object::as_dict).ok();studio.calculated=calc.is_some();studio.inputs=calc.and_then(|d|d.get(b"Inputs").ok()).and_then(|o|o.as_array().ok()).map(|a|a.iter().map(pdf::object_text).collect::<Vec<_>>().join(", ")).unwrap_or_default();studio.calculation=match calc.and_then(|d|d.get(b"Operation").ok()).map(pdf::object_text).as_deref(){Some("Average")=>Calculation::Average,Some("Product")=>Calculation::Product,_=>Calculation::Sum};}
            let studio=&mut self.features.studio;
            ui.add(egui::TextEdit::singleline(&mut studio.properties.tooltip).hint_text("Accessible tooltip"));
            ui.checkbox(&mut studio.properties.required,"Required");ui.checkbox(&mut studio.properties.read_only,"Read only");ui.horizontal(|ui|{ui.label("Max characters (0 = unlimited)");ui.add(egui::DragValue::new(&mut studio.properties.max_length).range(0..=100_000));});
            ui.horizontal(|ui|{ui.checkbox(&mut studio.min_enabled,"Minimum");ui.add(egui::DragValue::new(&mut studio.min));});ui.horizontal(|ui|{ui.checkbox(&mut studio.max_enabled,"Maximum");ui.add(egui::DragValue::new(&mut studio.max));});
            ui.checkbox(&mut studio.calculated,"Calculated value");if studio.calculated{egui::ComboBox::from_id_salt("calculation").selected_text(studio.calculation.label()).show_ui(ui,|ui|{for kind in [Calculation::Sum,Calculation::Product,Calculation::Average]{ui.selectable_value(&mut studio.calculation,kind,kind.label());}});ui.add(egui::TextEdit::singleline(&mut studio.inputs).hint_text("Input field names, comma separated"));}
            ui.label(RichText::new("Validation and calculations run in PDFPilot; no arbitrary PDF scripts are executed.").size(11.).color(palette(ui.ctx()).muted));
            command(ui,action,Icon::Check,"Save field properties",StudioAction::ConfigureField);
            command(ui,action,Icon::Check,"Validate all form values",StudioAction::ValidateForm);
            command(ui,action,Icon::Form,"Tab order: top to bottom",StudioAction::OrderTabs);
        });
    }
    pub(super) fn export_settings(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        ui.collapsing("OCR settings", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.features.ocr.languages)
                    .hint_text("Language packs, e.g. eng+fra"),
            );
            ui.add(
                egui::Slider::new(&mut self.features.ocr.segmentation, 0..=13)
                    .text("Tesseract PSM"),
            );
            ui.add(egui::Slider::new(&mut self.features.ocr.width, 600..=4000).text("Page pixels"));
            ui.label(
                RichText::new("Selected language packs must be installed in Tesseract.")
                    .size(11.)
                    .color(palette(ui.ctx()).muted),
            );
        });
        ui.collapsing("Certificate signatures",|ui|{
            let studio=&mut self.features.studio;
            choose(ui,&mut studio.cert,"Signing certificate");choose(ui,&mut studio.key,"Private key");choose(ui,&mut studio.ca,"Trust root (optional)");ui.add(egui::TextEdit::singleline(&mut studio.key_password).password(true).hint_text("Private key password"));
            ui.label(RichText::new("Signs a separate copy. Existing signed or encrypted PDFs are refused. Trust and integrity are checked separately.").size(11.).color(palette(ui.ctx()).muted));
            command(ui,action,Icon::Pen,"Sign with certificate",StudioAction::Sign);command(ui,action,Icon::Check,"Verify signatures in saved PDF",StudioAction::Verify);
        });
        ui.collapsing("Security settings", |ui| {
            ui.label(pdfpilot::security::settings(
                &self.current().unwrap().pdf.doc,
            ));
            ui.add(
                egui::TextEdit::singleline(&mut self.features.studio.security.owner_password)
                    .password(true)
                    .hint_text("Owner password"),
            );
            command(
                ui,
                action,
                Icon::Lock,
                "Save unprotected copy",
                StudioAction::RemoveProtection,
            );
        });
        ui.collapsing("Scan cleanup & raster optimization",|ui|{
            let studio=&mut self.features.studio;ui.checkbox(&mut studio.scan.deskew,"Straighten scans");ui.checkbox(&mut studio.scan.grayscale,"Grayscale");ui.checkbox(&mut studio.scan.despeckle,"Remove speckles");ui.add(egui::Slider::new(&mut studio.scan.contrast,-50.0..=100.).text("Contrast"));ui.add(egui::Slider::new(&mut studio.scan.brightness,-100..=100).text("Brightness"));ui.checkbox(&mut studio.threshold_enabled,"Black / white threshold");if studio.threshold_enabled{ui.add(egui::Slider::new(&mut studio.threshold,0..=255).text("Threshold"));}ui.add(egui::Slider::new(&mut studio.scan.width,300..=4000).text("Page pixels"));ui.add(egui::Slider::new(&mut studio.scan.jpeg_quality,10..=100).text("JPEG quality"));
            ui.label(RichText::new("Creates flattened pages. Text, forms, links, tags and editable annotations are not retained; deskew may enlarge the page.").size(11.).color(palette(ui.ctx()).muted));command(ui,action,Icon::Scan,"Save enhanced / optimized copy",StudioAction::ScanCopy);
        });
    }
    pub(super) fn editing_settings(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        command(
            ui,
            action,
            Icon::Files,
            "Choose file for attachment comment",
            StudioAction::ChooseCommentAttachment,
        );
        ui.collapsing("Paragraphs & text blocks",|ui| {
            let page=self.current().unwrap().page;let blocks=self.current().unwrap().pdf.text_blocks(page).unwrap_or_default();
            let previous=self.features.studio.block_id;
            egui::ComboBox::from_id_salt("text_block").selected_text(if previous.is_some(){"Existing text block"}else{"New text block"}).show_ui(ui,|ui|{ui.selectable_value(&mut self.features.studio.block_id,None,"New text block");for block in blocks{ui.selectable_value(&mut self.features.studio.block_id,Some(block.id),block.text.chars().take(40).collect::<String>());}});
            if previous!=self.features.studio.block_id {if let Some(id)=self.features.studio.block_id{match self.current().unwrap().pdf.text_block_spec(page,id){Ok(block)=>self.features.studio.block=block,Err(e)=>self.error=Some(e.to_string())}}else{self.features.studio.block=Default::default();}}
            let block=&mut self.features.studio.block;
            ui.add(egui::TextEdit::multiline(&mut block.text).hint_text("Paragraph text").desired_rows(4));
            normalized_rect(ui,&mut block.rect);
            ui.add(egui::Slider::new(&mut block.style.size,6.0..=72.).text("Font size"));
            egui::ComboBox::from_id_salt("block_font").selected_text(&block.style.font).show_ui(ui,|ui|{for font in ["Helvetica","Helvetica-Bold","Times-Roman","Courier"]{ui.selectable_value(&mut block.style.font,font.into(),font);}});
            ui.color_edit_button_rgb(&mut block.style.color);
            ui.horizontal(|ui|{for (alignment,label) in [(pdfpilot::editing::Alignment::Left,"Left"),(pdfpilot::editing::Alignment::Center,"Center"),(pdfpilot::editing::Alignment::Right,"Right")]{ui.selectable_value(&mut block.alignment,alignment,label);}});
            ui.add(egui::Slider::new(&mut block.line_spacing,0.8..=4.).text("Line spacing"));ui.add(egui::Slider::new(&mut block.character_spacing,-2.0..=20.).text("Character spacing"));
            ui.label(RichText::new("Edit PDFPilot-created blocks. Western Latin text; line wrapping uses estimated font widths.").size(11.).color(palette(ui.ctx()).muted));
            command(ui,action,Icon::Text,"Write / update text block",StudioAction::WriteBlock);if self.features.studio.block_id.is_some(){command(ui,action,Icon::Delete,"Delete selected text block",StudioAction::DeleteBlock);}
        });
        ui.collapsing("Existing images",|ui| {
            let page=self.current().unwrap().page;let placements=self.current().unwrap().pdf.image_placements(page).unwrap_or_default();
            if placements.is_empty(){ui.label("No editable top-level image placements found.");}
            let old=self.features.studio.image_op;
            egui::ComboBox::from_id_salt("image_placement").selected_text(self.features.studio.image_op.map(|n|format!("Placement {n}")).unwrap_or_else(||"Select image".into())).show_ui(ui,|ui|{for image in &placements{ui.selectable_value(&mut self.features.studio.image_op,Some(image.operation),format!("{} / object {}",String::from_utf8_lossy(&image.resource),image.image.0));}});
            if old!=self.features.studio.image_op && let Some(placement)=placements.iter().find(|p|Some(p.operation)==self.features.studio.image_op) && let Ok(geo)=self.current().unwrap().pdf.geometry(page){let m=placement.matrix;let points=[[m[4],m[5]],[m[0]+m[4],m[1]+m[5]],[m[2]+m[4],m[3]+m[5]],[m[0]+m[2]+m[4],m[1]+m[3]+m[5]]].map(|p|pdfpilot::editing::normalized(geo,p));self.features.studio.image_rect=[points.iter().map(|p|p[0]).fold(1.,f32::min).clamp(0.,1.),points.iter().map(|p|p[1]).fold(1.,f32::min).clamp(0.,1.),points.iter().map(|p|p[0]).fold(0.,f32::max).clamp(0.,1.),points.iter().map(|p|p[1]).fold(0.,f32::max).clamp(0.,1.)];}
            ui.label("Placement on displayed page");normalized_rect(ui,&mut self.features.studio.image_rect);ui.add(egui::Slider::new(&mut self.features.studio.image_rotation,-180.0..=180.).text("Rotation"));ui.label("Crop in source image coordinates");normalized_rect(ui,&mut self.features.studio.image_crop);
            command(ui,action,Icon::Image,"Resize / move / crop / rotate image",StudioAction::TransformImage);command(ui,action,Icon::Delete,"Delete image placement",StudioAction::DeleteImage);
            ui.label(RichText::new("Supports top-level image Do operations. Nested Form XObjects and inline images require additional editing support. Deletion is not secure redaction.").size(11.).color(palette(ui.ctx()).muted));
        });
        ui.collapsing("Page background & boxes", |ui| {
            ui.color_edit_button_rgb(&mut self.features.studio.background);
            command(
                ui,
                action,
                Icon::Rectangle,
                "Add page background",
                StudioAction::Background,
            );
            egui::ComboBox::from_id_salt("page_box")
                .selected_text(&self.features.studio.box_name)
                .show_ui(ui, |ui| {
                    for name in ["MediaBox", "CropBox", "TrimBox", "BleedBox", "ArtBox"] {
                        ui.selectable_value(&mut self.features.studio.box_name, name.into(), name);
                    }
                });
            for (label, value) in ["Left", "Bottom", "Right", "Top"]
                .iter()
                .zip(&mut self.features.studio.box_rect)
            {
                ui.horizontal(|ui| {
                    ui.label(*label);
                    ui.add(egui::DragValue::new(value).suffix(" pt"));
                });
            }
            command(
                ui,
                action,
                Icon::Crop,
                "Set page box",
                StudioAction::PageBox,
            );
        });
        if self.features.studio.redaction_document == self.current().map(|s| s.id)
            && !self.features.studio.redactions.is_empty()
        {
            ui.label(format!(
                "{} regions staged for redaction",
                self.features.studio.redactions.len()
            ));
            command(
                ui,
                action,
                Icon::Redact,
                "Export all staged redactions",
                StudioAction::ExportRedactions,
            );
            if ui.button("Clear staged regions").clicked() {
                self.features.studio.redactions.clear();
            }
        }
    }
    pub(super) fn inspection_settings(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        ui.add_space(16.);
        command(
            ui,
            action,
            Icon::Info,
            "Inspect fonts, storage & accessibility",
            StudioAction::Inspect,
        );
        ui.collapsing("Structure tags",|ui|{
            let studio=&mut self.features.studio;egui::ComboBox::from_id_salt("tag_role").selected_text(&studio.tag_role).show_ui(ui,|ui|{for role in ["P","H1","H2","H3","Figure","Sect","Span"]{ui.selectable_value(&mut studio.tag_role,role.into(),role);}});ui.add(egui::TextEdit::singleline(&mut studio.tag_alt).hint_text("Alternative text"));ui.label(RichText::new("Manual page group tagging; review semantic reading order. This does not certify PDF/UA compliance.").size(11.).color(palette(ui.ctx()).muted));command(ui,action,Icon::Labels,"Tag current untagged page",StudioAction::TagPage);
            for tag in self.current().unwrap().pdf.tags(){ui.label(format!("{} / Page {:?} / {}",tag.role,tag.page,tag.alternative));command(ui,action,Icon::Text,"Apply role / alt text to element",StudioAction::EditTag(tag.id));}
        });
    }
    pub(super) fn studio_panel(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        section(ui, "REUSABLE ACTION SEQUENCE");
        let studio = &mut self.features.studio;
        let labels = [
            "Compress",
            "Remove metadata",
            "Watermark",
            "Header",
            "Footer",
            "Page numbers",
            "Language",
        ];
        egui::ComboBox::from_id_salt("batch_step")
            .selected_text(labels[studio.next_step])
            .show_ui(ui, |ui| {
                for (index, label) in labels.iter().enumerate() {
                    ui.selectable_value(&mut studio.next_step, index, *label);
                }
            });
        if studio.next_step > 1 {
            ui.add(
                egui::TextEdit::singleline(&mut studio.step_text)
                    .hint_text("Text / prefix / language"),
            );
        }
        if wide_button(ui, Icon::New, "Add operation").clicked() {
            let text = studio.step_text.clone();
            studio.plan.steps.push(match studio.next_step {
                0 => Step::Compress,
                1 => Step::StripMetadata,
                2 => Step::Watermark(text),
                3 => Step::Header(text),
                4 => Step::Footer(text),
                5 => Step::PageNumbers {
                    prefix: text,
                    start: 1,
                },
                _ => Step::Language(text),
            });
        }
        let mut move_step = None;
        let mut remove = None;
        let count = studio.plan.steps.len();
        for (index, step) in studio.plan.steps.iter().enumerate() {
            ui.push_id(index, |ui| {
                ui.label(format!("{}. {step:?}", index + 1));
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            index > 0,
                            IconButton::new(Icon::Left, "Move action earlier").size(24.),
                        )
                        .clicked()
                    {
                        move_step = Some((index, index - 1));
                    }
                    if ui
                        .add_enabled(
                            index + 1 < count,
                            IconButton::new(Icon::Right, "Move action later").size(24.),
                        )
                        .clicked()
                    {
                        move_step = Some((index, index + 1));
                    }
                    if ui
                        .add(IconButton::new(Icon::Delete, "Remove operation").size(24.))
                        .clicked()
                    {
                        remove = Some(index);
                    }
                });
            });
        }
        if let Some((a, b)) = move_step {
            studio.plan.steps.swap(a, b);
        }
        if let Some(index) = remove {
            studio.plan.steps.remove(index);
        }
        ui.collapsing("Batch protection", |ui| {
            let studio = &mut self.features.studio;
            ui.checkbox(&mut studio.batch_protect, "Protect batch outputs");
            if studio.batch_protect {
                ui.add(
                    egui::TextEdit::singleline(&mut studio.security.open_password)
                        .password(true)
                        .hint_text("Open password"),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut studio.security.owner_password)
                        .password(true)
                        .hint_text("Owner password"),
                );
                ui.checkbox(&mut studio.security.print, "Allow printing");
                ui.checkbox(&mut studio.security.edit, "Allow editing");
                ui.checkbox(&mut studio.security.copy, "Allow copying");
            }
        });
        command(
            ui,
            action,
            Icon::Save,
            "Save action preset",
            StudioAction::SavePreset,
        );
        command(
            ui,
            action,
            Icon::Open,
            "Load action preset",
            StudioAction::LoadPreset,
        );
        command(
            ui,
            action,
            Icon::Batch,
            "Run actions on multiple PDFs",
            StudioAction::RunPlan,
        );
        ui.add_space(16.);
        section(ui, "INDEPENDENT REVIEW SERVICE");
        let studio = &mut self.features.studio;
        if let Some(running) = &studio.running {
            ui.label(format!("Running at {}", running.address));
            command(
                ui,
                action,
                Icon::Stop,
                "Stop local service",
                StudioAction::StopService,
            );
        } else {
            ui.horizontal(|ui| {
                ui.label("Port");
                ui.add(egui::DragValue::new(&mut studio.port).range(1024..=65535));
            });
            command(
                ui,
                action,
                Icon::Link,
                "Start local service",
                StudioAction::StartService,
            );
        }
        ui.label(RichText::new("Local service binds to this computer. Remote sharing requires your own TLS hosting; files are not uploaded automatically.").size(11.).color(palette(ui.ctx()).muted));
        ui.add(
            egui::TextEdit::multiline(&mut self.features.studio.recipients)
                .hint_text("Signer names, one per line (blank = review only)")
                .desired_rows(3),
        );
        ui.checkbox(&mut self.features.studio.ordered, "Sign in listed order");
        command(
            ui,
            action,
            Icon::Link,
            "Publish a document snapshot locally",
            StudioAction::PublishReview,
        );
        for (name, url) in &self.features.studio.links {
            ui.label(name);
            if ui.button("Copy private link").clicked() {
                ui.ctx().copy_text(url.clone());
            }
            if ui.button("Open review").clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(url));
            }
        }
        command(
            ui,
            action,
            Icon::Reset,
            "Refresh review activity",
            StudioAction::RefreshReview,
        );
        command(
            ui,
            action,
            Icon::Comment,
            "Post a reminder in review activity",
            StudioAction::RemindReview,
        );
        command(
            ui,
            action,
            Icon::Export,
            "Save completed signing receipt PDF",
            StudioAction::DownloadSigned,
        );
        if !self.features.studio.activity.is_empty() {
            ui.label(&self.features.studio.activity);
        }
    }
    pub(super) fn reset_studio_selection(&mut self) {
        let studio = &mut self.features.studio;
        studio.field = None;
        studio.block_id = None;
        studio.image_op = None;
        studio.candidates.clear();
    }
    pub(super) fn poll_review(&mut self, ctx: &egui::Context) {
        let studio = &mut self.features.studio;
        if studio.running.is_none() || studio.review.is_none() {
            return;
        }
        ctx.request_repaint_after(Duration::from_secs(5));
        if studio.polling || studio.last_poll.elapsed() < Duration::from_secs(5) {
            return;
        }
        let Some(service) = studio.service.clone() else {
            return;
        };
        let Some(id) = studio
            .review
            .as_ref()
            .and_then(|v| v["id"].as_str())
            .map(str::to_string)
        else {
            return;
        };
        studio.polling = true;
        studio.last_poll = std::time::Instant::now();
        let sender = self.sender.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = service.snapshot(&id);
            let _ = sender.send(Event::Review { id, result });
            ctx.request_repaint();
        });
    }
    pub(super) fn receive_review(&mut self, id: String, result: Result<serde_json::Value>) {
        let studio = &mut self.features.studio;
        studio.polling = false;
        if studio.review.as_ref().and_then(|v| v["id"].as_str()) != Some(id.as_str()) {
            return;
        }
        match result {
            Ok(state) => {
                let old_count = studio
                    .review
                    .as_ref()
                    .and_then(|v| v["events"].as_array())
                    .map(Vec::len)
                    .unwrap_or(0);
                let events = state["events"].as_array().unwrap();
                if events.len() > old_count {
                    let signed = state["recipients"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|r| r["signed"] == true)
                        .count();
                    self.status = format!("Review activity updated: {signed} signatures received");
                }
                studio.activity = events
                    .iter()
                    .rev()
                    .take(20)
                    .map(|v| format!("{}: {}", v["time"], v["kind"].as_str().unwrap_or_default()))
                    .collect::<Vec<_>>()
                    .join("\n");
                studio.review = Some(state);
            }
            Err(error) => self.error = Some(format!("Review service: {error}")),
        }
    }
    pub(super) fn execute_studio(&mut self, action: StudioAction) {
        let Some(page) = self.current().map(|s| s.page) else {
            return;
        };
        let result = (|| -> Result<()> {
            if matches!(
                action,
                StudioAction::PublishReview | StudioAction::ScanCopy | StudioAction::BookletPreview
            ) {
                anyhow::ensure!(
                    self.current()
                        .unwrap()
                        .pdf
                        .permits(lopdf::Permissions::COPYABLE),
                    "Document permissions prohibit creating this copy"
                );
            }
            match action {
                StudioAction::ChooseCommentAttachment => {
                    self.features.studio.comment_attachment = rfd::FileDialog::new().pick_file();
                    if self.features.studio.comment_attachment.is_some() {
                        self.tool = Tool::Attachment;
                    }
                }
                StudioAction::AttachComment(point) => {
                    let path = self
                        .features
                        .studio
                        .comment_attachment
                        .clone()
                        .context("Choose a file for the attachment comment first")?;
                    let note = self.note.clone();
                    self.current_mut()
                        .unwrap()
                        .pdf
                        .attach_comment(page, point, &path, &note)?;
                }
                StudioAction::Correction(point) => {
                    let note = self.note.clone();
                    self.current_mut()
                        .unwrap()
                        .pdf
                        .add_correction(page, point, &note)?;
                }
                StudioAction::BookletPreview => {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("PDF", &["pdf"])
                        .set_file_name("booklet-preview.pdf")
                        .save_file()
                    {
                        let pdf = &self.current().unwrap().pdf;
                        let pages = if self.features.print_all {
                            (1..=pdf.page_count()).collect()
                        } else {
                            pdf::parse_pages(&self.features.print_range, pdf.page_count())?
                        };
                        let source = pdf.doc.clone();
                        let paper = self.features.print_paper;
                        let marks = self.features.print_marks;
                        self.background(move || {
                            pdfpilot::printing::booklet(&source, &pages, paper, marks, &path)?;
                            Ok(format!("Booklet sheet preview saved to {}", path.display()))
                        });
                    }
                }
                StudioAction::DetectFields => {
                    self.features.studio.candidates = self
                        .current()
                        .unwrap()
                        .pdf
                        .detect_field_candidates(page)?
                        .into_iter()
                        .map(|c| (false, c))
                        .collect();
                }
                StudioAction::CreateFields => {
                    let candidates = self
                        .features
                        .studio
                        .candidates
                        .iter()
                        .filter(|(selected, _)| *selected)
                        .map(|(_, c)| c.clone())
                        .collect::<Vec<_>>();
                    self.current_mut()
                        .unwrap()
                        .pdf
                        .create_candidate_fields(page, &candidates)?;
                    self.features.studio.candidates.clear();
                }
                StudioAction::WriteBlock => {
                    let block = self.features.studio.block.clone();
                    let id = self.features.studio.block_id;
                    self.current_mut()
                        .unwrap()
                        .pdf
                        .write_text_block(page, id, &block)?;
                    self.features.studio.block_id = self
                        .current()
                        .unwrap()
                        .pdf
                        .text_blocks(page)?
                        .last()
                        .map(|b| b.id);
                }
                StudioAction::DeleteBlock => {
                    let id = self
                        .features
                        .studio
                        .block_id
                        .context("Select a text block")?;
                    self.current_mut()
                        .unwrap()
                        .pdf
                        .delete_text_block(page, id)?;
                    self.features.studio.block_id = None;
                }
                StudioAction::TransformImage => {
                    let studio = &self.features.studio;
                    let id = studio.image_op.context("Select an image")?;
                    let rect = studio.image_rect;
                    let crop = studio.image_crop;
                    let rotation = studio.image_rotation;
                    self.current_mut()
                        .unwrap()
                        .pdf
                        .transform_image(page, id, rect, rotation, crop)?;
                    self.features.studio.image_op = None;
                }
                StudioAction::DeleteImage => {
                    let id = self.features.studio.image_op.context("Select an image")?;
                    self.current_mut()
                        .unwrap()
                        .pdf
                        .delete_image_placement(page, id)?;
                    self.features.studio.image_op = None;
                }
                StudioAction::Background => {
                    let color = self.features.studio.background;
                    self.current_mut()
                        .unwrap()
                        .pdf
                        .page_background(page, color)?;
                }
                StudioAction::PageBox => {
                    let rect = self.features.studio.box_rect;
                    let name = self.features.studio.box_name.clone();
                    self.current_mut()
                        .unwrap()
                        .pdf
                        .set_page_box(page, &name, rect)?;
                }
                StudioAction::StageRedaction(start, end) => {
                    let id = self.current().unwrap().id;
                    let revision = self.current().unwrap().pdf.revision;
                    let studio = &mut self.features.studio;
                    if studio.redaction_document != Some(id) {
                        studio.redactions.clear();
                        studio.redaction_document = Some(id);
                    }
                    studio.redaction_revision = Some(revision);
                    studio
                        .redactions
                        .push(pdfpilot::workflows::Redaction { page, start, end });
                }
                StudioAction::ExportRedactions => {
                    let id = self.current().unwrap().id;
                    anyhow::ensure!(
                        self.features.studio.redaction_document == Some(id),
                        "Staged redactions belong to another document"
                    );
                    let source = self.current().unwrap().pdf.doc.clone();
                    let regions = self.features.studio.redactions.clone();
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("PDF", &["pdf"])
                        .set_file_name("redacted.pdf")
                        .save_file()
                    {
                        self.background(move || {
                            pdfpilot::workflows::redacted_copy(&source, &regions, &path)?;
                            Ok("Redacted copy saved".into())
                        });
                    }
                }
                StudioAction::ConfigureField => {
                    let studio = &self.features.studio;
                    let id = studio.field.context("Choose a field")?;
                    let mut props = studio.properties.clone();
                    props.numeric_min = studio.min_enabled.then_some(studio.min);
                    props.numeric_max = studio.max_enabled.then_some(studio.max);
                    props.calculation = studio.calculated.then(|| {
                        (
                            studio.calculation,
                            studio
                                .inputs
                                .split(',')
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                                .map(str::to_string)
                                .collect(),
                        )
                    });
                    self.current_mut()
                        .unwrap()
                        .pdf
                        .configure_field(id, &props)?;
                }
                StudioAction::ValidateForm => {
                    let issues = self.current().unwrap().pdf.validate_form();
                    self.features.report = if issues.is_empty() {
                        "Form validation passed".into()
                    } else {
                        issues.join("\n")
                    };
                    self.features.report_html.clear();
                    self.modal = Some(Modal::Report);
                }
                StudioAction::OrderTabs => self.current_mut().unwrap().pdf.order_form_tabs(page)?,
                StudioAction::Inspect => {
                    self.features.report = inspection::report(&self.current().unwrap().pdf);
                    self.features.report_html.clear();
                    self.modal = Some(Modal::Report);
                }
                StudioAction::TagPage | StudioAction::EditTag(_) => {
                    let role = self.features.studio.tag_role.clone();
                    let alt = self.features.studio.tag_alt.clone();
                    let pdf = &mut self.current_mut().unwrap().pdf;
                    match action {
                        StudioAction::TagPage => pdf.tag_page(page, &role, &alt)?,
                        StudioAction::EditTag(id) => pdf.edit_tag(id, &role, &alt)?,
                        _ => {}
                    }
                }
                StudioAction::ScanCopy => {
                    let mut options = self.features.studio.scan.clone();
                    options.threshold = self
                        .features
                        .studio
                        .threshold_enabled
                        .then_some(self.features.studio.threshold);
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("PDF", &["pdf"])
                        .set_file_name("enhanced.pdf")
                        .save_file()
                    {
                        let source = self.current().unwrap().pdf.doc.clone();
                        self.background(move || {
                            pdfpilot::scans::optimized_copy(&source, &options, &path)?;
                            Ok(format!("Enhanced PDF saved to {}", path.display()))
                        });
                    }
                }
                StudioAction::Sign => {
                    let cert = self
                        .features
                        .studio
                        .cert
                        .clone()
                        .context("Choose a signing certificate")?;
                    let key = self
                        .features
                        .studio
                        .key
                        .clone()
                        .context("Choose a private key")?;
                    let password = std::mem::take(&mut self.features.studio.key_password);
                    let source = self.current().unwrap().pdf.doc.clone();
                    if let Some(output) = rfd::FileDialog::new()
                        .add_filter("PDF", &["pdf"])
                        .set_file_name("signed.pdf")
                        .save_file()
                    {
                        self.background(move || {
                            signatures::signed_copy(&source, &cert, &key, &password, &output)?;
                            Ok(format!(
                                "Certificate-signed PDF saved to {}",
                                output.display()
                            ))
                        });
                    }
                }
                StudioAction::Verify => {
                    let path = self
                        .current()
                        .unwrap()
                        .pdf
                        .path
                        .clone()
                        .context("Save the PDF before verifying its signatures")?;
                    let reports = signatures::verify(&path, self.features.studio.ca.as_deref())?;
                    self.features.report = if reports.is_empty() {
                        "No certificate signatures found".into()
                    } else {
                        reports.iter().map(|r|format!("{}\nIntegrity: {}\nTrusted by selected root: {:?}\nCovers current file: {}\n{}",r.field,r.integrity,r.trusted,r.covers_current_file,r.detail)).collect::<Vec<_>>().join("\n\n")
                    };
                    self.features.report_html.clear();
                    self.modal = Some(Modal::Report);
                }
                StudioAction::RemoveProtection => {
                    let owner = std::mem::take(&mut self.features.studio.security.owner_password);
                    if let Some(output) = rfd::FileDialog::new()
                        .add_filter("PDF", &["pdf"])
                        .set_file_name("unprotected.pdf")
                        .save_file()
                    {
                        self.current()
                            .unwrap()
                            .pdf
                            .remove_protection_copy(&owner, &output)?;
                    }
                }
                StudioAction::SavePreset => {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("JSON", &["json"])
                        .set_file_name("actions.json")
                        .save_file()
                    {
                        self.features.studio.plan.save(&path)?;
                    }
                }
                StudioAction::LoadPreset => {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("JSON", &["json"])
                        .pick_file()
                    {
                        self.features.studio.plan = Plan::load(&path)?;
                    }
                }
                StudioAction::RunPlan => {
                    if let Some(inputs) = rfd::FileDialog::new()
                        .add_filter("PDF", &["pdf"])
                        .pick_files()
                        && let Some(folder) = rfd::FileDialog::new().pick_folder()
                    {
                        let plan = self.features.studio.plan.clone();
                        let security = self
                            .features
                            .studio
                            .batch_protect
                            .then(|| self.features.studio.security.clone());
                        self.features.studio.security.open_password.clear();
                        self.features.studio.security.owner_password.clear();
                        self.background(move || {
                            let outputs = plan.run(&inputs, &folder, security.as_ref())?;
                            Ok(format!("Processed {} PDFs", outputs.len()))
                        });
                    }
                }
                StudioAction::StartService => {
                    if let Some(folder) = rfd::FileDialog::new()
                        .set_title("Choose private review service storage")
                        .pick_folder()
                    {
                        let service = Service::open(&folder.join("PDFPilot Reviews"))?;
                        self.features.studio.running =
                            Some(service.start(self.features.studio.port)?);
                        self.features.studio.service = Some(service);
                    }
                }
                StudioAction::StopService => {
                    self.features.studio.running = None;
                }
                StudioAction::PublishReview => {
                    let studio = &self.features.studio;
                    let service = studio
                        .service
                        .as_ref()
                        .context("Start a local service first")?;
                    let running = studio
                        .running
                        .as_ref()
                        .context("Start a local service first")?;
                    let recipients = studio
                        .recipients
                        .lines()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .collect::<Vec<_>>();
                    let review = service.publish(
                        &self.current().unwrap().pdf.doc,
                        &self.current().unwrap().pdf.title,
                        &recipients,
                        studio.ordered,
                    )?;
                    let id = review["id"].as_str().unwrap();
                    let mut links = vec![(
                        "Review link".into(),
                        format!(
                            "{}/review/{id}#{}",
                            running.address,
                            review["review_token"].as_str().unwrap()
                        ),
                    )];
                    for recipient in review["recipients"].as_array().unwrap() {
                        links.push((
                            format!("Signing link: {}", recipient["name"].as_str().unwrap()),
                            format!(
                                "{}/review/{id}#{}",
                                running.address,
                                recipient["token"].as_str().unwrap()
                            ),
                        ));
                    }
                    self.features.studio.links = links;
                    self.features.studio.review = Some(review);
                }
                StudioAction::RefreshReview
                | StudioAction::RemindReview
                | StudioAction::DownloadSigned => {
                    let studio = &mut self.features.studio;
                    let service = studio.service.as_ref().context("Start a service first")?;
                    let id = studio
                        .review
                        .as_ref()
                        .and_then(|v| v["id"].as_str())
                        .context("Publish a review first")?
                        .to_owned();
                    match action {
                        StudioAction::RemindReview => service.remind(&id)?,
                        StudioAction::DownloadSigned => {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("PDF", &["pdf"])
                                .set_file_name("signed-receipt.pdf")
                                .save_file()
                            {
                                service.completed_pdf(&id, &path)?;
                            }
                        }
                        _ => {}
                    }
                    let state = service.snapshot(&id)?;
                    studio.activity = state["events"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| {
                            format!("{}: {}", v["time"], v["kind"].as_str().unwrap_or_default())
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    studio.review = Some(state);
                }
            }
            Ok(())
        })();
        self.run(result, "Operation complete");
    }
}

fn normalized_rect(ui: &mut egui::Ui, rect: &mut [f32; 4]) {
    egui::Grid::new(ui.next_auto_id())
        .num_columns(2)
        .show(ui, |ui| {
            for (label, value) in ["Left", "Top / bottom", "Right", "Bottom / top"]
                .iter()
                .zip(rect)
            {
                ui.label(*label);
                ui.add(egui::DragValue::new(value).range(0.0..=1.0).speed(0.005));
                ui.end_row();
            }
        });
}
