use crate::pdf::{self, PdfDocument};
use anyhow::{Result, ensure};
use lopdf::{Object, dictionary};
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Distance,
    Perimeter,
    Area,
    Calibrate,
}
impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Distance => "Distance",
            Self::Perimeter => "Rectangle perimeter",
            Self::Area => "Rectangle area",
            Self::Calibrate => "Calibrate scale",
        }
    }
}
impl PdfDocument {
    pub fn measure(
        &mut self,
        page: u32,
        a: [f32; 2],
        b: [f32; 2],
        mode: Mode,
        scale: f32,
        persist: bool,
    ) -> Result<f32> {
        ensure!(scale.is_finite() && scale > 0., "Invalid measurement scale");
        let geo = self.geometry(page)?;
        let p = geo.point(a);
        let q = geo.point(b);
        let x = (q[0] - p[0]).abs();
        let y = (q[1] - p[1]).abs();
        let factor = 25.4 / 72. * scale;
        let value = match mode {
            Mode::Perimeter => 2. * (x + y) * factor,
            Mode::Area => x * y * factor * factor,
            _ => x.hypot(y) * factor,
        };
        ensure!(
            value.is_finite() && value > 0.,
            "Draw a nonzero measurement"
        );
        if persist && mode != Mode::Calibrate {
            let page_id = self.page_id(page)?;
            let mut rect = geo.rect(a, b);
            if mode == Mode::Distance {
                rect = [rect[0] - 2., rect[1] - 2., rect[2] + 2., rect[3] + 2.];
            }
            let unit = if mode == Mode::Area { "mm2" } else { "mm" };
            self.edit_with_permissions(lopdf::Permissions::ANNOTABLE, |doc|{let format=dictionary!{"Type"=>"NumberFormat","U"=>"mm","C"=>factor};let measure=dictionary!{"Type"=>"Measure","Subtype"=>"RL","R"=>pdf::pdf_string(&format!("1:{scale}")),"X"=>vec![Object::Dictionary(format.clone())],"Y"=>vec![Object::Dictionary(format.clone())],"D"=>vec![Object::Dictionary(format)],"A"=>vec![Object::Dictionary(dictionary!{"Type"=>"NumberFormat","U"=>"mm2","C"=>factor*factor})]};let mut annot=dictionary!{"Type"=>"Annot","Subtype"=>if mode==Mode::Distance{"Line"}else{"Square"},"P"=>page_id,"Rect"=>rect.into_iter().map(Object::Real).collect::<Vec<_>>(),"F"=>4,"C"=>vec![Object::Real(0.25),Object::Real(0.37),Object::Real(0.9)],"Contents"=>pdf::pdf_string(&format!("{}: {value:.2} {unit} (scale 1:{scale})",mode.label())),"Measure"=>measure};if mode==Mode::Distance{annot.set("L",vec![p[0].into(),p[1].into(),q[0].into(),q[1].into()]);annot.set("IT","LineDimension");annot.set("Cap",true);}let annot=doc.add_object(annot);let mut annotations=doc.get_dictionary(page_id)?.get(b"Annots").ok().and_then(|o|doc.dereference(o).ok()).and_then(|(_,o)|o.as_array().ok()).cloned().unwrap_or_default();annotations.push(annot.into());doc.get_dictionary_mut(page_id)?.set("Annots",annotations);Ok(())})?;
        }
        Ok(value)
    }
}
