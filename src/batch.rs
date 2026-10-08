use crate::{
    pdf::{PdfDocument, TextStyle},
    security::{self, SecurityOptions},
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Compress,
    StripMetadata,
    Watermark(String),
    Header(String),
    Footer(String),
    PageNumbers { prefix: String, start: u32 },
    Language(String),
}
#[derive(Clone, Default, Debug, PartialEq)]
pub struct Plan {
    pub steps: Vec<Step>,
}
impl Plan {
    pub fn save(&self, path: &Path) -> Result<()> {
        let steps = self
            .steps
            .iter()
            .map(|step| match step {
                Step::Compress => json!({"operation":"compress"}),
                Step::StripMetadata => json!({"operation":"sanitize_metadata"}),
                Step::Watermark(text) => json!({"operation":"watermark","text":text}),
                Step::Header(text) => json!({"operation":"header","text":text}),
                Step::Footer(text) => json!({"operation":"footer","text":text}),
                Step::Language(text) => json!({"operation":"language","text":text}),
                Step::PageNumbers { prefix, start } => {
                    json!({"operation":"page_numbers","text":prefix,"start":start})
                }
            })
            .collect::<Vec<_>>();
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&json!({"version":1,"steps":steps}))?,
        )?;
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path)?;
        ensure!(bytes.len() < 1_000_000, "Preset is too large");
        let v: Value = serde_json::from_slice(&bytes)?;
        ensure!(v["version"] == 1, "Unsupported action preset version");
        let steps = v["steps"].as_array().context("Preset needs steps")?;
        ensure!(steps.len() <= 100, "Too many operations");
        let mut parsed = vec![];
        for s in steps {
            let text = || {
                s["text"]
                    .as_str()
                    .map(str::to_owned)
                    .context("Operation needs text")
            };
            parsed.push(
                match s["operation"].as_str().context("Missing operation")? {
                    "compress" => Step::Compress,
                    "sanitize_metadata" => Step::StripMetadata,
                    "watermark" => Step::Watermark(text()?),
                    "header" => Step::Header(text()?),
                    "footer" => Step::Footer(text()?),
                    "language" => Step::Language(text()?),
                    "page_numbers" => {
                        let start =
                            u32::try_from(s["start"].as_u64().context("Invalid starting number")?)?;
                        ensure!(start > 0, "Page numbers start at one or higher");
                        Step::PageNumbers {
                            prefix: text()?,
                            start,
                        }
                    }
                    _ => anyhow::bail!("Unsupported batch operation"),
                },
            );
        }
        Ok(Self { steps: parsed })
    }
    pub fn apply(&self, pdf: &mut PdfDocument) -> Result<()> {
        ensure!(!self.steps.is_empty(), "Add at least one action");
        for step in &self.steps {
            match step {
                Step::Compress => pdf.compress()?,
                Step::StripMetadata => pdf.strip_metadata()?,
                Step::Language(language) => pdf.set_language(language)?,
                Step::Watermark(text) | Step::Header(text) | Step::Footer(text) => {
                    let (position, size) = match step {
                        Step::Watermark(_) => ([0.2, 0.5], 36.),
                        Step::Header(_) => ([0.08, 0.06], 11.),
                        _ => ([0.08, 0.94], 11.),
                    };
                    pdf.add_running_text(
                        text,
                        position,
                        &TextStyle {
                            size,
                            ..Default::default()
                        },
                        false,
                        1,
                        0,
                    )?;
                }
                Step::PageNumbers { prefix, start } => pdf.add_running_text(
                    prefix,
                    [0.08, 0.94],
                    &TextStyle::default(),
                    true,
                    *start,
                    0,
                )?,
            }
        }
        Ok(())
    }
    /// Prepare all files before publishing any output. Existing files are never overwritten.
    pub fn run(
        &self,
        inputs: &[PathBuf],
        folder: &Path,
        protection: Option<&SecurityOptions>,
    ) -> Result<Vec<PathBuf>> {
        ensure!(!inputs.is_empty(), "Select input PDFs");
        ensure!(folder.is_dir(), "Choose an output folder");
        let stage = tempfile::tempdir_in(folder)?;
        let mut prepared = vec![];
        for (index, input) in inputs.iter().enumerate() {
            let stem = input
                .file_stem()
                .context("Input needs a filename")?
                .to_string_lossy();
            let output = folder.join(format!("{stem}-processed.pdf"));
            ensure!(
                !output.exists() && !prepared.iter().any(|(_, p)| p == &output),
                "Output already exists or names conflict"
            );
            let mut pdf = PdfDocument::open(input, "")?;
            self.apply(&mut pdf)?;
            let path = stage.path().join(format!("{index}.pdf"));
            if let Some(security) = protection {
                security::protect(&pdf.doc, security, &path)?;
            } else {
                pdf.save_copy(&path)?;
            }
            prepared.push((path, output));
        }
        let mut published = vec![];
        for (source, output) in prepared {
            let result = (|| -> Result<()> {
                let mut file = tempfile::NamedTempFile::new_in(folder)?;
                std::io::copy(&mut std::fs::File::open(source)?, file.as_file_mut())?;
                file.as_file().sync_all()?;
                file.persist_noclobber(&output).map_err(|e| e.error)?;
                Ok(())
            })();
            if let Err(error) = result {
                for path in &published {
                    let _ = std::fs::remove_file(path);
                }
                return Err(error);
            }
            published.push(output);
        }
        Ok(published)
    }
}
