use crate::pdf::{self, PdfDocument, object_text, pdf_string};
use anyhow::{Context, Result, ensure};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, dictionary};
use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
};

#[derive(Clone, Debug)]
pub struct Outline {
    pub id: ObjectId,
    pub title: String,
    pub page: Option<u32>,
    pub depth: usize,
}
#[derive(Clone, Debug)]
pub struct Attachment {
    pub id: ObjectId,
    pub name: String,
    pub size: Option<i64>,
}
#[derive(Clone, Debug)]
pub struct Link {
    pub id: ObjectId,
    pub page: u32,
    pub uri: String,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FormKind {
    Text,
    Checkbox,
    Dropdown,
    Radio,
    Listbox,
    Date,
    Signature,
}
impl FormKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Text => "Text field",
            Self::Checkbox => "Checkbox",
            Self::Dropdown => "Dropdown",
            Self::Radio => "Radio group",
            Self::Listbox => "List box",
            Self::Date => "Date (YYYY-MM-DD)",
            Self::Signature => "Signature field",
        }
    }
}
pub struct FormSpec {
    pub kind: FormKind,
    pub name: String,
    pub required: bool,
    pub options: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Strikeout,
    Line,
    Arrow,
    Circle,
    Stamp,
}

impl PdfDocument {
    pub fn add_image(&mut self, page: u32, a: [f32; 2], b: [f32; 2], path: &Path) -> Result<()> {
        let id = self.page_id(page)?;
        let geo = self.geometry(page)?;
        let display = geo.display_size();
        let lo = [a[0].min(b[0]), a[1].min(b[1])];
        let hi = [a[0].max(b[0]), a[1].max(b[1])];
        let w = (hi[0] - lo[0]) * display[0];
        let h = (hi[1] - lo[1]) * display[1];
        ensure!(w > 2. && h > 2., "Draw a larger image area");
        let mut img = image::open(path)?.to_rgba8();
        for pixel in img.pixels_mut() {
            let alpha = pixel[3] as f32 / 255.;
            for channel in &mut pixel.0[..3] {
                *channel = (*channel as f32 * alpha + 255. * (1. - alpha)).round() as u8;
            }
            pixel[3] = 255;
        }
        let rgb = image::DynamicImage::ImageRgba8(img).into_rgb8();
        let dimensions = rgb.dimensions();
        let mut jpeg = vec![];
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 92)
            .encode_image(&image::DynamicImage::ImageRgb8(rgb))?;
        let [x, y] = geo.point([lo[0], hi[1]]);
        self.edit(|doc| {
            pdf::materialize_inheritance(doc,&[id])?;
            let image=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>dimensions.0 as i64,"Height"=>dimensions.1 as i64,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8,"Filter"=>"DCTDecode"},jpeg));
            let name=format!("PilotImage{}",image.0);doc.add_xobject(id,name.as_bytes(),image)?;
            let matrix=match geo.rotation {90=>[0.,w,-h,0.,x,y],180=>[-w,0.,0.,-h,x,y],270=>[0.,-w,h,0.,x,y],_=>[w,0.,0.,h,x,y]};
            let bytes=format!("q {} {} {} {} {} {} cm /{name} Do Q",matrix[0],matrix[1],matrix[2],matrix[3],matrix[4],matrix[5]).into_bytes();
            pdf::append_isolated(doc,id,bytes)
        })
    }
    pub fn add_running_text(
        &mut self,
        text: &str,
        position: [f32; 2],
        style: &pdf::TextStyle,
        numbered: bool,
        start: u32,
        padding: usize,
    ) -> Result<()> {
        self.edit(|doc| {
            for (index, id) in doc.get_pages() {
                let line = if numbered {
                    format!("{text}{:0padding$}", start + index - 1)
                } else {
                    text.into()
                };
                pdf::write_text(doc, id, pdf::geometry(doc, id)?, position, &line, style)?;
            }
            Ok(())
        })
    }
    pub fn duplicate_page(&mut self, page: u32) -> Result<()> {
        let id = self.page_id(page)?;
        ensure!(
            !has_widgets(&self.doc, id),
            "Pages with form widgets cannot be duplicated here"
        );
        self.edit_with_permissions(lopdf::Permissions::ASSEMBLABLE, |doc| {
            let mut incoming = doc.clone();
            incoming.renumber_objects_with(doc.max_id + 1);
            let duplicated = *incoming
                .get_pages()
                .get(&page)
                .context("Page is out of range")?;
            pdf::materialize_inheritance(&mut incoming, &[duplicated])?;
            let mut order: Vec<_> = doc.get_pages().values().copied().collect();
            doc.max_id = incoming.max_id;
            doc.objects.extend(incoming.objects);
            order.insert(page as usize, duplicated);
            pdf::rebuild_pages(doc, &order)?;
            doc.prune_objects();
            Ok(())
        })
    }

    pub fn crop_page(&mut self, page: u32, a: [f32; 2], b: [f32; 2]) -> Result<()> {
        let id = self.page_id(page)?;
        let rect = self.geometry(page)?.rect(a, b);
        ensure!(
            rect[2] - rect[0] >= 5. && rect[3] - rect[1] >= 5.,
            "Crop area must be at least 5 PDF points wide and tall"
        );
        self.edit(|doc| {
            doc.get_dictionary_mut(id)?.set("CropBox", nums(&rect));
            Ok(())
        })
    }
    pub fn reset_crop(&mut self, page: u32) -> Result<()> {
        let id = self.page_id(page)?;
        self.edit(|doc| {
            pdf::materialize_inheritance(doc, &[id])?;
            let media = doc.get_dictionary(id)?.get(b"MediaBox")?.clone();
            doc.get_dictionary_mut(id)?.set("CropBox", media);
            Ok(())
        })
    }
    pub fn rotate_range(&mut self, pages: &[u32], degrees: i64) -> Result<()> {
        let rotations = pages
            .iter()
            .map(|p| Ok((self.page_id(*p)?, self.geometry(*p)?.rotation)))
            .collect::<Result<Vec<_>>>()?;
        self.edit_with_permissions(lopdf::Permissions::ASSEMBLABLE, |doc| {
            for (id, rotation) in rotations {
                doc.get_dictionary_mut(id)?
                    .set("Rotate", (rotation + degrees).rem_euclid(360));
            }
            Ok(())
        })
    }
    pub fn split_to_folder(&self, size: u32, folder: &Path) -> Result<usize> {
        ensure!(size > 0, "Pages per file must be positive");
        let stem = self
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .unwrap_or_default()
            .to_string_lossy();
        let stem = if stem.is_empty() { "document" } else { &stem };
        let pages: Vec<_> = (1..=self.page_count()).collect();
        let destinations = pages
            .chunks(size as usize)
            .enumerate()
            .map(|(i, chunk)| {
                (
                    folder.join(format!("{stem}-part-{:03}.pdf", i + 1)),
                    chunk.to_vec(),
                )
            })
            .collect::<Vec<_>>();
        ensure!(
            destinations.iter().all(|(path, _)| !path.exists()),
            "A split output already exists. Choose an empty folder"
        );
        for (path, chunk) in &destinations {
            self.extract(chunk, path)?;
        }
        Ok(destinations.len())
    }
    pub fn replace_page(&mut self, page: u32, source: &Path, source_page: u32) -> Result<()> {
        let old = self.page_id(page)?;
        ensure!(
            !has_widgets(&self.doc, old),
            "Remove form widgets before replacing this page"
        );
        let incoming = PdfDocument::open(source, "")?;
        ensure!(
            !has_widgets(&incoming.doc, incoming.page_id(source_page)?),
            "The replacement page contains form widgets"
        );
        self.edit_with_permissions(lopdf::Permissions::ASSEMBLABLE, |doc| {
            let mut incoming = incoming.doc;
            incoming.renumber_objects_with(doc.max_id + 1);
            let new = *incoming
                .get_pages()
                .get(&source_page)
                .context("Replacement page is out of range")?;
            pdf::materialize_inheritance(&mut incoming, &[new])?;
            let mut order: Vec<_> = doc.get_pages().values().copied().collect();
            order[(page - 1) as usize] = new;
            doc.max_id = incoming.max_id;
            doc.objects.extend(incoming.objects);
            pdf::rebuild_pages(doc, &order)?;
            doc.prune_objects();
            Ok(())
        })
    }

    pub fn outlines(&self) -> Vec<Outline> {
        let mut out = vec![];
        let pages = self.doc.get_pages();
        if let Ok(root) = self
            .doc
            .catalog()
            .and_then(|c| self.doc.get_dict_in_dict(c, b"Outlines"))
            && let Ok(first) = root.get(b"First").and_then(Object::as_reference)
        {
            walk_outlines(&self.doc, first, 0, &pages, &mut HashSet::new(), &mut out);
        }
        out
    }
    pub fn add_outline(&mut self, page: u32, title: &str) -> Result<()> {
        let page = self.page_id(page)?;
        ensure!(!title.trim().is_empty(), "Enter a bookmark title");
        self.edit_with_permissions(lopdf::Permissions::ASSEMBLABLE, |doc| {
            let root=doc.catalog()?.get(b"Outlines").ok().and_then(|o|o.as_reference().ok()).unwrap_or_else(||doc.add_object(dictionary!{"Type"=>"Outlines","Count"=>0}));
            let last=doc.get_dictionary(root)?.get(b"Last").ok().and_then(|o|o.as_reference().ok());
            let mut item=dictionary!{"Title"=>pdf_string(title),"Parent"=>root,"Dest"=>vec![Object::Reference(page),Object::Name(b"Fit".to_vec())]};
            if let Some(last)=last {item.set("Prev",last);}
            let id=doc.add_object(item);
            if let Some(last)=last {doc.get_dictionary_mut(last)?.set("Next",id);}else {doc.get_dictionary_mut(root)?.set("First",id);}
            let count=doc.get_dictionary(root)?.get(b"Count").ok().and_then(|o|o.as_i64().ok()).unwrap_or(0).abs()+1;
            let rootdict=doc.get_dictionary_mut(root)?;rootdict.set("Last",id);rootdict.set("Count",count);
            doc.catalog_mut()?.set("Outlines",root);Ok(())
        })
    }
    pub fn page_labels(&self) -> Vec<String> {
        let mut labels = (1..=self.page_count())
            .map(|n| n.to_string())
            .collect::<Vec<_>>();
        if let Ok(tree) = self
            .doc
            .catalog()
            .and_then(|c| self.doc.get_dict_in_dict(c, b"PageLabels"))
        {
            let mut ranges = vec![];
            walk_number_tree(&self.doc, tree, &mut HashSet::new(), &mut ranges);
            ranges.sort_by_key(|(index, _)| *index);
            for (position, (start, dict)) in ranges.iter().enumerate() {
                let end = ranges
                    .get(position + 1)
                    .map(|(n, _)| *n as usize)
                    .unwrap_or(labels.len());
                let prefix = dict.get(b"P").map(object_text).unwrap_or_default();
                let number = dict
                    .get(b"St")
                    .ok()
                    .and_then(|o| o.as_i64().ok())
                    .unwrap_or(1);
                let decimal = dict.get(b"S").ok().and_then(|o| o.as_name().ok()) == Some(b"D");
                for (offset, label) in labels
                    .iter_mut()
                    .enumerate()
                    .take(end)
                    .skip(*start as usize)
                {
                    *label = if decimal {
                        format!("{prefix}{}", number + offset as i64 - start)
                    } else if dict.get(b"S").is_err() {
                        prefix.clone()
                    } else {
                        label.clone()
                    };
                }
            }
        }
        labels
    }
    pub fn set_page_labels(&mut self, prefix: &str, start: i64) -> Result<()> {
        ensure!(start > 0, "Starting page label must be positive");
        self.edit_with_permissions(lopdf::Permissions::ASSEMBLABLE, |doc| {let tree=doc.add_object(dictionary!{"Nums"=>vec![0.into(),Object::Dictionary(dictionary!{"S"=>"D","St"=>start,"P"=>pdf_string(prefix)})]});doc.catalog_mut()?.set("PageLabels",tree);Ok(())})
    }

    pub fn links(&self) -> Vec<Link> {
        let mut out = vec![];
        for (page, id) in self.doc.get_pages() {
            for annot in annotation_ids(&self.doc, id) {
                let Ok(dict) = self.doc.get_dictionary(annot) else {
                    continue;
                };
                if dict.get(b"Subtype").ok().and_then(|o| o.as_name().ok()) != Some(b"Link") {
                    continue;
                }
                let uri = self
                    .doc
                    .get_dict_in_dict(dict, b"A")
                    .ok()
                    .and_then(|a| a.get(b"URI").ok())
                    .map(object_text)
                    .unwrap_or_default();
                if uri.starts_with("https://") || uri.starts_with("http://") {
                    out.push(Link {
                        id: annot,
                        page,
                        uri,
                    });
                }
            }
        }
        out
    }
    pub fn add_link(&mut self, page: u32, a: [f32; 2], b: [f32; 2], uri: &str) -> Result<()> {
        ensure!(
            uri.starts_with("https://") || uri.starts_with("http://"),
            "Links must use https:// or http://"
        );
        let id = self.page_id(page)?;
        let rect = self.geometry(page)?.rect(a, b);
        self.edit(|doc| {let annot=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Link","Rect"=>nums(&rect),"P"=>id,"Border"=>vec![0.into(),0.into(),1.into()],"C"=>nums(&[0.2,0.35,0.85]),"A"=>dictionary!{"S"=>"URI","URI"=>Object::string_literal(uri)},"F"=>4});append_annotation(doc,id,annot)})
    }
    pub fn update_link(&mut self, id: ObjectId, uri: &str) -> Result<()> {
        ensure!(
            uri.starts_with("https://") || uri.starts_with("http://"),
            "Links must use https:// or http://"
        );
        self.edit(|doc| {
            doc.get_dictionary_mut(id)?.set(
                "A",
                dictionary! {"S"=>"URI","URI"=>Object::string_literal(uri)},
            );
            Ok(())
        })
    }

    pub fn attachments(&self) -> Vec<Attachment> {
        let mut out = vec![];
        if let Ok(names) = self
            .doc
            .catalog()
            .and_then(|c| self.doc.get_dict_in_dict(c, b"Names"))
            && let Ok(tree) = self.doc.get_dict_in_dict(names, b"EmbeddedFiles")
        {
            let mut entries = vec![];
            walk_name_tree(&self.doc, tree, &mut HashSet::new(), &mut entries);
            for (name, id) in entries {
                let size = self
                    .doc
                    .get_dictionary(id)
                    .ok()
                    .and_then(|d| self.doc.get_dict_in_dict(d, b"EF").ok())
                    .and_then(|d| d.get(b"F").ok())
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| o.as_stream().ok())
                    .and_then(|s| self.doc.get_dict_in_dict(&s.dict, b"Params").ok())
                    .and_then(|p| p.get(b"Size").ok())
                    .and_then(|o| o.as_i64().ok());
                out.push(Attachment { id, name, size });
            }
        }
        out
    }
    pub fn attach_file(&mut self, path: &Path) -> Result<()> {
        let name = path
            .file_name()
            .context("The attachment has no file name")?
            .to_string_lossy()
            .into_owned();
        let bytes = std::fs::read(path)?;
        ensure!(
            bytes.len() <= pdf::TEXT_LIMIT,
            "Attachments are limited to 64 MiB"
        );
        let mut entries = self
            .attachments()
            .into_iter()
            .map(|a| (a.name, a.id))
            .collect::<Vec<_>>();
        ensure!(
            entries.iter().all(|(n, _)| n != &name),
            "An attachment with this name already exists"
        );
        self.edit(|doc| {
            let mut stream=Stream::new(dictionary!{"Type"=>"EmbeddedFile","Params"=>dictionary!{"Size"=>bytes.len() as i64}},bytes);stream.compress()?;
            let embedded=doc.add_object(stream);
            let spec=doc.add_object(dictionary!{"Type"=>"Filespec","F"=>Object::string_literal(name.as_str()),"UF"=>pdf_string(&name),"EF"=>dictionary!{"F"=>embedded}});
            entries.push((name,spec));set_attachments(doc,entries)
        })
    }
    pub fn export_attachment(&self, attachment: &Attachment, path: &Path) -> Result<()> {
        let spec = self.doc.get_dictionary(attachment.id)?;
        let ef = self.doc.get_dict_in_dict(spec, b"EF")?;
        let (_, object) = self
            .doc
            .dereference(ef.get(b"F").or_else(|_| ef.get(b"UF"))?)?;
        std::fs::write(
            path,
            object
                .as_stream()?
                .decompressed_content_with_limit(pdf::TEXT_LIMIT)?,
        )?;
        Ok(())
    }

    pub fn add_form_field(
        &mut self,
        page: u32,
        a: [f32; 2],
        b: [f32; 2],
        spec: &FormSpec,
    ) -> Result<()> {
        let FormSpec {
            kind,
            name,
            required,
            options,
        } = spec;
        let kind = *kind;
        let required = *required;
        let name = name.as_str();
        ensure!(
            !name.trim().is_empty() && !name.contains('.'),
            "Use a nonempty field name without periods"
        );
        ensure!(
            self.fields().iter().all(|f| f.name != name),
            "Field names must be unique"
        );
        if matches!(
            kind,
            FormKind::Dropdown | FormKind::Listbox | FormKind::Radio
        ) {
            ensure!(
                !options.is_empty() && options.iter().all(|s| !s.trim().is_empty()),
                "Enter nonempty options"
            );
            let unique = options.iter().collect::<HashSet<_>>();
            ensure!(unique.len() == options.len(), "Options must be unique");
        }
        let id = self.page_id(page)?;
        let rect = self.geometry(page)?.rect(a, b);
        let w = rect[2] - rect[0];
        let h = rect[3] - rect[1];
        ensure!(w >= 8. && h >= 8., "Draw a larger form field");
        self.edit(|doc| {
            let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica","Encoding"=>"WinAnsiEncoding"});
            let make_ap=|bytes:Vec<u8>|Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>nums(&[0.,0.,w,h]),"Resources"=>dictionary!{"Font"=>dictionary!{"F"=>font}}},bytes);
            let border=format!("q .94 .96 1 rg 0 0 {w} {h} re f .3 .4 .6 RG 1 w .5 .5 {} {} re S Q",w-1.,h-1.);
            let blank=doc.add_object(make_ap(border.as_bytes().to_vec()));
            let mut field=dictionary!{"Type"=>"Annot","Subtype"=>"Widget","T"=>pdf_string(name),"Rect"=>nums(&rect),"P"=>id,"F"=>4,"Ff"=>if required {2}else {0},"DA"=>Object::string_literal("/F 12 Tf 0 g"),"AP"=>dictionary!{"N"=>blank}};
            match kind {
                FormKind::Text | FormKind::Date=>{field.set("FT","Tx");field.set("V",pdf_string(""));if kind==FormKind::Date {field.set("PilotFormat",pdf_string("Date"));field.set("TU",pdf_string("Date in YYYY-MM-DD format"));}},
                FormKind::Signature=>{field.set("FT","Sig");field.set("TU",pdf_string("Certificate signature"));},
                FormKind::Listbox=>{field.set("FT","Ch");field.set("Opt",options.iter().map(|s|pdf_string(s)).collect::<Vec<_>>());field.set("V",pdf_string(""));},
                FormKind::Radio=>{field.set("FT","Btn");field.set("Ff",(1<<15)|if required {2}else{0});field.set("V","Off");},
                FormKind::Dropdown=>{field.set("FT","Ch");field.set("Ff",(1<<17)|if required {2}else {0});field.set("Opt",options.iter().map(|s|pdf_string(s)).collect::<Vec<_>>());field.set("V",pdf_string(""));},
                FormKind::Checkbox=>{
                    let checked=doc.add_object(make_ap(format!("{border} q .1 .2 .4 RG 2 w 3 {} m {} 3 l {} {} l S Q",h/2.,w/2.,w-3.,h-3.).into_bytes()));
                    field.set("FT","Btn");field.set("V","Off");field.set("AS","Off");field.set("AP",dictionary!{"N"=>dictionary!{"Off"=>blank,"Yes"=>checked}});
                },
            }
            if kind==FormKind::Signature {
                field.set("AP",dictionary!{"N"=>doc.add_object(make_ap(format!("{border} BT /F 10 Tf .3 g 4 {} Td (Sign here) Tj ET",h/2.).into_bytes()))});
            }
            let field=doc.add_object(field);
            if kind==FormKind::Radio {
                let row=h/options.len() as f32; ensure!(row>=12., "Draw a taller area for this radio group");
                let mut children=vec![];
                for (index, option) in options.iter().enumerate() {
                    let bottom=rect[3]-(index+1) as f32*row; let bytes=pdf::latin_text(option)?;
                    let base=format!("q .3 .4 .6 RG 1 w 2 2 {} {} re S Q",(row-4.).min(14.),(row-4.).min(14.));
                    let label=lopdf::content::Content{operations:vec![lopdf::content::Operation::new("BT",vec![]),lopdf::content::Operation::new("Tf",vec![Object::Name(b"F".to_vec()),10.into()]),lopdf::content::Operation::new("Td",vec![22.into(),(row/2.-3.).into()]),lopdf::content::Operation::new("Tj",vec![Object::string_literal(bytes)]),lopdf::content::Operation::new("ET",vec![])]}.encode()?;
                    let mut off=base.as_bytes().to_vec(); off.extend(&label);
                    let mut on=off.clone(); on.extend(format!("q .1 .2 .4 rg 5 5 {} {} re f Q",(row-10.).clamp(3.,8.),(row-10.).clamp(3.,8.)).as_bytes());
                    let make=|bytes|Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>nums(&[0.,0.,w,row]),"Resources"=>dictionary!{"Font"=>dictionary!{"F"=>font}}},bytes);
                    let off=doc.add_object(make(off));let on=doc.add_object(make(on));let mut appearances=dictionary!{"Off"=>off};appearances.set(option.as_bytes(),on);
                    let child=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","Parent"=>field,"P"=>id,"F"=>4,"Rect"=>nums(&[rect[0],bottom,rect[2],bottom+row]),"AS"=>"Off","AP"=>dictionary!{"N"=>appearances}});
                    append_annotation(doc,id,child)?;children.push(Object::Reference(child));
                }
                let parent=doc.get_dictionary_mut(field)?;parent.remove(b"Type");parent.remove(b"Subtype");parent.remove(b"Rect");parent.remove(b"AP");parent.remove(b"P");parent.set("Kids",children);
            } else {append_annotation(doc,id,field)?;}
            let mut form=doc.catalog().ok().and_then(|c|doc.get_dict_in_dict(c,b"AcroForm").ok()).cloned().unwrap_or_default();
            let mut fields=form.get(b"Fields").ok().and_then(|o|doc.dereference(o).ok()).and_then(|(_,o)|o.as_array().ok()).cloned().unwrap_or_default();fields.push(field.into());form.set("Fields",fields);
            let mut resources=form.get(b"DR").ok().and_then(|o|doc.dereference(o).ok()).and_then(|(_,o)|o.as_dict().ok()).cloned().unwrap_or_default();
            let mut fonts=resources.get(b"Font").ok().and_then(|o|doc.dereference(o).ok()).and_then(|(_,o)|o.as_dict().ok()).cloned().unwrap_or_default();fonts.set("F",font);resources.set("Font",fonts);form.set("DR",resources);
            let form=doc.add_object(form);doc.catalog_mut()?.set("AcroForm",form);Ok(())
        })
    }
    pub fn form_data(&self) -> BTreeMap<String, String> {
        self.fields()
            .into_iter()
            .map(|f| (f.name, f.value))
            .collect()
    }
    pub fn import_form_data(&mut self, values: &BTreeMap<String, String>) -> Result<()> {
        let fields = self.fields();
        ensure!(
            values
                .keys()
                .all(|name| fields.iter().any(|f| &f.name == name)),
            "Form data includes missing or unsupported fields"
        );
        self.edit_with_permissions(
            lopdf::Permissions::FILLABLE | lopdf::Permissions::ANNOTABLE,
            |doc| {
                let mut temp = PdfDocument::from_document(doc.clone(), "form.pdf");
                for field in fields {
                    if let Some(value) = values.get(&field.name) {
                        if !field.editable {
                            let calculated = temp
                                .doc
                                .get_dictionary(field.id)?
                                .get(b"PilotCalculation")
                                .is_ok();
                            ensure!(
                                calculated || value == &field.value,
                                "Read-only field data cannot be changed"
                            );
                            continue;
                        }
                        temp.set_field(&field, value)?;
                    }
                }
                *doc = temp.doc;
                Ok(())
            },
        )
    }

    pub fn add_shape(
        &mut self,
        page: u32,
        shape: Shape,
        a: [f32; 2],
        b: [f32; 2],
        text: &str,
    ) -> Result<()> {
        let id = self.page_id(page)?;
        let geo = self.geometry(page)?;
        let mut rect = geo.rect(a, b);
        if matches!(shape, Shape::Line | Shape::Arrow) {
            rect = [rect[0] - 8., rect[1] - 8., rect[2] + 8., rect[3] + 8.];
        }
        let [x, y, r, t] = rect;
        let w = (r - x).max(1.);
        let h = (t - y).max(1.);
        self.edit_with_permissions(lopdf::Permissions::ANNOTABLE, |doc| {
            let mut resources=Dictionary::new();
            let subtype=match shape {Shape::Strikeout=>"StrikeOut",Shape::Circle=>"Circle",Shape::Stamp=>"Stamp",_=>"Line"};
            let mut annot=dictionary!{"Type"=>"Annot","Subtype"=>subtype,"Rect"=>nums(&rect),"P"=>id,"F"=>4,"C"=>nums(&[0.2,0.35,0.85]),"Contents"=>pdf_string(text),"T"=>pdf_string("PDFPilot")};
            let appearance=match shape {
                Shape::Circle=>{
                    let cx=w/2.;let cy=h/2.;let rx=(cx-1.).max(0.5);let ry=(cy-1.).max(0.5);let k=0.552_284_8;
                    format!("q .2 .35 .85 RG 1.5 w {} {cy} m {} {} {} {} {cx} {} c {} {} {} {} {} {cy} c {} {} {} {} {cx} {} c {} {} {} {} {} {cy} c S Q",cx+rx,cx+rx,cy+k*ry,cx+k*rx,cy+ry,cy+ry,cx-k*rx,cy+ry,cx-rx,cy+k*ry,cx-rx,cx-rx,cy-k*ry,cx-k*rx,cy-ry,cy-ry,cx+k*rx,cy-ry,cx+rx,cy-k*ry,cx+rx)
                },
                Shape::Stamp=>{
                    let bytes=pdf::latin_text(if text.trim().is_empty(){"APPROVED"}else{text})?;
                    let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica-Bold","Encoding"=>"WinAnsiEncoding"});resources.set("Font",dictionary!{"F"=>font});
                    let size=(w/(bytes.len().max(1) as f32*0.65)).min(h*0.55).clamp(4.,30.);
                    let hex=bytes.iter().map(|b|format!("{b:02X}")).collect::<String>();
                    annot.set("Name","Approved");format!("q .2 .35 .85 RG 2 w 1 1 {} {} re S BT /F {size} Tf .2 .35 .85 rg 5 {} Td <{hex}> Tj ET Q",w-2.,h-2.,(h-size)/2.+2.)
                },
                Shape::Strikeout=>{
                    let lo=[a[0].min(b[0]),a[1].min(b[1])];let hi=[a[0].max(b[0]),a[1].max(b[1])];
                    annot.set("QuadPoints",nums(&[geo.point(lo),geo.point([hi[0],lo[1]]),geo.point([lo[0],hi[1]]),geo.point(hi)].concat()));
                    let p=geo.point([lo[0],(lo[1]+hi[1])/2.]);let q=geo.point([hi[0],(lo[1]+hi[1])/2.]);line_path(p,q,x,y,false)
                },
                _=>{let p=geo.point(a);let q=geo.point(b);annot.set("L",nums(&[p,q].concat()));if shape==Shape::Arrow {annot.set("LE",vec![Object::Name(b"None".to_vec()),Object::Name(b"OpenArrow".to_vec())]);}line_path(p,q,x,y,shape==Shape::Arrow)},
            };
            let ap=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>nums(&[0.,0.,w,h]),"Resources"=>resources},appearance.into_bytes()));annot.set("AP",dictionary!{"N"=>ap});
            let annot=doc.add_object(annot);append_annotation(doc,id,annot)
        })
    }
    pub fn reply_comment(&mut self, parent: ObjectId, page: u32, text: &str) -> Result<()> {
        ensure!(!text.trim().is_empty(), "Enter reply text");
        let id = self.page_id(page)?;
        self.edit_with_permissions(lopdf::Permissions::ANNOTABLE, |doc| {doc.get_dictionary(parent)?;let annot=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>nums(&[0.,0.,0.,0.]),"P"=>id,"F"=>2,"IRT"=>parent,"RT"=>"R","Contents"=>pdf_string(text),"T"=>pdf_string("PDFPilot")});append_annotation(doc,id,annot)})
    }
    pub fn resolve_comment(&mut self, id: ObjectId, resolved: bool) -> Result<()> {
        self.edit_with_permissions(lopdf::Permissions::ANNOTABLE, |doc| {
            let dict = doc.get_dictionary_mut(id)?;
            dict.set("StateModel", pdf_string("Review"));
            dict.set(
                "State",
                pdf_string(if resolved { "Completed" } else { "None" }),
            );
            Ok(())
        })
    }
    pub fn comment_resolved(&self, id: ObjectId) -> bool {
        self.doc
            .get_dictionary(id)
            .ok()
            .and_then(|d| d.get(b"State").ok())
            .is_some_and(|o| object_text(o) == "Completed")
    }
    pub fn strip_metadata(&mut self) -> Result<()> {
        self.edit(|doc| {
            doc.trailer.remove(b"Info");
            doc.catalog_mut()?.remove(b"Metadata");
            for id in doc.get_pages().values() {
                doc.get_dictionary_mut(*id)?.remove(b"Metadata");
            }
            doc.prune_objects();
            Ok(())
        })
    }
    pub fn set_language(&mut self, language: &str) -> Result<()> {
        ensure!(
            !language.trim().is_empty(),
            "Enter a document language, such as en-US"
        );
        self.edit(|doc| {
            doc.catalog_mut()?.set("Lang", pdf_string(language));
            Ok(())
        })
    }
}

