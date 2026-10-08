# Feature coverage

This tracks every bullet in the feature list supplied for PDFPilot. It is an implementation inventory, not a claim of complete Acrobat parity.

**Available** means the described local workflow is implemented, within the notes. **Partial** means a meaningful subset exists. **Pending** means there is no implementation. Printer output and full assistive-technology interoperability have not been validated.

Inventory: 234 requested bullets — 112 available, 109 partial, 13 pending. Bullets overlap; these counts do not represent a feature-parity percentage.

Dark/light themes, saved theme preference, undo/redo, document tabs, atomic PDF saves and unsaved-change prompts are also implemented.

## 1. PDF viewing and navigation

| Requested feature | Status | Scope / limitation |
| --- | --- | --- |
| Open and display PDF documents. | Available | Implemented. |
| View multiple PDFs in separate tabs. | Available | Implemented. |
| Zoom in and out. | Available | Implemented. |
| Fit pages to window width or height. | Available | Implemented. |
| Rotate the page view. | Partial | Rotation changes the PDF page, rather than maintaining a separate view-only rotation. |
| Use single-page, continuous, and two-page viewing modes. | Available | Implemented. |
| Search for words and phrases. | Available | Matching pages and snippets; no on-canvas hit highlighting. |
| Navigate using bookmarks, thumbnails, and page numbers. | Partial | Direct page destinations, nested bookmarks and thumbnails; named destinations remain unsupported. |
| View document metadata and properties. | Available | Implemented. |
| Access embedded links and attachments. | Partial | HTTP(S) links and embedded file name trees; other action/link types remain unsupported. |
| Enter full-screen reading mode. | Available | Implemented. |
| Read documents aloud using supported accessibility features. | Partial | macOS speech reads extracted current-page text; reading-order interpretation is limited. |
| Print entire documents or selected pages. | Partial | CUPS default-printer submission; physical printer behavior has not been tested. |
| Print multiple PDF pages on a single sheet. | Partial | CUPS n-up options; physical output has not been tested. |

## 2. PDF creation

| Requested feature | Status | Scope / limitation |
| --- | --- | --- |
| Create PDFs from Microsoft Word documents. | Available | Requires local LibreOffice. |
| Create PDFs from Excel spreadsheets. | Available | Requires local LibreOffice. |
| Create PDFs from PowerPoint presentations. | Available | Requires local LibreOffice. |
| Convert JPG, PNG, and other supported images to PDF. | Available | PNG, JPEG and TIFF supported. |
| Create PDFs from scanned paper documents. | Partial | Import saved scan images; scanner acquisition is not implemented. |
| Create PDFs from web pages. | Partial | Local HTML files through LibreOffice; live URL capture is not implemented. |
| Create PDFs from supported application files. | Available | Implemented. |
| Combine multiple file formats into one PDF. | Partial | Open each supported format as PDF, save, then combine PDFs; no mixed-format combine dialog. |
| Create PDFs from clipboard content in supported desktop workflows. | Partial | Paste text into Create PDF; no dedicated clipboard-image command. |
| Generate PDFs using a virtual PDF printer. | Pending | Not implemented. |
| Create PDFs from blank pages. | Available | Implemented. |
| Preserve fonts, images, and formatting during conversion. | Partial | LibreOffice rendering depends on installed fonts and source compatibility. |
| Create PDFs using Microsoft Office integrations. | Pending | Not implemented. |

## 3. PDF editing

