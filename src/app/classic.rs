use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Landing {
    Welcome,
    Home,
    Tools,
}

struct Entry {
    name: &'static str,
    icon: Icon,
    color: Color32,
    panel: Option<Panel>,
}
fn entries() -> [Entry; 12] {
    [
        Entry {
            name: "Export PDF",
            icon: Icon::Export,
            color: Color32::from_rgb(0, 160, 170),
            panel: Some(Panel::Export),
        },
        Entry {
            name: "Create PDF",
            icon: Icon::New,
            color: Color32::from_rgb(240, 66, 82),
            panel: None,
        },
        Entry {
            name: "Edit PDF",
            icon: Icon::Text,
            color: Color32::from_rgb(224, 47, 145),
            panel: Some(Panel::Tools),
        },
        Entry {
            name: "Comment",
            icon: Icon::Comment,
            color: Color32::from_rgb(208, 171, 17),
            panel: Some(Panel::Comments),
        },
        Entry {
            name: "Combine Files",
            icon: Icon::Merge,
            color: Color32::from_rgb(113, 86, 235),
            panel: None,
        },
        Entry {
            name: "Organize Pages",
            icon: Icon::Pages,
            color: Color32::from_rgb(107, 178, 41),
            panel: Some(Panel::Pages),
        },
        Entry {
            name: "Redact",
            icon: Icon::Redact,
            color: Color32::from_rgb(222, 54, 139),
            panel: Some(Panel::Tools),
        },
        Entry {
            name: "Prepare Form",
            icon: Icon::Form,
            color: Color32::from_rgb(125, 82, 184),
            panel: Some(Panel::Forms),
        },
        Entry {
            name: "Bookmarks & Files",
            icon: Icon::Files,
            color: Color32::from_rgb(53, 135, 167),
            panel: Some(Panel::Files),
        },
        Entry {
            name: "Document Text",
            icon: Icon::Text,
            color: Color32::from_rgb(81, 129, 163),
            panel: Some(Panel::Text),
        },
        Entry {
            name: "Properties",
            icon: Icon::Info,
            color: Color32::from_rgb(124, 135, 148),
            panel: Some(Panel::Properties),
        },
        Entry {
            name: "Actions & Sharing",
            icon: Icon::Batch,
            color: Color32::from_rgb(43, 127, 237),
            panel: Some(Panel::Studio),
        },
    ]
}

fn row(ui: &mut egui::Ui, entry: &Entry, selected: bool) -> egui::Response {
    let p = palette(ui.ctx());
    let response = ui.add(
        egui::Button::new("")
            .min_size(Vec2::new(ui.available_width(), 43.))
            .frame(false),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), entry.name)
    });
    if selected || response.hovered() {
        ui.painter().rect_filled(
            response.rect,
            0.,
            if selected { p.selected } else { p.surface },
        );
    }
    let center = response.rect.center();
    icons::paint(
        ui.painter(),
        entry.icon,
        egui::Rect::from_center_size(
            egui::pos2(response.rect.left() + 25., center.y),
            Vec2::splat(23.),
        ),
        entry.color,
    );
    ui.painter().text(
        egui::pos2(response.rect.left() + 47., center.y),
        egui::Align2::LEFT_CENTER,
        entry.name,
        FontId::proportional(14.),
        p.text,
    );
    let x = response.rect.right() - 20.;
    let direction = if selected { -1. } else { 1. };
    ui.painter().line_segment(
        [
            egui::pos2(x - 4., center.y - 2. * direction),
            egui::pos2(x, center.y + 2. * direction),
        ],
        Stroke::new(1.5, p.muted),
    );
    ui.painter().line_segment(
        [
            egui::pos2(x, center.y + 2. * direction),
            egui::pos2(x + 4., center.y - 2. * direction),
        ],
        Stroke::new(1.5, p.muted),
    );
    response.on_hover_text(entry.name)
}

