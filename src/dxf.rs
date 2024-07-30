use std::{io::BufWriter, path::PathBuf};

use dxf::{Vector, XData, XDataItem};
use wasm_bindgen::JsValue;
use web_sys::js_sys::JsString;

use crate::{nas::{NasXMLFile, LATLON_STRING}, ui::Aenderungen};

pub fn export_aenderungen_dxf(aenderungen: &Aenderungen, xml: &NasXMLFile) -> Vec<u8> {
    use dxf::Drawing;
    use dxf::entities::*;

    let aenderungen = match crate::pdf::reproject_aenderungen_into_target_space(aenderungen, &xml.crs) {
        Ok(o) => o,
        Err(e) => return e.as_bytes().to_vec(),
    };

    let texte = aenderungen.get_texte(xml);

    web_sys::console::log_1(&"TEXTE: ".into());
    web_sys::console::log(&texte.iter().filter_map(|s| serde_json::to_string(s).ok()).map(JsString::from).map(JsValue::from).collect());

    let mut drawing = Drawing::new();

    for text in texte {
        let entity = Entity::new(EntityType::Text(dxf::entities::Text {
            thickness: 1.0,
            location: dxf::Point { x: text.pos.x, y: text.pos.y, z: 0.0 },
            text_height: 10.0,
            value: text.kuerzel.clone(),
            rotation: 0.0,
            relative_x_scale_factor: 0.0,
            oblique_angle: 0.0,
            text_style_name: match text.status {
                crate::ui::TextStatus::Old => "old",
                crate::ui::TextStatus::New => "new",
                crate::ui::TextStatus::StaysAsIs => "stayasis",
            }.to_string(),
            text_generation_flags: 0,
            second_alignment_point: dxf::Point { x: text.pos.x, y: text.pos.y, z: 0.0 },
            normal: Vector {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
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

pub fn export_aenderungen_shp(aenderungen: &Aenderungen, xml: &NasXMLFile) -> Vec<u8> {
    
    use std::io::Cursor;
    use shapefile::{dbase::{FieldName, Record}, Point};
    use shapefile::record::polyline::Polyline;

    let aenderungen = match crate::pdf::reproject_aenderungen_into_target_space(aenderungen, &xml.crs) {
        Ok(o) => o,
        Err(e) => return e.as_bytes().to_vec(),
    };

    let mut shp_dest = Cursor::new(Vec::<u8>::new());
    let mut shx_dest = Cursor::new(Vec::<u8>::new());
    let mut dbf_dest = Cursor::new(Vec::<u8>::new());

    let shape_writer = shapefile::ShapeWriter::with_shx(&mut shp_dest, &mut shx_dest);

    let dbase_writer = dbase::TableWriterBuilder::new()
        .add_character_field(FieldName::try_from("KUERZ").unwrap(), 10)
        .add_character_field(FieldName::try_from("POLYID").unwrap(), 50)
        .build_with_dest(&mut dbf_dest);

    let shape_writer = shapefile::Writer::new(shape_writer, dbase_writer);

    let mut shapes = Vec::new();
    for (k, v) in aenderungen.na_polygone_neu.iter() {
        let mut lines = v.poly.outer_rings.clone();
        lines.append(&mut v.poly.inner_rings.clone());
        
        for l in lines {
            if l.points.is_empty() {
                continue;
            }

            shapes.push((
                Polyline::new(l.points.iter().map(|p| Point::new(p.x, p.y)).collect()), 
                { 
                    let mut d = Record::default(); 
                    d.insert("KUERZ".to_string(), dbase::FieldValue::Character(v.nutzung.clone())); 
                    d.insert("POLYID".to_string(), dbase::FieldValue::Character(Some(k.clone()))); 
                    d 
                },
            ));
        }
    }

    let shapes_ref = shapes.iter().map(|(a, b)| (a, b)).collect::<Vec<_>>();

    let _ = shape_writer.write_shapes_and_records(shapes_ref.into_iter());

    // https://www.geoportal-mv.de/portal/downloads/prj/25833.prj
    let prj = include_str!("./25833.prj");

    crate::zip::write_files_to_zip(
        &[
            (None, PathBuf::from("test.shp"), shp_dest.into_inner()),
            (None, PathBuf::from("test.shx"), shx_dest.into_inner()),
            (None, PathBuf::from("test.dbf"), dbf_dest.into_inner()),
            (None, PathBuf::from("test.prj"), prj.as_bytes().to_vec()),
            (None, PathBuf::from("test.cpg"), "UTF-8".as_bytes().to_vec())
        ]
    )
}
