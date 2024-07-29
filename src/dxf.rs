use std::io::BufWriter;

use dxf::{Vector, XData, XDataItem};

use crate::ui::Aenderungen;

pub fn export_aenderungen_dxf(aenderungen: &Aenderungen) -> Vec<u8> {
    use dxf::Drawing;
    use dxf::entities::*;
    
    let mut drawing = Drawing::new();
    for (k, v) in aenderungen.na_polygone_neu.iter() {
        let mut lines = v.poly.outer_rings.clone();
        lines.append(&mut v.poly.inner_rings.clone());
        
        for l in lines {
            for ab in l.points.windows(2) {
                match &ab {
                    &[a, b] => { 
                        let mut entity = Entity::new(EntityType::Line(Line {
                            thickness: 1.0,
                            p1: dxf::Point { x: a.x, y: a.y, z: 0.0 },
                            p2: dxf::Point { x: b.x, y: b.y, z: 0.0 },
                            extrusion_direction: Vector::x_axis(),
                        }));

                        entity.common.x_data = vec![XData {
                            application_name: "tnviewer".to_string(),
                            items: vec![
                                XDataItem::Str(format!("category:{}", v.nutzung.clone().unwrap_or_default())),
                                XDataItem::Str(format!("changeset:polynew:{}", k.clone())),
                            ],
                        }];

                        let _entity_ref = drawing.add_entity(entity);
                    },
                    _ => { },
                }
            }
        }
    }

    let mut v = Vec::new();
    let mut buf = BufWriter::new(v);
    drawing.save_binary(&mut buf);
    buf.into_inner().unwrap_or_default()
}
