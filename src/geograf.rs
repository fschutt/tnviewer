use std::{io::BufWriter, path::PathBuf};

use dxf::{Vector, XData, XDataItem};
use wasm_bindgen::JsValue;
use web_sys::js_sys::JsString;

use crate::{nas::{NasXMLFile, SplitNasXml, SvgLine, LATLON_STRING}, ui::{Aenderungen, TextPlacement}, zip::write_files_to_zip};

/// Returns the dxf bytes
pub fn texte_zu_dxf_datei(texte: &[TextPlacement]) -> Vec<u8> {
    use dxf::Drawing;
    use dxf::entities::*;

    let mut drawing = Drawing::new();

    fn update_x(zone: usize, pos: f64) -> f64 {
        format!("{zone}{pos}").parse().unwrap_or_default()
    }

    for text in texte {
        let newx = update_x(33, text.pos.x);
        let entity = Entity::new(EntityType::Text(dxf::entities::Text {
            thickness: 0.0,
            location: dxf::Point { x: newx, y: text.pos.y, z: 0.0 },
            text_height: 5.0,
            value: text.kuerzel.clone(),
            rotation: 0.0,
            relative_x_scale_factor: 1.0,
            oblique_angle: 0.0,
            text_style_name: match text.status {
                crate::ui::TextStatus::Old => "old",
                crate::ui::TextStatus::New => "new",
                crate::ui::TextStatus::StaysAsIs => "stayasis",
            }.to_string(),
            text_generation_flags: 0,
            second_alignment_point: dxf::Point::origin(),
            normal: Vector::z_axis(),
            horizontal_text_justification: dxf::enums::HorizontalTextJustification::Center,
            vertical_text_justification: dxf::enums::VerticalTextJustification::Middle,
        }));
        let _entity_ref = drawing.add_entity(entity);
    }

    let v = Vec::new();
    let mut buf = BufWriter::new(v);
    let _ = drawing.save(&mut buf);
    buf.into_inner().unwrap_or_default()
}

pub struct ShpReturn {
    pub shp: Vec<u8>,
    pub shx: Vec<u8>,
    pub dbf: Vec<u8>,
    pub prj: Vec<u8>,
    pub cpg: Vec<u8>,
}

pub fn lines_to_shp(lines: &[SvgLine], xml: &NasXMLFile) -> ShpReturn {
    
    use std::io::Cursor;
    use shapefile::{dbase::{FieldName, Record}, Point};
    use shapefile::record::polyline::Polyline;


    let mut shp_dest = Cursor::new(Vec::<u8>::new());
    let mut shx_dest = Cursor::new(Vec::<u8>::new());
    let mut dbf_dest = Cursor::new(Vec::<u8>::new());

    let shape_writer = shapefile::ShapeWriter::with_shx(&mut shp_dest, &mut shx_dest);

    let dbase_writer = dbase::TableWriterBuilder::new()
        // .add_character_field(FieldName::try_from("KUERZ").unwrap(), 10)
        // .add_character_field(FieldName::try_from("POLYID").unwrap(), 50)
        .build_with_dest(&mut dbf_dest);

    let shape_writer = shapefile::Writer::new(shape_writer, dbase_writer);

    let mut shapes = Vec::new();
    for l in lines {
        if l.points.is_empty() {
            continue;
        }

        shapes.push((
            Polyline::new(l.points.iter().map(|p| Point::new(p.x, p.y)).collect()), 
            Record::default(),
        ));
    }

    let shapes_ref = shapes.iter().map(|(a, b)| (a, b)).collect::<Vec<_>>();

    let _ = shape_writer.write_shapes_and_records(shapes_ref.into_iter());

    // https://www.geoportal-mv.de/portal/downloads/prj/25833.prj
    let prj = include_str!("./25833.prj");

    ShpReturn {
        shp: shp_dest.into_inner(),
        shx: shx_dest.into_inner(),
        dbf: dbf_dest.into_inner(),
        prj: prj.as_bytes().to_vec(),
        cpg: "UTF-8".as_bytes().to_vec(),
    }
}

pub fn export_aenderungen_geograf(aenderungen: &Aenderungen, split_nas: &SplitNasXml) -> Vec<u8> {

    let mut files = Vec::new();

    /*
    let aenderungen = match serde_json::from_str::<Aenderungen>(aenderungen.as_str()) {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    let xml = match serde_json::from_str::<NasXMLFile>(&xml) {
        Ok(o) => o,
        Err(e) => return e.to_string().as_bytes().to_vec(),
    };
    crate::geograf::export_aenderungen_geograf(&aenderungen, &xml)
     */


    write_files_to_zip(&files)
}