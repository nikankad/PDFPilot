use anyhow::{Context, Result};
use eframe::egui::{self, Color32, FontId, RichText, Stroke, TextureHandle, Vec2};
use pdfpilot::{
    pdf::{self, Comment, Field, Mark, PdfDocument},
    renderer::{RenderKey, RenderRequest, Renderer},
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
    time::Duration,
};

use crate::icons::{self, Icon, IconButton};
use crate::theme::{self, PRIMARY, palette};

mod classic;
mod features;
#[cfg(target_os = "macos")]
mod native_menu;
mod studio;
use features::{FeatureAction, Features, RunningKind, feature};
use pdfpilot::advanced::Shape;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Single,
    Continuous,
    TwoPage,
}

struct Session {
    id: u64,
    pdf: PdfDocument,
    page: u32,
    zoom: f32,
    fit: bool,
    fit_page: bool,
    view: ViewMode,
    sizes: Vec<[f32; 2]>,
    labels: Vec<String>,
    cache: HashMap<RenderKey, TextureHandle>,
    failed: HashSet<RenderKey>,
    temp: tempfile::TempDir,
    snapshot: Option<(u64, PathBuf)>,
}

impl Session {
    fn snapshot(&mut self) -> Result<PathBuf> {
        if let Some((revision, path)) = &self.snapshot
            && *revision == self.pdf.revision
        {
            return Ok(path.clone());
        }
        let path = self
            .temp
            .path()
            .join(format!("document-{}.pdf", self.pdf.revision));
        pdf::atomic_save(&self.pdf.doc, &path)?;
        self.snapshot = Some((self.pdf.revision, path.clone()));
        Ok(path)
    }
    fn key(&self, page: u32, width: u32) -> RenderKey {
        RenderKey {
            document: self.id,
            revision: self.pdf.revision,
            page,
            width,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Panel {
    Tools,
    Comments,
    Forms,
    Text,
    Properties,
    Pages,
    Export,
    Files,
    Studio,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tool {
    Select,
    Highlight,
    Underline,
    Rectangle,
    Note,
    Text,
    Ink,
    Redact,
    Crop,
    Link,
    Form,
    Image,
    Strikeout,
    Line,
    Arrow,
    Circle,
    Stamp,
    Measure,
    Correction,
    Attachment,
}
impl Tool {
    fn name(self) -> &'static str {
        match self {
            Self::Select => "Read",
            Self::Highlight => "Highlight",
            Self::Underline => "Underline",
            Self::Rectangle => "Rectangle",
            Self::Note => "Comment",
            Self::Text => "Add text",
            Self::Ink => "Draw / sign",
            Self::Redact => "Redact copy",
            Self::Crop => "Crop",
            Self::Link => "Link",
            Self::Form => "Form field",
            Self::Image => "Place image",
            Self::Strikeout => "Strikeout",
            Self::Line => "Line",
            Self::Arrow => "Arrow",
            Self::Circle => "Circle",
            Self::Stamp => "Stamp",
            Self::Measure => "Measure",
            Self::Correction => "Text correction",
            Self::Attachment => "Attachment comment",
        }
    }
    fn icon(self) -> Icon {
        match self {
            Self::Select => Icon::Pointer,
            Self::Highlight => Icon::Highlight,
            Self::Underline => Icon::Underline,
            Self::Rectangle => Icon::Rectangle,
            Self::Note => Icon::Comment,
            Self::Text => Icon::Text,
            Self::Ink => Icon::Pen,
            Self::Redact => Icon::Redact,
            Self::Crop => Icon::Crop,
            Self::Link => Icon::Link,
            Self::Form => Icon::Form,
            Self::Image => Icon::Image,
            Self::Strikeout => Icon::Strikeout,
            Self::Line => Icon::Line,
            Self::Arrow => Icon::Arrow,
            Self::Circle => Icon::Circle,
            Self::Stamp => Icon::Stamp,
            Self::Measure => Icon::Ruler,
            Self::Correction => Icon::Text,
            Self::Attachment => Icon::Files,
        }
    }
    fn hint(self) -> &'static str {
        match self {
            Self::Select => "Scroll to read. Use the page controls to navigate.",
            Self::Highlight => "Drag over a passage to highlight it.",
            Self::Underline => "Drag across a line to underline it.",
            Self::Rectangle => "Drag to draw a rectangle.",
            Self::Note => "Write a comment, then click on the page.",
            Self::Text => "Enter text, then click where its baseline should start.",
            Self::Ink => "Drag to draw an editable ink annotation.",
            Self::Redact => "Drag over a region to remove from a separate raster PDF.",
            Self::Crop => "Drag to define the visible page area.",
            Self::Link => "Enter a URL in Files, then drag its clickable area.",
            Self::Form => "Configure the field in Forms, then drag its area.",
            Self::Image => "Choose an image, then drag its placement area.",
            Self::Strikeout => "Drag over text to strike it out.",
            Self::Line | Self::Arrow => "Drag between the endpoints.",
            Self::Circle => "Drag to draw an ellipse.",
            Self::Stamp => "Enter stamp text, then drag its area.",
            Self::Measure => "Drag between two points to measure distance.",
            Self::Correction => "Enter a correction comment, then click its insertion point.",
            Self::Attachment => "Choose an attachment file below, then click to place its comment.",
        }
    }
}

enum Modal {
    Create,
    Password(PathBuf),
    Protect,
    Close(usize),
    Quit,
    Delete,
    Redact([f32; 2], [f32; 2]),
    Help,
    Report,
    Print,
}
enum Event {
    Review {
        id: String,
        result: Result<serde_json::Value>,
    },
    Open {
        path: PathBuf,
        result: Result<Box<PdfDocument>>,
    },
    Done(Result<String>),
    Report(Result<pdfpilot::workflows::ComparisonReport>),
    Search {
        document: u64,
        revision: u64,
        token: u64,
        result: Vec<(u32, String)>,
    },
}
enum Action {
    Open,
    OpenPath(PathBuf),
    Sample,
    New,
    Images,
    Save(bool),
    Close(usize),
    Tab(usize),
    Undo,
    Redo,
    Page(u32),
    Zoom(f32),
    Fit,
    Rotate(i64),
    Move(u32),
    Insert,
    Delete,
    DeleteConfirmed,
    Extract,
    Merge,
    Compress,
    TextExport,
    Print,
    Protect,
    ProtectConfirmed,
    PasswordOpen(PathBuf),
    Create,
    Ocr,
    Redact([f32; 2], [f32; 2]),
    RedactConfirmed([f32; 2], [f32; 2]),
    ReplaceText,
    Mark(Mark, [f32; 2], [f32; 2]),
    Text([f32; 2]),
    Ink(Vec<[f32; 2]>),
    RemoveComment(Comment),
    SetField(Field, String),
    Metadata,
    Search,
    Quit,
    DiscardClose(usize),
    DiscardQuit,
    Feature(FeatureAction),
    OnPage(u32, Box<Action>),
    FitPage,
}

pub struct PdfApp {
    #[cfg(target_os = "macos")]
    native_menu: Option<native_menu::NativeMenu>,
    sessions: Vec<Session>,
    active: Option<usize>,
    next_id: u64,
    renderer: Renderer,
    render_busy: bool,
    sender: Sender<Event>,
    events: Receiver<Event>,
    jobs: usize,
    panel: Panel,
    tool: Tool,
    note: String,
    add_text: String,
    ink_width: f32,
    drag_start: Option<[f32; 2]>,
    stroke: Vec<[f32; 2]>,
    page_input: String,
    page_range: String,
    find: String,
    results: Vec<(u32, String)>,
    search_generation: Option<(u64, u64, u64)>,
    search_token: u64,
    replace_search: String,
    replace_value: String,
    text: String,
    field_values: HashMap<lopdf::ObjectId, String>,
    metadata: [String; 3],
    synced: Option<(u64, u64, u32)>,
    modal: Option<Modal>,
    password: String,
    password_confirm: String,
    create_title: String,
    create_body: String,
    status: String,
    error: Option<String>,
    recent: Vec<PathBuf>,
    allow_quit: bool,
    thumb_pages: Vec<u32>,
    dark_mode: bool,
    features: Features,
    main_keys: Vec<RenderKey>,
    drag_page: Option<u32>,
    jump_page: bool,
    fullscreen: bool,
    inspector_open: bool,
    thumbnails_open: bool,
    tools_open: bool,
    search_open: bool,
    landing: classic::Landing,
    catalog_selection: usize,
    catalog_jump: bool,
}

impl PdfApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let dark = cc
            .storage
            .and_then(|s| s.get_string("dark_mode"))
            .map(|s| s == "true")
            .unwrap_or(false);
        let mut app = Self::with_context(&cc.egui_ctx, dark);
        #[cfg(target_os = "macos")]
        {
            app.native_menu = Some(native_menu::NativeMenu::install(&cc.egui_ctx));
        }
        for arg in std::env::args_os().skip(1) {
            app.open_path(PathBuf::from(arg));
        }
        app
    }
    fn with_context(ctx: &egui::Context, dark: bool) -> Self {
        theme::apply(ctx, dark);
        let (sender, events) = mpsc::channel();
        Self {
            #[cfg(target_os = "macos")]
            native_menu: None,
            sessions: vec![],
            active: None,
            next_id: 1,
            renderer: Renderer::new(),
            render_busy: false,
            sender,
            events,
            jobs: 0,
            panel: Panel::Tools,
            tool: Tool::Select,
            note: String::new(),
            add_text: String::new(),
            ink_width: 2.,
            drag_start: None,
            stroke: vec![],
            page_input: "1".into(),
            page_range: "1".into(),
            find: String::new(),
            results: vec![],
            search_generation: None,
            search_token: 0,
            replace_search: String::new(),
            replace_value: String::new(),
            text: String::new(),
            field_values: HashMap::new(),
            metadata: Default::default(),
            synced: None,
            modal: None,
            password: String::new(),
            password_confirm: String::new(),
            create_title: "Untitled document".into(),
            create_body: String::new(),
            status: "Ready".into(),
            error: None,
            recent: vec![],
            allow_quit: false,
            thumb_pages: vec![],
            dark_mode: dark,
            features: Features::default(),
            main_keys: vec![],
            drag_page: None,
            jump_page: false,
            fullscreen: false,
            inspector_open: false,
            thumbnails_open: false,
            tools_open: true,
            search_open: false,
            landing: classic::Landing::Welcome,
            catalog_selection: usize::MAX,
            catalog_jump: false,
        }
    }

