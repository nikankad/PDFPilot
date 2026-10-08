use anyhow::{Context, Result, ensure};
use lopdf::{
    Dictionary, Document, Object, ObjectId, Stream,
    content::{Content, Operation},
    dictionary,
};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct TextStyle {
    pub font: String,
    pub size: f32,
    pub color: [f32; 3],
}
impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font: "Helvetica".into(),
            size: 14.,
            color: [0.08, 0.12, 0.2],
        }
    }
}

pub const HISTORY_LIMIT: usize = 20;
pub const TEXT_LIMIT: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub struct Geometry {
    pub bounds: [f32; 4],
    pub rotation: i64,
}

impl Geometry {
    pub fn width(self) -> f32 {
        self.bounds[2] - self.bounds[0]
    }
    pub fn height(self) -> f32 {
        self.bounds[3] - self.bounds[1]
    }
    pub fn display_size(self) -> [f32; 2] {
        if self.rotation % 180 == 0 {
            [self.width(), self.height()]
        } else {
            [self.height(), self.width()]
        }
    }
    /// Coordinates are normalized against the displayed, cropped, rotated page.
    pub fn point(self, p: [f32; 2]) -> [f32; 2] {
        let [u, v] = p;
        let [x, y] = match self.rotation {
            90 => [v, u],
            180 => [1.0 - u, v],
            270 => [1.0 - v, 1.0 - u],
            _ => [u, 1.0 - v],
        };
        [
            self.bounds[0] + x * self.width(),
            self.bounds[1] + y * self.height(),
        ]
    }
    pub fn rect(self, a: [f32; 2], b: [f32; 2]) -> [f32; 4] {
        let a = self.point(a);
        let b = self.point(b);
        [
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[0].max(b[0]),
            a[1].max(b[1]),
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Highlight,
    Underline,
    Rectangle,
    Note,
}

#[derive(Clone, Debug)]
pub struct Field {
    pub id: ObjectId,
    pub name: String,
    pub value: String,
    pub kind: String,
    pub editable: bool,
    pub options: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Comment {
    pub id: ObjectId,
    pub page: u32,
    pub kind: String,
    pub text: String,
}

pub struct PdfDocument {
    pub doc: Document,
    pub path: Option<PathBuf>,
    pub title: String,
    pub revision: u64,
    pub dirty: bool,
    undo: Vec<Document>,
    redo: Vec<Document>,
    owner_access: bool,
}

impl PdfDocument {
    pub fn open(path: &Path, password: &str) -> Result<Self> {
        let mut doc = Document::load_with_password(path, password).map_err(|error| {
            if matches!(
                error,
                lopdf::Error::Decryption(_) | lopdf::Error::InvalidPassword
            ) {
                anyhow::anyhow!("Password required or incorrect password: {error}")
            } else {
                anyhow::anyhow!("Could not read this PDF: {error}")
            }
        })?;
        if doc.is_encrypted() {
            doc.decrypt(password)
                .context("Password required or incorrect password")?;
        }
        ensure!(!doc.get_pages().is_empty(), "This PDF contains no pages");
        let owner_access = if let Some(state) = doc.encryption_state.clone() {
            let mut copy = doc.clone();
            copy.encrypt(&state)?;
            copy.authenticate_owner_password(password).is_ok()
        } else {
            true
        };
        Ok(Self {
            doc,
            path: Some(path.to_owned()),
            title: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            revision: 0,
            dirty: false,
            undo: vec![],
            redo: vec![],
            owner_access,
        })
    }

    pub fn from_document(doc: Document, title: &str) -> Self {
        Self {
            doc,
            path: None,
            title: title.into(),
            revision: 0,
            dirty: true,
            undo: vec![],
            redo: vec![],
            owner_access: true,
        }
    }

    pub fn page_count(&self) -> u32 {
        self.doc.get_pages().len() as u32
    }
    pub fn page_id(&self, page: u32) -> Result<ObjectId> {
        self.doc
            .get_pages()
            .get(&page)
            .copied()
            .context("Page is out of range")
    }
    pub fn geometry(&self, page: u32) -> Result<Geometry> {
        geometry(&self.doc, self.page_id(page)?)
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Edits commit only on success, preserving the original on any error.
    pub fn permits(&self, permission: lopdf::Permissions) -> bool {
        self.owner_access
            || self
                .doc
                .encryption_state
                .as_ref()
                .is_none_or(|s| s.permissions().intersects(permission))
    }
    pub fn owner_authenticated(&self) -> bool {
        self.owner_access
    }
    pub fn edit(&mut self, change: impl FnOnce(&mut Document) -> Result<()>) -> Result<()> {
        self.edit_with_permissions(lopdf::Permissions::MODIFIABLE, change)
    }
    pub(crate) fn edit_with_permissions(
        &mut self,
        permission: lopdf::Permissions,
        change: impl FnOnce(&mut Document) -> Result<()>,
    ) -> Result<()> {
        ensure!(
            self.permits(permission | lopdf::Permissions::MODIFIABLE),
            "Document permissions restrict this operation. Reopen with the owner password."
        );
        let mut next = self.doc.clone();
        change(&mut next)?;
        self.undo.push(std::mem::replace(&mut self.doc, next));
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.changed();
        Ok(())
    }
    fn changed(&mut self) {
        self.revision += 1;
        self.dirty = true;
    }
    pub fn undo(&mut self) {
        if let Some(doc) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.doc, doc));
            self.changed();
        }
    }
    pub fn redo(&mut self) {
        if let Some(doc) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.doc, doc));
            self.changed();
        }
    }

    pub fn save(&mut self, path: &Path) -> Result<()> {
        self.save_copy(path)?;
        self.path = Some(path.to_owned());
        self.title = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        self.dirty = false;
        Ok(())
    }

    pub fn save_copy(&self, path: &Path) -> Result<()> {
        let mut copy = self.doc.clone();
        if let Some(state) = copy.encryption_state.clone() {
            copy.encrypt(&state)?;
        }
        atomic_save(&copy, path)
    }

    pub fn text(&self, page: u32) -> Result<String> {
        self.doc
            .extract_text_with_limit(&[page], TEXT_LIMIT)
            .context("Could not extract text from this page")
    }

    pub fn replace_text(&mut self, page: u32, search: &str, replacement: &str) -> Result<()> {
        ensure!(!search.is_empty(), "Enter the text to replace");
        self.page_id(page)?;
        latin_text(replacement)?;
        self.edit(|doc| {
            let before = doc.extract_text_with_limit(&[page], TEXT_LIMIT)?;
            ensure!(before.contains(search), "Text was not found on this page");
            let count = doc.replace_partial_text(page, search, replacement, None)?;
            ensure!(
                count > 0,
                "This text spans separate PDF text runs and cannot be replaced here"
            );
            let after = doc.extract_text_with_limit(&[page], TEXT_LIMIT)?;
            ensure!(
                after != before && (replacement.is_empty() || after.contains(replacement)),
                "The existing font cannot represent the replacement text"
            );
            Ok(())
        })
    }

    pub fn rotate(&mut self, page: u32, degrees: i64) -> Result<()> {
        let id = self.page_id(page)?;
        let rotation = self.geometry(page)?.rotation;
        self.edit_with_permissions(lopdf::Permissions::ASSEMBLABLE, |doc| {
            doc.get_dictionary_mut(id)?
                .set("Rotate", (rotation + degrees).rem_euclid(360));
            Ok(())
        })
    }

    pub fn delete(&mut self, pages: &[u32]) -> Result<()> {
        ensure!(!pages.is_empty(), "Select at least one page");
        ensure!(
            pages.iter().all(|p| *p > 0 && *p <= self.page_count()),
            "Page is out of range"
        );
        let distinct: HashSet<_> = pages.iter().collect();
        ensure!(
            distinct.len() < self.page_count() as usize,
            "A PDF must have at least one page"
        );
        self.edit_with_permissions(lopdf::Permissions::ASSEMBLABLE, |doc| {
            doc.delete_pages(pages);
            doc.prune_objects();
            Ok(())
        })
    }

    pub fn move_page(&mut self, from: u32, to: u32) -> Result<()> {
        self.page_id(from)?;
        self.page_id(to)?;
        self.edit_with_permissions(lopdf::Permissions::ASSEMBLABLE, |doc| {
            let mut order: Vec<_> = doc.get_pages().values().copied().collect();
            let id = order.remove((from - 1) as usize);
            order.insert((to - 1) as usize, id);
            rebuild_pages(doc, &order)
        })
    }

    pub fn extract(&self, pages: &[u32], path: &Path) -> Result<()> {
        ensure!(
            self.permits(lopdf::Permissions::COPYABLE),
            "Document permissions restrict page extraction"
        );
        ensure!(!pages.is_empty(), "Select at least one page");
        let mut doc = self.doc.clone();
        let all = doc.get_pages();
        let order: Vec<_> = pages
            .iter()
            .map(|n| all.get(n).copied().context("Page is out of range"))
            .collect::<Result<_>>()?;
        ensure!(
            pages.iter().collect::<HashSet<_>>().len() == pages.len(),
            "Duplicate pages are not supported"
        );
        // Subset exports discard navigation/forms that could reference excluded pages.
        doc.catalog_mut()?.remove(b"Outlines");
        doc.catalog_mut()?.remove(b"AcroForm");
        doc.catalog_mut()?.remove(b"Names");
        rebuild_pages(&mut doc, &order)?;
        doc.prune_objects();
        atomic_save(&doc, path)
    }

    pub fn merge(&mut self, path: &Path) -> Result<()> {
        let incoming = PdfDocument::open(path, "")?;
        ensure!(
            incoming.permits(lopdf::Permissions::COPYABLE),
            "Incoming document permissions restrict page extraction"
        );
        self.edit_with_permissions(lopdf::Permissions::ASSEMBLABLE, |doc| {
            let mut incoming = incoming.doc;
            incoming.renumber_objects_with(doc.max_id + 1);
            let mut order: Vec<_> = doc.get_pages().values().copied().collect();
            let additions: Vec<_> = incoming.get_pages().values().copied().collect();
            materialize_inheritance(&mut incoming, &additions)?;
            ensure!(
                incoming.catalog()?.get(b"AcroForm").is_err(),
                "Merge PDFs with forms after flattening them in their originating application"
            );
            doc.max_id = doc.max_id.max(incoming.max_id);
            doc.objects.extend(incoming.objects);
            order.extend(additions);
            rebuild_pages(doc, &order)
        })
    }

    pub fn insert_blank(&mut self, after: u32) -> Result<()> {
        ensure!(after <= self.page_count(), "Page is out of range");
        self.edit_with_permissions(lopdf::Permissions::ASSEMBLABLE, |doc| {
            let mut order: Vec<_> = doc.get_pages().values().copied().collect();
            let id = doc.add_object(dictionary! {"Type" => "Page", "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()], "Resources" => Dictionary::new()});
            order.insert(after as usize, id); rebuild_pages(doc, &order)
        })
    }

    pub fn add_mark(
        &mut self,
        page: u32,
        mark: Mark,
        a: [f32; 2],
        b: [f32; 2],
        text: &str,
    ) -> Result<()> {
        let id = self.page_id(page)?;
        let geo = self.geometry(page)?;
        let rect = geo.rect(a, b);
        self.edit_with_permissions(lopdf::Permissions::ANNOTABLE, |doc| {
            let subtype = match mark { Mark::Highlight => "Highlight", Mark::Underline => "Underline", Mark::Rectangle => "Square", Mark::Note => "Text" };
            let [x, y, right, top] = rect;
            let w = (right - x).max(1.0); let h = (top - y).max(1.0);
            let color: Vec<Object> = if mark == Mark::Highlight { vec![1.0.into(), 0.82.into(), 0.16.into()] } else { vec![0.15.into(), 0.38.into(), 0.92.into()] };
            let mut annot = dictionary! {"Type" => "Annot", "Subtype" => subtype, "Rect" => numbers(&rect), "P" => id, "F" => 4, "C" => color, "Contents" => pdf_string(text), "T" => pdf_string("PDFPilot")};
            if mark == Mark::Highlight || mark == Mark::Underline {
                let lo = [a[0].min(b[0]), a[1].min(b[1])];
                let hi = [a[0].max(b[0]), a[1].max(b[1])];
                let points = [geo.point(lo), geo.point([hi[0], lo[1]]), geo.point([lo[0], hi[1]]), geo.point(hi)];
                annot.set("QuadPoints", numbers(&points.concat()));
            }
            let appearance = match mark {
                Mark::Highlight => format!("q /GS gs 1 .82 .16 rg 0 0 {w} {h} re f Q"),
                Mark::Rectangle => format!("q .15 .38 .92 RG 1.5 w .75 .75 {} {} re S Q", (w - 1.5).max(0.0), (h - 1.5).max(0.0)),
                Mark::Underline => {
                    let start = geo.point([a[0].min(b[0]), a[1].max(b[1])]);
                    let end = geo.point([a[0].max(b[0]), a[1].max(b[1])]);
                    format!("q .15 .38 .92 RG 1.5 w {} {} m {} {} l S Q", start[0]-x, start[1]-y, end[0]-x, end[1]-y)
                },
                Mark::Note => format!("q 1 .82 .16 rg 0 0 {w} {h} re f .5 .4 0 RG 1 w 0 0 {w} {h} re S Q"),
            };
            let ap = doc.add_object(Stream::new(dictionary! {
                "Type" => "XObject", "Subtype" => "Form", "BBox" => numbers(&[0.0, 0.0, w, h]),
                "Resources" => dictionary! {"ExtGState" => dictionary! {"GS" => dictionary! {"Type" => "ExtGState", "ca" => 0.32, "BM" => "Multiply"}}},
            }, appearance.into_bytes()));
            annot.set("AP", dictionary! {"N" => ap});
            let annot_id = doc.add_object(annot);
            let existing = doc.get_dictionary(id)?.get(b"Annots").ok().and_then(|o| doc.dereference(o).ok()).and_then(|(_, o)| o.as_array().ok()).cloned().unwrap_or_default();
            let mut annots = existing; annots.push(annot_id.into());
            doc.get_dictionary_mut(id)?.set("Annots", annots);
            Ok(())
        })
    }

    pub fn add_text(&mut self, page: u32, position: [f32; 2], text: &str, size: f32) -> Result<()> {
        self.add_styled_text(
            page,
            position,
            text,
            &TextStyle {
                size,
                ..Default::default()
            },
        )
    }
    pub fn add_styled_text(
        &mut self,
        page: u32,
        position: [f32; 2],
        text: &str,
        style: &TextStyle,
    ) -> Result<()> {
        let id = self.page_id(page)?;
        let geo = self.geometry(page)?;
        self.edit(|doc| write_text(doc, id, geo, position, text, style))
    }

    pub fn add_ink(&mut self, page: u32, points: &[[f32; 2]], width: f32) -> Result<()> {
        ensure!(
            points.len() > 1 && points.len() <= 100_000,
            "Draw a stroke first"
        );
        ensure!(
            width.is_finite()
                && (0.1..=100.).contains(&width)
                && points
                    .iter()
                    .flatten()
                    .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
            "Invalid ink stroke"
        );
        let id = self.page_id(page)?;
        let geo = self.geometry(page)?;
        let points = points.iter().map(|p| geo.point(*p)).collect::<Vec<_>>();
        let margin = width.max(1.);
        let rect = [
            points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min) - margin,
            points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min) - margin,
            points
                .iter()
                .map(|p| p[0])
                .fold(f32::NEG_INFINITY, f32::max)
                + margin,
            points
                .iter()
                .map(|p| p[1])
                .fold(f32::NEG_INFINITY, f32::max)
                + margin,
        ];
        self.edit_with_permissions(lopdf::Permissions::ANNOTABLE, |doc|{let mut ops=vec![Operation::new("q",vec![]),Operation::new("RG",numbers(&[0.08,0.15,0.3])),Operation::new("w",vec![width.into()]),Operation::new("J",vec![1.into()]),Operation::new("j",vec![1.into()]),Operation::new("m",numbers(&[points[0][0]-rect[0],points[0][1]-rect[1]]))];for p in &points[1..]{ops.push(Operation::new("l",numbers(&[p[0]-rect[0],p[1]-rect[1]])));}ops.extend([Operation::new("S",vec![]),Operation::new("Q",vec![])]);
            let ap=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>numbers(&[0.,0.,rect[2]-rect[0],rect[3]-rect[1]])},Content{operations:ops}.encode()?));
            let annot=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Ink","P"=>id,"F"=>4,"Rect"=>numbers(&rect),"C"=>numbers(&[0.08,0.15,0.3]),"BS"=>dictionary!{"W"=>width},"InkList"=>vec![Object::Array(points.iter().flat_map(|p|p.iter().copied()).map(Object::Real).collect())],"AP"=>dictionary!{"N"=>ap},"Contents"=>pdf_string("Handwritten stroke")});
            let mut annotations=doc.get_dictionary(id)?.get(b"Annots").ok().and_then(|o|doc.dereference(o).ok()).and_then(|(_,o)|o.as_array().ok()).cloned().unwrap_or_default();annotations.push(annot.into());doc.get_dictionary_mut(id)?.set("Annots",annotations);Ok(())
        })
    }

    pub fn comments(&self) -> Vec<Comment> {
        let mut result = vec![];
        for (page, id) in self.doc.get_pages() {
            if let Ok(annots) = self.doc.get_page_annotations(id) {
                for annot in annots {
                    let kind = annot
                        .get(b"Subtype")
                        .ok()
                        .and_then(|o| o.as_name().ok())
                        .map(|s| String::from_utf8_lossy(s).into_owned())
                        .unwrap_or_default();
                    if kind == "Widget" || kind == "Link" {
                        continue;
                    }
                    let text = annot.get(b"Contents").map(object_text).unwrap_or_default();
                    // Annotation IDs are resolved from the page's array for removal.
                    let annot_id = self
                        .doc
                        .get_dictionary(id)
                        .ok()
                        .and_then(|d| d.get(b"Annots").ok())
                        .and_then(|o| self.doc.dereference(o).ok())
                        .and_then(|(_, o)| o.as_array().ok())
                        .and_then(|a| {
                            a.iter()
                                .filter_map(|o| o.as_reference().ok())
                                .find(|id| self.doc.get_dictionary(*id).ok() == Some(annot))
                        });
                    if let Some(id) = annot_id {
                        result.push(Comment {
                            id,
                            page,
                            kind,
                            text,
                        });
                    }
                }
            }
        }
        result
    }

    pub fn remove_comment(&mut self, comment: &Comment) -> Result<()> {
        let page = self.page_id(comment.page)?;
        let target = comment.id;
        self.edit_with_permissions(lopdf::Permissions::ANNOTABLE, |doc| {
            let mut annotations = doc
                .get_dictionary(page)?
                .get(b"Annots")
                .ok()
                .and_then(|o| doc.dereference(o).ok())
                .and_then(|(_, o)| o.as_array().ok())
                .cloned()
                .unwrap_or_default();
            annotations.retain(|o| o.as_reference().ok() != Some(target));
            doc.get_dictionary_mut(page)?.set("Annots", annotations);
            doc.prune_objects();
            Ok(())
        })
    }

    pub fn fields(&self) -> Vec<Field> {
        let mut result = vec![];
        if let Ok(form) = self
            .doc
            .catalog()
            .and_then(|c| self.doc.get_dict_in_dict(c, b"AcroForm"))
            && let Ok(fields) = form.get(b"Fields").and_then(Object::as_array)
        {
            let mut visited = HashSet::new();
            for field in fields {
                walk_fields(&self.doc, field, "", "", 0, &mut visited, &mut result);
            }
        }
        result
    }

    pub fn set_field(&mut self, field: &Field, value: &str) -> Result<()> {
        ensure!(field.editable, "This field type cannot be edited here");
        if field.kind == "Ch" && !field.options.is_empty() {
            ensure!(
                value.is_empty() || field.options.iter().any(|x| x == value),
                "Choose a listed dropdown option"
            );
        }
        if field.kind == "Radio" {
            ensure!(
                value == "Off" || field.options.iter().any(|x| x == value),
                "Choose a listed radio option"
            );
        }
        if field.kind == "Btn" {
            ensure!(
                matches!(value, "Yes" | "Off"),
                "Checkbox value must be Yes or Off"
            );
        }
        crate::forms::validate_value(&self.doc, field, value)?;
        self.edit_with_permissions(
            lopdf::Permissions::FILLABLE | lopdf::Permissions::ANNOTABLE,
            |doc| {
                set_field_value(doc, field, value)?;
                crate::forms::recalculate(doc)
            },
        )
    }

    pub fn set_metadata(&mut self, title: &str, author: &str, subject: &str) -> Result<()> {
        self.edit(|doc| {
            let mut info = doc
                .trailer
                .get(b"Info")
                .ok()
                .and_then(|o| doc.dereference(o).ok())
                .and_then(|(_, o)| o.as_dict().ok())
                .cloned()
                .unwrap_or_default();
            info.set("Title", pdf_string(title));
            info.set("Author", pdf_string(author));
            info.set("Subject", pdf_string(subject));
            info.set("Producer", pdf_string("PDFPilot"));
            let id = doc.add_object(info);
            doc.trailer.set("Info", id);
            Ok(())
        })
    }
    pub fn metadata(&self, key: &[u8]) -> String {
        self.doc
            .trailer
            .get(b"Info")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| o.as_dict().ok())
            .and_then(|d| d.get(key).ok())
            .map(object_text)
            .unwrap_or_default()
    }

    pub fn compress(&mut self) -> Result<()> {
        self.edit(|doc| {
            doc.prune_objects();
            doc.compress();
            Ok(())
        })
    }
}

