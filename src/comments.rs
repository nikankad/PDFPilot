use crate::{
    advanced,
    pdf::{self, PdfDocument},
};
use anyhow::{Context, Result, ensure};
use lopdf::{Stream, dictionary};
use std::path::Path;
impl PdfDocument {
    pub fn add_correction(&mut self, page: u32, point: [f32; 2], text: &str) -> Result<()> {
        ensure!(
            point
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
            "Invalid correction location"
        );
        let id = self.page_id(page)?;
        let p = self.geometry(page)?.point(point);
        self.edit_with_permissions(lopdf::Permissions::ANNOTABLE,|doc|{let appearance=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),14.into(),12.into()]},b"q .2 .3 .8 RG 2 w 1 1 m 7 11 l 13 1 l S Q".to_vec()));let annotation=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Caret","Sy"=>"P","P"=>id,"F"=>4,"Rect"=>vec![p[0].into(),p[1].into(),(p[0]+14.).into(),(p[1]+12.).into()],"Contents"=>pdf::pdf_string(text),"AP"=>dictionary!{"N"=>appearance}});advanced::append_annotation(doc,id,annotation)})
    }
    pub fn attach_comment(
        &mut self,
        page: u32,
        point: [f32; 2],
        path: &Path,
        text: &str,
    ) -> Result<()> {
        ensure!(
            point
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
            "Invalid attachment location"
        );
        let id = self.page_id(page)?;
        let p = self.geometry(page)?.point(point);
        let name = path
            .file_name()
            .context("Attachment needs a file name")?
            .to_string_lossy()
            .to_string();
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
            !entries.iter().any(|(n, _)| n == &name),
            "An attachment with this name already exists"
        );
        self.edit_with_permissions(lopdf::Permissions::ANNOTABLE,|doc|{let mut stream=Stream::new(dictionary!{"Type"=>"EmbeddedFile","Params"=>dictionary!{"Size"=>bytes.len() as i64}},bytes);stream.compress()?;let embedded=doc.add_object(stream);let spec=doc.add_object(dictionary!{"Type"=>"Filespec","F"=>pdf::pdf_string(&name),"UF"=>pdf::pdf_string(&name),"EF"=>dictionary!{"F"=>embedded}});entries.push((name,spec));advanced::set_attachments(doc,entries)?;let annotation=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"FileAttachment","Name"=>"Paperclip","P"=>id,"F"=>4,"FS"=>spec,"Rect"=>vec![p[0].into(),p[1].into(),(p[0]+20.).into(),(p[1]+20.).into()],"Contents"=>pdf::pdf_string(text)});advanced::append_annotation(doc,id,annotation)})
    }
}