| Requested feature | Status | Scope / limitation |
| --- | --- | --- |
| Edit existing text directly inside PDF documents. | Partial | Text panel replacement within supported content runs; no direct on-canvas text selection/reflow. |
| Correct spelling and typographical errors. | Partial | Manual replacements; no spell-check engine. |
| Insert new text. | Available | Western Latin, single-line text with standard PDF fonts. |
| Delete existing text. | Partial | Replace supported text with an empty string; no arbitrary object deletion. |
| Change fonts, font sizes, and font colors. | Partial | Added text and editable PDFPilot paragraph blocks support font/size/color; arbitrary existing text styling remains unsupported. |
| Adjust text alignment and spacing. | Partial | Left/center/right alignment, line spacing and character spacing for PDFPilot paragraph blocks; font widths are estimated. |
| Edit text blocks. | Partial | Create, reopen, update and delete PDFPilot paragraph blocks; arbitrary existing blocks/reflow remain unsupported. |
| Add new paragraphs. | Available | Multi-line PDFPilot text blocks with wrapping and saved editable properties; Western Latin fonts. |
| Insert images. | Available | JPEG/PNG insertion with a drawn placement area. |
| Delete images. | Partial | Remove top-level image placements without affecting other content; nested/inline images remain unsupported. Deletion is not secure redaction. |
| Resize and reposition images. | Partial | Normalized page placement editor for top-level image Do operations. |
| Crop and rotate images. | Partial | Clipping and affine rotation for top-level image placements; nested/inline image editing remains unsupported. |
| Add hyperlinks. | Available | Implemented. |
| Modify existing hyperlinks. | Available | Implemented. |
| Insert page backgrounds. | Available | Solid RGB backgrounds on the selected page. |
| Add watermarks. | Partial | Added visible text watermarks; no opacity/image-watermark editor. |
| Create and modify headers and footers. | Partial | Add text headers/footers to all pages; previously added text cannot be managed as named header/footer objects. |
| Add Bates numbering for legal document workflows. | Available | Prefix, starting number and zero padding. |
| Add page numbers. | Available | Implemented. |
| Change page margins and page boxes using supported tools. | Available | MediaBox, CropBox, TrimBox, BleedBox and ArtBox point-coordinate editor, plus crop/reset. |
| Edit PDF objects and graphical elements. | Partial | Image-placement transforms/deletion, page backgrounds and PDFPilot text blocks; general vector-object editing remains unsupported. |

## 4. Page organization

| Requested feature | Status | Scope / limitation |
| --- | --- | --- |
| Insert additional pages. | Available | Implemented. |
| Delete unwanted pages. | Available | Implemented. |
| Rearrange pages using drag and drop. | Available | Drag thumbnails, or use Move earlier/later. |
| Extract selected pages into a new document. | Available | Implemented. |
| Split large PDFs into smaller documents. | Available | Fixed number of pages per file; refuses existing outputs. |
| Merge several PDFs into one. | Partial | Combine PDFs; incoming forms are refused and incoming bookmarks/attachments are not merged. |
| Rotate individual pages or page ranges. | Available | Implemented. |
| Duplicate pages. | Partial | Duplication supported for pages without form widgets. |
| Replace pages with content from another PDF. | Available | Implemented. |
| Crop pages. | Available | Implemented. |
| Organize document sections. | Partial | Page ordering and bookmarks; no section-management model. |
| Add bookmarks for navigation. | Available | Implemented. |
| Renumber page labels. | Available | Decimal labels and prefixes; viewer uses those labels. |
| Create page thumbnails for easier navigation. | Available | Implemented. |

## 5. PDF conversion and export

| Requested feature | Status | Scope / limitation |
| --- | --- | --- |
| PDF to Word (DOCX). | Partial | Editable extracted text in DOCX; original layout, tables and graphics are not reconstructed. |
| PDF to Excel (XLSX). | Partial | Extracted text lines in XLSX, tabs split into columns; table reconstruction is not implemented. |
| PDF to PowerPoint (PPTX). | Partial | One rendered page image per slide; text and shapes are not editable. |
| PDF to JPG. | Available | Current page at 2400 pixels wide. |
| PDF to PNG. | Available | Current page at 2400 pixels wide. |
| PDF to TIFF. | Available | Current page at 2400 pixels wide. |
| PDF to text. | Available | Implemented. |
| PDF to HTML. | Partial | Self-contained HTML page images; no semantic text layout. |
| Export PDF content to supported structured and accessible formats. | Partial | Basic extracted-text Office packages and form JSON; no accessibility-tag export. |
| Convert Word documents to PDF. | Available | Implemented. |
| Convert Excel spreadsheets to PDF. | Available | Implemented. |
| Convert PowerPoint presentations to PDF. | Available | Implemented. |
| Convert images to PDF. | Available | Implemented. |
| Preserve document layouts during export where possible. | Partial | Image-based exports preserve visual appearance at fixed resolution; DOCX/XLSX use text flow. |
| Export selected pages instead of entire documents. | Available | Range export for documents; single current page for image export. |