pub(crate) fn write_text(
    doc: &mut Document,
    id: ObjectId,
    geo: Geometry,
    position: [f32; 2],
    text: &str,
    style: &TextStyle,
) -> Result<()> {
    ensure!(!text.trim().is_empty(), "Enter some text first");
    ensure!(
        [
            "Helvetica",
            "Helvetica-Bold",
            "Helvetica-Oblique",
            "Times-Roman",
            "Courier"
        ]
        .contains(&style.font.as_str()),
        "Unsupported font"
    );
    ensure!(
        style.size.is_finite() && style.size > 0. && style.size <= 200.,
        "Invalid font size"
    );
    ensure!(
        style
            .color
            .iter()
            .all(|x| x.is_finite() && (0.0..=1.0).contains(x)),
        "Invalid text color"
    );
    ensure!(
        position
            .iter()
            .all(|x| x.is_finite() && (0.0..=1.0).contains(x)),
        "Invalid text position"
    );
    let [x, y] = geo.point(position);
    materialize_inheritance(doc, &[id])?;
    let font_id = doc.add_object(dictionary! {"Type" => "Font", "Subtype" => "Type1", "BaseFont" => style.font.as_str(), "Encoding" => "WinAnsiEncoding"});
    let mut resources = doc
        .get_dictionary(id)?
        .get(b"Resources")
        .ok()
        .and_then(|o| doc.dereference(o).ok())
        .and_then(|(_, o)| o.as_dict().ok())
        .cloned()
        .unwrap_or_default();
    let mut fonts = resources
        .get(b"Font")
        .ok()
        .and_then(|o| doc.dereference(o).ok())
        .and_then(|(_, o)| o.as_dict().ok())
        .cloned()
        .unwrap_or_default();
    let font_name = format!("PilotFont{}", font_id.0);
    fonts.set(font_name.as_bytes(), font_id);
    resources.set("Font", fonts);
    doc.get_dictionary_mut(id)?.set("Resources", resources);
    let matrix = match geo.rotation {
        90 => [0., 1., -1., 0., x, y],
        180 => [-1., 0., 0., -1., x, y],
        270 => [0., -1., 1., 0., x, y],
        _ => [1., 0., 0., 1., x, y],
    };
    let content = Content {
        operations: vec![
            Operation::new("q", vec![]),
            Operation::new("BT", vec![]),
            Operation::new(
                "Tf",
                vec![Object::Name(font_name.into_bytes()), style.size.into()],
            ),
            Operation::new("rg", numbers(&style.color)),
            Operation::new("Tm", numbers(&matrix)),
            Operation::new(
                "Tj",
                vec![Object::String(
                    latin_text(text)?,
                    lopdf::StringFormat::Literal,
                )],
            ),
            Operation::new("ET", vec![]),
            Operation::new("Q", vec![]),
        ],
    };
    append_isolated(doc, id, content.encode()?)
}

