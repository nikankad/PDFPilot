#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod icons;
mod theme;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("PDFPilot")
            .with_inner_size([1200.0, 820.0])
            .with_min_inner_size([760.0, 550.0]),
        ..Default::default()
    };
    eframe::run_native(
        "PDFPilot",
        options,
        Box::new(|cc| Ok(Box::new(app::PdfApp::new(cc)))),
    )
}
