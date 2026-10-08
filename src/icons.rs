//! Scalable native vector icons, with named, keyboard-accessible button controls.
use crate::theme::{PRIMARY, palette};
use eframe::egui::{self, Color32, Pos2, Rect, Response, Stroke, Vec2, Widget};

#[derive(Clone, Copy)]
pub enum Icon {
    Pointer,
    Highlight,
    Underline,
    Strikeout,
    Text,
    Pen,
    Comment,
    Rectangle,
    Circle,
    Line,
    Arrow,
    Stamp,
    Ruler,
    Crop,
    Link,
    Image,
    Form,
    Pages,
    Export,
    Files,
    Info,
    Open,
    Save,
    New,
    Home,
    Close,
    Undo,
    Redo,
    Left,
    Right,
    ZoomIn,
    ZoomOut,
    FitWidth,
    FitPage,
    Single,
    Continuous,
    TwoPages,
    Sun,
    Moon,
    Fullscreen,
    RotateLeft,
    RotateRight,
    Duplicate,
    Merge,
    Split,
    Delete,
    Replace,
    Bookmark,
    Lock,
    Scan,
    Search,
    Redact,
    Compress,
    Compare,
    Print,
    Volume,
    Stop,
    Check,
    Reset,
    Labels,
    Batch,
    Header,
    Footer,
    Watermark,
}

pub struct IconButton<'a> {
    icon: Icon,
    label: &'a str,
    selected: bool,
    selectable: bool,
    size: Vec2,
    quiet: bool,
}
impl<'a> IconButton<'a> {
    pub fn new(icon: Icon, label: &'a str) -> Self {
        Self {
            icon,
            label,
            selected: false,
            selectable: false,
            size: Vec2::splat(34.),
            quiet: false,
        }
    }
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self.selectable = true;
        self
    }
    pub fn size(mut self, size: f32) -> Self {
        self.size = Vec2::splat(size);
        self
    }
    pub fn quiet(mut self) -> Self {
        self.quiet = true;
        self
    }
}
impl Widget for IconButton<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let mut button = egui::Button::new("")
            .min_size(self.size)
            .selected(self.selected && !self.quiet)
            .corner_radius(2);
        if self.quiet {
            button = button.frame(false);
        }
        let response = ui.add(button);
        response.widget_info(|| {
            if self.selectable {
                egui::WidgetInfo::selected(
                    egui::WidgetType::Button,
                    ui.is_enabled(),
                    self.selected,
                    self.label,
                )
            } else {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), self.label)
            }
        });
        if ui.is_rect_visible(response.rect) {
            let p = palette(ui.ctx());
            let color = if self.selected && !self.quiet {
                Color32::WHITE
            } else if self.selected {
                p.accent
            } else if self.quiet {
                p.muted
            } else {
                p.text
            };
            let color = if ui.is_enabled() {
                color
            } else {
                color.gamma_multiply(0.35)
            };
            paint(
                ui.painter(),
                self.icon,
                Rect::from_center_size(
                    response.rect.center(),
                    Vec2::splat(if self.size.x >= 44. { 23. } else { 20. }),
                ),
                color,
            );
        }
        response.on_hover_text(self.label)
    }
}

/// Action tiles use a large icon and a short caption; the full operation name
/// remains available to assistive technology and in the hover tooltip.
pub fn tile(ui: &mut egui::Ui, icon: Icon, label: &str, caption: &str, enabled: bool) -> Response {
    let width = 66.;
    let response = ui.add_enabled(
        enabled,
        egui::Button::new("")
            .min_size(egui::vec2(width, 68.))
            .corner_radius(8),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled && ui.is_enabled(), label)
    });
    let color = palette(ui.ctx())
        .text
        .gamma_multiply(if enabled { 1. } else { 0.35 });
    paint(
        ui.painter(),
        icon,
        Rect::from_center_size(
            response.rect.center() - egui::vec2(0., 9.),
            Vec2::splat(23.),
        ),
        color,
    );
    ui.painter().text(
        response.rect.center() + egui::vec2(0., 20.),
        egui::Align2::CENTER_CENTER,
        caption,
        egui::FontId::proportional(11.),
        color,
    );
    response.on_hover_text(label)
}

