//! Detached CMS signatures. Integrity and certificate trust are separate results.
use crate::{
    advanced::{FormKind, FormSpec},
    pdf::{self, PdfDocument},
};
use anyhow::{Context, Result, ensure};
use lopdf::{Document, Object, StringFormat, dictionary};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Clone, Debug)]
pub struct Verification {
    pub field: String,
    pub integrity: bool,
    pub trusted: Option<bool>,
    pub covers_current_file: bool,
    pub detail: String,
}
fn openssl() -> PathBuf {
    std::env::var_os("PDFPILOT_OPENSSL")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("openssl"))
}
pub fn signed_copy(
    source: &Document,
    certificate: &Path,
    private_key: &Path,
    password: &str,
    output: &Path,
) -> Result<()> {
    ensure!(
        source.encryption_state.is_none(),
        "Remove authorized encryption before certificate signing"
    );
    ensure!(
        !source.objects.values().any(|o| o.as_dict().is_ok_and(|d| d
            .get(b"Type")
            .map(pdf::object_text)
            .unwrap_or_default()
            == "Sig")),
        "This signing workflow refuses already signed PDFs to preserve existing signatures"
    );
    let mut pdf = PdfDocument::from_document(source.clone(), "signed.pdf");
    let target = if let Some(field) = pdf.fields().iter().find(|f| f.kind == "Sig") {
        field.id
    } else {
        pdf.add_form_field(
            1,
            [0.08, 0.88],
            [0.48, 0.95],
            &FormSpec {
                kind: FormKind::Signature,
                name: format!("Signature{}", source.max_id + 1),
                required: false,
                options: vec![],
            },
        )?;
        pdf.fields()
            .iter()
            .find(|f| f.kind == "Sig")
            .context("Signature field missing")?
            .id
    };
    ensure!(
        !password.contains(['\n', '\r']),
        "Private-key password must fit on one line"
    );
    let field = pdf.doc.get_dictionary(target)?.clone();
    let widgets = field
        .get(b"Kids")
        .ok()
        .and_then(|o| o.as_array().ok())
        .map(|a| {
            a.iter()
                .filter_map(|o| o.as_reference().ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![target]);
    let font = pdf
        .doc
        .add_object(dictionary! {"Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>"Helvetica"});
    for widget in widgets {
        let rect = pdf
            .doc
            .get_dictionary(widget)?
            .get(b"Rect")
            .ok()
            .and_then(|o| o.as_array().ok())
            .filter(|a| a.len() == 4)
            .map(|a| {
                a.iter()
                    .map(|o| o.as_float())
                    .collect::<lopdf::Result<Vec<_>>>()
            });
        if let Some(Ok(rect)) = rect {
            let width = (rect[2] - rect[0]).abs();
            let height = (rect[3] - rect[1]).abs();
            let size = (width / 100.).min(height / 2.).clamp(1., 10.);
            let appearance = pdf.doc.add_object(lopdf::Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![Object::Integer(0),Object::Integer(0),Object::Real(width),Object::Real(height)],"Resources"=>dictionary!{"Font"=>dictionary!{"F"=>font}}},format!("q BT /F {size} Tf 0 g 2 {} Td (Certificate signed) Tj ET Q",height/2.).into_bytes()));
            pdf.doc
                .get_dictionary_mut(widget)?
                .set("AP", dictionary! {"N"=>appearance});
        }
    }
    let mut form = pdf
        .doc
        .get_dict_in_dict(pdf.doc.catalog()?, b"AcroForm")?
        .clone();
    form.set("SigFlags", 3);
    let form_id = pdf.doc.add_object(form);
    pdf.doc.catalog_mut()?.set("AcroForm", form_id);
    let reserve = 32768;
    let signature=pdf.doc.add_object(dictionary!{"Type"=>"Sig","Filter"=>"Adobe.PPKLite","SubFilter"=>"adbe.pkcs7.detached","ByteRange"=>vec![Object::Integer(0),Object::Integer(9999999997),Object::Integer(9999999998),Object::Integer(9999999999)],"Contents"=>Object::String(vec![0;reserve],StringFormat::Hexadecimal),"Reason"=>pdf::pdf_string("Signed with PDFPilot")});
    pdf.doc.get_dictionary_mut(target)?.set("V", signature);
    let mut bytes = vec![];
    pdf.doc.save_to(&mut bytes)?;
    let parsed = Document::load_mem(&bytes)?;
    let object_start = match parsed.reference_table.get(signature.0) {
        Some(lopdf::xref::XrefEntry::Normal { offset, .. }) => *offset as usize,
        _ => anyhow::bail!("Signature object must be stored directly"),
    };
    let object_end = object_start
        + bytes[object_start..]
            .windows(6)
            .position(|w| w == b"endobj")
            .context("Signature object missing")?;
    let zeros = vec![b'0'; reserve * 2];
    let contents = object_start
        + bytes[object_start..object_end]
            .windows(zeros.len())
            .position(|w| w == zeros)
            .context("Signature placeholder missing")?;
    ensure!(
        contents > 0 && bytes[contents - 1] == b'<' && bytes[contents + zeros.len()] == b'>',
        "Invalid signature placeholder"
    );
    let gap_start = contents - 1;
    let gap_end = contents + zeros.len() + 1;
    for (placeholder, n) in [
        (b"9999999997".as_slice(), gap_start),
        (b"9999999998", gap_end),
        (b"9999999999", bytes.len() - gap_end),
    ] {
        let pos = object_start
            + bytes[object_start..object_end]
                .windows(placeholder.len())
                .position(|w| w == placeholder)
                .context("ByteRange placeholder missing")?;
        let text = format!("{n:<10}");
        ensure!(text.len() == 10, "PDF too large for signing");
        bytes[pos..pos + 10].copy_from_slice(text.as_bytes());
    }
    let temp = tempfile::tempdir()?;
    let data = temp.path().join("data.bin");
    let cms = temp.path().join("signature.der");
    let mut ranges = bytes[..gap_start].to_vec();
    ranges.extend(&bytes[gap_end..]);
    std::fs::write(&data, &ranges)?;
    let mut command = Command::new(openssl());
    command
        .args([
            "cms",
            "-sign",
            "-binary",
            "-nosmimecap",
            "-md",
            "sha256",
            "-outform",
            "DER",
            "-in",
        ])
        .arg(&data)
        .arg("-signer")
        .arg(certificate)
        .arg("-inkey")
        .arg(private_key)
        .args(["-passin", "stdin", "-out"])
        .arg(&cms)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .context("Certificate signing requires OpenSSL with CMS support")?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(password.as_bytes())?;
        stdin.write_all(b"\n")?;
    }
    let result = child.wait_with_output()?;
    ensure!(
        result.status.success(),
        "Signing failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let cms = std::fs::read(cms)?;
    ensure!(cms.len() <= reserve, "Certificate chain is too large");
    for (i, b) in cms.iter().enumerate() {
        let text = format!("{b:02X}");
        bytes[contents + i * 2..contents + i * 2 + 2].copy_from_slice(text.as_bytes());
    }
    let mut saved = tempfile::NamedTempFile::new_in(output.parent().unwrap_or(Path::new(".")))?;
    saved.write_all(&bytes)?;
    saved.as_file().sync_all()?;
    saved.persist(output).map_err(|e| e.error)?;
    Ok(())
}
fn der_length(bytes: &[u8]) -> Result<usize> {
    ensure!(
        bytes.len() >= 2 && bytes[0] == 0x30,
        "Invalid CMS DER sequence"
    );
    let b = bytes[1] as usize;
    let (header, body) = if b & 0x80 == 0 {
        (2, b)
    } else {
        let n = b & 0x7f;
        ensure!(
            n > 0 && n <= 4 && bytes.len() >= 2 + n,
            "Invalid DER length"
        );
        let len = bytes[2..2 + n]
            .iter()
            .fold(0usize, |v, b| (v << 8) | *b as usize);
        (2 + n, len)
    };
    ensure!(header + body <= bytes.len(), "Truncated CMS signature");
    Ok(header + body)
}
pub fn verify(path: &Path, trust_roots: Option<&Path>) -> Result<Vec<Verification>> {
    let bytes = std::fs::read(path)?;
    let doc = Document::load_mem(&bytes)?;
    let pdf = PdfDocument::from_document(doc, "verify.pdf");
    let mut reports = vec![];
    for field in pdf.fields().into_iter().filter(|f| f.kind == "Sig") {
        let dict = pdf.doc.get_dictionary(field.id)?;
        let Ok(value) = dict.get(b"V") else {
            continue;
        };
        let signature = pdf.doc.dereference(value)?.1.as_dict()?;
        ensure!(
            signature
                .get(b"SubFilter")
                .map(pdf::object_text)
                .unwrap_or_default()
                == "adbe.pkcs7.detached",
            "Unsupported signature format"
        );
        let ranges = signature
            .get(b"ByteRange")?
            .as_array()?
            .iter()
            .map(|o| Ok(usize::try_from(o.as_i64()?)?))
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            ranges.len() == 4 && ranges[0] == 0 && ranges[1] < ranges[2],
            "Invalid signature byte ranges"
        );
        let end = ranges[2]
            .checked_add(ranges[3])
            .context("Invalid byte range overflow")?;
        ensure!(end <= bytes.len(), "Signature range exceeds file length");
        // Ensure the excluded span contains exactly this signature's hex string.
        let contents = signature.get(b"Contents")?.as_str()?;
        let gap = &bytes[ranges[1]..ranges[2]];
        ensure!(
            gap.first() == Some(&b'<') && gap.last() == Some(&b'>'),
            "Excluded signature span is not a hex string"
        );
        let hex = gap[1..gap.len() - 1]
            .iter()
            .filter(|b| !b.is_ascii_whitespace())
            .copied()
            .collect::<Vec<_>>();
        ensure!(
            hex.len() == contents.len() * 2,
            "Excluded contents do not match signature dictionary"
        );
        for (pair, b) in hex.chunks_exact(2).zip(contents) {
            let s = std::str::from_utf8(pair)?;
            ensure!(
                u8::from_str_radix(s, 16)? == *b,
                "Excluded contents do not match signature dictionary"
            );
        }
        let temp = tempfile::tempdir()?;
        let cms = temp.path().join("signature.der");
        std::fs::write(&cms, &contents[..der_length(contents)?])?;
        let data = temp.path().join("data.bin");
        let mut signed = bytes[..ranges[1]].to_vec();
        signed.extend(&bytes[ranges[2]..end]);
        std::fs::write(&data, signed)?;
        let run = |trust: Option<&Path>| -> Result<std::process::Output> {
            let mut c = Command::new(openssl());
            c.args(["cms", "-verify", "-binary", "-inform", "DER", "-in"])
                .arg(&cms)
                .arg("-content")
                .arg(&data)
                .arg("-out")
                .arg(temp.path().join("verified.bin"));
            if let Some(ca) = trust {
                c.arg("-CAfile").arg(ca);
            } else {
                c.arg("-noverify");
            }
            c.output().context("OpenSSL CMS verification unavailable")
        };
        let integrity = run(None)?;
        let trusted = trust_roots
            .map(|ca| run(Some(ca)))
            .transpose()?
            .map(|o| o.status.success());
        reports.push(Verification {
            field: field.name,
            integrity: integrity.status.success(),
            trusted,
            covers_current_file: end == bytes.len(),
            detail: String::from_utf8_lossy(&integrity.stderr).trim().into(),
        });
    }
    Ok(reports)
}