## 6. OCR and scanned document processing

| Requested feature | Status | Scope / limitation |
| --- | --- | --- |
| Optical character recognition (OCR). | Available | Tesseract language/segmentation/resolution settings; language packs must be installed locally. |
| Convert scanned text into searchable text. | Available | Creates an image with an invisible recognized text layer. |
| Make recognized text editable. | Partial | Searchable text layer can be extracted/exported; OCR text is not exposed as editable positioned blocks. |
| Recognize text across multiple pages. | Available | All source pages processed. |
| Process scanned documents containing mixed text and images. | Available | Original visual content is flattened; editable objects are not retained. |
| Automatically detect text in scanned pages. | Available | Tesseract automatic text detection. |
| Correct recognized text. | Partial | General supported-run replacement; no OCR confidence/correction interface. |
| Straighten skewed scans. | Available | Projection-based automatic deskew within +/-8 degrees; expands raster canvas to preserve corners. |
| Improve scanned page appearance. | Available | Grayscale, contrast, brightness and optional black/white threshold in an exported raster copy. |
| Remove scanning artifacts using available enhancement tools. | Available | Median despeckle filter; does not reconstruct damaged content. |
| Optimize scanned documents for readability. | Available | Configurable cleanup, page raster width and JPEG quality; exports flattened pages. |
| Convert photographed documents into PDFs using mobile scanning. | Pending | Not implemented. |
| Search for recognized text in previously image-only documents. | Available | Implemented. |

## 7. Comments, annotations, and review

| Requested feature | Status | Scope / limitation |
| --- | --- | --- |
| Highlight text. | Available | Draw rectangular regions; no glyph-snapping selection. |
| Underline text. | Available | Draw rectangular regions. |
| Strike through text. | Available | Draw rectangular regions. |
| Add sticky notes. | Available | Implemented. |
| Add text comments. | Available | Implemented. |
| Insert text correction marks. | Available | Caret insertion/correction annotations with comments and saved appearances. |
| Draw freehand annotations. | Available | Standard Ink annotations with editable InkList, saved appearances, undo and removal through Comments. |
| Add arrows, lines, circles, and rectangles. | Available | Implemented. |
| Add text boxes. | Partial | Added page text, without a rich FreeText/text-box editor. |
| Use stamps such as Approved or Draft. | Available | Approved by default; custom text also supported. |
| Create custom stamps. | Partial | Custom Western Latin text stamps; no custom image stamp library. |
| Attach files to comments. | Available | FileAttachment annotations plus embedded files, paperclip markers and file export. |
| Reply to comments. | Available | Implemented. |
| Review and resolve comments. | Available | Implemented. |
| Filter comments by author, type, or status. | Partial | Filter by text/type and hide resolved comments; no author selector. |
| Share documents for collaborative review. | Partial | Independent local HTTP review service with private token links, browser comments and resolve/reopen. Remote access requires your own TLS deployment. |
| Collect feedback through shared links. | Partial | Private browser review links, page comments and replies; local service until deployed remotely. |
| Track document review activity. | Available | Durable review event history with a verifiable SHA-256 hash chain. Server operators can rewrite the whole history; no external trusted audit anchor. |
| Export comment summaries using supported workflows. | Available | Plain text summaries. |

## 8. Forms and data collection

