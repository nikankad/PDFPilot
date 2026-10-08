use eframe::egui::{self, Color32, FontId, Stroke};

#[derive(Clone, Copy)]
pub struct Palette {
    pub background: Color32,
    pub panel: Color32,
    pub surface: Color32,
    pub canvas: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub line: Color32,
    pub accent: Color32,
    pub selected: Color32,
    pub error: Color32,
}

pub fn palette(ctx: &egui::Context) -> Palette {
    if ctx.style().visuals.dark_mode {
        Palette {
            background: Color32::from_rgb(39, 41, 44),
            panel: Color32::from_rgb(31, 33, 36),
            surface: Color32::from_rgb(48, 51, 55),
            canvas: Color32::from_rgb(24, 26, 29),
            text: Color32::from_rgb(232, 237, 247),
            muted: Color32::from_rgb(153, 167, 189),
            line: Color32::from_rgb(65, 68, 72),
            accent: Color32::from_rgb(92, 172, 255),
            selected: Color32::from_rgb(43, 61, 80),
            error: Color32::from_rgb(255, 151, 151),
        }
    } else {
        Palette {
            background: Color32::from_rgb(235, 235, 235),
            panel: Color32::from_rgb(250, 250, 250),
            surface: Color32::from_rgb(242, 242, 242),
            canvas: Color32::from_rgb(232, 232, 232),
            text: Color32::from_rgb(62, 62, 62),
            muted: Color32::from_rgb(112, 112, 112),
            line: Color32::from_rgb(205, 205, 205),
            accent: Color32::from_rgb(20, 119, 208),
            selected: Color32::from_rgb(226, 239, 252),
            error: Color32::from_rgb(176, 42, 42),
        }
    }
}

pub const PRIMARY: Color32 = Color32::from_rgb(43, 127, 237);

pub fn apply(ctx: &egui::Context, dark: bool) {
    ctx.set_visuals(if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    });
    let p = palette(ctx);
    let mut style = (*ctx.style()).clone();
    style.visuals.override_text_color = None;
    style.visuals.panel_fill = p.panel;
    style.visuals.window_fill = p.panel;
    style.visuals.extreme_bg_color = p.background;
    style.visuals.faint_bg_color = p.surface;
    style.visuals.window_stroke = Stroke::new(1., p.line);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1., p.text);
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1., p.text);
    style.visuals.widgets.hovered.fg_stroke = Stroke::new(1., p.text);
    style.visuals.widgets.active.fg_stroke = Stroke::new(1., p.text);
    style.visuals.widgets.inactive.bg_fill = p.surface;
    style.visuals.widgets.inactive.weak_bg_fill = p.surface;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1., p.line);
    style.visuals.widgets.hovered.bg_fill = p.selected;
    style.visuals.widgets.hovered.weak_bg_fill = p.selected;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1., p.accent);
    style.visuals.selection.bg_fill = PRIMARY;
    style.visuals.selection.stroke = Stroke::new(1., Color32::WHITE);
    style.visuals.hyperlink_color = p.accent;
    style.spacing.item_spacing = egui::vec2(6., 6.);
    style.spacing.button_padding = egui::vec2(9., 5.);
    style.visuals.widgets.inactive.corner_radius = 2.into();
    style.visuals.widgets.hovered.corner_radius = 2.into();
    style.visuals.widgets.active.corner_radius = 2.into();
    for (kind, size) in [
        (egui::TextStyle::Body, 13.),
        (egui::TextStyle::Button, 13.),
        (egui::TextStyle::Heading, 24.),
    ] {
        style.text_styles.insert(kind, FontId::proportional(size));
    }
    ctx.set_style(style);
}