pub fn atomic_save(doc: &Document, path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .context("Could not create a temporary file in the output folder")?;
    let mut copy = doc.clone();
    copy.save_to(temp.as_file_mut())?;
    temp.as_file().sync_all()?;
    temp.persist(path)
        .map_err(|e| e.error)
        .context("Could not save PDF")?;
    Ok(())
}

pub fn parse_pages(input: &str, count: u32) -> Result<Vec<u32>> {
    let mut result = vec![];
    for part in input.split(',') {
        let part = part.trim();
        ensure!(!part.is_empty(), "Use page numbers such as 1, 3-5");
        let bounds: Vec<_> = part.split('-').map(str::trim).collect();
        ensure!(bounds.len() <= 2, "Invalid page range");
        let start: u32 = bounds[0].parse().context("Invalid page number")?;
        let end: u32 = if bounds.len() == 2 {
            bounds[1].parse().context("Invalid page number")?
        } else {
            start
        };
        ensure!(
            start > 0 && end >= start && end <= count,
            "Page range must be between 1 and {count}"
        );
        for n in start..=end {
            if !result.contains(&n) {
                result.push(n);
            }
        }
    }
    Ok(result)
}

fn inherited(doc: &Document, id: ObjectId, key: &[u8]) -> Option<Object> {
    let mut current = id;
    let mut seen = HashSet::new();
    while seen.insert(current) {
        let dict = doc.get_dictionary(current).ok()?;
        if let Ok(value) = dict.get(key) {
            return Some(value.clone());
        }
        current = dict.get(b"Parent").ok()?.as_reference().ok()?;
    }
    None
}