| Requested feature | Status | Scope / limitation |
| --- | --- | --- |
| Fill existing PDF forms. | Partial | AcroForm text, checkbox, radio and single-choice widgets; signatures use the certificate workflow, XFA/multi-select remain unsupported. |
| Create fillable PDF forms. | Partial | Text, checkbox, radio, dropdown, list, date and unsigned signature widgets; complex scripts/XFA remain unsupported. |
| Convert supported documents into interactive forms. | Partial | Draw supported widgets on imported pages; no auto-detection. |
| Automatically detect potential form fields. | Partial | Proposes vector rectangular fields, excludes existing widgets, requires selection before authoring; scans and semantic field recognition remain unsupported. |
| Insert text fields. | Available | Implemented. |
| Add checkboxes. | Available | Implemented. |
| Add radio buttons. | Available | Named mutually exclusive radio groups with per-option appearances and export values. |
| Create dropdown menus. | Available | Implemented. |
| Insert list boxes. | Available | Single-select list fields with saved values/appearances; multi-select remains unsupported. |
| Add date fields. | Available | Date text widgets; PDFPilot validates YYYY-MM-DD including leap years. Other viewers do not execute PDFPilot validation rules. |
| Insert signature fields. | Available | Unsigned AcroForm signature widgets; certificate signing fills the first signature field or creates one. |
| Make fields required. | Available | Sets the PDF required-field flag; enforcement on submission is not implemented. |
| Configure validation rules. | Partial | Required, maximum length, date and numeric range checks in PDFPilot; arbitrary Acrobat JavaScript rules remain unsupported. |
| Set calculated form fields. | Partial | Sum/product/average dependency calculations with cycle detection and transactional rollback; calculated fields become read only. Rules run in PDFPilot; supported Acrobat calculation actions are also written, without executing arbitrary imported scripts. |
| Adjust tab order. | Available | Selected-page row tab order; annotations sorted top-to-bottom, then left-to-right. |
| Configure form field properties. | Partial | Required/read-only flags, accessible tooltip, max length, numeric limits and supported calculations; geometry and arbitrary scripts remain unsupported. |
| Export form data. | Available | JSON field-name/value map. |
| Import data into supported PDF forms. | Available | Transactional JSON import; unknown/unsupported fields are rejected. |
| Create reusable form templates. | Partial | Save blank authored forms as reusable PDFs; no template library. |
| Create web forms with supported Acrobat Pro capabilities. | Partial | Browser review/signature-request forms in the independent service; general web-form builder remains unsupported. |

## 9. Electronic and digital signatures

| Requested feature | Status | Scope / limitation |
| --- | --- | --- |
| Add typed signatures. | Partial | Use Add text; no signature-specific typed-signature style selector. |
| Draw handwritten signatures. | Available | Visual strokes; no cryptographic certificate. |
| Insert saved signature images. | Available | Use Place an image; no identity validation. |
| Save signatures for reuse. | Partial | Save an image file locally and place it again; no managed signature library. |
| Sign PDFs electronically. | Partial | Visual marks only; no identity verification or cryptographic signing. |
| Request signatures from other people. | Partial | Independent service creates distinct private recipient links; no automatic email delivery or identity verification. |
| Specify where recipients must sign. | Partial | Recipient workflow collects typed signatures in a receipt page; arbitrary per-recipient on-page placements remain unsupported. |
| Set signing order for multiple recipients. | Available | Independent service enforces ordered signing, or allows parallel signing. |
| Track signature requests. | Available | Recipient signed/waiting status, persistent events and desktop/browser refresh. |
| Send signature reminders. | Partial | Post reminder events to browser review activity; email/SMS delivery remains unsupported. |
| Receive notifications when documents are signed. | Partial | Desktop status/activity and browser status refresh while running; no OS push/email notifications. |
| Maintain audit trails for supported e-signature workflows. | Partial | Document SHA-256, timestamps, recipient events and chained event hashes; no independently anchored/qualified audit trail or real-world identity verification. |
| Validate certificate-based digital signatures. | Partial | OpenSSL detached CMS verification distinguishes byte integrity, selected-root trust and whole-current-file coverage. No revocation/DocMDP/LTV analysis. |
| Apply certificate-based digital signatures. | Available | Detached SHA-256 CMS signatures using PEM certificate/private key and optional key password; refuses encrypted or already signed PDFs. |
| Use timestamping with compatible digital-signature configurations. | Pending | Not implemented. |
| Send signature requests to multiple recipients using eligible plans. | Partial | Independent service supports up to 100 named recipients and unique token links; no hosted plan or automatic message delivery. |
| Create reusable e-signature templates. | Partial | Save PDFs with unsigned signature fields; recipient workflow templates/library remain unsupported. |