impl PdfApp {
    pub(super) fn header(&mut self, ctx: &egui::Context, action: &mut Option<Action>) {
        let p = palette(ctx);
        #[cfg(not(target_os = "macos"))]
        egui::TopBottomPanel::top("menu_bar")
            .frame(
                egui::Frame::new()
                    .fill(p.panel)
                    .inner_margin(egui::Margin::symmetric(7, 1)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().button_padding = Vec2::new(5., 2.);
                    ui.menu_button("File", |ui| {
                        for (label, enabled, command) in [
                            ("Open…        Cmd/Ctrl+O", true, Action::Open),
                            ("Create PDF…", true, Action::New),
                            ("PDF from images…", true, Action::Images),
                            (
                                "Save        Cmd/Ctrl+S",
                                self.active.is_some(),
                                Action::Save(false),
                            ),
                            ("Save a copy…", self.active.is_some(), Action::Save(true)),
                            ("Print…", self.active.is_some(), Action::Print),
                            ("Quit", true, Action::Quit),
                        ] {
                            if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                                *action = Some(command);
                                ui.close();
                            }
                        }
                    });
                    ui.menu_button("Edit", |ui| {
                        if ui
                            .add_enabled(
                                self.current().is_some_and(|s| s.pdf.can_undo()),
                                egui::Button::new("Undo        Cmd/Ctrl+Z"),
                            )
                            .clicked()
                        {
                            *action = Some(Action::Undo);
                            ui.close();
                        }
                        if ui
                            .add_enabled(
                                self.current().is_some_and(|s| s.pdf.can_redo()),
                                egui::Button::new("Redo        Cmd/Ctrl+Shift+Z"),
                            )
                            .clicked()
                        {
                            *action = Some(Action::Redo);
                            ui.close();
                        }
                        if ui
                            .add_enabled(
                                self.active.is_some(),
                                egui::Button::new("Find…        Cmd/Ctrl+F"),
                            )
                            .clicked()
                        {
                            self.search_open = true;
                            ctx.memory_mut(|m| m.request_focus(egui::Id::new("find_input")));
                            ui.close();
                        }
                    });
                    ui.menu_button("View", |ui| {
                        ui.checkbox(&mut self.thumbnails_open, "Page thumbnails");
                        ui.checkbox(&mut self.tools_open, "Tools pane");
                        if ui.checkbox(&mut self.dark_mode, "Dark mode").changed() {
                            theme::apply(ctx, self.dark_mode);
                        }
                        if ui
                            .add(
                                IconButton::new(
                                    Icon::Fullscreen,
                                    if self.fullscreen {
                                        "Exit fullscreen"
                                    } else {
                                        "Fullscreen"
                                    },
                                )
                                .quiet(),
                            )
                            .clicked()
                        {
                            self.fullscreen = !self.fullscreen;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(
                                self.fullscreen,
                            ));
                            ui.close();
                        }
                    });
                    ui.menu_button("Window", |ui| {
                        for (index, session) in self.sessions.iter().enumerate() {
                            if ui
                                .selectable_label(self.active == Some(index), &session.pdf.title)
                                .clicked()
                            {
                                *action = Some(Action::Tab(index));
                                ui.close();
                            }
                        }
                        if self.sessions.is_empty() {
                            ui.label("No documents open");
                        }
                    });
                    ui.menu_button("Help", |ui| {
                        if ui.button("Shortcuts and capabilities").clicked() {
                            self.modal = Some(Modal::Help);
                            ui.close();
                        }
                        if ui.button("Open sample document").clicked() {
                            *action = Some(Action::Sample);
                            ui.close();
                        }
                    });
                });
            });
        egui::TopBottomPanel::top("document_tabs")
            .exact_height(37.)
            .frame(egui::Frame::new().fill(p.background).inner_margin(0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.;
                    for (label, landing) in [("Home", Landing::Home), ("Tools", Landing::Tools)] {
                        let selected = self.active.is_none() && self.landing == landing;
                        if ui
                            .add_sized(
                                [72., 36.],
                                egui::Button::new(RichText::new(label).size(15.))
                                    .fill(if selected { p.panel } else { p.background })
                                    .stroke(Stroke::NONE)
                                    .corner_radius(0),
                            )
                            .clicked()
                        {
                            self.active = None;
                            self.landing = landing;
                        }
                    }
                    ui.separator();
                    let right_space = 76.;
                    egui::ScrollArea::horizontal()
                        .max_width((ui.available_width() - right_space).max(100.))
                        .id_salt("tabs_scroll")
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.;
                                let welcome =
                                    self.active.is_none() && self.landing == Landing::Welcome;
                                if ui
                                    .add_sized(
                                        [152., 36.],
                                        egui::Button::new("Welcome")
                                            .fill(if welcome { p.panel } else { p.background })
                                            .stroke(Stroke::NONE)
                                            .corner_radius(0),
                                    )
                                    .clicked()
                                {
                                    self.active = None;
                                    self.landing = Landing::Welcome;
                                }
                                for (index, session) in self.sessions.iter().enumerate() {
                                    let selected = self.active == Some(index);
                                    egui::Frame::new()
                                        .fill(if selected { p.panel } else { p.background })
                                        .inner_margin(egui::Margin::symmetric(6, 2))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                let name = format!(
                                                    "{}{}",
                                                    session.pdf.title,
                                                    if session.pdf.dirty { " *" } else { "" }
                                                );
                                                let short = if name.chars().count() > 25 {
                                                    format!(
                                                        "{}…",
                                                        name.chars().take(24).collect::<String>()
                                                    )
                                                } else {
                                                    name.clone()
                                                };
                                                if ui
                                                    .add_sized(
                                                        [145., 30.],
                                                        egui::Button::new(short).frame(false),
                                                    )
                                                    .on_hover_text(name)
                                                    .clicked()
                                                {
                                                    *action = Some(Action::Tab(index));
                                                }
                                                if ui
                                                    .add(
                                                        IconButton::new(
                                                            Icon::Close,
                                                            "Close document",
                                                        )
                                                        .quiet()
                                                        .size(24.),
                                                    )
                                                    .clicked()
                                                {
                                                    *action = Some(Action::Close(index));
                                                }
                                            });
                                        });
                                }
                            });
                        });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                IconButton::new(
                                    if self.dark_mode {
                                        Icon::Sun
                                    } else {
                                        Icon::Moon
                                    },
                                    if self.dark_mode {
                                        "Light mode"
                                    } else {
                                        "Dark mode"
                                    },
                                )
                                .quiet()
                                .size(32.),
                            )
                            .clicked()
                        {
                            self.dark_mode = !self.dark_mode;
                            theme::apply(ctx, self.dark_mode);
                        }
                        if ui
                            .add(IconButton::new(Icon::Info, "Help").quiet().size(32.))
                            .clicked()
                        {
                            self.modal = Some(Modal::Help);
                        }
                    });
                });
            });
    }

    pub(super) fn toolbar(&mut self, ctx: &egui::Context, action: &mut Option<Action>) {
        let enabled = self.active.is_some();
        let (page, count, zoom) = self
            .current()
            .map(|s| (s.page, s.pdf.page_count(), s.zoom))
            .unwrap_or((0, 0, 1.));
        egui::TopBottomPanel::top("document_toolbar")
            .exact_height(46.)
            .frame(
                egui::Frame::new()
                    .fill(palette(ctx).panel)
                    .inner_margin(egui::Margin::symmetric(8, 5)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 7.;
                    for (icon, label, available, command) in [
                        (
                            Icon::Save,
                            "Save PDF (Cmd/Ctrl+S)",
                            enabled,
                            Action::Save(false),
                        ),
                        (Icon::Open, "Open document (Cmd/Ctrl+O)", true, Action::Open),
                        (Icon::Print, "Print", enabled, Action::Print),
                    ] {
                        if ui
                            .add_enabled(available, IconButton::new(icon, label).quiet().size(31.))
                            .clicked()
                        {
                            *action = Some(command);
                        }
                    }
                    if ui
                        .add_enabled(
                            enabled,
                            IconButton::new(Icon::Search, "Find text (Cmd/Ctrl+F)")
                                .quiet()
                                .size(31.),
                        )
                        .clicked()
                    {
                        self.search_open = !self.search_open;
                        if self.search_open {
                            ctx.memory_mut(|m| m.request_focus(egui::Id::new("find_input")));
                        }
                    }
                    if ui
                        .add_enabled(
                            enabled && page > 1,
                            IconButton::new(Icon::Left, "Previous page")
                                .quiet()
                                .size(31.),
                        )
                        .clicked()
                    {
                        *action = Some(Action::Page(page - 1));
                    }
                    if ui
                        .add_enabled(
                            enabled && page < count,
                            IconButton::new(Icon::Right, "Next page").quiet().size(31.),
                        )
                        .clicked()
                    {
                        *action = Some(Action::Page(page + 1));
                    }
                    ui.add_enabled_ui(enabled, |ui| {
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut self.page_input)
                                .desired_width(28.)
                                .horizontal_align(egui::Align::Center)
                                .frame(false),
                        );
                        if response.lost_focus()
                            && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            && let Ok(n) = self.page_input.parse()
                        {
                            *action = Some(Action::Page(n));
                        }
                        ui.label(format!(
                            "/ {}",
                            if enabled {
                                count.to_string()
                            } else {
                                "—".into()
                            }
                        ));
                    });
                    ui.separator();
                    if ui
                        .add_enabled(
                            enabled,
                            IconButton::new(Icon::Pointer, "Read / select")
                                .selected(self.tool == Tool::Select)
                                .quiet()
                                .size(31.),
                        )
                        .clicked()
                    {
                        self.tool = Tool::Select;
                    }
                    ui.add_enabled_ui(enabled, |ui| {
                        egui::ComboBox::from_id_salt("zoom_menu")
                            .width(70.)
                            .selected_text(format!("{:.1}%", zoom * 100.))
                            .show_ui(ui, |ui| {
                                if wide_button(ui, Icon::FitWidth, "Fit width").clicked() {
                                    *action = Some(Action::Fit);
                                    ui.close();
                                }
                                if wide_button(ui, Icon::FitPage, "Fit page").clicked() {
                                    *action = Some(Action::FitPage);
                                    ui.close();
                                }
                                for percent in [50, 75, 100, 125, 150, 200, 300, 400] {
                                    if ui.button(format!("{percent}%")).clicked() {
                                        *action = Some(Action::Zoom(percent as f32 / 100.));
                                        ui.close();
                                    }
                                }
                            });
                    });
                    ui.menu_button("•••", |ui| {
                        ui.add_enabled_ui(enabled, |ui| {
                            for (icon, name, command) in [
                                (Icon::ZoomIn, "Zoom in", Action::Zoom(zoom * 1.2)),
                                (Icon::ZoomOut, "Zoom out", Action::Zoom(zoom / 1.2)),
                                (Icon::RotateRight, "Rotate clockwise", Action::Rotate(90)),
                                (Icon::Undo, "Undo", Action::Undo),
                                (Icon::Redo, "Redo", Action::Redo),
                            ] {
                                if wide_button(ui, icon, name).clicked() {
                                    *action = Some(command);
                                    ui.close();
                                }
                            }
                            for (mode, icon, name) in [
                                (ViewMode::Single, Icon::Single, "Single page"),
                                (ViewMode::Continuous, Icon::Continuous, "Continuous"),
                                (ViewMode::TwoPage, Icon::TwoPages, "Two pages"),
                            ] {
                                if ui
                                    .add(
                                        IconButton::new(icon, name)
                                            .selected(
                                                self.current().is_some_and(|s| s.view == mode),
                                            )
                                            .quiet(),
                                    )
                                    .clicked()
                                {
                                    if let Some(s) = self.current_mut() {
                                        s.view = mode;
                                    }
                                    self.jump_page = true;
                                    ui.close();
                                }
                            }
                        });
                        ui.separator();
                        ui.checkbox(&mut self.tools_open, "Tools pane");
                        ui.checkbox(&mut self.thumbnails_open, "Page thumbnails");
                        if wide_button(
                            ui,
                            Icon::Fullscreen,
                            if self.fullscreen {
                                "Exit fullscreen"
                            } else {
                                "Fullscreen"
                            },
                        )
                        .clicked()
                        {
                            self.fullscreen = !self.fullscreen;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(
                                self.fullscreen,
                            ));
                            ui.close();
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let share = ui
                            .add_enabled(
                                enabled,
                                egui::Button::new("")
                                    .fill(PRIMARY)
                                    .stroke(Stroke::NONE)
                                    .corner_radius(18)
                                    .min_size(Vec2::new(99., 32.)),
                            )
                            .on_hover_text("Open independent review and signing tools");
                        share.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Share")
                        });
                        let color = Color32::WHITE.gamma_multiply(if enabled { 1. } else { 0.45 });
                        icons::paint(
                            ui.painter(),
                            Icon::Export,
                            egui::Rect::from_center_size(
                                egui::pos2(share.rect.left() + 20., share.rect.center().y),
                                Vec2::splat(18.),
                            ),
                            color,
                        );
                        ui.painter().text(
                            egui::pos2(share.rect.left() + 38., share.rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            "Share",
                            FontId::proportional(14.),
                            color,
                        );
                        if share.clicked() {
                            self.panel = Panel::Studio;
                            self.catalog_selection = 11;
                            self.catalog_jump = true;
                            self.inspector_open = true;
                            self.tools_open = true;
                        }
                    });
                });
            });
        if self.search_open && enabled {
            egui::TopBottomPanel::top("search_bar")
                .frame(egui::Frame::new().fill(palette(ctx).panel).inner_margin(6))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Find:");
                        let input = ui.add(
                            egui::TextEdit::singleline(&mut self.find)
                                .id(egui::Id::new("find_input"))
                                .hint_text("Search document")
                                .desired_width(220.),
                        );
                        if (input.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                            || ui.button("Search").clicked()
                        {
                            *action = Some(Action::Search);
                            self.inspector_open = true;
                            self.tools_open = true;
                        }
                        if ui
                            .add(
                                IconButton::new(Icon::Close, "Close search")
                                    .quiet()
                                    .size(24.),
                            )
                            .clicked()
                        {
                            self.search_open = false;
                        }
                    });
                });
        }
    }

    pub(super) fn navigation_rail(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("navigation_rail")
            .exact_width(44.)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(palette(ctx).panel)
                    .inner_margin(egui::Margin::symmetric(5, 13)),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    if ui
                        .add_enabled(
                            self.active.is_some(),
                            IconButton::new(Icon::Pages, "Page thumbnails")
                                .selected(self.thumbnails_open)
                                .quiet()
                                .size(33.),
                        )
                        .clicked()
                    {
                        self.thumbnails_open = !self.thumbnails_open;
                    }
                    ui.add_space(9.);
                    for (icon, name) in
                        [(Icon::Bookmark, "Bookmarks"), (Icon::Files, "Attachments")]
                    {
                        if ui
                            .add_enabled(
                                self.active.is_some(),
                                IconButton::new(icon, name).quiet().size(33.),
                            )
                            .clicked()
                        {
                            self.panel = Panel::Files;
                            self.catalog_selection = 8;
                            self.catalog_jump = true;
                            self.inspector_open = true;
                            self.tools_open = true;
                        }
                        ui.add_space(9.);
                    }
                });
            });
    }

    fn choose_entry(&mut self, index: usize, action: &mut Option<Action>) {
        match index {
            1 => *action = Some(Action::New),
            4 => {
                *action = Some(if self.active.is_some() {
                    Action::Merge
                } else {
                    Action::Open
                })
            }
            _ => {
                if let Some(panel) = entries()[index].panel {
                    self.inspector_open = !(self.inspector_open && self.catalog_selection == index);
                    self.panel = panel;
                    self.catalog_selection = index;
                    self.catalog_jump = self.inspector_open;
                    self.tools_open = true;
                    if index == 6 {
                        self.tool = Tool::Redact;
                    }
                }
            }
        }
    }

    pub(super) fn sidebar(&mut self, ctx: &egui::Context, action: &mut Option<Action>) {
        if !self.tools_open {
            return;
        }
        if self.inspector_open
            && entries().get(self.catalog_selection).and_then(|e| e.panel) != Some(self.panel)
        {
            self.catalog_selection = if self.panel == Panel::Tools && self.tool == Tool::Redact {
                6
            } else {
                entries()
                    .iter()
                    .position(|e| e.panel == Some(self.panel))
                    .unwrap_or(2)
            };
        }
        egui::SidePanel::right("tools")
            .exact_width(252.)
            .resizable(false)
            .frame(egui::Frame::new().fill(palette(ctx).panel).inner_margin(0))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("tool_catalog")
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.;
                        ui.add_space(13.);
                        for (index, entry) in entries().iter().enumerate() {
                            let selected = self.inspector_open && self.catalog_selection == index;
                            let response = row(ui, entry, selected);
                            if response.clicked() {
                                self.choose_entry(index, action);
                            }
                            if self.inspector_open && self.catalog_selection == index {
                                if self.catalog_jump {
                                    response.scroll_to_me(Some(egui::Align::Min));
                                    self.catalog_jump = false;
                                }
                                egui::Frame::new()
                                    .fill(palette(ctx).surface)
                                    .inner_margin(12)
                                    .show(ui, |ui| {
                                        ui.spacing_mut().item_spacing = Vec2::new(6., 7.);
                                        if self.active.is_some() {
                                            self.inspector_contents(ui, action);
                                        } else {
                                            ui.label("Open a document to use this tool.");
                                            if ui.button("Open a PDF").clicked() {
                                                *action = Some(Action::Open);
                                            }
                                        }
                                    });
                            }
                        }
                        ui.add_space(12.);
                        ui.separator();
                        egui::Frame::new().inner_margin(18).show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    RichText::new("Make more of your PDFs").strong().size(13.),
                                );
                                ui.add_space(4.);
                                ui.label(
                                    RichText::new("Read, edit, and review with PDFPilot")
                                        .size(12.)
                                        .color(palette(ctx).muted),
                                );
                                ui.add_space(9.);
                                if ui.link("Explore an editable sample").clicked() {
                                    *action = Some(Action::Sample);
                                }
                            });
                        });
                    });
            });
    }

    pub(super) fn home(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        match self.landing {
            Landing::Tools => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::Frame::new().inner_margin(28).show(ui, |ui| {
                        ui.heading("Tools");
                        ui.label("Choose a PDF tool to get started.");
                        ui.add_space(20.);
                        for (index, entry) in entries().iter().enumerate() {
                            if row(ui, entry, false).clicked() {
                                self.choose_entry(index, action);
                            }
                        }
                    });
                });
            }
            Landing::Home => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::Frame::new().inner_margin(28).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (_, rect) = ui.allocate_space(Vec2::splat(22.));
                            icons::paint(ui.painter(), Icon::Home, rect, palette(ui.ctx()).muted);
                            ui.heading("Home");
                        });
                        ui.add_space(15.);
                        if primary(ui, Icon::Open, "Open a document").clicked() {
                            *action = Some(Action::Open);
                        }
                        ui.add_space(24.);
                        ui.label(RichText::new("Recent documents").strong().size(17.));
                        ui.add_space(10.);
                        if self.recent.is_empty() {
                            ui.label("Documents you open will appear here.");
                        }
                        for path in &self.recent {
                            if ui
                                .link(path.file_name().unwrap_or_default().to_string_lossy())
                                .clicked()
                            {
                                *action = Some(Action::OpenPath(path.clone()));
                            }
                        }
                        ui.add_space(20.);
                        if ui.link("Welcome to PDFPilot").clicked() {
                            self.landing = Landing::Welcome;
                        }
                    });
                });
            }
            Landing::Welcome => self.welcome_page(ui, action),
        }
    }

    fn welcome_page(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        egui::ScrollArea::vertical().id_salt("welcome_page").show(ui,|ui|{
            let width=(ui.available_width()-32.).clamp(330.,720.);
            ui.add_space(12.);
            ui.horizontal(|ui|{
                ui.add_space(((ui.available_width()-width)/2.).max(0.));
                // Keep the welcome page paper white in both appearance modes.
                egui::Frame::new().fill(Color32::WHITE).stroke(Stroke::new(1.,Color32::from_gray(205))).inner_margin(egui::Margin::symmetric(32,30)).show(ui,|ui|{
                    ui.set_width(width-64.);ui.set_min_height(810.);
                    ui.visuals_mut().override_text_color=Some(Color32::from_gray(105));
                    let blue=Color32::from_rgb(0,142,180);
                    ui.horizontal_wrapped(|ui|{ui.label(RichText::new("02").color(blue).size(30.));ui.label(RichText::new("Prepare polished PDFs").color(Color32::from_gray(98)).size(if width<450. {25.} else {28.}));});
                    ui.add_space(17.);ui.label(RichText::new("Create, combine, and organize").italics().color(blue).size(20.));ui.add_space(15.);
                    let illustration_width=130.;
                    if ui.available_width()>390.{
                        ui.horizontal_top(|ui|{
                            let text_width=(ui.available_width()-illustration_width-18.).max(190.);
                            ui.vertical(|ui|{ui.set_width(text_width);ui.label(RichText::new("It’s easy to create a PDF with exactly the content you want.").size(15.));ui.add_space(15.);ui.label(RichText::new("Make trusted PDFs.").strong().size(14.));ui.label("Turn documents, images, and saved scans into a PDF that you can read, review, and share.");});
                            document_illustration(ui,illustration_width);
                        });
                    }else{ui.label(RichText::new("It’s easy to create a PDF with exactly the content you want.").size(15.));ui.add_space(14.);document_illustration(ui,130.);}
                    ui.add_space(24.);
                    for (heading,body) in [
                        ("Merge multiple files into one PDF.","Bring pages together into a single document. Use Combine Files to add another PDF to the document you’re working on."),
                        ("Organize pages.","Drag thumbnails to reorder pages. Insert, delete, rotate, extract, and crop pages from the Organize Pages tools."),
                        ("Review and share.","Add comments, highlights, and signatures. Use Share to open your independent PDFPilot review and signing tools."),
                    ]{ui.label(RichText::new(heading).strong().size(14.));ui.label(RichText::new(body).size(14.));ui.add_space(17.);}
                    ui.add_space(6.);
                    if ui.add(egui::Button::new(RichText::new("Open a PDF").color(Color32::WHITE)).fill(PRIMARY).corner_radius(2)).clicked(){*action=Some(Action::Open);}
                    ui.add_space(10.);if ui.link(RichText::new("Try an editable sample").color(blue)).clicked(){*action=Some(Action::Sample);}
                    ui.add_space(28.);ui.separator();ui.label(RichText::new("PDFPilot · Your document workspace").size(11.).color(Color32::from_gray(130)));
                });
            });
        });
    }
}