pub(crate) fn materialize_inheritance(doc: &mut Document, pages: &[ObjectId]) -> Result<()> {
    for id in pages {
        for key in [b"Resources".as_slice(), b"MediaBox", b"CropBox", b"Rotate"] {
            if let Some(value) = inherited(doc, *id, key) {
                doc.get_dictionary_mut(*id)?.set(key, value);
            }
        }
    }
    Ok(())
}

pub(crate) fn rebuild_pages(doc: &mut Document, order: &[ObjectId]) -> Result<()> {
    materialize_inheritance(doc, order)?;
    let root=doc.add_object(dictionary!{"Type"=>"Pages","Count"=>order.len() as i64,"Kids"=>order.iter().map(|id|Object::Reference(*id)).collect::<Vec<_>>()});
    for id in order {
        doc.get_dictionary_mut(*id)?.set("Parent", root);
    }
    doc.catalog_mut()?.set("Pages", root);
    Ok(())
}

fn array4(object: &Object) -> Result<[f32; 4]> {
    let values = object.as_array()?;
    ensure!(values.len() == 4, "Invalid page rectangle");
    Ok([
        values[0].as_float()?,
        values[1].as_float()?,
        values[2].as_float()?,
        values[3].as_float()?,
    ])
}

pub fn geometry(doc: &Document, page: ObjectId) -> Result<Geometry> {
    let media = inherited(doc, page, b"MediaBox").context("Page has no MediaBox")?;
    let (_, media) = doc.dereference(&media)?;
    let media = array4(media)?;
    let crop = inherited(doc, page, b"CropBox")
        .and_then(|o| doc.dereference(&o).ok().and_then(|(_, o)| array4(o).ok()))
        .unwrap_or(media);
    let bounds = [
        media[0].max(crop[0]),
        media[1].max(crop[1]),
        media[2].min(crop[2]),
        media[3].min(crop[3]),
    ];
    ensure!(
        bounds.iter().all(|n| n.is_finite()) && bounds[2] > bounds[0] && bounds[3] > bounds[1],
        "Invalid page dimensions"
    );
    let rotation = inherited(doc, page, b"Rotate")
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(0)
        .rem_euclid(360);
    ensure!(rotation % 90 == 0, "Unsupported page rotation");
    Ok(Geometry { bounds, rotation })
}