## 10. Document security and protection

| Requested feature | Status | Scope / limitation |
| --- | --- | --- |
| Password-protect PDFs. | Available | Separate AES-256 copy; no permissions editor. |
| Require a password to open a document. | Available | Password required on encrypted-copy opening. |
| Restrict document editing. | Available | AES-256 owner-password permissions; PDFPilot blocks restricted content edits for reader-password sessions. |
| Restrict printing. | Available | Owner-password print permissions; PDFPilot checks permission before printer submission. |
| Restrict copying text and images. | Available | Copy-permission flags; PDFPilot checks exports and extracted-text controls. Accessibility reading remains allowed. |
| Configure document permissions. | Available | Print/edit/copy/annotate/fill/assemble switches and separate open/owner passwords. |
| Encrypt PDFs. | Available | AES-256 output; existing encryption preserved by regular save. |
| Remove authorized password protection. | Available | Writes a separate unprotected copy only after owner-password authentication. |
| Inspect document security settings. | Available | Security handler/revision, metadata encryption and permission inspector. |
| Apply certificate-based protection in supported workflows. | Pending | Not implemented. |
| Verify digital signatures. | Partial | Detached CMS integrity/trust checks for saved PDFs; revocation/timestamps/LTV remain unsupported. |
| Inspect signature validity. | Partial | Separate integrity, optional selected-root trust, and current-file coverage results; no full signature-policy validation. |
| Remove sensitive document metadata using sanitization tools. | Partial | Remove Info and document/page Metadata; no exhaustive sanitization scanner. |
| Permanently redact selected content using Acrobat Pro. | Partial | Secure raster reconstruction removes selected pixels and all original objects; vector editing is lost. |
| Search for sensitive terms before redaction. | Partial | Search for matching pages, then manually mark a region; no automatic redaction search. |
| Remove hidden information through supported cleanup workflows. | Partial | Raster redaction reconstruction discards original objects/hidden text; no independent selective-cleanup wizard. |

## 11. PDF comparison and inspection

| Requested feature | Status | Scope / limitation |
| --- | --- | --- |
| Compare two versions of a PDF. | Partial | Page-by-position comparison; inserted pages can shift later comparisons. |
| Identify text changes. | Partial | Page-level text changes, without word-level edit alignment. |
| Identify image changes. | Partial | Rendered pixel differences; no image-object inventory comparison. |
| Identify formatting and layout changes. | Partial | Rendered pixel changes capture visual layout differences, without semantic classification. |
| Highlight differences visually. | Available | Red pixel mask, originals side-by-side in self-contained HTML. |
| Navigate between detected differences. | Partial | Scroll the exported HTML report; no in-app next/previous difference controls. |
| Review an automatically generated comparison report. | Available | In-app summary and downloadable HTML report. |
| Compare revised contracts and technical documents. | Partial | Uses the same general comparison workflow; no document-specific legal/technical interpretation. |
| Inspect PDF properties and embedded resources. | Available | Metadata, geometry, fonts, encoded stream storage, links and embedded attachments. |
| Analyze document compatibility using supported preflight tools. | Partial | Conservative structural/accessibility/resource diagnostics; no PDF/A, PDF/X or PDF/UA certification. |

## 12. File optimization and compression