    fn current(&self) -> Option<&Session> {
        self.active.and_then(|i| self.sessions.get(i))
    }
    fn current_mut(&mut self) -> Option<&mut Session> {
        self.active.and_then(|i| self.sessions.get_mut(i))
    }
    fn run(&mut self, result: Result<()>, success: &str) {
        match result {
            Ok(()) => {
                self.status = success.into();
                self.error = None;
            }
            Err(e) => {
                self.error = Some(format!("{e:#}"));
                self.status = "Action failed".into();
            }
        }
    }
    fn add_document(&mut self, pdf: PdfDocument) -> Result<()> {
        let temp = tempfile::tempdir()?;
        self.sessions.push(Session {
            id: self.next_id,
            pdf,
            page: 1,
            zoom: 1.,
            fit: true,
            fit_page: false,
            view: ViewMode::Single,
            sizes: vec![],
            labels: vec![],
            cache: HashMap::new(),
            failed: HashSet::new(),
            temp,
            snapshot: None,
        });
        self.next_id += 1;
        self.active = Some(self.sessions.len() - 1);
        self.synced = None;
        self.panel = Panel::Tools;
        self.tool = Tool::Select;
        self.status = "Document opened".into();
        self.error = None;
        Ok(())
    }
    fn open_path(&mut self, path: PathBuf) {
        if let Some(index) = self
            .sessions
            .iter()
            .position(|s| s.pdf.path.as_ref() == Some(&path))
        {
            self.active = Some(index);
            return;
        }
        let tx = self.sender.clone();
        self.jobs += 1;
        self.status = "Opening document...".into();
        std::thread::spawn(move || {
            let result = match path
                .extension()
                .and_then(|x| x.to_str())
                .unwrap_or("")
                .to_ascii_lowercase()
                .as_str()
            {
                "pdf" => PdfDocument::open(&path, ""),
                "png" | "jpg" | "jpeg" | "tif" | "tiff" => {
                    pdf::images_to_document(std::slice::from_ref(&path))
                        .map(|doc| PdfDocument::from_document(doc, "Images.pdf"))
                }
                "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "ods" | "odp"
                | "rtf" | "html" | "htm" => pdfpilot::conversion::office_to_document(&path)
                    .map(|doc| PdfDocument::from_document(doc, "Converted.pdf")),
                _ => Err(anyhow::anyhow!("Unsupported file type")),
            }
            .map(Box::new);
            let _ = tx.send(Event::Open { path, result });
        });
    }
    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.events.try_recv() {
            self.jobs = self.jobs.saturating_sub(1);
            match event {
                Event::Open { path, result } => match result {
                    Ok(pdf) => {
                        self.recent.retain(|p| p != &path);
                        self.recent.insert(0, path);
                        self.recent.truncate(8);
                        let result = self.add_document(*pdf);
                        self.run(result, "Document opened");
                    }
                    Err(e) => {
                        let message = format!("{e:#}");
                        if message.contains("Password required") {
                            self.modal = Some(Modal::Password(path));
                            self.password.clear();
                        }
                        self.error = Some(message);
                    }
                },
                Event::Review { id, result } => self.receive_review(id, result),
                Event::Report(result) => match result {
                    Ok(report) => {
                        self.features.report = report.summary;
                        self.features.report_html = report.html;
                        self.modal = Some(Modal::Report);
                        self.status = "Comparison ready".into();
                    }
                    Err(e) => self.error = Some(format!("{e:#}")),
                },
                Event::Done(result) => match result {
                    Ok(message) => {
                        self.status = message;
                        self.error = None;
                    }
                    Err(e) => self.error = Some(format!("{e:#}")),
                },
                Event::Search {
                    document,
                    revision,
                    token,
                    result,
                } => {
                    if self.search_generation == Some((document, revision, token))
                        && self
                            .current()
                            .is_some_and(|s| s.id == document && s.pdf.revision == revision)
                    {
                        self.results = result;
                        self.status = format!("Found matches on {} pages", self.results.len());
                    }
                }
            }
        }
        while let Ok(rendered) = self.renderer.results.try_recv() {
            self.render_busy = false;
            if let Some(session) = self
                .sessions
                .iter_mut()
                .find(|s| s.id == rendered.key.document && s.pdf.revision == rendered.key.revision)
            {
                match rendered.pixels {
                    Ok(pixels) => {
                        let size = [pixels.width() as usize, pixels.height() as usize];
                        let texture = ctx.load_texture(
                            format!("page-{:?}", rendered.key),
                            egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_raw()),
                            egui::TextureOptions::LINEAR,
                        );
                        session.cache.insert(rendered.key, texture);
                        if session.cache.len() > 64 {
                            let candidate = session
                                .cache
                                .keys()
                                .filter(|k| **k != rendered.key)
                                .max_by_key(|k| {
                                    (
                                        k.revision != session.pdf.revision,
                                        k.page.abs_diff(session.page),
                                        k.width == 144,
                                    )
                                })
                                .copied();
                            if let Some(key) = candidate {
                                session.cache.remove(&key);
                            }
                        }
                    }
                    Err(e) => {
                        session.failed.insert(rendered.key);
                        self.error = Some(format!("{e:#}"));
                    }
                }
            }
        }
    }
    fn sync(&mut self) {
        if let Some(session) = self.current()
            && self.features.studio.redaction_document == Some(session.id)
            && self.features.studio.redaction_revision != Some(session.pdf.revision)
            && !self.features.studio.redactions.is_empty()
        {
            self.features.studio.redactions.clear();
            self.status =
                "Document changed; staged redactions cleared. Mark the regions again.".into();
        }
        let key = self.current().map(|s| (s.id, s.pdf.revision, s.page));
        if key == self.synced {
            return;
        }
        let document_changed =
            self.synced.map(|(id, rev, _)| (id, rev)) != key.map(|(id, rev, _)| (id, rev));
        if self.synced.map(|(id, _, page)| (id, page)) != key.map(|(id, _, page)| (id, page)) {
            self.reset_studio_selection();
        }
        self.synced = key;
        if let Some(s) = self.current() {
            let page = s.page;
            let text = s.pdf.text(page).unwrap_or_else(|e| format!("{e:#}"));
            let fields = s
                .pdf
                .fields()
                .into_iter()
                .map(|f| (f.id, f.value))
                .collect();
            let metadata = [
                s.pdf.metadata(b"Title"),
                s.pdf.metadata(b"Author"),
                s.pdf.metadata(b"Subject"),
            ];
            self.page_input = page.to_string();
            self.text = text;
            self.field_values = fields;
            self.metadata = metadata;
        }
        if document_changed {
            if let Some(s) = self.current_mut() {
                s.labels = s.pdf.page_labels();
                s.sizes = (1..=s.pdf.page_count())
                    .map(|p| {
                        s.pdf
                            .geometry(p)
                            .map(|g| g.display_size())
                            .unwrap_or([612., 792.])
                    })
                    .collect();
            }
            self.features.link_values.clear();
            self.results.clear();
            self.search_generation = None;
        }
        self.drag_start = None;
        self.stroke.clear();
    }

    fn thumbnails(&mut self, ctx: &egui::Context, action: &mut Option<Action>) {
        self.thumb_pages.clear();
        if self.active.is_none() || !self.thumbnails_open {
            return;
        }
        let s = self.current().unwrap();
        let count = s.pdf.page_count();
        let page = s.page;
        let mut visible = vec![];
        egui::SidePanel::left("pages")
            .exact_width(188.)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(palette(ctx).background)
                    .inner_margin(egui::Margin::symmetric(16, 18)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Pages").strong().size(15.));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(count.to_string()).color(palette(ui.ctx()).muted));
                    });
                });
                ui.add_space(12.);
                egui::ScrollArea::vertical()
                    .id_salt("thumbnails")
                    .show_rows(ui, 230., count as usize, |ui, range| {
                        for index in range {
                            let n = index as u32 + 1;
                            visible.push(n);
                            let key = s.key(n, 144);
                            egui::Frame::new()
                                .fill(if n == page {
                                    palette(ui.ctx()).selected
                                } else {
                                    palette(ui.ctx()).background
                                })
                                .corner_radius(8)
                                .inner_margin(8)
                                .show(ui, |ui| {
                                    ui.vertical_centered(|ui| {
                                        let display = s
                                            .pdf
                                            .geometry(n)
                                            .map(|g| g.display_size())
                                            .unwrap_or([612., 792.]);
                                        let scale = (130. / display[0]).min(178. / display[1]);
                                        let size = Vec2::new(display[0], display[1]) * scale;
                                        let response = if let Some(texture) = s.cache.get(&key) {
                                            ui.add(
                                                egui::Image::new(texture)
                                                    .fit_to_exact_size(size)
                                                    .sense(egui::Sense::click_and_drag()),
                                            )
                                        } else {
                                            let (rect, response) = ui.allocate_exact_size(
                                                size,
                                                egui::Sense::click_and_drag(),
                                            );
                                            ui.painter().rect_filled(rect, 2., Color32::WHITE);
                                            ui.painter().text(
                                                rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                n,
                                                FontId::proportional(28.),
                                                palette(ui.ctx()).line,
                                            );
                                            response
                                        };
                                        if response.drag_started() {
                                            ui.ctx().data_mut(|d| {
                                                d.insert_temp(egui::Id::new("dragged_page"), n)
                                            });
                                        }
                                        if response.contains_pointer()
                                            && ui.input(|i| i.pointer.any_released())
                                            && let Some(from) = ui.ctx().data_mut(|d| {
                                                d.remove_temp::<u32>(egui::Id::new("dragged_page"))
                                            })
                                            && from != n
                                        {
                                            feature(action, FeatureAction::Reorder(from, n));
                                        }
                                        if response.clicked() {
                                            *action = Some(Action::Page(n));
                                        }
                                        ui.label(
                                            RichText::new(
                                                s.labels
                                                    .get(n as usize - 1)
                                                    .cloned()
                                                    .unwrap_or_else(|| n.to_string()),
                                            )
                                            .color(
                                                if n == page {
                                                    palette(ui.ctx()).accent
                                                } else {
                                                    palette(ui.ctx()).muted
                                                },
                                            ),
                                        );
                                    });
                                });
                        }
                    });
            });
        self.thumb_pages = visible;
    }

    fn inspector_contents(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        egui::ScrollArea::vertical().max_height(440.).id_salt("tool_scroll").show(ui,|ui| {
                if !self.results.is_empty() {
                    section(ui,"SEARCH RESULTS");
                    for (page,snippet) in &self.results {
                        if ui.button(format!("Page {page}  {snippet}")).clicked() {*action=Some(Action::Page(*page));}
                    }ui.separator();
                }
                match self.panel {
                    Panel::Studio=>self.studio_panel(ui,action),
                    Panel::Tools=>self.tools_panel(ui,action),
                    Panel::Pages=>self.pages_panel(ui,action),
                    Panel::Export=>self.export_panel(ui,action),
                    Panel::Files=>self.files_panel(ui,action),
                    Panel::Comments=> {
                        section(ui,"DOCUMENT COMMENTS");
                        ui.add(egui::TextEdit::singleline(&mut self.features.comment_filter).hint_text("Filter comments"));
                        ui.checkbox(&mut self.features.show_resolved,"Show resolved");
                        ui.add(egui::TextEdit::multiline(&mut self.note).hint_text("Reply text").desired_rows(2));
                        let comments=self.current().unwrap().pdf.comments();
                        if comments.is_empty() {ui.label(RichText::new("No comments yet. Add a highlight or comment to start reviewing.").color(palette(ui.ctx()).muted));}
                        for comment in comments {
                            let resolved=self.current().unwrap().pdf.comment_resolved(comment.id);
                            if (!self.features.show_resolved && resolved) || !format!("{} {}",comment.kind,comment.text).to_lowercase().contains(&self.features.comment_filter.to_lowercase()){continue;}
                            egui::Frame::new().fill(palette(ui.ctx()).surface).corner_radius(6).inner_margin(12).show(ui,|ui| {
                                ui.horizontal(|ui| {
                                    if ui.link(format!("Page {}",comment.page)).clicked() {*action=Some(Action::Page(comment.page));}
                                    ui.label(RichText::new(&comment.kind).size(11.).color(palette(ui.ctx()).muted));
                                });
                                if !comment.text.is_empty() {ui.label(&comment.text);}
                                ui.horizontal(|ui|{
                                    if ui.small_button(if resolved{"Reopen"}else{"Resolve"}).clicked(){feature(action,FeatureAction::Resolve(comment.id,!resolved));}
                                    if ui.add_enabled(!self.note.trim().is_empty(),egui::Button::new("Reply").small()).clicked(){feature(action,FeatureAction::Reply(comment.id,comment.page));}
                                });
                                if ui.small_button("Remove").clicked() {*action=Some(Action::RemoveComment(comment.clone()));}
                            });ui.add_space(6.);
                        }
                    },
                    Panel::Forms=> {
                        self.form_author_panel(ui,action);
                        section(ui,"FILL FORM FIELDS");
                        let fields=self.current().unwrap().pdf.fields();
                        if fields.is_empty() {ui.label(RichText::new("This document has no AcroForm fields. Use Add text or Draw / sign to fill a flat document.").color(palette(ui.ctx()).muted));}
                        for field in fields {
                            ui.label(RichText::new(&field.name).strong());
                            if field.editable {
                                let value=self.field_values.entry(field.id).or_insert_with(||field.value.clone());
                                if field.kind=="Btn" {
                                    let mut checked=*value=="Yes";
                                    if ui.checkbox(&mut checked,"Checked").changed() {*action=Some(Action::SetField(field.clone(),if checked {"Yes"} else {"Off"}.into()));}
                                } else {
                                    if (field.kind=="Ch" || field.kind=="Radio") && !field.options.is_empty() {egui::ComboBox::from_id_salt(field.id).selected_text(value.as_str()).show_ui(ui,|ui|{for option in &field.options {ui.selectable_value(value,option.clone(),option);}});} else {ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY));}

                                    if ui.small_button("Apply field").clicked() {*action=Some(Action::SetField(field.clone(),value.clone()));}
                                }
                            } else {ui.label(RichText::new(format!("{} (read only / unsupported type)",field.value)).color(palette(ui.ctx()).muted));}
                            ui.add_space(10.);
                        }
                    },
                    Panel::Text=> {
                        section(ui,"TEXT FROM CURRENT PAGE");
                        ui.label(RichText::new("Select and copy extracted text below.").size(12.).color(palette(ui.ctx()).muted));
                        ui.add_space(10.);if self.current().unwrap().pdf.permits(lopdf::Permissions::COPYABLE){ui.add(egui::TextEdit::multiline(&mut self.text.as_str()).desired_width(f32::INFINITY).desired_rows(30));}else{ui.label("Text extraction is restricted. Reopen with the owner password.");}
                        if ui.button("Export document text").clicked() {*action=Some(Action::TextExport);}
                        ui.add_space(16.);section(ui,"REPLACE TEXT ON THIS PAGE");
                        ui.add(egui::TextEdit::singleline(&mut self.replace_search).hint_text("Existing text").desired_width(f32::INFINITY));
                        ui.add(egui::TextEdit::singleline(&mut self.replace_value).hint_text("Replacement").desired_width(f32::INFINITY));
                        ui.label(RichText::new("Keeps the existing font and position. Text does not reflow.").size(11.).color(palette(ui.ctx()).muted));
                        if ui.button("Replace on current page").clicked() {*action=Some(Action::ReplaceText);}
                    },
                    Panel::Properties=> {
                        section(ui,"DOCUMENT PROPERTIES");
                        let s=self.current().unwrap();
                        ui.label(format!("{} pages  /  PDF {}",s.pdf.page_count(),s.pdf.doc.version));
                        if let Ok(g)=s.pdf.geometry(s.page) {ui.label(format!("Page: {:.0} x {:.0} pt",g.width(),g.height()));ui.label(format!("Rotation: {} degrees",g.rotation));}
                        ui.add_space(12.);
                        for (index,name) in ["Title","Author","Subject"].iter().enumerate() {
                            ui.label(*name);ui.add(egui::TextEdit::singleline(&mut self.metadata[index]).desired_width(f32::INFINITY));
                        }
                        if ui.button("Apply properties").clicked() {*action=Some(Action::Metadata);}
                        ui.add_space(12.);ui.label("Document language");ui.text_edit_singleline(&mut self.features.language);
                        if ui.button("Set language").clicked(){feature(action,FeatureAction::Language);}
                        self.inspection_settings(ui,action);
                        ui.add_space(20.);ui.label(RichText::new(concat!("PDFPilot ", env!("CARGO_PKG_VERSION"), "\nDocument workspace")).color(palette(ui.ctx()).muted).size(12.));
                    },
                }
            });
    }

    fn tools_panel(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        section(ui, "READ & ANNOTATE");
        egui::Grid::new("mark_tools")
            .num_columns(4)
            .spacing([8., 8.])
            .show(ui, |ui| {
                for (index, tool) in [
                    Tool::Select,
                    Tool::Highlight,
                    Tool::Underline,
                    Tool::Rectangle,
                    Tool::Note,
                    Tool::Text,
                    Tool::Ink,
                    Tool::Redact,
                    Tool::Strikeout,
                    Tool::Line,
                    Tool::Arrow,
                    Tool::Circle,
                    Tool::Stamp,
                    Tool::Measure,
                    Tool::Correction,
                    Tool::Attachment,
                ]
                .iter()
                .enumerate()
                {
                    if ui
                        .add(
                            IconButton::new(tool.icon(), tool.name())
                                .size(48.)
                                .selected(self.tool == *tool),
                        )
                        .clicked()
                    {
                        self.tool = *tool;
                        self.drag_start = None;
                        self.stroke.clear();
                    }
                    if index % 4 == 3 {
                        ui.end_row();
                    }
                }
            });
        ui.add_space(12.);
        ui.label(
            RichText::new(self.tool.name())
                .strong()
                .color(palette(ui.ctx()).accent),
        );
        ui.label(
            RichText::new(self.tool.hint())
                .size(12.)
                .color(palette(ui.ctx()).muted),
        );
        if matches!(self.tool, Tool::Note | Tool::Correction | Tool::Attachment)
            || self.tool == Tool::Highlight
            || self.tool == Tool::Underline
            || self.tool == Tool::Rectangle
            || matches!(
                self.tool,
                Tool::Strikeout | Tool::Circle | Tool::Line | Tool::Arrow | Tool::Stamp
            )
        {
            ui.add_space(8.);
            ui.add(
                egui::TextEdit::multiline(&mut self.note)
                    .hint_text("Comment (optional)")
                    .desired_rows(3)
                    .desired_width(f32::INFINITY),
            );
        }
        if self.tool == Tool::Text {
            ui.add_space(8.);
            ui.add(
                egui::TextEdit::singleline(&mut self.add_text)
                    .hint_text("Text to add")
                    .desired_width(f32::INFINITY),
            );
            ui.add(egui::Slider::new(&mut self.features.style.size, 6.0..=72.0).text("pt"));
            egui::ComboBox::from_id_salt("text_font")
                .selected_text(&self.features.style.font)
                .show_ui(ui, |ui| {
                    for font in [
                        "Helvetica",
                        "Helvetica-Bold",
                        "Helvetica-Oblique",
                        "Times-Roman",
                        "Courier",
                    ] {
                        ui.selectable_value(&mut self.features.style.font, font.into(), font);
                    }
                });
            ui.color_edit_button_rgb(&mut self.features.style.color);
        }
        if self.tool == Tool::Ink {
            ui.add(egui::Slider::new(&mut self.ink_width, 0.5..=8.).text("stroke"));
            ui.label(
                RichText::new(
                    "Drawn signatures are visual marks; they are not certificate signatures.",
                )
                .size(11.)
                .color(palette(ui.ctx()).muted),
            );
        }
        ui.add_space(20.);
        ui.separator();
        ui.add_space(12.);
        if wide_button(ui, Icon::Image, "Place an image").clicked() {
            feature(action, FeatureAction::ImageImport);
        }
        if self.tool == Tool::Measure {
            egui::ComboBox::from_id_salt("measurement_mode")
                .selected_text(self.features.studio.measure_mode.label())
                .show_ui(ui, |ui| {
                    for mode in [
                        pdfpilot::measurements::Mode::Distance,
                        pdfpilot::measurements::Mode::Perimeter,
                        pdfpilot::measurements::Mode::Area,
                        pdfpilot::measurements::Mode::Calibrate,
                    ] {
                        ui.selectable_value(
                            &mut self.features.studio.measure_mode,
                            mode,
                            mode.label(),
                        );
                    }
                });
            ui.checkbox(
                &mut self.features.studio.persist_measurement,
                "Save measurement annotation",
            );
            if self.features.studio.measure_mode == pdfpilot::measurements::Mode::Calibrate {
                ui.horizontal(|ui| {
                    ui.label("Known length");
                    ui.add(
                        egui::DragValue::new(&mut self.features.studio.known_mm)
                            .range(0.01..=1_000_000.)
                            .suffix(" mm"),
                    );
                });
            }
            ui.horizontal(|ui| {
                ui.label("Drawing scale 1:");
                ui.add(
                    egui::DragValue::new(&mut self.features.measurement_scale)
                        .range(0.01..=100_000.),
                );
            });
        }
        self.editing_settings(ui, action);
        section(ui, "HEADERS & NUMBERING");
        ui.horizontal(|ui| {
            for (kind, icon) in [
                (RunningKind::Header, Icon::Header),
                (RunningKind::Footer, Icon::Footer),
                (RunningKind::Watermark, Icon::Watermark),
                (RunningKind::PageNumbers, Icon::Labels),
                (RunningKind::Bates, Icon::Stamp),
            ] {
                if ui
                    .add(
                        IconButton::new(icon, kind.label())
                            .selected(self.features.running_kind == kind)
                            .size(38.),
                    )
                    .clicked()
                {
                    self.features.running_kind = kind;
                }
            }
        });
        ui.label(self.features.running_kind.label());
        ui.add(
            egui::TextEdit::singleline(&mut self.features.running_text)
                .hint_text("Text / number prefix"),
        );
        if matches!(
            self.features.running_kind,
            RunningKind::PageNumbers | RunningKind::Bates
        ) {
            ui.horizontal(|ui| {
                ui.label("Start at");
                ui.add(egui::DragValue::new(&mut self.features.number_start).range(1..=1_000_000));
            });
        }
        if self.features.running_kind == RunningKind::Bates {
            ui.horizontal(|ui| {
                ui.label("Digits");
                ui.add(egui::DragValue::new(&mut self.features.number_padding).range(1..=12));
            });
        }
        if wide_button(ui, Icon::Check, "Apply to all pages").clicked() {
            feature(action, FeatureAction::RunningText);
        }
        ui.add_space(12.);
        section(ui, "READ ALOUD");
        ui.horizontal(|ui| {
            if ui
                .add(IconButton::new(Icon::Volume, "Read page aloud"))
                .clicked()
            {
                feature(action, FeatureAction::ReadAloud);
            }
            if ui
                .add(IconButton::new(Icon::Stop, "Stop reading aloud"))
                .clicked()
            {
                feature(action, FeatureAction::StopReading);
            }
        });
    }

    fn viewer(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        self.main_keys.clear();
        let Some(index) = self.active else {
            self.home(ui, action);
            return;
        };
        let session = &mut self.sessions[index];
        let display = session
            .sizes
            .get(session.page as usize - 1)
            .copied()
            .unwrap_or([612., 792.]);
        let columns = if session.view == ViewMode::TwoPage {
            2.
        } else {
            1.
        };
        let base_width = (ui.available_width() - 64.) / columns;
        if session.fit || session.fit_page {
            let width = base_width.max(100.) / (display[0] * 96. / 72.);
            let height = (ui.available_height() - 72.).max(100.) / (display[1] * 96. / 72.);
            session.zoom = if session.fit_page {
                width.min(height)
            } else {
                width
            }
            .clamp(0.1, 4.);
        }
        let scale = 96. / 72. * session.zoom;
        let row_height = session
            .sizes
            .iter()
            .map(|s| s[1] * scale)
            .fold(0., f32::max)
            + 64.;
        let mut drag_start = self.drag_start;
        let mut drag_page = self.drag_page;
        let mut stroke = self.stroke.clone();
        let mut keys = vec![];
        let mut draw = |ui: &mut egui::Ui, page: u32| {
            let display = session
                .sizes
                .get(page as usize - 1)
                .copied()
                .unwrap_or([612., 792.]);
            let size = Vec2::new(display[0], display[1]) * scale;
            let width =
                (((size.x * ui.ctx().pixels_per_point()) as u32).clamp(600, 2200) / 100) * 100;
            let key = session.key(page, width);
            let texture = session
                .cache
                .get(&key)
                .or_else(|| {
                    session
                        .cache
                        .iter()
                        .find(|(k, _)| {
                            k.page == page && k.revision == session.pdf.revision && k.width > 144
                        })
                        .map(|(_, v)| v)
                })
                .cloned();
            let mut page_action = None;
            let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
            ui.painter().rect_filled(
                rect.translate(Vec2::new(0., 4.)).expand(2.),
                1.,
                Color32::from_black_alpha(14),
            );
            ui.painter().rect_filled(rect, 0., Color32::WHITE);
            if let Some(texture) = texture {
                ui.painter().image(
                    texture.id(),
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0., 0.), egui::pos2(1., 1.)),
                    Color32::WHITE,
                );
            } else {
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    if self.renderer.available {
                        "Rendering page..."
                    } else {
                        "Install Poppler to render this PDF. See README."
                    },
                    FontId::proportional(16.),
                    palette(ui.ctx()).muted,
                );
            }
            if response.hovered() && self.tool != Tool::Select {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            }
            if self.features.studio.redaction_document == Some(session.id) {
                for region in self
                    .features
                    .studio
                    .redactions
                    .iter()
                    .filter(|r| r.page == page)
                {
                    let a = rect.min
                        + Vec2::new(
                            region.start[0] * rect.width(),
                            region.start[1] * rect.height(),
                        );
                    let b = rect.min
                        + Vec2::new(region.end[0] * rect.width(), region.end[1] * rect.height());
                    ui.painter().rect_filled(
                        egui::Rect::from_two_pos(a, b),
                        0.,
                        Color32::from_black_alpha(160),
                    );
                }
            }
            let normalize = |pos: egui::Pos2| {
                [
                    ((pos.x - rect.min.x) / rect.width()).clamp(0., 1.),
                    ((pos.y - rect.min.y) / rect.height()).clamp(0., 1.),
                ]
            };
            let denormalize = |p: [f32; 2]| {
                egui::pos2(
                    rect.min.x + p[0] * rect.width(),
                    rect.min.y + p[1] * rect.height(),
                )
            };
            if response.clicked() && self.tool == Tool::Select {
                page_action = Some(Action::Page(page));
            }
            if texture_is_ready(session, key) {
                if response.clicked()
                    && let Some(pos) = response.interact_pointer_pos()
                {
                    let point = normalize(pos);
                    match self.tool {
                        Tool::Text => page_action = Some(Action::Text(point)),
                        Tool::Correction => feature(
                            &mut page_action,
                            FeatureAction::Studio(studio::StudioAction::Correction(point)),
                        ),
                        Tool::Attachment => feature(
                            &mut page_action,
                            FeatureAction::Studio(studio::StudioAction::AttachComment(point)),
                        ),
                        Tool::Note => {
                            page_action = Some(Action::Mark(
                                Mark::Note,
                                point,
                                [
                                    (point[0] + 20. / rect.width()).min(1.),
                                    (point[1] + 20. / rect.height()).min(1.),
                                ],
                            ))
                        }
                        _ => {}
                    }
                }
                if response.drag_started()
                    && let Some(pos) = response.interact_pointer_pos()
                {
                    let origin = normalize(pos - response.drag_delta());
                    drag_start = Some(origin);
                    drag_page = Some(page);
                    if self.tool == Tool::Ink {
                        stroke = vec![origin];
                    }
                }
                if response.dragged()
                    && drag_page == Some(page)
                    && let Some(pos) = response.interact_pointer_pos()
                {
                    let point = normalize(pos);
                    if self.tool == Tool::Ink {
                        stroke.push(point);
                    } else if self.tool != Tool::Select
                        && let Some(start) = drag_start
                    {
                        let selection = egui::Rect::from_two_pos(
                            denormalize(start),
                            pos.clamp(rect.min, rect.max),
                        );
                        if self.tool == Tool::Redact {
                            ui.painter()
                                .rect_filled(selection, 0., Color32::from_black_alpha(200));
                        } else if self.tool == Tool::Rectangle {
                            ui.painter().rect_stroke(
                                selection,
                                0.,
                                Stroke::new(1.5, palette(ui.ctx()).accent),
                                egui::StrokeKind::Inside,
                            );
                        } else {
                            ui.painter().rect_filled(
                                selection,
                                0.,
                                Color32::from_rgba_unmultiplied(255, 207, 57, 100),
                            );
                        }
                    }
                }
                if response.drag_stopped() && drag_page == Some(page) {
                    if let (Some(start), Some(pos)) = (drag_start, response.interact_pointer_pos())
                    {
                        let end = normalize(pos);
                        if self.tool == Tool::Ink && stroke.len() > 1 {
                            page_action = Some(Action::Ink(stroke.clone()));
                        } else if (end[0] - start[0]).abs().max((end[1] - start[1]).abs()) > 0.003 {
                            if self.tool == Tool::Redact {
                                page_action = Some(Action::Redact(start, end));
                            }
                            let extra = match self.tool {
                                Tool::Crop => Some(FeatureAction::Crop(start, end)),
                                Tool::Link => Some(FeatureAction::Link(start, end)),
                                Tool::Form => Some(FeatureAction::AddField(start, end)),
                                Tool::Image => Some(FeatureAction::Image(start, end)),
                                Tool::Measure => Some(FeatureAction::Measure(start, end)),
                                Tool::Strikeout => {
                                    Some(FeatureAction::Shape(Shape::Strikeout, start, end))
                                }
                                Tool::Line => Some(FeatureAction::Shape(Shape::Line, start, end)),
                                Tool::Arrow => Some(FeatureAction::Shape(Shape::Arrow, start, end)),
                                Tool::Circle => {
                                    Some(FeatureAction::Shape(Shape::Circle, start, end))
                                }
                                Tool::Stamp => Some(FeatureAction::Shape(Shape::Stamp, start, end)),
                                _ => None,
                            };
                            if let Some(extra) = extra {
                                feature(&mut page_action, extra);
                            }
                            let kind = match self.tool {
                                Tool::Highlight => Some(Mark::Highlight),
                                Tool::Underline => Some(Mark::Underline),
                                Tool::Rectangle => Some(Mark::Rectangle),
                                _ => None,
                            };
                            if let Some(kind) = kind {
                                page_action = Some(Action::Mark(kind, start, end));
                            }
                        }
                    }
                    drag_start = None;
                    drag_page = None;
                    stroke.clear();
                }
            }
            if self.tool == Tool::Ink && stroke.len() > 1 {
                ui.painter().add(egui::Shape::line(
                    stroke.iter().map(|p| denormalize(*p)).collect(),
                    Stroke::new(
                        self.ink_width * session.zoom * 96. / 72.,
                        Color32::from_rgb(28, 35, 49),
                    ),
                ));
            }
            if ui.is_rect_visible(rect) {
                keys.push(key);
            }
            ui.label(
                RichText::new(format!("Page {page}"))
                    .size(11.)
                    .color(palette(ui.ctx()).muted),
            );
            if let Some(next) = page_action {
                *action = Some(Action::OnPage(page, Box::new(next)));
            }
        };
        let mut scroll =
            egui::ScrollArea::both().id_salt(("page_canvas", session.id, session.view as u8));
        if self.jump_page && session.view == ViewMode::Continuous {
            scroll = scroll.vertical_scroll_offset((session.page - 1) as f32 * row_height);
        }
        self.jump_page = false;
        match session.view {
            ViewMode::Continuous => {
                scroll.show_rows(
                    ui,
                    row_height - ui.spacing().item_spacing.y,
                    session.pdf.page_count() as usize,
                    |ui, range| {
                        for index in range {
                            ui.horizontal(|ui| {
                                let width = session
                                    .sizes
                                    .get(index)
                                    .map(|s| s[0] * scale)
                                    .unwrap_or(612. * scale);
                                ui.add_space(((ui.available_width() - width) / 2.).max(24.));
                                ui.vertical(|ui| {
                                    draw(ui, index as u32 + 1);
                                    ui.add_space(
                                        (row_height - 64. - session.sizes[index][1] * scale)
                                            .max(0.)
                                            + 36.,
                                    );
                                });
                            });
                        }
                    },
                );
            }
            ViewMode::Single | ViewMode::TwoPage => {
                scroll.show(ui, |ui| {
                    ui.add_space(24.);
                    let start = if session.view == ViewMode::TwoPage {
                        (session.page - 1) / 2 * 2 + 1
                    } else {
                        session.page
                    };
                    let end = if session.view == ViewMode::TwoPage {
                        (start + 1).min(session.pdf.page_count())
                    } else {
                        start
                    };
                    let total = (start..=end)
                        .map(|p| {
                            session
                                .sizes
                                .get(p as usize - 1)
                                .map(|d| d[0] * scale)
                                .unwrap_or(612. * scale)
                        })
                        .sum::<f32>()
                        + 24. * (end - start) as f32;
                    ui.horizontal_top(|ui| {
                        ui.add_space(((ui.available_width() - total) / 2.).max(24.));
                        for p in start..=end {
                            ui.vertical(|ui| {
                                draw(ui, p);
                            });
                            ui.add_space(16.);
                        }
                    });
                    ui.add_space(28.);
                });
            }
        }
        self.drag_start = drag_start;
        self.drag_page = drag_page;
        self.stroke = stroke;
        self.main_keys = keys;
    }

    fn dialogs(&mut self, ctx: &egui::Context, action: &mut Option<Action>) {
        let Some(modal) = self.modal.take() else {
            return;
        };
        let mut keep = true;
        let title = match modal {
            Modal::Create => "Create a PDF",
            Modal::Password(_) => "Unlock PDF",
            Modal::Protect => "Protect a copy",
            Modal::Close(_) => "Unsaved changes",
            Modal::Quit => "Unsaved documents",
            Modal::Delete => "Delete pages",
            Modal::Redact(_, _) => "Export a redacted copy",
            Modal::Help => "PDFPilot help",
            Modal::Report => "Document comparison",
            Modal::Print => "Print document",
        };
        egui::Modal::new(egui::Id::new("active_dialog")).show(ctx,|ui| {
            ui.heading(title);ui.add_space(12.);
            ui.set_width(430.);
            match &modal {
                Modal::Create=> {
                    ui.label("Document title");ui.add(egui::TextEdit::singleline(&mut self.create_title).desired_width(f32::INFINITY));
                    ui.label("Document text");ui.add(egui::TextEdit::multiline(&mut self.create_body).desired_rows(15).desired_width(f32::INFINITY));
                    ui.label(RichText::new("Western Latin text; long documents flow onto multiple pages.").size(11.).color(palette(ui.ctx()).muted));
                    if primary(ui, Icon::New, "Create PDF").clicked() {*action=Some(Action::Create);keep=false;}
                },
                Modal::Password(path)=> {
                    ui.label(path.file_name().unwrap_or_default().to_string_lossy());
                    ui.add(egui::TextEdit::singleline(&mut self.password).password(true).hint_text("Document password").desired_width(f32::INFINITY));
                    if primary(ui, Icon::Lock, "Unlock").clicked() {*action=Some(Action::PasswordOpen(path.clone()));keep=false;}
                },
                Modal::Protect=> {
                    ui.label("Save a separate PDF encrypted with AES-256.");
                    ui.add(egui::TextEdit::singleline(&mut self.password).password(true).hint_text("Password").desired_width(f32::INFINITY));
                    ui.add(egui::TextEdit::singleline(&mut self.password_confirm).password(true).hint_text("Confirm password").desired_width(f32::INFINITY));
                    ui.add(egui::TextEdit::singleline(&mut self.features.studio.security.owner_password).password(true).hint_text("Owner password (optional if unrestricted)"));
                    let permissions=&mut self.features.studio.security;
                    ui.checkbox(&mut permissions.print,"Allow printing");ui.checkbox(&mut permissions.edit,"Allow editing");ui.checkbox(&mut permissions.copy,"Allow copying");ui.checkbox(&mut permissions.annotate,"Allow annotations");ui.checkbox(&mut permissions.fill,"Allow form filling");ui.checkbox(&mut permissions.assemble,"Allow page assembly");
                    let valid=!self.password.is_empty() && self.password==self.password_confirm;
                    if ui.add_enabled(valid,egui::Button::new("Save protected copy").fill(PRIMARY)).clicked() {*action=Some(Action::ProtectConfirmed);keep=false;}
                },
                Modal::Close(index)=> {
                    ui.label(format!("Save changes to {} before closing?",self.sessions[*index].pdf.title));
                    ui.horizontal(|ui| {
                        if primary(ui, Icon::Save, "Save and close").clicked() {
                            self.active=Some(*index);
                            if self.save_current(false) {*action=Some(Action::DiscardClose(*index));keep=false;}
                        }
                        if ui.button("Discard changes").clicked() {*action=Some(Action::DiscardClose(*index));keep=false;}
                    });
                },
                Modal::Quit=> {
                    ui.label("There are unsaved changes. Save each document or discard all changes to quit.");
                    if ui.button("Discard all and quit").clicked() {*action=Some(Action::DiscardQuit);keep=false;}
                },
                Modal::Delete=> {
                    ui.label(format!("Remove pages {} from this document?",self.page_range));
                    ui.label(RichText::new("You can undo this before closing the document.").size(12.).color(palette(ui.ctx()).muted));
                    if ui.button("Delete selected pages").clicked() {*action=Some(Action::DeleteConfirmed);keep=false;}
                },
                Modal::Redact(start,end)=> {
                    ui.label("The marked region will be removed from a separate PDF. Every page will become an image; text layers, forms, links, attachments, and metadata will be discarded.");
                    ui.label("Your editable document remains open.");
                    if wide_button(ui,Icon::New,"Stage region and continue marking").clicked(){feature(action,FeatureAction::Studio(studio::StudioAction::StageRedaction(*start,*end)));keep=false;}
                    if primary(ui, Icon::Redact, "Export redacted copy").clicked() {*action=Some(Action::RedactConfirmed(*start,*end));keep=false;}
                },
                Modal::Report=>{
                    egui::ScrollArea::vertical().max_height(460.).show(ui,|ui|{ui.add(egui::TextEdit::multiline(&mut self.features.report.as_str()).desired_width(f32::INFINITY).desired_rows(15));});
                    if primary(ui, Icon::Export, if self.features.report_html.is_empty(){"Save text report"}else{"Save visual HTML report"}).clicked(){feature(action,FeatureAction::ExportReport);keep=false;}
                },
                Modal::Print=>{
                    ui.checkbox(&mut self.features.print_all,"All pages");if !self.features.print_all{ui.text_edit_singleline(&mut self.features.print_range);}
                    egui::ComboBox::from_id_salt("nup").selected_text(format!("{} pages per sheet",self.features.print_nup)).show_ui(ui,|ui|{for n in [1,2,4,6,9,16]{ui.selectable_value(&mut self.features.print_nup,n,n.to_string());}});
                    egui::ComboBox::from_id_salt("print_paper").selected_text(self.features.print_paper.label()).show_ui(ui,|ui|{for paper in [pdfpilot::printing::Paper::Letter,pdfpilot::printing::Paper::A4,pdfpilot::printing::Paper::Legal]{ui.selectable_value(&mut self.features.print_paper,paper,paper.label());}});
                    ui.add(egui::Slider::new(&mut self.features.print_scale,10..=400).text("Scale percent"));ui.checkbox(&mut self.features.print_booklet,"Booklet (short-edge duplex)");ui.checkbox(&mut self.features.print_marks,"Booklet fold marks");if wide_button(ui,Icon::Pages,"Export booklet sheet preview").clicked(){feature(action,FeatureAction::Studio(studio::StudioAction::BookletPreview));keep=false;}
                    ui.checkbox(&mut self.features.print_gray,"Grayscale");ui.checkbox(&mut self.features.print_landscape,"Landscape");ui.checkbox(&mut self.features.print_fit,"Fit to printable area");
                    ui.label("Uses your system default printer.");if primary(ui, Icon::Print, "Print").clicked(){feature(action,FeatureAction::PrintNow);keep=false;}
                },
                Modal::Help=> {
                    ui.label(RichText::new("PDFPilot / Native Rust PDF workspace").strong().size(18.));ui.add_space(10.);
                    for line in ["Cmd/Ctrl+O   Open PDF","Cmd/Ctrl+S   Save","Cmd/Ctrl+Shift+S   Save a copy","Cmd/Ctrl+Z   Undo","Cmd/Ctrl+Shift+Z   Redo","Cmd/Ctrl+F   Find","Cmd/Ctrl+W   Close document","Page Up / Page Down   Previous / next page","Escape   Read tool"] {ui.label(line);}
                    ui.separator();ui.label("Supported: PDF viewing, annotations, added/replaced text, ink, page operations, form authoring, bookmarks, attachments, Office export/import, comparison, OCR, raster redaction, compression, and AES-256 copies.");
                    ui.label("Content reflow, certificate signatures, XFA, PDF/A validation, and cloud collaboration remain unsupported. See FEATURES.md for checklist coverage.");
                    if ui.button("Close").clicked() {keep=false;}
                },
            }
            if !matches!(modal,Modal::Help) && ui.button("Cancel").clicked() {keep=false;self.password.clear();self.password_confirm.clear();}
        });
        if keep {
            self.modal = Some(modal);
        }
    }

    fn save_current(&mut self, copy: bool) -> bool {
        let Some(s) = self.current() else {
            return false;
        };
        let path = if copy || s.pdf.path.is_none() {
            rfd::FileDialog::new()
                .add_filter("PDF", &["pdf"])
                .set_file_name(if copy {
                    "document-copy.pdf"
                } else {
                    &s.pdf.title
                })
                .save_file()
        } else {
            s.pdf.path.clone()
        };
        let Some(path) = path else {
            return false;
        };
        let result = if copy {
            self.current().unwrap().pdf.save_copy(&path)
        } else {
            self.current_mut().unwrap().pdf.save(&path)
        };
        let ok = result.is_ok();
        self.run(result, if copy { "PDF copy saved" } else { "PDF saved" });
        ok
    }
    fn background(&mut self, job: impl FnOnce() -> Result<String> + Send + 'static) {
        let tx = self.sender.clone();
        self.jobs += 1;
        self.status = "Working...".into();
        std::thread::spawn(move || {
            let _ = tx.send(Event::Done(job()));
        });
    }

    fn execute(&mut self, action: Action, ctx: &egui::Context) {
        if let Some(session) = self.current() {
            let copy = matches!(
                &action,
                Action::TextExport | Action::Ocr | Action::RedactConfirmed(_, _)
            );
            let protection = matches!(&action, Action::Protect | Action::ProtectConfirmed);
            if (copy && !session.pdf.permits(lopdf::Permissions::COPYABLE))
                || (protection && !session.pdf.owner_authenticated())
            {
                self.error = Some(
                    "Document permissions restrict this operation. Reopen with the owner password."
                        .into(),
                );
                return;
            }
        }

        match action {
            Action::Feature(action) => self.execute_feature(action),
            Action::OnPage(page, action) => {
                if let Some(s) = self.current_mut() {
                    s.page = page;
                }
                self.execute(*action, ctx);
            }
            Action::FitPage => {
                if let Some(s) = self.current_mut() {
                    s.fit_page = true;
                    s.fit = false;
                }
            }
            Action::Open => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(
                        "Documents",
                        &[
                            "pdf", "docx", "xlsx", "pptx", "doc", "xls", "ppt", "odt", "ods",
                            "odp", "rtf", "html", "png", "jpg", "jpeg", "tiff",
                        ],
                    )
                    .pick_file()
                {
                    self.open_path(path);
                }
            }
            Action::OpenPath(path) => self.open_path(path),
            Action::Sample => {
                let result = pdf::demo_document().and_then(|doc| {
                    self.add_document(PdfDocument::from_document(doc, "Welcome to PDFPilot.pdf"))
                });
                self.run(result, "Sample opened");
            }
            Action::New => self.modal = Some(Modal::Create),
            Action::Images => {
                if let Some(paths) = rfd::FileDialog::new()
                    .add_filter("Images", &["png", "jpg", "jpeg"])
                    .pick_files()
                {
                    let result = pdf::images_to_document(&paths).and_then(|doc| {
                        self.add_document(PdfDocument::from_document(doc, "Images.pdf"))
                    });
                    self.run(result, "PDF created from images");
                }
            }
            Action::Create => {
                let result = pdf::create_text_document(&self.create_title, &self.create_body)
                    .and_then(|doc| {
                        self.add_document(PdfDocument::from_document(doc, "Untitled.pdf"))
                    });
                self.run(result, "PDF created");
            }
            Action::Save(copy) => {
                self.save_current(copy);
            }
            Action::Tab(index) => {
                self.active = Some(index);
                self.synced = None;
            }
            Action::Close(index) => {
                if self.sessions[index].pdf.dirty {
                    self.modal = Some(Modal::Close(index));
                } else {
                    self.close(index);
                }
            }
            Action::DiscardClose(index) => self.close(index),
            Action::Quit => {
                if self.sessions.iter().any(|s| s.pdf.dirty) {
                    self.modal = Some(Modal::Quit);
                } else {
                    self.allow_quit = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            Action::DiscardQuit => {
                self.allow_quit = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Action::PasswordOpen(path) => {
                let password = std::mem::take(&mut self.password);
                let tx = self.sender.clone();
                self.jobs += 1;
                std::thread::spawn(move || {
                    let result = PdfDocument::open(&path, &password).map(Box::new);
                    let _ = tx.send(Event::Open { path, result });
                });
            }
            Action::Page(page) => {
                self.jump_page = true;
                if let Some(s) = self.current_mut() {
                    if page > 0 && page <= s.pdf.page_count() {
                        s.page = page;
                    } else {
                        self.error = Some("Page is out of range".into());
                    }
                }
            }
            Action::Zoom(zoom) => {
                if let Some(s) = self.current_mut() {
                    s.fit = false;
                    s.fit_page = false;
                    s.zoom = zoom.clamp(0.1, 4.);
                }
            }
            Action::Fit => {
                if let Some(s) = self.current_mut() {
                    s.fit = true;
                    s.fit_page = false;
                }
            }
            Action::Undo => {
                if let Some(s) = self.current_mut() {
                    s.pdf.undo();
                    s.page = s.page.min(s.pdf.page_count());
                }
            }
            Action::Redo => {
                if let Some(s) = self.current_mut() {
                    s.pdf.redo();
                    s.page = s.page.min(s.pdf.page_count());
                }
            }
            Action::Delete => self.modal = Some(Modal::Delete),
            Action::Protect => {
                self.password.clear();
                self.password_confirm.clear();
                self.modal = Some(Modal::Protect);
            }
            Action::Redact(start, end) => self.modal = Some(Modal::Redact(start, end)),
            Action::RedactConfirmed(start, end) => {
                let Some(s) = self.current() else {
                    return;
                };
                let doc = s.pdf.doc.clone();
                let page = s.page;
                let mut regions = if self.features.studio.redaction_document == Some(s.id) {
                    self.features.studio.redactions.clone()
                } else {
                    vec![]
                };
                regions.push(pdfpilot::workflows::Redaction { page, start, end });
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("PDF", &["pdf"])
                    .set_file_name("redacted.pdf")
                    .save_file()
                {
                    self.background(move || {
                        pdfpilot::workflows::redacted_copy(&doc, &regions, &path)?;
                        Ok(format!("Redacted raster PDF saved to {}", path.display()))
                    });
                }
            }
            Action::Ocr => {
                let Some(s) = self.current() else {
                    return;
                };
                let doc = s.pdf.doc.clone();
                let options = self.features.ocr.clone();
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("PDF", &["pdf"])
                    .set_file_name("searchable.pdf")
                    .save_file()
                {
                    self.background(move || {
                        pdfpilot::workflows::searchable_copy_with_options(&doc, &options, &path)?;
                        Ok(format!("Searchable PDF saved to {}", path.display()))
                    });
                }
            }
            Action::ProtectConfirmed => {
                let password = std::mem::take(&mut self.password);
                self.password_confirm.clear();
                let mut security = self.features.studio.security.clone();
                security.open_password = password.clone();
                if security.owner_password.is_empty() {
                    security.owner_password = password;
                }
                self.features.studio.security.owner_password.clear();
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("PDF", &["pdf"])
                    .set_file_name("protected.pdf")
                    .save_file()
                {
                    let doc = self.current().unwrap().pdf.doc.clone();
                    self.background(move || {
                        pdfpilot::security::protect(&doc, &security, &path)?;
                        Ok(format!("Protected PDF saved to {}", path.display()))
                    });
                }
            }
            Action::Search => {
                self.panel = Panel::Text;
                self.catalog_selection = 9;
                self.catalog_jump = true;
                self.tools_open = true;
                self.inspector_open = true;
                if self.find.trim().is_empty() {
                    self.results.clear();
                    return;
                }
                let Some(s) = self.current() else {
                    return;
                };
                let document = s.id;
                let revision = s.pdf.revision;
                let doc = s.pdf.doc.clone();
                let query = self.find.to_lowercase();
                let tx = self.sender.clone();
                self.jobs += 1;
                self.search_token += 1;
                let token = self.search_token;
                self.search_generation = Some((document, revision, token));
                self.status = "Searching...".into();
                std::thread::spawn(move || {
                    let result = doc
                        .get_pages()
                        .keys()
                        .filter_map(|page| {
                            let text = doc
                                .extract_text_with_limit(&[*page], pdf::TEXT_LIMIT)
                                .ok()?;
                            text.to_lowercase().contains(&query).then(|| {
                                (
                                    *page,
                                    text.lines()
                                        .find(|line| line.to_lowercase().contains(&query))
                                        .unwrap_or("")
                                        .chars()
                                        .take(60)
                                        .collect(),
                                )
                            })
                        })
                        .collect();
                    let _ = tx.send(Event::Search {
                        document,
                        revision,
                        token,
                        result,
                    });
                });
            }
            Action::TextExport => {
                let Some(s) = self.current() else {
                    return;
                };
                let doc = s.pdf.doc.clone();
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Text", &["txt"])
                    .set_file_name("document.txt")
                    .save_file()
                {
                    self.background(move || {
                        let pages = doc.get_pages().keys().copied().collect::<Vec<_>>();
                        let text = doc.extract_text_with_limit(&pages, pdf::TEXT_LIMIT)?;
                        std::fs::write(&path, text)?;
                        Ok(format!("Text exported to {}", path.display()))
                    });
                }
            }
            Action::Print => self.execute_feature(FeatureAction::Print),
            Action::Extract => {
                let Some(s) = self.current() else {
                    return;
                };
                let result = pdf::parse_pages(&self.page_range, s.pdf.page_count());
                match result {
                    Ok(pages) => {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("PDF", &["pdf"])
                            .set_file_name("extracted.pdf")
                            .save_file()
                        {
                            let result = self.current().unwrap().pdf.extract(&pages, &path);
                            self.run(result, "Pages extracted");
                        }
                    }
                    Err(e) => self.error = Some(format!("{e:#}")),
                }
            }
            Action::Merge => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("PDF", &["pdf"])
                    .pick_file()
                {
                    let result = self.current_mut().unwrap().pdf.merge(&path);
                    self.run(result, "PDFs combined");
                }
            }
            other => {
                let note = self.note.clone();
                let text = self.add_text.clone();
                let style = self.features.style.clone();
                let width = self.ink_width;
                let range = self.page_range.clone();
                let metadata = self.metadata.clone();
                let replace_search = self.replace_search.clone();
                let replace_value = self.replace_value.clone();
                let Some(s) = self.current_mut() else {
                    return;
                };
                let page = s.page;
                let result = match other {
                    Action::Rotate(degrees) => s.pdf.rotate(page, degrees),
                    Action::Move(to) => s.pdf.move_page(page, to).map(|_| s.page = to),
                    Action::Insert => s.pdf.insert_blank(page).map(|_| s.page = page + 1),
                    Action::DeleteConfirmed => pdf::parse_pages(&range, s.pdf.page_count())
                        .and_then(|pages| s.pdf.delete(&pages))
                        .map(|_| s.page = s.page.min(s.pdf.page_count())),
                    Action::Compress => s.pdf.compress(),
                    Action::Mark(kind, a, b) => s.pdf.add_mark(page, kind, a, b, &note),
                    Action::Text(position) => s.pdf.add_styled_text(page, position, &text, &style),
                    Action::Ink(points) => s.pdf.add_ink(page, &points, width),
                    Action::RemoveComment(comment) => s.pdf.remove_comment(&comment),
                    Action::SetField(field, value) => s.pdf.set_field(&field, &value),
                    Action::Metadata => {
                        s.pdf.set_metadata(&metadata[0], &metadata[1], &metadata[2])
                    }
                    Action::ReplaceText => {
                        s.pdf.replace_text(page, &replace_search, &replace_value)
                    }
                    _ => Ok(()),
                };
                s.cache.retain(|key, _| key.revision == s.pdf.revision);
                s.failed.clear();
                self.run(result, "Document updated");
            }
        }
    }

    fn close(&mut self, index: usize) {
        self.sessions.remove(index);
        self.active = if self.sessions.is_empty() {
            None
        } else {
            Some(index.min(self.sessions.len() - 1))
        };
        self.synced = None;
    }
    fn schedule_render(&mut self) {
        if self.render_busy || !self.renderer.available {
            return;
        }
        let main_keys = self.main_keys.clone();
        let thumbs = self.thumb_pages.clone();
        let Some(s) = self.current_mut() else {
            return;
        };
        let mut keys = main_keys;
        keys.extend(thumbs.into_iter().map(|p| s.key(p, 144)));
        let Some(key) = keys
            .into_iter()
            .find(|k| !s.cache.contains_key(k) && !s.failed.contains(k))
        else {
            return;
        };
        match s.snapshot() {
            Ok(path) => {
                let result = self.renderer.request(RenderRequest { key, path });
                if result.is_ok() {
                    self.render_busy = true;
                } else {
                    self.run(result, "Rendering");
                }
            }
            Err(e) => self.error = Some(format!("{e:#}")),
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context, action: &mut Option<Action>) {
        if self.modal.is_some() {
            return;
        }
        let editing_text = ctx.wants_keyboard_input();
        ctx.input_mut(|input| {
            let cmd = egui::Modifiers::COMMAND;
            let shift = egui::Modifiers { shift: true, ..cmd };
            for (mods, key, a) in [
                (cmd, egui::Key::O, Action::Open),
                (shift, egui::Key::S, Action::Save(true)),
                (cmd, egui::Key::S, Action::Save(false)),
                (shift, egui::Key::Z, Action::Redo),
                (cmd, egui::Key::Z, Action::Undo),
            ] {
                if key == egui::Key::Z && editing_text {
                    continue;
                }
                if input.consume_key(mods, key) {
                    *action = Some(a);
                }
            }
            if input.consume_key(cmd, egui::Key::W)
                && let Some(index) = self.active
            {
                *action = Some(Action::Close(index));
            }
        });
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F)) {
            self.search_open = true;
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("find_input")));
        }
        if !ctx.wants_keyboard_input() {
            if let Some(s) = self.current() {
                if ctx.input(|i| i.key_pressed(egui::Key::PageDown)) {
                    *action = Some(Action::Page((s.page + 1).min(s.pdf.page_count())));
                }
                if ctx.input(|i| i.key_pressed(egui::Key::PageUp)) {
                    *action = Some(Action::Page(s.page.saturating_sub(1).max(1)));
                }
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.tool = Tool::Select;
            }
        }
    }
}