fn document_illustration(ui: &mut egui::Ui, width: f32) {
    let (_, rect) = ui.allocate_space(Vec2::new(width, 155.));
    let painter = ui.painter();
    painter.circle_filled(
        rect.center(),
        width * 0.47,
        Color32::from_rgb(239, 241, 243),
    );
    for (offset, icon, color) in [
        ([-30., -44.], Icon::Text, Color32::from_rgb(96, 164, 222)),
        ([34., -35.], Icon::Image, Color32::from_rgb(173, 142, 196)),
        ([-42., 13.], Icon::Form, Color32::from_rgb(73, 162, 133)),
        ([43., 14.], Icon::Files, Color32::from_rgb(229, 145, 89)),
        ([0., 55.], Icon::New, Color32::from_rgb(237, 61, 74)),
    ] {
        let r = egui::Rect::from_center_size(
            rect.center() + Vec2::new(offset[0], offset[1]),
            Vec2::new(30., 36.),
        );
        painter.rect_filled(r, 0., Color32::WHITE);
        painter.rect_stroke(
            r,
            0.,
            Stroke::new(1., Color32::from_gray(188)),
            egui::StrokeKind::Inside,
        );
        icons::paint(painter, icon, r.shrink(4.), color);
    }
    icons::paint(
        painter,
        Icon::Arrow,
        egui::Rect::from_center_size(rect.center() + Vec2::new(0., 12.), Vec2::splat(28.)),
        Color32::from_gray(168),
    );
}