pub(crate) fn append_isolated(doc: &mut Document, page: ObjectId, bytes: Vec<u8>) -> Result<()> {
    // Isolate previous content so its graphics state cannot affect new content.
    let before = doc.add_object(Stream::new(Dictionary::new(), b"q\n".to_vec()));
    let after = doc.add_object(Stream::new(Dictionary::new(), b"\nQ\n".to_vec()));
    let added = doc.add_object(Stream::new(Dictionary::new(), bytes));
    let mut contents = vec![Object::Reference(before)];
    contents.extend(
        doc.get_page_contents(page)
            .into_iter()
            .map(Object::Reference),
    );
    contents.extend([Object::Reference(after), Object::Reference(added)]);
    doc.get_dictionary_mut(page)?.set("Contents", contents);
    Ok(())
}

fn numbers(values: &[f32]) -> Vec<Object> {
    values.iter().map(|n| Object::Real(*n)).collect()
}

pub fn pdf_string(text: &str) -> Object {
    let mut bytes = vec![0xfe, 0xff];
    for unit in text.encode_utf16() {
        bytes.extend(unit.to_be_bytes());
    }
    Object::String(bytes, lopdf::StringFormat::Hexadecimal)
}

pub fn object_text(object: &Object) -> String {
    if let Ok(bytes) = object.as_str() {
        if bytes.starts_with(&[0xfe, 0xff]) {
            return String::from_utf16_lossy(
                &bytes[2..]
                    .chunks_exact(2)
                    .map(|p| u16::from_be_bytes([p[0], p[1]]))
                    .collect::<Vec<_>>(),
            );
        }
        return lopdf::decode_text_string(object)
            .unwrap_or_else(|_| String::from_utf8_lossy(bytes).into_owned());
    }
    if let Ok(name) = object.as_name() {
        return String::from_utf8_lossy(name).into_owned();
    }
    String::new()
}