impl eframe::App for PdfApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("dark_mode", self.dark_mode.to_string());
    }
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.dark_mode = ctx.style().visuals.dark_mode;
        self.poll(ctx);
        self.sync();
        #[cfg(target_os = "macos")]
        self.poll_native_menu(ctx);
        let mut action = None;
        if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_quit
            && self.sessions.iter().any(|s| s.pdf.dirty)
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.modal = Some(Modal::Quit);
        }
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for file in dropped {
            if let Some(path) = file.path {
                self.open_path(path);
            }
        }
        self.shortcuts(ctx, &mut action);
        self.header(ctx, &mut action);
        self.toolbar(ctx, &mut action);
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(palette(ctx).panel)
                    .inner_margin(egui::Margin::symmetric(10, 3)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if self.jobs > 0 || self.render_busy {
                        ui.spinner();
                    }
                    ui.label(
                        RichText::new(&self.status)
                            .size(11.)
                            .color(palette(ui.ctx()).muted),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(self.tool.name())
                                .size(10.)
                                .color(palette(ui.ctx()).muted),
                        );
                    });
                });
                if let Some(error) = self.error.clone() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new(error)
                                .size(12.)
                                .color(palette(ui.ctx()).error),
                        );
                        if ui.small_button("Dismiss").clicked() {
                            self.error = None;
                        }
                    });
                }
                if !self.renderer.available {
                    ui.label(
                        RichText::new("PDF rendering needs Poppler. See README for installation.")
                            .color(palette(ui.ctx()).error)
                            .size(12.),
                    );
                }
            });
        self.navigation_rail(ctx);
        self.thumbnails(ctx, &mut action);
        self.sidebar(ctx, &mut action);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(if self.active.is_some() {
                palette(ctx).canvas
            } else {
                palette(ctx).background
            }))
            .show(ctx, |ui| {
                self.viewer(ui, &mut action);
            });
        self.dialogs(ctx, &mut action);
        if let Some(action) = action {
            self.execute(action, ctx);
        }
        #[cfg(target_os = "macos")]
        self.sync_native_menu(ctx);
        self.poll_review(ctx);
        self.schedule_render();
        if self.jobs > 0 || self.render_busy {
            ctx.request_repaint_after(Duration::from_millis(80));
        }
    }
}

