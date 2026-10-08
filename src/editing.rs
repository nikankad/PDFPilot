use crate::pdf::{self, Geometry, PdfDocument, TextStyle};
use anyhow::{Context, Result, ensure};
use lopdf::{
    Document, Object, ObjectId, Stream,
    content::{Content, Operation},
    dictionary,
};

#[derive(Clone, Debug)]
pub struct ImagePlacement {
    pub operation: usize,
    pub resource: Vec<u8>,
    pub image: ObjectId,
    pub matrix: [f32; 6],
}
pub(crate) fn multiply(a: [f32; 6], b: [f32; 6]) -> [f32; 6] {
    [
        a[0] * b[0] + a[2] * b[1],
        a[1] * b[0] + a[3] * b[1],
        a[0] * b[2] + a[2] * b[3],
        a[1] * b[2] + a[3] * b[3],
        a[0] * b[4] + a[2] * b[5] + a[4],
        a[1] * b[4] + a[3] * b[5] + a[5],
    ]
}
fn inverse(a: [f32; 6]) -> Result<[f32; 6]> {
    let d = a[0] * a[3] - a[1] * a[2];
    ensure!(
        d.is_finite() && d.abs() > 1e-8,
        "Image is in a singular graphics transform"
    );
    Ok([
        a[3] / d,
        -a[1] / d,
        -a[2] / d,
        a[0] / d,
        (a[2] * a[5] - a[3] * a[4]) / d,
        (a[1] * a[4] - a[0] * a[5]) / d,
    ])
}
fn nums<const N: usize>(values: [f32; N]) -> Vec<Object> {
    values.into_iter().map(Object::Real).collect()
}
fn placements(doc: &Document, page: ObjectId) -> Result<Vec<ImagePlacement>> {
    let content = Content::decode(&doc.get_page_content_with_limit(page, pdf::TEXT_LIMIT)?)?;
    let (local, parents) = doc.get_page_resources(page)?;
    let mut resources = std::collections::BTreeMap::new();
    for dictionary in local
        .into_iter()
        .chain(parents.iter().filter_map(|id| doc.get_dictionary(*id).ok()))
    {
        if let Ok(objects) = doc.get_dict_in_dict(dictionary, b"XObject") {
            for (name, value) in objects {
                if !resources.contains_key(name) {
                    resources.insert(name.clone(), value.clone());
                }
            }
        }
    }
    let mut current = [1., 0., 0., 1., 0., 0.];
    let mut stack = vec![];
    let mut out = vec![];
    for (index, operation) in content.operations.iter().enumerate() {
        match operation.operator.as_str() {
            "q" => stack.push(current),
            "Q" => current = stack.pop().context("Unbalanced graphics state")?,
            "cm" => {
                ensure!(operation.operands.len() == 6, "Invalid graphics transform");
                let values = operation
                    .operands
                    .iter()
                    .map(Object::as_float)
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                let mut matrix = [0.; 6];
                matrix.copy_from_slice(&values);
                current = multiply(current, matrix);
            }
            "Do" => {
                let Some(name) = operation.operands.first().and_then(|o| o.as_name().ok()) else {
                    continue;
                };
                if let Some(value) = resources.get(name) {
                    let (id, object) = doc.dereference(value)?;
                    if let (Some(id), Ok(stream)) = (id, object.as_stream())
                        && stream
                            .dict
                            .get(b"Subtype")
                            .map(pdf::object_text)
                            .unwrap_or_default()
                            == "Image"
                    {
                        out.push(ImagePlacement {
                            operation: index,
                            resource: name.to_vec(),
                            image: id,
                            matrix: current,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    Ok(out)
}
fn rewrite_operation(
    doc: &mut Document,
    page: ObjectId,
    operation: usize,
    replacement: Vec<Operation>,
) -> Result<()> {
    let contents = doc.get_page_contents(page);
    let mut offset = 0;
    for (index, id) in contents.iter().enumerate() {
        let stream = doc.get_object(*id)?.as_stream()?;
        let bytes = stream
            .decompressed_content_with_limit(pdf::TEXT_LIMIT)
            .unwrap_or_else(|_| stream.content.clone());
        let mut content = Content::decode(&bytes)?;
        if operation < offset + content.operations.len() {
            let local = operation - offset;
            content.operations.splice(local..=local, replacement);
            let mut dict = stream.dict.clone();
            dict.remove(b"Filter");
            dict.remove(b"DecodeParms");
            let mut stream = Stream::new(dict, content.encode()?);
            stream.compress()?;
            let new = doc.add_object(stream);
            let mut entries = contents
                .into_iter()
                .map(Object::Reference)
                .collect::<Vec<_>>();
            entries[index] = new.into();
            doc.get_dictionary_mut(page)?.set("Contents", entries);
            doc.prune_objects();
            return Ok(());
        }
        offset += content.operations.len();
    }
    anyhow::bail!("Content operation not found")
}

impl PdfDocument {
    pub fn image_placements(&self, page: u32) -> Result<Vec<ImagePlacement>> {
        placements(&self.doc, self.page_id(page)?)
    }
    pub fn delete_image_placement(&mut self, page: u32, operation: usize) -> Result<()> {
        let id = self.page_id(page)?;
        ensure!(
            self.image_placements(page)?
                .iter()
                .any(|p| p.operation == operation),
            "Select an image placement"
        );
        self.edit(|doc| {
            rewrite_operation(doc, id, operation, vec![])?;
            Ok(())
        })
    }
    pub fn transform_image(
        &mut self,
        page: u32,
        operation: usize,
        rect: [f32; 4],
        rotation: f32,
        crop: [f32; 4],
    ) -> Result<()> {
        ensure!(
            rect.iter()
                .chain(crop.iter())
                .all(|x| x.is_finite() && (0.0..=1.0).contains(x))
                && rect[2] > rect[0]
                && rect[3] > rect[1]
                && crop[2] > crop[0]
                && crop[3] > crop[1]
                && rotation.is_finite(),
            "Invalid placement or crop bounds"
        );
        let placement = self
            .image_placements(page)?
            .into_iter()
            .find(|p| p.operation == operation)
            .context("Select an image placement")?;
        let geo = self.geometry(page)?;
        let size = geo.display_size();
        let w = (rect[2] - rect[0]) * size[0];
        let h = (rect[3] - rect[1]) * size[1];
        let [x, y] = geo.point([rect[0], rect[3]]);
        let target = match geo.rotation {
            90 => [0., w, -h, 0., x, y],
            180 => [-w, 0., 0., -h, x, y],
            270 => [0., -w, h, 0., x, y],
            _ => [w, 0., 0., h, x, y],
        };
        let (s, c) = rotation.to_radians().sin_cos();
        let rotate = [
            c,
            s,
            -s,
            c,
            0.5 - 0.5 * c + 0.5 * s,
            0.5 - 0.5 * s - 0.5 * c,
        ];
        let cw = crop[2] - crop[0];
        let ch = crop[3] - crop[1];
        let mapping = [1. / cw, 0., 0., 1. / ch, -crop[0] / cw, -crop[1] / ch];
        let delta = multiply(
            inverse(placement.matrix)?,
            multiply(target, multiply(rotate, mapping)),
        );
        let id = self.page_id(page)?;
        self.edit(|doc| {
            rewrite_operation(
                doc,
                id,
                operation,
                vec![
                    Operation::new("q", vec![]),
                    Operation::new("cm", nums(delta)),
                    Operation::new("re", nums([crop[0], crop[1], cw, ch])),
                    Operation::new("W", vec![]),
                    Operation::new("n", vec![]),
                    Operation::new("Do", vec![Object::Name(placement.resource.clone())]),
                    Operation::new("Q", vec![]),
                ],
            )
        })
    }

    pub fn page_background(&mut self, page: u32, color: [f32; 3]) -> Result<()> {
        ensure!(
            color
                .iter()
                .all(|c| c.is_finite() && (0.0..=1.0).contains(c)),
            "Invalid color"
        );
        let id = self.page_id(page)?;
        let g = self.geometry(page)?;
        self.edit(|doc| {
            let content = Content {
                operations: vec![
                    Operation::new("q", vec![]),
                    Operation::new("rg", nums(color)),
                    Operation::new(
                        "re",
                        nums([g.bounds[0], g.bounds[1], g.width(), g.height()]),
                    ),
                    Operation::new("f", vec![]),
                    Operation::new("Q", vec![]),
                ],
            }
            .encode()?;
            let background = doc.add_object(Stream::new(dictionary! {}, content));
            let old = doc.get_page_contents(id);
            let mut contents = vec![Object::Reference(background)];
            contents.extend(old.into_iter().map(Object::Reference));
            doc.get_dictionary_mut(id)?.set("Contents", contents);
            Ok(())
        })
    }
    pub fn set_page_box(&mut self, page: u32, name: &str, rect: [f32; 4]) -> Result<()> {
        ensure!(
            ["MediaBox", "CropBox", "TrimBox", "BleedBox", "ArtBox"].contains(&name),
            "Unknown page box"
        );
        ensure!(
            rect.iter().all(|v| v.is_finite()) && rect[2] > rect[0] && rect[3] > rect[1],
            "Invalid page box"
        );
        let id = self.page_id(page)?;
        self.edit(|doc| {
            doc.get_dictionary_mut(id)?.set(name, nums(rect));
            Ok(())
        })
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    Left,
    Center,
    Right,
}
#[derive(Clone)]
pub struct TextBlock {
    pub text: String,
    pub rect: [f32; 4],
    pub style: TextStyle,
    pub alignment: Alignment,
    pub line_spacing: f32,
    pub character_spacing: f32,
}
impl Default for TextBlock {
    fn default() -> Self {
        Self {
            text: String::new(),
            rect: [0.1, 0.2, 0.8, 0.5],
            style: Default::default(),
            alignment: Alignment::Left,
            line_spacing: 1.4,
            character_spacing: 0.,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Block {
    pub id: ObjectId,
    pub text: String,
}
impl PdfDocument {
    pub fn text_blocks(&self, page: u32) -> Result<Vec<Block>> {
        let id = self.page_id(page)?;
        Ok(self
            .doc
            .get_page_contents(id)
            .into_iter()
            .filter_map(|id| {
                let stream = self.doc.get_object(id).ok()?.as_stream().ok()?;
                let text = stream.dict.get(b"PilotText").ok().map(pdf::object_text)?;
                Some(Block { id, text })
            })
            .collect())
    }
    pub fn write_text_block(
        &mut self,
        page: u32,
        existing: Option<ObjectId>,
        block: &TextBlock,
    ) -> Result<()> {
        let id = self.page_id(page)?;
        let geo = self.geometry(page)?;
        ensure!(
            block
                .rect
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
                && block.rect[2] > block.rect[0]
                && block.rect[3] > block.rect[1],
            "Invalid text block bounds"
        );
        ensure!(
            !block.text.trim().is_empty() && block.text.len() <= 100_000,
            "Enter text, up to 100 KB"
        );
        ensure!(
            (0.8..=4.).contains(&block.line_spacing)
                && (-2.0..=20.).contains(&block.character_spacing),
            "Invalid text spacing"
        );
        if let Some(existing) = existing {
            ensure!(
                self.text_blocks(page)?.iter().any(|b| b.id == existing),
                "Select a PDFPilot text block"
            );
        }
        let width = (block.rect[2] - block.rect[0]) * geo.display_size()[0];
        let char_width = block.style.size
            * if block.style.font == "Courier" {
                0.6
            } else {
                0.55
            }
            + block.character_spacing;
        ensure!(char_width > 0., "Invalid font spacing");
        let limit = (width / char_width).floor() as usize;
        ensure!(limit >= 1, "Text area is too narrow");
        let mut lines = vec![];
        for paragraph in block.text.split('\n') {
            let mut line = String::new();
            for word in paragraph.split_whitespace() {
                if !line.is_empty() && line.chars().count() + word.chars().count() + 1 > limit {
                    lines.push(std::mem::take(&mut line));
                }
                if !line.is_empty() {
                    line.push(' ');
                }
                for character in word.chars() {
                    if line.chars().count() == limit {
                        lines.push(std::mem::take(&mut line));
                    }
                    line.push(character);
                }
            }
            lines.push(line);
        }
        let height = geo.display_size()[1];
        let line_height = block.style.size * block.line_spacing;
        ensure!(
            lines.len() as f32 * line_height <= (block.rect[3] - block.rect[1]) * height,
            "Text overflows its area; enlarge it or reduce the font size"
        );
        self.edit(|doc| {
            let max = doc.max_id;
            let mut generated = doc.clone();
            generated.get_dictionary_mut(id)?.remove(b"Contents");
            for (index, line) in lines.iter().enumerate() {
                if line.is_empty() {
                    continue;
                }
                let line_width = line.chars().count() as f32 * char_width;
                let offset = match block.alignment {
                    Alignment::Left => 0.,
                    Alignment::Center => (width - line_width).max(0.) / 2.,
                    Alignment::Right => (width - line_width).max(0.),
                };
                let position = [
                    block.rect[0] + offset / geo.display_size()[0],
                    block.rect[1] + (index as f32 * line_height + block.style.size) / height,
                ];
                pdf::write_text(&mut generated, id, geo, position, line, &block.style)?;
            }
            let bytes = generated.get_page_content_with_limit(id, pdf::TEXT_LIMIT)?;
            let mut content = Content::decode(&bytes)?;
            let mut ops = vec![];
            for op in content.operations.drain(..) {
                let is_begin = op.operator == "BT";
                ops.push(op);
                if is_begin {
                    ops.push(Operation::new(
                        "Tc",
                        vec![Object::Real(block.character_spacing)],
                    ));
                }
            }
            let resources = generated.get_dictionary(id)?.get(b"Resources")?.clone();
            for (object, value) in generated.objects {
                if object.0 > max {
                    doc.objects.insert(object, value);
                }
            }
            doc.max_id = generated.max_id;
            doc.get_dictionary_mut(id)?.set("Resources", resources);
            let mut stream = Stream::new(
                dictionary! {"PilotText"=>pdf::pdf_string(&block.text)},
                Content { operations: ops }.encode()?,
            );
            stream.dict.set("PilotBlockSpec", pdf::pdf_string(&serde_json::json!({"rect":block.rect,"font":block.style.font,"size":block.style.size,"color":block.style.color,"alignment":match block.alignment{Alignment::Left=>"left",Alignment::Center=>"center",Alignment::Right=>"right"},"line_spacing":block.line_spacing,"character_spacing":block.character_spacing}).to_string()));
            stream.compress()?;
            if let Some(existing) = existing {
                doc.objects.insert(existing, Object::Stream(stream));
            } else {
                let block = doc.add_object(stream);
                let mut contents = doc
                    .get_page_contents(id)
                    .into_iter()
                    .map(Object::Reference)
                    .collect::<Vec<_>>();
                contents.push(block.into());
                doc.get_dictionary_mut(id)?.set("Contents", contents);
            }
            doc.prune_objects();
            Ok(())
        })
    }
    pub fn text_block_spec(&self, page: u32, id: ObjectId) -> Result<TextBlock> {
        ensure!(
            self.text_blocks(page)?.iter().any(|b| b.id == id),
            "Unknown text block"
        );
        let d = &self.doc.get_object(id)?.as_stream()?.dict;
        let data: serde_json::Value =
            serde_json::from_str(&pdf::object_text(d.get(b"PilotBlockSpec")?))?;
        let mut block = TextBlock {
            text: pdf::object_text(d.get(b"PilotText")?),
            ..Default::default()
        };
        for (i, v) in block.rect.iter_mut().enumerate() {
            *v = data["rect"][i].as_f64().context("Invalid text area")? as f32;
        }
        block.style.font = data["font"].as_str().context("Invalid font")?.into();
        block.style.size = data["size"].as_f64().context("Invalid font size")? as f32;
        for (i, v) in block.style.color.iter_mut().enumerate() {
            *v = data["color"][i].as_f64().context("Invalid text color")? as f32;
        }
        block.alignment = match data["alignment"].as_str() {
            Some("center") => Alignment::Center,
            Some("right") => Alignment::Right,
            _ => Alignment::Left,
        };
        block.line_spacing = data["line_spacing"].as_f64().unwrap_or(1.4) as f32;
        block.character_spacing = data["character_spacing"].as_f64().unwrap_or(0.) as f32;
        Ok(block)
    }
    pub fn delete_text_block(&mut self, page: u32, id: ObjectId) -> Result<()> {
        let page_id = self.page_id(page)?;
        ensure!(
            self.text_blocks(page)?.iter().any(|b| b.id == id),
            "Unknown text block"
        );
        self.edit(|doc| {
            let contents = doc
                .get_page_contents(page_id)
                .into_iter()
                .filter(|other| *other != id)
                .map(Object::Reference)
                .collect::<Vec<_>>();
            doc.get_dictionary_mut(page_id)?.set("Contents", contents);
            doc.prune_objects();
            Ok(())
        })
    }
}

pub fn normalized(geo: Geometry, p: [f32; 2]) -> [f32; 2] {
    let x = (p[0] - geo.bounds[0]) / geo.width();
    let y = (p[1] - geo.bounds[1]) / geo.height();
    match geo.rotation {
        90 => [y, x],
        180 => [1. - x, y],
        270 => [1. - y, 1. - x],
        _ => [x, 1. - y],
    }
}