pub(crate) fn latin_text(text: &str) -> Result<Vec<u8>> {
    let mut bytes = vec![];
    for ch in text.chars() {
        ensure!(
            ch == ' ' || ('!'..='~').contains(&ch) || ('\u{00a0}'..='\u{00ff}').contains(&ch),
            "Added text currently supports single-line Western Latin characters"
        );
        bytes.push(ch as u8);
    }
    Ok(bytes)
}

fn walk_fields(
    doc: &Document,
    object: &Object,
    parent: &str,
    inherited_kind: &str,
    inherited_flags: i64,
    seen: &mut HashSet<ObjectId>,
    out: &mut Vec<Field>,
) {
    let Ok(id) = object.as_reference() else {
        return;
    };
    if !seen.insert(id) {
        return;
    }
    let Ok(dict) = doc.get_dictionary(id) else {
        return;
    };
    let name = dict.get(b"T").map(object_text).unwrap_or_default();
    let name = if parent.is_empty() {
        name
    } else if name.is_empty() {
        parent.into()
    } else {
        format!("{parent}.{name}")
    };
    let kind = dict
        .get(b"FT")
        .map(object_text)
        .unwrap_or_else(|_| inherited_kind.into());
    let flags = dict
        .get(b"Ff")
        .ok()
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(inherited_flags);
    let kids = dict.get(b"Kids").ok().and_then(|o| o.as_array().ok());
    let field_children = kids
        .map(|kids| {
            kids.iter()
                .filter(|o| {
                    o.as_reference()
                        .ok()
                        .and_then(|id| doc.get_dictionary(id).ok())
                        .is_some_and(|d| d.get(b"T").is_ok())
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !field_children.is_empty() {
        for child in field_children {
            walk_fields(doc, child, &name, &kind, flags, seen, out);
        }
    } else if !kind.is_empty() && !name.is_empty() {
        let mut value = dict.get(b"V").map(object_text).unwrap_or_default();
        if kind == "Btn" && flags & (1 << 15) == 0 {
            value = if value.is_empty() || value == "Off" {
                "Off"
            } else {
                "Yes"
            }
            .into();
        }
        let editable = flags & 1 == 0
            && (kind == "Tx" || kind == "Ch" || (kind == "Btn" && flags & (1 << 16) == 0));
        let options = dict
            .get(b"Opt")
            .ok()
            .and_then(|o| o.as_array().ok())
            .map(|values| {
                values
                    .iter()
                    .map(|o| {
                        if let Ok(pair) = o.as_array() {
                            pair.first().map(object_text).unwrap_or_default()
                        } else {
                            object_text(o)
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        let (kind, options) = if kind == "Btn" && flags & (1 << 15) != 0 {
            let widgets = kids
                .map(|a| a.to_vec())
                .unwrap_or_else(|| vec![Object::Reference(id)]);
            let mut states = vec![];
            for widget in widgets {
                if let Some(normal) = widget
                    .as_reference()
                    .ok()
                    .and_then(|i| doc.get_dictionary(i).ok())
                    .and_then(|d| doc.get_dict_in_dict(d, b"AP").ok())
                    .and_then(|d| doc.get_dict_in_dict(d, b"N").ok())
                {
                    for (state, _) in normal {
                        if state != b"Off"
                            && !states.contains(&String::from_utf8_lossy(state).to_string())
                        {
                            states.push(String::from_utf8_lossy(state).to_string());
                        }
                    }
                }
            }
            ("Radio".to_string(), states)
        } else {
            (kind, options)
        };
        out.push(Field {
            id,
            name,
            value,
            kind,
            editable,
            options,
        });
    }
}

pub fn create_text_document(title: &str, body: &str) -> Result<Document> {
    let mut doc = Document::with_version("1.7");
    let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica","Encoding"=>"WinAnsiEncoding"});
    let bold=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica-Bold","Encoding"=>"WinAnsiEncoding"});
    let resources = doc.add_object(dictionary! {"Font"=>dictionary!{"Body"=>font,"Heading"=>bold}});
    let mut lines = vec![];
    for line in body.lines() {
        if line.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        for word in line.split_whitespace() {
            if current.len() + word.len() + 1 > 82 && !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            if !current.is_empty() {
                current.push(' ');
            }
            for character in word.chars() {
                if current.chars().count() == 82 {
                    lines.push(std::mem::take(&mut current));
                }
                current.push(character);
            }
        }
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    let mut order = vec![];
    for (index, chunk) in lines.chunks(34).enumerate() {
        let mut ops = vec![
            Operation::new("rg", numbers(&[0.13, 0.32, 0.85])),
            Operation::new("re", numbers(&[48., 735., 34., 4.])),
            Operation::new("f", vec![]),
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec![Object::Name(b"Heading".to_vec()), 24.into()]),
            Operation::new("rg", numbers(&[0.08, 0.12, 0.2])),
            Operation::new("Td", numbers(&[48., 693.])),
            Operation::new("Tj", vec![Object::string_literal(latin_text(title)?)]),
            Operation::new("ET", vec![]),
        ];
        ops.extend([
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec![Object::Name(b"Body".to_vec()), 11.into()]),
            Operation::new("TL", vec![17.into()]),
            Operation::new("rg", numbers(&[0.2, 0.24, 0.32])),
            Operation::new("Td", numbers(&[48., 644.])),
        ]);
        for line in chunk {
            ops.push(Operation::new(
                "Tj",
                vec![Object::string_literal(latin_text(line)?)],
            ));
            ops.push(Operation::new("T*", vec![]));
        }
        ops.push(Operation::new("ET", vec![]));
        ops.extend([
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec![Object::Name(b"Body".to_vec()), 9.into()]),
            Operation::new("Td", numbers(&[48., 35.])),
            Operation::new(
                "Tj",
                vec![Object::string_literal(format!(
                    "PDFPILOT    /    {}",
                    index + 1
                ))],
            ),
            Operation::new("ET", vec![]),
        ]);
        let contents = doc.add_object(Stream::new(
            Dictionary::new(),
            Content { operations: ops }.encode()?,
        ));
        order.push(doc.add_object(dictionary!{"Type"=>"Page","MediaBox"=>vec![0.into(),0.into(),612.into(),792.into()],"Resources"=>resources,"Contents"=>contents}));
    }
    let catalog = doc.add_object(dictionary! {"Type"=>"Catalog"});
    doc.trailer.set("Root", catalog);
    rebuild_pages(&mut doc, &order)?;
    let info =
        doc.add_object(dictionary! {"Title"=>pdf_string(title),"Producer"=>pdf_string("PDFPilot")});
    doc.trailer.set("Info", info);
    Ok(doc)
}

pub fn images_to_document(paths: &[PathBuf]) -> Result<Document> {
    ensure!(!paths.is_empty(), "Select at least one image");
    let mut doc = Document::with_version("1.7");
    let mut pages = vec![];
    for path in paths {
        let img = image::open(path).context("Could not read image")?.to_rgb8();
        let (width, height) = img.dimensions();
        ensure!(width > 0 && height > 0, "Empty image");
        let mut jpeg = vec![];
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 92)
            .encode_image(&image::DynamicImage::ImageRgb8(img))?;
        let image=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>width as i64,"Height"=>height as i64,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8,"Filter"=>"DCTDecode"},jpeg));
        let scale = (540. / width as f32).min(720. / height as f32);
        let w = width as f32 * scale;
        let h = height as f32 * scale;
        let contents = doc.add_object(Stream::new(
            Dictionary::new(),
            format!(
                "q {w} 0 0 {h} {} {} cm /Photo Do Q",
                (612. - w) / 2.,
                (792. - h) / 2.
            )
            .into_bytes(),
        ));
        pages.push(doc.add_object(dictionary!{"Type"=>"Page","MediaBox"=>numbers(&[0.,0.,612.,792.]),"Resources"=>dictionary!{"XObject"=>dictionary!{"Photo"=>image}},"Contents"=>contents}));
    }
    let catalog = doc.add_object(dictionary! {"Type"=>"Catalog"});
    doc.trailer.set("Root", catalog);
    rebuild_pages(&mut doc, &pages)?;
    Ok(doc)
}

pub fn demo_document() -> Result<Document> {
    create_text_document(
        "A better space for your PDFs",
        "PDFPilot / Your document workspace\n\nEverything you need, close at hand.\n\nRead clearly\nBrowse page thumbnails, jump between pages, zoom in, and search your document. Open multiple PDFs and switch between them without losing your place.\n\nMake your mark\nHighlight passages, underline details, add comments, and draw directly on the page. Use Add text to fill a line, or draw a handwritten signature with Ink.\n\nKeep pages in order\nRotate, reorder, remove, extract, and combine pages. Create a PDF from text or images, then save a copy when you are ready.\n\nWork with forms\nFill supported text fields and checkboxes. Changes are saved inside the PDF so they travel with your document.\n\nTry it now\nChoose Highlight from the right panel, then drag across this paragraph. Your annotation will appear in Comments and in the saved PDF. Undo is always one shortcut away.\n\nBuilt in Rust. Designed for focus.\n\nThis sample is editable. Your original files are only overwritten when you choose Save.",
    )
}

pub fn protected_copy(doc: &Document, password: &str, path: &Path) -> Result<()> {
    use lopdf::encryption::crypt_filters::{Aes256CryptFilter, CryptFilter};
    use lopdf::{EncryptionState, EncryptionVersion, Permissions};
    use rand::Rng;
    use std::{collections::BTreeMap, sync::Arc};
    ensure!(!password.is_empty(), "Enter a password");
    let mut key = [0u8; 32];
    rand::rng().fill(&mut key);
    let mut filters: BTreeMap<Vec<u8>, Arc<dyn CryptFilter>> = BTreeMap::new();
    filters.insert(b"StdCF".to_vec(), Arc::new(Aes256CryptFilter));
    let state = EncryptionState::try_from(EncryptionVersion::V5 {
        encrypt_metadata: true,
        crypt_filters: filters,
        file_encryption_key: &key,
        stream_filter: b"StdCF".to_vec(),
        string_filter: b"StdCF".to_vec(),
        owner_password: password,
        user_password: password,
        permissions: Permissions::all(),
    })?;
    let mut copy = doc.clone();
    copy.version = "2.0".into();
    copy.encrypt(&state)?;
    atomic_save(&copy, path)
}

pub(crate) fn set_field_value(doc: &mut Document, field: &Field, value: &str) -> Result<()> {
    let id = field.id;
    let kind = field.kind.as_str();
    let children = doc
        .get_dictionary(id)?
        .get(b"Kids")
        .ok()
        .and_then(|o| o.as_array().ok())
        .map(|a| {
            a.iter()
                .filter_map(|o| o.as_reference().ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![id]);
    if kind == "Btn" || kind == "Radio" {
        let on = value != "Off";
        let state = if kind == "Radio" {
            value.as_bytes().to_vec()
        } else if on {
            children
                .iter()
                .find_map(|wid| {
                    doc.get_dictionary(*wid)
                        .ok()
                        .and_then(|d| doc.get_dict_in_dict(d, b"AP").ok())
                        .and_then(|d| doc.get_dict_in_dict(d, b"N").ok())
                        .and_then(|d| {
                            d.iter()
                                .find(|(k, _)| k.as_slice() != b"Off")
                                .map(|(k, _)| k.clone())
                        })
                })
                .context("This checkbox has no checked appearance")?
        } else {
            b"Off".to_vec()
        };
        doc.get_dictionary_mut(id)?
            .set("V", Object::Name(state.clone()));
        for wid in children {
            let valid = doc
                .get_dictionary(wid)
                .ok()
                .and_then(|d| doc.get_dict_in_dict(d, b"AP").ok())
                .and_then(|d| doc.get_dict_in_dict(d, b"N").ok())
                .is_some_and(|d| d.get(&state).is_ok());
            doc.get_dictionary_mut(wid)?.set(
                "AS",
                Object::Name(if valid {
                    state.clone()
                } else {
                    b"Off".to_vec()
                }),
            );
        }
    } else {
        let bytes = latin_text(value)?;
        doc.get_dictionary_mut(id)?.set("V", pdf_string(value));
        for wid in children {
            let rect = doc
                .get_dictionary(wid)?
                .get(b"Rect")
                .ok()
                .and_then(|o| array4(o).ok());
            if let Some([x, y, r, t]) = rect {
                let w = r - x;
                let h = t - y;
                let size = (h - 5.).clamp(6., 12.);
                let font = doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica","Encoding"=>"WinAnsiEncoding"});
                let ops = vec![
                    Operation::new("q", vec![]),
                    Operation::new("re", numbers(&[0., 0., w, h])),
                    Operation::new("W", vec![]),
                    Operation::new("n", vec![]),
                    Operation::new("BT", vec![]),
                    Operation::new("Tf", vec![Object::Name(b"F".to_vec()), size.into()]),
                    Operation::new("Td", numbers(&[3., (h - size) / 2. + 2.])),
                    Operation::new(
                        "Tj",
                        vec![Object::String(bytes.clone(), lopdf::StringFormat::Literal)],
                    ),
                    Operation::new("ET", vec![]),
                    Operation::new("Q", vec![]),
                ];
                let ap=doc.add_object(Stream::new(dictionary! {"Type"=>"XObject","Subtype"=>"Form","BBox"=>numbers(&[0.,0.,w,h]),"Resources"=>dictionary!{"Font"=>dictionary!{"F"=>font}}},Content{operations:ops}.encode()?));
                doc.get_dictionary_mut(wid)?
                    .set("AP", dictionary! {"N"=>ap});
            }
        }
    }
    Ok(())
}