/// A labeled action keeps settings clear while adding a consistent visual cue.
pub fn action(ui: &mut egui::Ui, icon: Icon, label: &str, primary: bool) -> Response {
    let color = if primary {
        Color32::WHITE
    } else {
        palette(ui.ctx()).text
    };
    let color = color.gamma_multiply(if ui.is_enabled() { 1. } else { 0.35 });
    let width = ui.available_width().clamp(130., 350.);
    let galley = ui.painter().layout(
        label.to_owned(),
        egui::FontId::proportional(12.5),
        color,
        width - 46.,
    );
    let mut button = egui::Button::new("")
        .min_size(egui::vec2(width, (galley.size().y + 16.).max(36.)))
        .corner_radius(7);
    if primary {
        button = button.fill(PRIMARY);
    }
    let response = ui.add(button);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    paint(
        ui.painter(),
        icon,
        Rect::from_center_size(
            egui::pos2(response.rect.left() + 19., response.rect.center().y),
            Vec2::splat(19.),
        ),
        color,
    );
    ui.painter().galley(
        egui::pos2(
            response.rect.left() + 36.,
            response.rect.center().y - galley.size().y / 2.,
        ),
        galley,
        color,
    );
    response.on_hover_text(label)
}

pub fn paint(painter: &egui::Painter, icon: Icon, rect: Rect, color: Color32) {
    let scale = rect.width() / 24.;
    let point = |x: f32, y: f32| Pos2::new(rect.left() + x * scale, rect.top() + y * scale);
    let stroke = Stroke::new(1.65 * scale, color);
    let path = |points: &[[f32; 2]]| {
        painter.add(egui::Shape::line(
            points.iter().map(|p| point(p[0], p[1])).collect(),
            stroke,
        ));
    };
    let line = |a: [f32; 2], b: [f32; 2]| {
        painter.line_segment([point(a[0], a[1]), point(b[0], b[1])], stroke);
    };
    let box_ = |x: f32, y: f32, w: f32, h: f32| {
        painter.rect_stroke(
            Rect::from_min_size(point(x, y), egui::vec2(w * scale, h * scale)),
            1.5 * scale,
            stroke,
            egui::StrokeKind::Middle,
        );
    };
    let circle = |x: f32, y: f32, r: f32| {
        painter.circle_stroke(point(x, y), r * scale, stroke);
    };
    let page = || {
        path(&[
            [6., 2.],
            [15., 2.],
            [20., 7.],
            [20., 22.],
            [6., 22.],
            [6., 2.],
        ]);
        path(&[[15., 2.], [15., 7.], [20., 7.]]);
    };
    let arrow = |left: bool| {
        let dir = if left { -1. } else { 1. };
        path(&[
            [12. - dir * 4., 7.],
            [12. + dir * 4., 12.],
            [12. - dir * 4., 17.],
        ]);
    };
    match icon {
        Icon::Pointer => path(&[
            [5., 3.],
            [6., 19.],
            [10., 15.],
            [14., 21.],
            [17., 19.],
            [13., 13.],
            [19., 12.],
            [5., 3.],
        ]),
        Icon::Highlight => {
            path(&[
                [7., 15.],
                [5., 13.],
                [14., 3.],
                [20., 9.],
                [10., 18.],
                [7., 15.],
            ]);
            path(&[[7., 15.], [3., 20.], [8., 20.], [10., 18.]]);
            line([2., 23.], [21., 23.]);
        }
        Icon::Underline => {
            path(&[
                [6., 3.],
                [6., 12.],
                [8., 16.],
                [12., 18.],
                [16., 16.],
                [18., 12.],
                [18., 3.],
            ]);
            line([4., 22.], [20., 22.]);
        }
        Icon::Strikeout => {
            path(&[
                [18., 4.],
                [14., 2.],
                [8., 3.],
                [6., 6.],
                [9., 10.],
                [15., 14.],
                [18., 17.],
                [16., 21.],
                [10., 22.],
                [6., 20.],
            ]);
            line([3., 12.], [21., 12.]);
        }
        Icon::Text => {
            line([4., 4.], [20., 4.]);
            line([12., 4.], [12., 21.]);
            line([8., 21.], [16., 21.]);
            line([4., 4.], [4., 7.]);
            line([20., 4.], [20., 7.]);
        }
        Icon::Pen => {
            path(&[
                [4., 15.],
                [15., 4.],
                [20., 9.],
                [9., 20.],
                [3., 21.],
                [4., 15.],
            ]);
            line([12., 7.], [17., 12.]);
            line([4., 15.], [9., 20.]);
        }
        Icon::Comment => {
            path(&[
                [3., 3.],
                [21., 3.],
                [21., 17.],
                [11., 17.],
                [5., 22.],
                [5., 17.],
                [3., 17.],
                [3., 3.],
            ]);
            line([7., 8.], [17., 8.]);
            line([7., 12.], [14., 12.]);
        }
        Icon::Rectangle => box_(3., 5., 18., 14.),
        Icon::Circle => circle(12., 12., 9.),
        Icon::Line => {
            line([4., 20.], [20., 4.]);
            circle(4., 20., 1.5);
            circle(20., 4., 1.5);
        }
        Icon::Arrow => {
            line([4., 20.], [20., 4.]);
            path(&[[10., 4.], [20., 4.], [20., 14.]]);
        }
        Icon::Stamp => {
            path(&[
                [3., 20.],
                [3., 16.],
                [8., 14.],
                [9., 10.],
                [8., 6.],
                [10., 3.],
                [14., 3.],
                [16., 6.],
                [15., 10.],
                [16., 14.],
                [21., 16.],
                [21., 20.],
                [3., 20.],
            ]);
            line([3., 23.], [21., 23.]);
        }
        Icon::Ruler => {
            path(&[[2., 16.], [16., 2.], [22., 8.], [8., 22.], [2., 16.]]);
            for i in [6., 10., 14.] {
                line([i, 18. - i], [i + 3., 21. - i]);
            }
        }
        Icon::Crop => {
            path(&[[6., 2.], [6., 18.], [22., 18.]]);
            path(&[[2., 6.], [18., 6.], [18., 22.]]);
            line([9., 15.], [21., 3.]);
        }
        Icon::Link => {
            path(&[
                [10., 8.],
                [13., 5.],
                [16., 3.],
                [20., 4.],
                [22., 7.],
                [21., 10.],
                [17., 14.],
                [14., 14.],
            ]);
            path(&[
                [10., 10.],
                [7., 10.],
                [3., 14.],
                [2., 17.],
                [4., 20.],
                [8., 21.],
                [11., 19.],
                [14., 16.],
            ]);
            line([8., 16.], [16., 8.]);
        }
        Icon::Image => {
            box_(2., 3., 20., 18.);
            circle(7., 8., 2.);
            path(&[[3., 19.], [9., 13.], [13., 17.], [17., 10.], [22., 16.]]);
        }
        Icon::Form => {
            box_(3., 3., 18., 18.);
            box_(6., 6., 3., 3.);
            box_(6., 14., 3., 3.);
            line([12., 7.5], [18., 7.5]);
            line([12., 15.5], [18., 15.5]);
        }
        Icon::Pages | Icon::Duplicate => {
            box_(7., 6., 14., 16.);
            path(&[[4., 18.], [3., 18.], [3., 2.], [17., 2.], [17., 3.]]);
        }
        Icon::Export => {
            path(&[[13., 3.], [4., 3.], [4., 21.], [21., 21.], [21., 12.]]);
            path(&[[13., 2.], [22., 2.], [22., 11.]]);
            line([10., 14.], [22., 2.]);
        }
        Icon::Files => {
            path(&[
                [8., 16.],
                [8., 5.],
                [10., 2.],
                [14., 2.],
                [17., 5.],
                [17., 18.],
                [14., 22.],
                [8., 22.],
                [5., 18.],
                [5., 7.],
            ]);
            path(&[[11., 7.], [11., 17.], [13., 18.], [14., 17.], [14., 5.]]);
        }
        Icon::Info => {
            circle(12., 12., 10.);
            circle(12., 6., 0.7);
            line([12., 10.], [12., 18.]);
            line([10., 18.], [14., 18.]);
        }
        Icon::Open => {
            path(&[
                [3., 19.],
                [3., 4.],
                [10., 4.],
                [13., 7.],
                [20., 7.],
                [20., 10.],
            ]);
            path(&[[3., 20.], [7., 10.], [23., 10.], [19., 20.], [3., 20.]]);
        }
        Icon::Save => {
            path(&[
                [3., 3.],
                [18., 3.],
                [21., 6.],
                [21., 21.],
                [3., 21.],
                [3., 3.],
            ]);
            box_(7., 3., 8., 6.);
            box_(7., 14., 10., 7.);
        }
        Icon::New => {
            page();
            line([9., 14.], [17., 14.]);
            line([13., 10.], [13., 18.]);
        }
        Icon::Home => {
            path(&[[2., 11.], [12., 2.], [22., 11.]]);
            path(&[
                [5., 9.],
                [5., 22.],
                [10., 22.],
                [10., 15.],
                [14., 15.],
                [14., 22.],
                [19., 22.],
                [19., 9.],
            ]);
        }
        Icon::Close => {
            line([6., 6.], [18., 18.]);
            line([18., 6.], [6., 18.]);
        }
        Icon::Undo | Icon::Redo => {
            let mirror = matches!(icon, Icon::Redo);
            let p = |x: f32, y: f32| [if mirror { 24. - x } else { x }, y];
            path(&[p(8., 4.), p(3., 9.), p(8., 14.)]);
            path(&[
                p(3., 9.),
                p(15., 9.),
                p(20., 12.),
                p(21., 16.),
                p(19., 20.),
                p(14., 21.),
            ]);
        }
        Icon::Left => arrow(true),
        Icon::Right => arrow(false),
        Icon::Search => {
            circle(10., 10., 7.);
            line([15., 15.], [22., 22.]);
        }
        Icon::Redact => {
            painter.rect_filled(
                Rect::from_min_size(point(3., 6.), egui::vec2(18. * scale, 5. * scale)),
                1. * scale,
                color,
            );
            painter.rect_filled(
                Rect::from_min_size(point(3., 15.), egui::vec2(12. * scale, 5. * scale)),
                1. * scale,
                color,
            );
        }
        Icon::ZoomIn | Icon::ZoomOut => {
            circle(9.5, 9.5, 6.5);
            line([14., 14.], [21., 21.]);
            line([6., 9.5], [13., 9.5]);
            if matches!(icon, Icon::ZoomIn) {
                line([9.5, 6.], [9.5, 13.]);
            }
        }
        Icon::FitWidth => {
            line([2., 3.], [2., 21.]);
            line([22., 3.], [22., 21.]);
            line([3., 12.], [21., 12.]);
            path(&[[6., 8.], [2., 12.], [6., 16.]]);
            path(&[[18., 8.], [22., 12.], [18., 16.]]);
        }
        Icon::FitPage => {
            box_(7., 5., 10., 14.);
            path(&[[2., 8.], [2., 2.], [8., 2.]]);
            path(&[[16., 2.], [22., 2.], [22., 8.]]);
            path(&[[22., 16.], [22., 22.], [16., 22.]]);
            path(&[[8., 22.], [2., 22.], [2., 16.]]);
        }
        Icon::Single => box_(5., 2., 14., 20.),
        Icon::Continuous => {
            box_(5., 5., 14., 14.);
            line([5., 1.], [19., 1.]);
            line([5., 23.], [19., 23.]);
        }
        Icon::TwoPages | Icon::Compare => {
            box_(2., 4., 8., 16.);
            box_(14., 4., 8., 16.);
            if matches!(icon, Icon::Compare) {
                line([6., 9.], [18., 9.]);
                line([6., 15.], [18., 15.]);
            }
        }
        Icon::Sun => {
            circle(12., 12., 4.);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                line(
                    [12. + 7. * a.cos(), 12. + 7. * a.sin()],
                    [12. + 10. * a.cos(), 12. + 10. * a.sin()],
                );
            }
        }
        Icon::Moon => path(&[
            [19., 16.],
            [15., 17.],
            [11., 15.],
            [8., 11.],
            [8., 6.],
            [10., 2.],
            [6., 4.],
            [3., 8.],
            [3., 14.],
            [6., 19.],
            [11., 22.],
            [17., 21.],
            [21., 18.],
            [22., 14.],
            [19., 16.],
        ]),
        Icon::Fullscreen => {
            path(&[[9., 3.], [3., 3.], [3., 9.]]);
            path(&[[15., 3.], [21., 3.], [21., 9.]]);
            path(&[[21., 15.], [21., 21.], [15., 21.]]);
            path(&[[9., 21.], [3., 21.], [3., 15.]]);
        }
        Icon::RotateLeft | Icon::RotateRight | Icon::Reset => {
            let mirror = matches!(icon, Icon::RotateRight);
            let p = |x: f32, y: f32| [if mirror { 24. - x } else { x }, y];
            path(&[
                p(3., 8.),
                p(8., 3.),
                p(15., 3.),
                p(21., 8.),
                p(21., 15.),
                p(16., 21.),
                p(9., 21.),
                p(5., 17.),
            ]);
            path(&[p(3., 2.), p(3., 8.), p(9., 8.)]);
        }
        Icon::Merge => {
            path(&[[3., 3.], [3., 7.], [12., 14.], [21., 7.], [21., 3.]]);
            line([12., 14.], [12., 22.]);
            path(&[[8., 18.], [12., 22.], [16., 18.]]);
        }
        Icon::Split => {
            path(&[[3., 21.], [3., 17.], [12., 10.], [21., 17.], [21., 21.]]);
            line([12., 10.], [12., 2.]);
            path(&[[8., 6.], [12., 2.], [16., 6.]]);
        }
        Icon::Delete => {
            line([3., 6.], [21., 6.]);
            path(&[[8., 6.], [8., 2.], [16., 2.], [16., 6.]]);
            path(&[[5., 6.], [6., 22.], [18., 22.], [19., 6.]]);
            line([10., 10.], [10., 18.]);
            line([14., 10.], [14., 18.]);
        }
        Icon::Replace => {
            path(&[[3., 7.], [20., 7.], [16., 3.]]);
            path(&[[21., 17.], [4., 17.], [8., 21.]]);
            box_(7., 9., 10., 6.);
        }
        Icon::Bookmark => path(&[
            [5., 2.],
            [19., 2.],
            [19., 22.],
            [12., 17.],
            [5., 22.],
            [5., 2.],
        ]),
        Icon::Lock => {
            box_(4., 10., 16., 12.);
            path(&[
                [7., 10.],
                [7., 6.],
                [9., 2.],
                [15., 2.],
                [17., 6.],
                [17., 10.],
            ]);
            circle(12., 15., 1.5);
            line([12., 16.], [12., 19.]);
        }
        Icon::Scan => {
            path(&[[3., 8.], [3., 3.], [8., 3.]]);
            path(&[[16., 3.], [21., 3.], [21., 8.]]);
            path(&[[21., 16.], [21., 21.], [16., 21.]]);
            path(&[[8., 21.], [3., 21.], [3., 16.]]);
            line([1., 12.], [23., 12.]);
            line([8., 7.], [16., 7.]);
            line([8., 17.], [16., 17.]);
        }
        Icon::Compress => {
            path(&[[2., 2.], [9., 9.], [9., 4.]]);
            line([9., 9.], [4., 9.]);
            path(&[[22., 22.], [15., 15.], [15., 20.]]);
            line([15., 15.], [20., 15.]);
            box_(3., 14., 7., 7.);
            box_(14., 3., 7., 7.);
        }
        Icon::Print => {
            box_(7., 2., 10., 6.);
            path(&[
                [7., 17.],
                [3., 17.],
                [3., 8.],
                [21., 8.],
                [21., 17.],
                [17., 17.],
            ]);
            box_(7., 14., 10., 8.);
            circle(17., 11., 0.5);
        }
        Icon::Volume => {
            path(&[
                [3., 9.],
                [7., 9.],
                [12., 4.],
                [12., 20.],
                [7., 15.],
                [3., 15.],
                [3., 9.],
            ]);
            path(&[[16., 7.], [18., 10.], [18., 14.], [16., 17.]]);
            path(&[[19., 3.], [22., 8.], [22., 16.], [19., 21.]]);
        }
        Icon::Stop => box_(5., 5., 14., 14.),
        Icon::Check => path(&[[4., 12.], [10., 18.], [21., 5.]]),
        Icon::Labels => {
            page();
            line([10., 11.], [16., 11.]);
            line([10., 15.], [16., 15.]);
            line([10., 19.], [14., 19.]);
        }
        Icon::Batch => {
            box_(2., 2., 7., 7.);
            box_(15., 2., 7., 7.);
            box_(2., 15., 7., 7.);
            box_(15., 15., 7., 7.);
        }
        Icon::Header | Icon::Footer | Icon::Watermark => {
            page();
            match icon {
                Icon::Header => box_(9., 9., 8., 2.),
                Icon::Footer => box_(9., 18., 8., 2.),
                _ => {
                    path(&[[8., 18.], [17., 9.]]);
                    line([8., 13.], [13., 18.]);
                }
            }
        }
    }
}
