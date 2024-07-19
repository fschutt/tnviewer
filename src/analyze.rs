use serde_derive::{Serialize, Deserialize};
use crate::SvgPolygon;
use crate::nas::SplitNasXml;
use crate::nas::SvgLine;
use crate::nas::SvgPoint;

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct LatLng {
    pub lat: f64,
    pub lng: f64,
}

pub fn fixup_polyline_internal(points: &[LatLng], split_fs: &SplitNasXml) -> Option<SvgPolygon> {
    
    let mut points = points.to_vec();
    
    /*
    // TODO 
    let bounds = get_bounds(&points);
    let potential_touching_objects = split_fs.get_all_obj_in_bounds(&bounds);
    let path_to_close_poly = a_start_search(&potential_touching_objects, points.first()?, points.last()?);
    */

    if points.first()? != points.last()? {
        points.push(points.first()?.clone());
    }

    Some(SvgPolygon {
        outer_rings: vec![SvgLine {
            points: points.iter().map(|p| {
                SvgPoint {
                    x: p.lng,
                    y: p.lat,
                }
            }).collect(),
        }],
        inner_rings: Vec::new()
    })
}

/* 
fn get_bounds(points: &[LatLng]) -> [[f64;2];2] {

    let mut min_x = self.poly.outer_rings.get(0).and_then(|s| s.points.get(0)).map(|p| p.x).unwrap_or(0.0);
    let mut max_x = self.poly.outer_rings.get(0).and_then(|s| s.points.get(0)).map(|p| p.x).unwrap_or(0.0);
    let mut min_y = self.poly.outer_rings.get(0).and_then(|s| s.points.get(0)).map(|p| p.y).unwrap_or(0.0);
    let mut max_y = self.poly.outer_rings.get(0).and_then(|s| s.points.get(0)).map(|p| p.y).unwrap_or(0.0);
    for l in self.poly.outer_rings.iter() {
        for p in l.points.iter() {
            if p.x > max_x {
                max_x = p.x;
            }
            if p.y > max_y {
                max_y = p.y;
            }
            if p.x < min_x {
                min_x = p.x;
            }
            if p.y < min_y {
                min_y = p.y;
            }
        }
    }

    [
        [min_y, min_x],
        [max_y, max_x]
    ]
}
*/