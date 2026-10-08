# PDFPilot

A native Rust PDF desktop workspace inspired by Acrobat. This is an independent application with its own design. Full Acrobat parity is **unfinished**; [FEATURES.md](FEATURES.md) records all 234 requested bullets and their limits.

## Run

Rust 1.95 is selected by `rust-toolchain.toml`.

```sh
# macOS
brew install poppler tesseract
brew install --cask libreoffice  # Office / local HTML import
# Debian / Ubuntu
sudo apt install poppler-utils tesseract-ocr libgtk-3-dev libxkbcommon-dev libwayland-dev openssl

cargo run --release
cargo run --release -- /absolute/path/document.pdf
cargo run --example sample
bash scripts/package-macos.sh
```

Windows/Linux builds have not been verified. On Windows, install Poppler/Tesseract/OpenSSL and add them to `PATH`. Printing currently uses CUPS `lp` on macOS/Linux. The macOS bundle detects local engines, including Codex's bundled Poppler. `PDFPILOT_PDFTOPPM`, `PDFPILOT_TESSERACT`, `PDFPILOT_SOFFICE`, and `PDFPILOT_OPENSSL` override engine paths. Files stay local unless you explicitly publish a snapshot to your own service.

## Workflows

| Area | Implemented workflows |
| --- | --- |
| Interface | Icon navigation/tools, tooltips, accessible names, dark/light themes, saved theme preference, collapsible inspector, tabs, undo/redo, atomic saves and unsaved prompts |
| Reading | PDF/image/Office opening, thumbnails, bookmarks, navigation, zoom, fit modes, single/continuous/spread views, fullscreen, search, extracted text, macOS speech |
| Editing | Styled text; editable paragraph blocks with alignment/spacing; supported text-run replacement; image insertion and top-level placement move/resize/crop/rotation/deletion; backgrounds; five page boxes; headers/footers/watermarks/Bates/page numbers |
| Review | Highlights, underline/strikeout, notes, correction carets, editable Ink annotations, shapes, stamps, attachment comments, replies/resolution/filtering and summaries |
| Pages | Rotate/ranges, drag ordering, insert/delete/extract/split/duplicate/replace/crop/combine, decimal labels and bookmarks |
| Forms | Text/checkbox/radio/dropdown/list/date/signature widgets, reviewed vector field detection, required/read-only/tooltip/max-length/numeric rules, sum/product/average calculations, tab order and JSON import/export |
| Security | Separate open/owner passwords, AES-256 permission settings, encryption-preserving saves, reader permission checks, owner-authenticated protection removal |
| Signatures | PEM certificate/key signing through OpenSSL detached CMS, optional key password, integrity and selected-root trust verification, signed-file coverage check |
| Scan/OCR | Configurable Tesseract languages/segmentation/resolution; automatic deskew, median despeckle, grayscale/contrast/brightness/threshold; configurable flattened JPEG optimization |
| Accessibility/inspection | Basic page-group tags and existing role/alt-text edits, language/title/tooltips, conservative diagnostics, font/object/stream-storage inventory |
| Measurements | Distances, rectangular area/perimeter, known-length calibration, persisted measurement annotations |
| Print | CUPS ranges/n-up/color/orientation/fit/scaling/paper; padded two-up booklet imposition, fold marks, saved sheet previews and short-edge duplex submission |
| Batch | Reorderable operations, versioned JSON presets, optional password protection, staged output publication and collision refusal |
| Service | Independent local browser reviews, private recipient links, ordered/parallel typed signing, comments/replies/resolution, signing receipts, reminders/activity and desktop/browser refresh |

Use **Help > Open sample document** to explore. On macOS, File/Edit/View/Window/Help live in the system menu bar, with native shortcuts and document-aware menu states. Windows/Linux retain an in-window menu. The classic layout has Home/Tools/document tabs, a reading toolbar, a narrow left navigation rail and colored tool rows on the right. Click a row to expand its controls. The left page icon shows/hides thumbnails; bookmarks and attachments open the file tools. Hover for a control’s name. Page ranges use `1, 3-5`. Light mode is the initial default; your chosen dark/light preference is saved.

Paragraph/image editing is in **Edit PDF**. Configure and draw fields in **Prepare Form**, then fill their values. **Export PDF** contains certificate, OCR and scan settings. **Properties** contains inspection and tag tools. **Actions & Sharing**, also opened by the blue **Share** button, contains reusable batch sequences and the review service. **View** controls dark mode, thumbnails, the tools pane and fullscreen; the toolbar’s overflow menu contains zoom/view options and undo/redo.

## Independent review service