fn nums(values: &[f32]) -> Vec<Object> {
    values.iter().copied().map(Object::Real).collect()
}
fn annotation_ids(doc: &Document, page: ObjectId) -> Vec<ObjectId> {
    doc.get_dictionary(page)
        .ok()
        .and_then(|d| d.get(b"Annots").ok())
        .and_then(|o| doc.dereference(o).ok())
        .and_then(|(_, o)| o.as_array().ok())
        .map(|a| a.iter().filter_map(|o| o.as_reference().ok()).collect())
        .unwrap_or_default()
}
fn has_widgets(doc: &Document, page: ObjectId) -> bool {
    annotation_ids(doc, page).iter().any(|id| {
        doc.get_dictionary(*id).ok().is_some_and(|d| {
            d.get(b"Subtype").ok().and_then(|o| o.as_name().ok()) == Some(b"Widget")
        })
    })
}
pub(crate) fn append_annotation(doc: &mut Document, page: ObjectId, annot: ObjectId) -> Result<()> {
    let mut ids = doc
        .get_dictionary(page)?
        .get(b"Annots")
        .ok()
        .and_then(|o| doc.dereference(o).ok())
        .and_then(|(_, o)| o.as_array().ok())
        .cloned()
        .unwrap_or_default();
    ids.push(annot.into());
    doc.get_dictionary_mut(page)?.set("Annots", ids);
    Ok(())
}
fn walk_outlines(
    doc: &Document,
    first: ObjectId,
    depth: usize,
    pages: &BTreeMap<u32, ObjectId>,
    seen: &mut HashSet<ObjectId>,
    out: &mut Vec<Outline>,
) {
    if depth > 32 {
        return;
    }
    let mut current = Some(first);
    while let Some(id) = current {
        if !seen.insert(id) {
            break;
        }
        let Ok(dict) = doc.get_dictionary(id) else {
            break;
        };
        let dest = dict.get(b"Dest").ok().or_else(|| {
            doc.get_dict_in_dict(dict, b"A")
                .ok()
                .and_then(|a| a.get(b"D").ok())
        });
        let page = dest
            .and_then(|o| doc.dereference(o).ok())
            .and_then(|(_, o)| o.as_array().ok())
            .and_then(|a| a.first())
            .and_then(|o| o.as_reference().ok())
            .and_then(|id| pages.iter().find(|(_, pid)| **pid == id).map(|(n, _)| *n));
        out.push(Outline {
            id,
            title: dict.get(b"Title").map(object_text).unwrap_or_default(),
            page,
            depth,
        });
        if let Ok(child) = dict.get(b"First").and_then(Object::as_reference) {
            walk_outlines(doc, child, depth + 1, pages, seen, out);
        }
        current = dict.get(b"Next").ok().and_then(|o| o.as_reference().ok());
    }
}
fn walk_name_tree(
    doc: &Document,
    dict: &Dictionary,
    seen: &mut HashSet<ObjectId>,
    out: &mut Vec<(String, ObjectId)>,
) {
    if seen.len() > 10_000 {
        return;
    }
    if let Ok(names) = dict.get(b"Names").and_then(Object::as_array) {
        for pair in names.chunks_exact(2) {
            if let Ok(id) = pair[1].as_reference() {
                out.push((object_text(&pair[0]), id));
            }
        }
    }
    if let Ok(kids) = dict.get(b"Kids").and_then(Object::as_array) {
        for kid in kids {
            if let Ok(id) = kid.as_reference()
                && seen.insert(id)
                && let Ok(dict) = doc.get_dictionary(id)
            {
                walk_name_tree(doc, dict, seen, out);
            }
        }
    }
}
fn walk_number_tree(
    doc: &Document,
    dict: &Dictionary,
    seen: &mut HashSet<ObjectId>,
    out: &mut Vec<(i64, Dictionary)>,
) {
    if seen.len() > 10_000 {
        return;
    }
    if let Ok(nums) = dict.get(b"Nums").and_then(Object::as_array) {
        for pair in nums.chunks_exact(2) {
            if let Ok(index) = pair[0].as_i64()
                && let Ok((_, o)) = doc.dereference(&pair[1])
                && let Ok(dict) = o.as_dict()
            {
                out.push((index, dict.clone()));
            }
        }
    }
    if let Ok(kids) = dict.get(b"Kids").and_then(Object::as_array) {
        for kid in kids {
            if let Ok(id) = kid.as_reference()
                && seen.insert(id)
                && let Ok(dict) = doc.get_dictionary(id)
            {
                walk_number_tree(doc, dict, seen, out);
            }
        }
    }
}
pub(crate) fn set_attachments(
    doc: &mut Document,
    mut entries: Vec<(String, ObjectId)>,
) -> Result<()> {
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let names = entries
        .into_iter()
        .flat_map(|(name, id)| [pdf_string(&name), id.into()])
        .collect::<Vec<_>>();
    let tree = doc.add_object(dictionary! {"Names"=>names});
    let mut root = doc
        .catalog()
        .ok()
        .and_then(|c| doc.get_dict_in_dict(c, b"Names").ok())
        .cloned()
        .unwrap_or_default();
    root.set("EmbeddedFiles", tree);
    let root = doc.add_object(root);
    doc.catalog_mut()?.set("Names", root);
    Ok(())
}
fn line_path(p: [f32; 2], q: [f32; 2], x: f32, y: f32, arrow: bool) -> String {
    let mut path = format!(
        "q .2 .35 .85 RG 1.5 w {} {} m {} {} l S",
        p[0] - x,
        p[1] - y,
        q[0] - x,
        q[1] - y
    );
    if arrow {
        let angle = (q[1] - p[1]).atan2(q[0] - p[0]);
        let length = ((q[0] - p[0]).hypot(q[1] - p[1]) * 0.25).clamp(3., 10.);
        for offset in [-0.5, 0.5] {
            let a = angle + offset;
            path.push_str(&format!(
                " {} {} m {} {} l S",
                q[0] - x,
                q[1] - y,
                q[0] - x - length * a.cos(),
                q[1] - y - length * a.sin()
            ));
        }
    }
    path.push_str(" Q");
    path
}
