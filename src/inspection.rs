//! Conservative diagnostics. These checks do not certify PDF/A, PDF/X or PDF/UA.
use crate::pdf::{self, PdfDocument};
use anyhow::{Result, ensure};
use lopdf::{Object, ObjectId, dictionary};
use std::collections::BTreeMap;

pub fn report(doc: &PdfDocument) -> String {
    let mut lines = vec![
        format!(
            "{} pages / PDF {} / {} objects",
            doc.page_count(),
            doc.doc.version,
            doc.doc.objects.len()
        ),
        crate::security::settings(&doc.doc),
    ];
    let mut usage = BTreeMap::<String, usize>::new();
    let mut fonts = vec![];
    for (id, obj) in &doc.doc.objects {
        let dict = obj
            .as_dict()
            .ok()
            .or_else(|| obj.as_stream().ok().map(|s| &s.dict));
        let name = dict
            .and_then(|d| d.get(b"Subtype").or_else(|_| d.get(b"Type")).ok())
            .map(pdf::object_text)
            .unwrap_or_else(|| "Other".into());
        let bytes = obj.as_stream().map(|s| s.content.len()).unwrap_or(0);
        *usage.entry(name).or_default() += bytes;
        if let Some(d) =
            dict.filter(|d| d.get(b"Type").map(pdf::object_text).unwrap_or_default() == "Font")
        {
            let embedded = doc
                .doc
                .get_dict_in_dict(d, b"FontDescriptor")
                .is_ok_and(|d| {
                    [b"FontFile".as_slice(), b"FontFile2", b"FontFile3"]
                        .iter()
                        .any(|k| d.get(k).is_ok())
                });
            fonts.push(format!(
                "{}.{} {} ({})",
                id.0,
                id.1,
                d.get(b"BaseFont").map(pdf::object_text).unwrap_or_default(),
                if embedded {
                    "embedded"
                } else {
                    "not embedded / descendant font may carry program"
                }
            ));
        }
    }
    lines.push("\nStream storage (encoded bytes):".into());
    for (kind, bytes) in usage {
        lines.push(format!("{kind}: {bytes}"));
    }
    lines.push("\nFont objects:".into());
    lines.extend(fonts);
    lines.push("\nAccessibility / preflight findings:".into());
    if doc.doc.catalog().is_ok_and(|d| d.get(b"Lang").is_err()) {
        lines.push("Document language is missing".into());
    }
    if doc.metadata(b"Title").trim().is_empty() {
        lines.push("Document title is missing".into());
    }
    if doc
        .doc
        .catalog()
        .is_ok_and(|d| d.get(b"StructTreeRoot").is_err())
    {
        lines.push("Document has no structure tree".into());
    }
    if doc
        .doc
        .catalog()
        .is_ok_and(|d| d.get(b"OutputIntents").is_err())
    {
        lines.push("No production output intent".into());
    }
    for (number, id) in doc.doc.get_pages() {
        if let Err(e) = pdf::geometry(&doc.doc, id) {
            lines.push(format!("Page {number}: invalid page geometry: {e}"));
        }
    }
    for field in doc.fields() {
        if doc.doc.get_dictionary(field.id).is_ok_and(|d| {
            d.get(b"TU")
                .map(pdf::object_text)
                .unwrap_or_default()
                .trim()
                .is_empty()
        }) {
            lines.push(format!("Field {}: missing descriptive tooltip", field.name));
        }
    }
    for (id, obj) in &doc.doc.objects {
        if let Ok(d) = obj.as_dict() {
            if d.get(b"S").map(pdf::object_text).unwrap_or_default() == "JavaScript" {
                lines.push(format!("Object {}: JavaScript action present", id.0));
            }
            if d.get(b"Type").map(pdf::object_text).unwrap_or_default() == "StructElem"
                && d.get(b"S").map(pdf::object_text).unwrap_or_default() == "Figure"
                && d.get(b"Alt")
                    .map(pdf::object_text)
                    .unwrap_or_default()
                    .is_empty()
            {
                lines.push(format!("Figure {}: alternative text missing", id.0));
            }
        }
    }
    lines.push("\nDiagnostics only. Standards conformance, semantic reading order and rendering compatibility require a dedicated validator and manual review.".into());
    lines.join("\n")
}
#[derive(Clone, Debug)]
pub struct Tag {
    pub id: ObjectId,
    pub role: String,
    pub alternative: String,
    pub page: Option<u32>,
}
impl PdfDocument {
    pub fn tags(&self) -> Vec<Tag> {
        self.doc
            .objects
            .iter()
            .filter_map(|(id, o)| {
                let d = o.as_dict().ok()?;
                if d.get(b"Type").map(pdf::object_text).unwrap_or_default() != "StructElem" {
                    return None;
                }
                let page = d
                    .get(b"Pg")
                    .and_then(Object::as_reference)
                    .ok()
                    .and_then(|p| {
                        self.doc
                            .get_pages()
                            .iter()
                            .find_map(|(n, id)| (*id == p).then_some(*n))
                    });
                Some(Tag {
                    id: *id,
                    role: d.get(b"S").map(pdf::object_text).unwrap_or_default(),
                    alternative: d.get(b"Alt").map(pdf::object_text).unwrap_or_default(),
                    page,
                })
            })
            .collect()
    }
    pub fn edit_tag(&mut self, id: ObjectId, role: &str, alt: &str) -> Result<()> {
        ensure!(
            ["P", "H1", "H2", "H3", "Figure", "Sect", "Span"].contains(&role),
            "Unsupported structural role"
        );
        ensure!(
            self.tags().iter().any(|t| t.id == id),
            "Unknown structure element"
        );
        self.edit(|doc| {
            let d = doc.get_dictionary_mut(id)?;
            d.set("S", Object::Name(role.as_bytes().to_vec()));
            d.set("Alt", pdf::pdf_string(alt));
            Ok(())
        })
    }
    /// Tag a whole, currently untagged page as one manually chosen content group.
    /// Existing tagged content is refused rather than guessed or overwritten.
    pub fn tag_page(&mut self, page: u32, role: &str, alt: &str) -> Result<()> {
        ensure!(
            ["P", "H1", "H2", "H3", "Figure", "Sect", "Span"].contains(&role),
            "Unsupported structural role"
        );
        let id = self.page_id(page)?;
        ensure!(
            self.doc.get_dictionary(id)?.get(b"StructParents").is_err(),
            "This page is already tagged"
        );
        ensure!(
            self.doc.catalog()?.get(b"StructTreeRoot").is_err()
                || self.doc.catalog()?.get(b"PilotTagTree").is_ok(),
            "Existing structure trees require element editing rather than page auto-tagging"
        );
        self.edit(|doc|{
            let root=match doc.catalog()?.get(b"StructTreeRoot").and_then(Object::as_reference){Ok(id)=>id,Err(_)=>{let root=doc.add_object(dictionary!{"Type"=>"StructTreeRoot","K"=>Vec::<Object>::new(),"ParentTree"=>dictionary!{"Nums"=>Vec::<Object>::new()}});doc.catalog_mut()?.set("StructTreeRoot",root);doc.catalog_mut()?.set("PilotTagTree",true);root}};
            let key=doc.get_dictionary(root)?.get(b"ParentTreeNextKey").and_then(Object::as_i64).unwrap_or(0);
            let tag=doc.add_object(dictionary!{"Type"=>"StructElem","S"=>Object::Name(role.as_bytes().to_vec()),"P"=>root,"Pg"=>id,"K"=>0,"Alt"=>pdf::pdf_string(alt)});
            let mut content=format!("/{role} <</MCID 0>> BDC\n").into_bytes();content.extend(doc.get_page_content_with_limit(id,pdf::TEXT_LIMIT)?);content.extend(b"\nEMC");doc.change_page_content(id,content)?;doc.get_dictionary_mut(id)?.set("StructParents",key);
            let tree=doc.get_dictionary_mut(root)?;let mut kids=tree.get(b"K")?.as_array()?.clone();kids.push(tag.into());tree.set("K",kids);let parent=tree.get_mut(b"ParentTree")?.as_dict_mut()?;let nums=parent.get_mut(b"Nums")?.as_array_mut()?;nums.extend([key.into(),Object::Array(vec![tag.into()])]);tree.set("ParentTreeNextKey",key+1);
            doc.catalog_mut()?.set("MarkInfo",dictionary!{"Marked"=>true});Ok(())
        })
    }
}
