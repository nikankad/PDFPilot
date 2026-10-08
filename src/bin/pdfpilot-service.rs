fn main() -> anyhow::Result<()> {
    let mut arguments = std::env::args_os().skip(1);
    let root = arguments
        .next()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "service-data".into());
    let port = arguments
        .next()
        .map(|s| s.to_string_lossy().parse::<u16>())
        .transpose()?
        .unwrap_or(8787);
    let service = pdfpilot::service::Service::open(&root)?;
    let running = service.start(port)?;
    println!(
        "PDFPilot review service: {}\nData: {}\nLoopback only. Use a TLS reverse proxy for remote deployment.",
        running.address,
        root.display()
    );
    loop {
        std::thread::park();
    }
}