fn texture_is_ready(session: &Session, key: RenderKey) -> bool {
    session
        .cache
        .keys()
        .any(|k| k.page == key.page && k.revision == key.revision && k.width > 144)
}
fn primary(ui: &mut egui::Ui, icon: Icon, label: &str) -> egui::Response {
    icons::action(ui, icon, label, true)
}
fn wide_button(ui: &mut egui::Ui, icon: Icon, label: &str) -> egui::Response {
    icons::action(ui, icon, label, false)
}
fn section(ui: &mut egui::Ui, label: &str) {
    ui.label(
        RichText::new(label)
            .size(10.)
            .strong()
            .color(palette(ui.ctx()).muted),
    );
    ui.add_space(7.);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(app: &mut PdfApp, ctx: &egui::Context, events: Vec<egui::Event>) -> Option<Action> {
        let mut action = None;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440., 960.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.viewer(ui, &mut action));
            },
        );
        action
    }
    fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }
    #[test]
    fn form_drag_on_spread_targets_the_interacted_page() {
        let ctx = egui::Context::default();
        let mut app = PdfApp::with_context(&ctx, true);
        let mut pdf = PdfDocument::from_document(pdf::demo_document().unwrap(), "sample.pdf");
        pdf.duplicate_page(1).unwrap();
        app.add_document(pdf).unwrap();
        app.sync();
        app.tool = Tool::Form;
        let texture = ctx.load_texture(
            "fixture",
            egui::ColorImage::new([1, 1], vec![Color32::WHITE]),
            Default::default(),
        );
        let s = app.current_mut().unwrap();
        s.view = ViewMode::TwoPage;
        s.cache.insert(s.key(1, 600), texture.clone());
        s.cache.insert(s.key(2, 600), texture);
        frame(&mut app, &ctx, vec![]);
        let a = egui::pos2(900., 400.);
        let b = egui::pos2(1100., 450.);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(a), button(a, true)],
        );
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(b)]);
        let action =
            frame(&mut app, &ctx, vec![button(b, false)]).expect("A drag must create an action");
        assert!(
            matches!(&action,Action::OnPage(2,next) if matches!(**next,Action::Feature(FeatureAction::AddField(_, _))))
        );
        app.execute(action, &ctx);
        assert!(app.error.is_none(), "{:?}", app.error);
        let fields = app.current().unwrap().pdf.fields();
        assert_eq!(fields.len(), 1);
        let pdf = &app.current().unwrap().pdf;
        assert_eq!(
            pdf.doc
                .get_dictionary(fields[0].id)
                .unwrap()
                .get(b"P")
                .unwrap()
                .as_reference()
                .unwrap(),
            pdf.page_id(2).unwrap()
        );
    }
    #[test]
    fn continuous_view_navigation_requests_the_target_page() {
        let ctx = egui::Context::default();
        let mut app = PdfApp::with_context(&ctx, true);
        let mut doc = PdfDocument::from_document(pdf::demo_document().unwrap(), "sample.pdf");
        for _ in 0..4 {
            doc.duplicate_page(1).unwrap();
        }
        app.add_document(doc).unwrap();
        app.sync();
        app.current_mut().unwrap().view = ViewMode::Continuous;
        frame(&mut app, &ctx, vec![]);
        app.execute(Action::Page(5), &ctx);
        app.sync();
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        assert!(app.main_keys.iter().any(|k| k.page == 5));
        assert_eq!(app.current().unwrap().page, 5);
    }
    #[test]
    fn staged_redactions_survive_navigation_and_clear_after_editing() {
        let ctx = egui::Context::default();
        let mut app = PdfApp::with_context(&ctx, true);
        let mut doc = PdfDocument::from_document(pdf::demo_document().unwrap(), "sample.pdf");
        doc.duplicate_page(1).unwrap();
        app.add_document(doc).unwrap();
        app.sync();
        app.execute_studio(studio::StudioAction::StageRedaction([0.1, 0.1], [0.2, 0.2]));
        assert_eq!(app.features.studio.redactions.len(), 1);
        app.execute(Action::Page(2), &ctx);
        app.sync();
        assert_eq!(app.features.studio.redactions.len(), 1);
        app.current_mut().unwrap().pdf.rotate(2, 90).unwrap();
        app.sync();
        assert!(app.features.studio.redactions.is_empty());
    }
}