| Requested feature | Status | Scope / limitation |
| --- | --- | --- |
| Reduce PDF file size. | Partial | Stream compression and pruning; output is not guaranteed to be smaller. |
| Compress PDFs for sharing. | Partial | Eligible stream compression; no image downsampling. |
| Optimize embedded images. | Partial | Flattened JPEG raster-copy optimization at configurable page width/quality; original image objects are not selectively re-encoded. |
| Adjust image compression and resolution. | Partial | Raster-copy page width (300-4000 pixels) and JPEG quality; destroys editable structure in output. |
| Remove unnecessary document objects through optimization tools. | Available | Prunes unreachable objects. |
| Optimize PDFs for web distribution. | Partial | Stream compression only; no linearization. |
| Save optimized copies. | Available | Implemented. |
| Inspect PDF storage usage with advanced tools. | Available | Encoded stream bytes grouped by object subtype/type; object count and font inventory. |
| Reduce oversized scanned-document files. | Partial | Configurable JPEG raster copies plus stream compression; output size is not guaranteed. |
| Configure advanced PDF optimization settings. | Partial | Raster resolution, JPEG quality and scan enhancements; selective object/image/font optimization remains unsupported. |

## 13. Accessibility features

| Requested feature | Status | Scope / limitation |
| --- | --- | --- |
| Add and edit PDF tags for assistive technologies. | Partial | Manual whole-page content groups and existing element role/alt-text editing; complex structure-tree remediation remains unsupported. |
| Review logical reading order. | Partial | Lists structure elements and associated pages; arbitrary reading-order rearrangement remains unsupported. |
| Configure document language. | Available | Writes the document Lang attribute. |
| Add alternative text to images. | Partial | Alternative text editor for Figure structure elements; automatic image-to-tag association remains unsupported. |
| Identify headings, paragraphs, lists, and tables. | Partial | Manually assign P/H1-H3/Figure/Sect/Span roles; automatic semantics and full list/table structures remain unsupported. |
| Check document accessibility. | Partial | Checks missing title/language/structure, figure alt text and field tooltips; no full PDF/UA validator. |
| Review accessibility errors. | Partial | Exportable diagnostics report; semantic correctness still requires manual review. |
| Improve keyboard navigation. | Partial | Native egui keyboard controls and shortcuts; complete navigation auditing remains pending. |
| Create accessible PDF forms. | Partial | Supported AcroForm widgets have names/appearances; no PDF/UA compliance or reading-order validation. |
| Work with screen readers. | Partial | Native UI accessibility bridge enabled; full screen-reader and PDF-tag behavior remains unverified. |
| Export accessible text. | Partial | UTF-8 extracted text; semantic structure/reading order is not guaranteed. |
| Apply accessibility corrections using advanced tools. | Partial | Title/language, field tooltip and basic tag role/alt-text corrections; full remediation remains unsupported. |

## 14. Engineering, architecture, and technical document tools

| Requested feature | Status | Scope / limitation |
| --- | --- | --- |
| Measure distances on PDF drawings. | Available | Distance in millimeters; configured drawing scale. |
| Measure perimeters. | Partial | Rectangular perimeter from a dragged bounding box; arbitrary polygon perimeter remains unsupported. |
| Measure areas. | Partial | Rectangular area from a dragged bounding box; arbitrary polygon area remains unsupported. |
| Configure drawing measurement scales. | Available | Manual scale factor. |
| Calibrate measurements against known dimensions. | Available | Drag a known distance and enter its real length in millimeters to set the drawing scale. |
| Add measurement annotations. | Available | Persist distance/rectangle measurements as Line/Square annotations with measurement dictionaries and unit/scale readouts. |
| Review engineering drawings. | Partial | 2D PDF viewing and annotation; no CAD layer controls. |
| Add revision comments and markups. | Available | Implemented. |
| Compare revisions of technical specifications. | Partial | General text/pixel comparison. |
| Work with large-format PDF pages. | Partial | Geometry-aware rendering at bounded raster width; no tiled large-page rendering. |
| Inspect embedded fonts and graphical elements. | Partial | Font-object and image-placement inventory with storage diagnostics; nested graphics inspection remains limited. |
| View supported embedded 3D PDF models. | Pending | Not implemented. |
| Rotate, zoom, and inspect supported 3D models. | Pending | Not implemented. |
| View model trees in supported 3D PDFs. | Pending | Not implemented. |
| Review technical documents using shared annotation workflows. | Partial | Local annotation and save workflows; no synchronized shared review. |

