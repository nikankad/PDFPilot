fn main() -> anyhow::Result<()> {
    let directory = std::path::Path::new("assets");
    std::fs::create_dir_all(directory)?;
    pdfpilot::pdf::atomic_save(
        &pdfpilot::pdf::demo_document()?,
        &directory.join("welcome.pdf"),
    )?;
    println!("Created assets/welcome.pdf");
    Ok(())
}
