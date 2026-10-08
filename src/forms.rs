//! AcroForm validation, calculation and field-property operations.
use crate::pdf::{self, Field, PdfDocument};
use anyhow::{Context, Result, ensure};
use lopdf::{Object, ObjectId, dictionary};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Calculation {
    Sum,
    Product,
    Average,
}
impl Calculation {
    pub fn label(self) -> &'static str {
        match self {
            Self::Sum => "Sum",
            Self::Product => "Product",
            Self::Average => "Average",
        }
    }
}
#[derive(Default, Clone)]
pub struct FieldProperties {
    pub tooltip: String,
    pub required: bool,
    pub read_only: bool,
    pub max_length: usize,
    pub numeric_min: Option<f64>,
    pub numeric_max: Option<f64>,
    pub calculation: Option<(Calculation, Vec<String>)>,
}
impl PdfDocument {
    pub fn configure_field(&mut self, id: ObjectId, properties: &FieldProperties) -> Result<()> {
        ensure!(
            self.fields().iter().any(|f| f.id == id),
            "Unknown form field"
        );
        ensure!(
            properties.max_length <= 100_000,
            "Maximum length is too large"
        );
        if let (Some(a), Some(b)) = (properties.numeric_min, properties.numeric_max) {
            ensure!(
                a.is_finite() && b.is_finite() && a <= b,
                "Invalid numeric limits"
            );
        }
        if let Some((_, inputs)) = &properties.calculation {
            ensure!(!inputs.is_empty(), "Select calculation inputs");
            for name in inputs {
                ensure!(
                    self.fields().iter().any(|f| &f.name == name && f.id != id),
                    "Unknown input or self-reference: {name}"
                );
            }
        }
        self.edit(|doc| {
            let mut actions = doc.get_dictionary(id)?.get(b"AA").ok().and_then(|o| doc.dereference(o).ok()).and_then(|(_, o)| o.as_dict().ok()).cloned().unwrap_or_default();
            let field = doc.get_dictionary_mut(id)?;
            let mut flags = field.get(b"Ff").and_then(Object::as_i64).unwrap_or(0) & !3;
            if properties.required { flags |= 2; }
            if properties.read_only || properties.calculation.is_some() { flags |= 1; }
            field.set("Ff", flags); field.set("TU", pdf::pdf_string(&properties.tooltip));
            if properties.max_length > 0 { field.set("MaxLen", properties.max_length as i64); } else { field.remove(b"MaxLen"); }
            let mut rules = dictionary!{};
            if let Some(n) = properties.numeric_min { ensure!((n as f32).is_finite(), "Minimum is out of range"); rules.set("Min", Object::Real(n as f32)); }
            if let Some(n) = properties.numeric_max { ensure!((n as f32).is_finite(), "Maximum is out of range"); rules.set("Max", Object::Real(n as f32)); }
            field.set("PilotValidation", rules);
            let had_pilot=field.get(b"PilotCalculation").is_ok();

            if let Some((kind,inputs))=&properties.calculation {
                let code=format!("AFSimple_Calculate({}, {});",serde_json::to_string(match kind{Calculation::Sum=>"SUM",Calculation::Product=>"PRD",Calculation::Average=>"AVG"})?,serde_json::to_string(inputs)?);
                actions.set("C",dictionary!{"S"=>"JavaScript","JS"=>pdf::pdf_string(&code)});
            }else if had_pilot{actions.remove(b"C");}
            if !actions.is_empty(){field.set("AA",actions);}else{field.remove(b"AA");}
            if let Some((kind, inputs)) = &properties.calculation { field.set("PilotCalculation", dictionary!{"Operation"=>pdf::pdf_string(kind.label()),"Inputs"=>inputs.iter().map(|s|pdf::pdf_string(s)).collect::<Vec<_>>()}); } else { field.remove(b"PilotCalculation"); }
            recalculate(doc)
        })
    }
    pub fn validate_form(&self) -> Vec<String> {
        let mut issues = vec![];
        for field in self.fields() {
            let dict = self.doc.get_dictionary(field.id).unwrap();
            let required = dict.get(b"Ff").and_then(Object::as_i64).unwrap_or(0) & 2 != 0;
            if required && (field.value.trim().is_empty() || field.value == "Off") {
                issues.push(format!("{}: required value is missing", field.name));
            }
            if let Err(e) = validate_value(&self.doc, &field, &field.value) {
                issues.push(format!("{}: {e}", field.name));
            }
        }
        issues
    }
    pub fn order_form_tabs(&mut self, page: u32) -> Result<()> {
        let id = self.page_id(page)?;
        self.edit(|doc| {
            let mut annotations = doc
                .get_dictionary(id)?
                .get(b"Annots")
                .ok()
                .and_then(|o| doc.dereference(o).ok())
                .and_then(|(_, o)| o.as_array().ok())
                .cloned()
                .unwrap_or_default();
            let coordinate = |o: &Object| -> (f32, f32) {
                o.as_reference()
                    .ok()
                    .and_then(|id| doc.get_dictionary(id).ok())
                    .and_then(|d| d.get(b"Rect").ok())
                    .and_then(|o| o.as_array().ok())
                    .filter(|a| a.len() == 4)
                    .map(|r| {
                        (
                            -r[3].as_float().unwrap_or(0.),
                            r[0].as_float().unwrap_or(0.),
                        )
                    })
                    .unwrap_or((f32::MAX, f32::MAX))
            };
            annotations.sort_by(|a, b| {
                let a = coordinate(a);
                let b = coordinate(b);
                a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1))
            });
            let page = doc.get_dictionary_mut(id)?;
            page.set("Annots", annotations);
            page.set("Tabs", "R");
            Ok(())
        })
    }
}
pub(crate) fn valid_date(value: &str) -> bool {
    let parts = value.split('-').collect::<Vec<_>>();
    if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return false;
    }
    let (Ok(y), Ok(m), Ok(d)) = (
        parts[0].parse::<u32>(),
        parts[1].parse::<u32>(),
        parts[2].parse::<u32>(),
    ) else {
        return false;
    };
    let max = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 0,
    };
    y > 0 && d > 0 && d <= max
}
pub(crate) fn validate_value(doc: &lopdf::Document, field: &Field, value: &str) -> Result<()> {
    let dict = doc.get_dictionary(field.id)?;
    if let Ok(max) = dict.get(b"MaxLen").and_then(Object::as_i64) {
        ensure!(
            value.chars().count() as i64 <= max,
            "Value exceeds maximum length {max}"
        );
    }
    if value.is_empty() {
        return Ok(());
    }
    if dict
        .get(b"PilotFormat")
        .map(pdf::object_text)
        .unwrap_or_default()
        == "Date"
    {
        ensure!(valid_date(value), "Use a valid date in YYYY-MM-DD format");
    }
    if let Ok(rules) = dict.get(b"PilotValidation").and_then(Object::as_dict)
        && (rules.get(b"Min").is_ok() || rules.get(b"Max").is_ok())
    {
        let n: f64 = value.parse().context("Enter a numeric value")?;
        ensure!(n.is_finite(), "Value must be finite");
        if let Ok(min) = rules.get(b"Min").and_then(Object::as_float) {
            ensure!(n >= min as f64, "Value must be at least {min}");
        }
        if let Ok(max) = rules.get(b"Max").and_then(Object::as_float) {
            ensure!(n <= max as f64, "Value must be at most {max}");
        }
    }
    Ok(())
}
pub(crate) fn recalculate(doc: &mut lopdf::Document) -> Result<()> {
    let snapshot = PdfDocument::from_document(doc.clone(), "calculation.pdf");
    let fields = snapshot.fields();
    let mut pending = fields
        .iter()
        .filter(|f| {
            doc.get_dictionary(f.id)
                .is_ok_and(|d| d.get(b"PilotCalculation").is_ok())
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut calculation_order = vec![];
    let mut values = fields
        .iter()
        .map(|f| (f.name.clone(), f.value.clone()))
        .collect::<std::collections::BTreeMap<_, _>>();
    while !pending.is_empty() {
        let names = pending.iter().map(|f| f.name.clone()).collect::<Vec<_>>();
        let before = pending.len();
        let mut waiting = vec![];
        for field in pending {
            let calc = doc
                .get_dictionary(field.id)?
                .get(b"PilotCalculation")?
                .as_dict()?;
            let inputs = calc
                .get(b"Inputs")?
                .as_array()?
                .iter()
                .map(pdf::object_text)
                .collect::<Vec<_>>();
            if inputs.iter().any(|n| names.contains(n)) {
                waiting.push(field);
                continue;
            }
            let numbers = inputs
                .iter()
                .map(|name| {
                    let value = values.get(name).context("Missing calculation input")?;
                    if value.trim().is_empty() {
                        Ok(0.)
                    } else {
                        value
                            .parse::<f64>()
                            .context("Calculation input must be numeric")
                    }
                })
                .collect::<Result<Vec<_>>>()?;
            ensure!(!numbers.is_empty(), "Calculation needs an input");
            let result = match pdf::object_text(calc.get(b"Operation")?).as_str() {
                "Sum" => numbers.iter().sum(),
                "Product" => numbers.iter().product(),
                "Average" => numbers.iter().sum::<f64>() / numbers.len() as f64,
                _ => anyhow::bail!("Unknown calculation operation"),
            };
            ensure!(result.is_finite(), "Calculation overflow");
            let value = result.to_string();
            validate_value(doc, &field, &value)?;
            pdf::set_field_value(doc, &field, &value)?;
            calculation_order.push(Object::Reference(field.id));
            values.insert(field.name, value);
        }
        ensure!(
            waiting.len() < before,
            "Calculated fields contain a dependency cycle"
        );
        pending = waiting;
    }
    if !calculation_order.is_empty()
        || doc
            .catalog()
            .and_then(|c| doc.get_dict_in_dict(c, b"AcroForm"))
            .is_ok_and(|f| f.get(b"CO").is_ok())
    {
        let mut form = doc
            .catalog()
            .and_then(|c| doc.get_dict_in_dict(c, b"AcroForm"))?
            .clone();
        let mut existing = form
            .get(b"CO")
            .and_then(Object::as_array)
            .cloned()
            .unwrap_or_default();
        existing.retain(|o| {
            o.as_reference().ok().is_some_and(|id| {
                doc.get_dictionary(id).is_ok_and(|d| {
                    d.get(b"PilotCalculation").is_err()
                        && doc
                            .get_dict_in_dict(d, b"AA")
                            .is_ok_and(|aa| aa.get(b"C").is_ok())
                })
            })
        });
        existing.extend(calculation_order);
        form.set("CO", existing);
        let form_id = doc.add_object(form);
        doc.catalog_mut()?.set("AcroForm", form_id);
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub kind: crate::advanced::FormKind,
    pub rect: [f32; 4],
}
impl PdfDocument {
    /// Propose widgets from rectangular graphics. Scans and arbitrary layout
    /// interpretation need OCR/vision, so candidates must be reviewed first.
    pub fn detect_field_candidates(&self, page: u32) -> Result<Vec<Candidate>> {
        use lopdf::content::Content;
        let id = self.page_id(page)?;
        let geo = self.geometry(page)?;
        let content = Content::decode(&self.doc.get_page_content_with_limit(id, pdf::TEXT_LIMIT)?)?;
        let mut matrix = [1., 0., 0., 1., 0., 0.];
        let mut stack = vec![];
        let mut candidates = vec![];
        for op in content.operations {
            match op.operator.as_str() {
                "q" => stack.push(matrix),
                "Q" => matrix = stack.pop().context("Unbalanced graphics state")?,
                "cm" => {
                    ensure!(op.operands.len() == 6, "Invalid graphics transform");
                    let values = op
                        .operands
                        .iter()
                        .map(Object::as_float)
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    let mut m = [0.; 6];
                    m.copy_from_slice(&values);
                    matrix = crate::editing::multiply(matrix, m);
                }
                "re" if op.operands.len() == 4 => {
                    let r = op
                        .operands
                        .iter()
                        .map(Object::as_float)
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    let map = |x: f32, y: f32| {
                        crate::editing::normalized(
                            geo,
                            [
                                matrix[0] * x + matrix[2] * y + matrix[4],
                                matrix[1] * x + matrix[3] * y + matrix[5],
                            ],
                        )
                    };
                    let a = map(r[0], r[1]);
                    let b = map(r[0] + r[2], r[1] + r[3]);
                    let rect = [
                        a[0].min(b[0]),
                        a[1].min(b[1]),
                        a[0].max(b[0]),
                        a[1].max(b[1]),
                    ];
                    let size = geo.display_size();
                    let w = (rect[2] - rect[0]) * size[0];
                    let h = (rect[3] - rect[1]) * size[1];
                    let kind = if (8.0..=24.).contains(&w)
                        && (8.0..=24.).contains(&h)
                        && (w - h).abs() < 4.
                    {
                        Some(crate::advanced::FormKind::Checkbox)
                    } else if (40.0..=500.).contains(&w) && (12.0..=70.).contains(&h) {
                        Some(crate::advanced::FormKind::Text)
                    } else {
                        None
                    };
                    if let Some(kind) = kind
                        && rect.iter().all(|v| (0.0..=1.0).contains(v))
                        && !candidates.iter().any(|c: &Candidate| {
                            c.rect.iter().zip(rect).all(|(a, b)| (a - b).abs() < 0.005)
                        })
                    {
                        candidates.push(Candidate { kind, rect });
                    }
                }
                _ => {}
            }
        }
        // Exclude rectangles that already contain authored widgets.
        let widgets = self
            .doc
            .get_page_annotations(id)?
            .into_iter()
            .filter(|d| d.get(b"Subtype").map(pdf::object_text).unwrap_or_default() == "Widget")
            .filter_map(|d| d.get(b"Rect").ok()?.as_array().ok())
            .filter(|r| r.len() == 4)
            .map(|r| {
                r.iter()
                    .map(Object::as_float)
                    .collect::<std::result::Result<Vec<_>, _>>()
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        candidates.retain(|c| {
            let r = geo.rect([c.rect[0], c.rect[1]], [c.rect[2], c.rect[3]]);
            !widgets
                .iter()
                .any(|w| r[0] < w[2] && r[2] > w[0] && r[1] < w[3] && r[3] > w[1])
        });
        Ok(candidates)
    }
    pub fn create_candidate_fields(&mut self, page: u32, candidates: &[Candidate]) -> Result<()> {
        ensure!(!candidates.is_empty(), "Select field candidates");
        self.edit(|doc| {
            let mut temp = PdfDocument::from_document(doc.clone(), "forms.pdf");
            let mut n = 1;
            for candidate in candidates {
                while temp
                    .fields()
                    .iter()
                    .any(|f| f.name == format!("DetectedField{n}"))
                {
                    n += 1;
                }
                temp.add_form_field(
                    page,
                    [candidate.rect[0], candidate.rect[1]],
                    [candidate.rect[2], candidate.rect[3]],
                    &crate::advanced::FormSpec {
                        kind: candidate.kind,
                        name: format!("DetectedField{n}"),
                        required: false,
                        options: vec![],
                    },
                )?;
            }
            *doc = temp.doc;
            Ok(())
        })
    }
}