## 15. Printing and prepress

| Requested feature | Status | Scope / limitation |
| --- | --- | --- |
| Configure print scaling. | Partial | CUPS fit-to-page and 10-400 percent scaling; physical printer output remains unverified. |
| Print selected page ranges. | Partial | Validated page ranges sent to CUPS. |
| Print multiple pages per sheet. | Partial | CUPS n-up submission. |
| Print booklet layouts. | Partial | Duplex-order, padded two-up raster booklet sheets with short-edge duplex CUPS submission; physical output unverified. |
| Choose grayscale or color output. | Partial | CUPS monochrome/color option. |
| Configure paper sizes and orientation. | Partial | Letter, A4 and Legal plus portrait/landscape; printer support and physical output unverified. |
| Preview documents before printing. | Partial | Export/open imposed booklet-sheet preview; general n-up printer preview remains unsupported. |
| Work with printing marks and advanced output settings. | Partial | Booklet fold marks, paper/scaling/duplex settings; crop/registration/color bars and advanced prepress remain unsupported. |
| Inspect color separations with supported print-production tools. | Pending | Not implemented. |
| Convert document colors for production workflows. | Pending | Not implemented. |
| Inspect PDF/X compliance. | Pending | Not implemented. |
| Run preflight profiles. | Partial | One conservative diagnostics profile; no standards-compliance profiles or certification. |
| Embed or inspect fonts. | Partial | Font-object and embedding-descriptor inspection; font embedding/substitution remains unsupported. |
| Prepare documents for professional printing. | Pending | Not implemented. |

## 16. Automation and batch processing

| Requested feature | Status | Scope / limitation |
| --- | --- | --- |
| Apply operations to multiple PDFs through Action Wizard. | Available | Reorderable sequences: compression, metadata cleanup, watermark/header/footer/page numbers/language; optional batch protection. |
| Create reusable action sequences. | Available | Versioned JSON presets; operations can be added, removed and reordered. |
| Batch watermark documents. | Available | Implemented. |
| Batch add headers and footers. | Available | Reusable header/footer text steps. |
| Batch perform document optimization. | Partial | Batch stream compression/pruning; no advanced image optimizer. |
| Batch run supported security operations. | Available | AES-256 protection with separate passwords and permission flags; passwords are excluded from presets. |
| Process multiple files with predefined settings. | Available | Saved action presets, staged outputs and collision refusal. |
| Automate certain document preparation workflows. | Available | Reusable ordered local preparation actions, with all files prepared before outputs are published. |
| Use JavaScript for supported Acrobat PDF automation. | Pending | Not implemented. |
| Configure supported interactive PDF actions and form scripts. | Partial | Supported calculations emit JavaScript Calculate actions and calculation order; arbitrary scripts/actions remain unsupported. |
| Generate standardized document output. | Available | Reusable header/footer/watermark/page-number/language/compression presets. |


## Remaining implementation work

1. Arbitrary existing text/vector/nested-image editing, exact paragraph typography and layout-aware Office reconstruction.
2. Full form properties, XFA/multi-select controls, interoperable JavaScript validation/calculations and template libraries.
3. Cancellable jobs, semantic/OCR form detection, OCR confidence correction and robust language-pack management.
4. Signature revocation/DocMDP/LTV, trusted timestamps, hardware keys and multi-signature incremental signing.
5. Identity verification, email/push delivery, recipient placement templates, remote TLS service deployment and operational hardening.
6. Full tagging/reading-order remediation, table/list semantics and PDF/UA/PDF-A/PDF-X validation.
7. Color separations/conversion, embedded fonts, selective image optimization, linearization and large-page tiling.
8. Scanner/mobile/virtual-printer/Office add-in integrations and embedded 3D support.

The independent review service is functional locally. It is not deployed publicly, and link possession does not verify a signer's real-world identity. No placeholder controls are counted as working features.
