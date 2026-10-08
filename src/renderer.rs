use anyhow::{Context, Result, bail, ensure};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RenderKey {
    pub document: u64,
    pub revision: u64,
    pub page: u32,
    pub width: u32,
}

pub struct RenderRequest {
    pub key: RenderKey,
    pub path: PathBuf,
}
pub struct Rendered {
    pub key: RenderKey,
    pub pixels: Result<image::RgbaImage>,
}

pub struct Renderer {
    sender: Sender<RenderRequest>,
    pub results: Receiver<Rendered>,
    pub available: bool,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer {
    pub fn new() -> Self {
        let (sender, requests) = mpsc::channel::<RenderRequest>();
        let (results_tx, results) = mpsc::channel();
        let available = find_poppler().is_some();
        thread::spawn(move || {
            while let Ok(request) = requests.recv() {
                let pixels = render_page(&request.path, request.key.page, request.key.width);
                if results_tx
                    .send(Rendered {
                        key: request.key,
                        pixels,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            sender,
            results,
            available,
        }
    }
    pub fn request(&self, request: RenderRequest) -> Result<()> {
        self.sender
            .send(request)
            .context("The rendering worker stopped")
    }
}

pub fn find_poppler() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("PDFPILOT_PDFTOPPM") {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }
    let mut candidates = vec![];
    if let Some(paths) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&paths).map(|p| {
            p.join(if cfg!(windows) {
                "pdftoppm.exe"
            } else {
                "pdftoppm"
            })
        }));
    }
    candidates.extend(
        [
            "/opt/homebrew/bin/pdftoppm",
            "/usr/local/bin/pdftoppm",
            "/usr/bin/pdftoppm",
        ]
        .map(PathBuf::from),
    );
    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(PathBuf::from(home).join(
            ".cache/codex-runtimes/codex-primary-runtime/dependencies/bin/override/pdftoppm",
        ));
    }
    candidates.into_iter().find(|p| p.is_file())
}

pub fn render_page(path: &Path, page: u32, width: u32) -> Result<image::RgbaImage> {
    ensure!(page > 0, "Page numbers start at 1");
    ensure!((64..=6000).contains(&width), "Render width is out of range");
    let binary=find_poppler().context("Install Poppler to view PDFs: brew install poppler (macOS), or apt install poppler-utils (Linux). You can also set PDFPILOT_PDFTOPPM to the binary path.")?;
    let temp = tempfile::tempdir()?;
    let prefix = temp.path().join("page");
    let mut command = Command::new(binary);
    if std::env::var_os("FONTCONFIG_FILE").is_none() {
        for config in [
            "/opt/homebrew/etc/fonts/fonts.conf",
            "/usr/local/etc/fonts/fonts.conf",
            "/etc/fonts/fonts.conf",
        ] {
            if Path::new(config).is_file() {
                command.env("FONTCONFIG_FILE", config);
                break;
            }
        }
    }
    let output = command
        .arg("-f")
        .arg(page.to_string())
        .arg("-l")
        .arg(page.to_string())
        .arg("-singlefile")
        .arg("-cropbox")
        .arg("-scale-to-x")
        .arg(width.to_string())
        .arg("-scale-to-y")
        .arg("-1")
        .arg("-png")
        .arg(path)
        .arg(&prefix)
        .output()
        .context("Could not start the Poppler renderer")?;
    if !output.status.success() {
        bail!(
            "Rendering failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(image::open(prefix.with_extension("png"))
        .context("Could not decode the rendered page")?
        .to_rgba8())
}