Start it from **Actions & sharing**, choosing a private storage folder. Remove authorized password protection before publishing. Publishing explicitly saves a document snapshot and produces private review/signing links. Opening a signer link collects a typed electronic signature and enforces the selected signing order. Completed requests produce the original PDF plus a signing receipt and an attached UTF-8 record. The receipt displays non-ASCII names as Unicode codepoints. No email or SMS is sent automatically.

The standalone server is also available:

```sh
cargo run --release --bin pdfpilot-service -- /private/storage/folder 8787
```

It binds **127.0.0.1**. Remote use requires your own TLS reverse proxy and an operational deployment; no public server has been deployed. The current desktop publication flow uses local storage. Protect service files and links, and back up the storage folder. [SERVICE.md](SERVICE.md) describes the API and limits.

## Limits

- Existing text replacement preserves supported runs, font and position. It does not reflow arbitrary documents. Paragraph blocks support Western Latin fonts and estimated widths; complex scripts and exact typography require additional support. Image editing supports top-level image `Do` placements; nested Form XObjects and inline images are refused or omitted. Deletion/replacement is not secure redaction.
- DOCX exports extracted text, XLSX exports text rows/tab-separated columns, and PPTX exports page images. These do not reconstruct original editable layouts/tables. LibreOffice imports depend on fonts and compatibility. Live web capture, scanner acquisition, mobile scanning, virtual printer drivers and Office add-ins remain unfinished.
- Form validation/calculations run in PDFPilot. Supported calculations also emit Acrobat calculation actions; arbitrary imported PDF JavaScript is not executed. XFA, multi-select controls, full property editing and interoperable general validation scripts remain unfinished. Field detection handles vector rectangles, not scanned or semantic layouts.
- Certificate signing rewrites an unsigned, unencrypted PDF. It refuses already signed/encrypted files. Verification supports detached CMS integrity and optional selected-root trust, not revocation, DocMDP changes, timestamp/LTV policy or hardware identities. Editing a signed PDF can invalidate its signatures. Service typed signatures are link-authenticated visual/electronic receipts, not certificate signatures or real-world identity verification.
- Review event hashes detect altered events but are not an externally anchored audit trail: a server operator can rewrite the whole history. Service reminders are activity events; OS push, email/SMS, recipient placement templates and production hosting/hardening remain unfinished.
- Diagnostics and basic tagging do not certify PDF/UA, PDF/A or PDF/X. Full reading-order remediation, list/table semantics, embedded-font creation, color management/separations, 3D models, rich media and Adobe services remain unfinished.
- OCR, redaction, scan cleanup, raster optimization and booklet copies flatten visual content. Editable forms/objects/annotations/tags/links and attachments are not retained. OCR adds searchable text; raster optimization does not. Deskew expands page dimensions. These workflows export separate copies.
- Redaction can stage multiple regions across pages in one document. Editing the document clears staged regions so they must be marked again. Export reconstructs all pages, discarding original hidden objects/metadata. It does not overwrite the source.
- Measurements support distance and rectangular area/perimeter; arbitrary polygons remain unfinished. Printer options and imposed PDFs are tested, but physical printer output is unverified. Booklets require short-edge duplex support.
- Existing form-widget pages cannot be duplicated/replaced, and incoming AcroForms cannot be merged. Incoming document-level bookmarks/attachments are not merged. Subset exports discard navigation/form trees while retaining visible widget appearances.
- Passwords and keys pass through memory and local subprocesses. Temporary render files are unencrypted; crash cleanup/memory hardening, cancellable jobs, broad PDF compatibility testing and large-page tiling remain unfinished. Undo stores up to 20 full document copies and can consume substantial memory.

## Verify

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release
```

Tests cover saved PDF values/appearances, edit rollback, forms/calculation cycles, encryption/owner authentication, certificate integrity/trust, image/text editing, scans, measurements, booklet order, batch staging and authenticated review workflows. Office tests require LibreOffice. Poppler/Tesseract tests require the corresponding engine. The macOS bundle is not a self-contained, signed or notarized installer.

## Architecture

Core modules: `pdf`, `advanced`, `editing`, `forms`, `comments`, `security`, `signatures`, `inspection`, `scans`, `measurements`, `printing`, `batch`, `conversion`, `workflows`, `renderer` and `service`. The native egui app is in `src/app.rs`, `src/app/classic.rs`, `src/app/native_menu.rs`, `src/app/features.rs` and `src/app/studio.rs`.

PDF structures follow [ISO 32000-1](https://opensource.adobe.com/dc-acrobat-sdk-docs/standards/pdfstandards/pdf/PDF32000_2008.pdf). Generated calculation actions use the [Acrobat Forms API](https://opensource.adobe.com/dc-acrobat-sdk-docs/library/interapp/IAC_API_FormsIntro.html). Saved dictionary/appearance behavior is tested locally; full Acrobat interoperability has not been validated.
