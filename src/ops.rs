use crate::nas::translate_from_geo_poly;
use crate::nas::translate_to_geo_poly_special_shared;
use crate::nas::SvgPolygonInner;
use crate::nas::translate_to_geo_poly_special;
use crate::uuid_wasm::log_status;

// only called in stage5 (subtracting overlapping Aenderungen)
pub fn subtract_from_poly(
    original: &SvgPolygonInner,
    subtract: &[&SvgPolygonInner],
) -> Vec<SvgPolygonInner> {
    use geo::BooleanOps;
    log_status("subtract from poly...");
    log_status(&serde_json::to_string(original).unwrap_or_default());
    log_status(&serde_json::to_string(subtract).unwrap_or_default());

    let mut first = vec![original.round_to_3dec().correct_winding_order_cloned()];
    let mut to_subtract = subtract.iter().filter_map(|s| {
        let s = s.round_to_3dec().correct_winding_order_cloned();
        if s.is_zero_area() {
            None
        } else {
            Some(s)
        }
    }).collect::<Vec<_>>();

    if to_subtract.is_empty() {
        return first;
    }

    for s in to_subtract.iter_mut() {
        *s = s.round_to_3dec();
        for q in first.iter() {
            s.correct_almost_touching_points(q, 0.05, true);
        }
        let a = translate_to_geo_poly_special(&first);
        let b = translate_to_geo_poly_special_shared(&[&s]);
        let join = a.difference(&b);
        first = translate_from_geo_poly(&join)
        .into_iter()
        .filter_map(|s| if s.is_zero_area() { 
            None 
        } else { 
            Some(s.round_to_3dec().correct_winding_order_cloned()) 
        })
        .collect();
    }

    log_status("subtract from poly done!");

    first
}

pub fn join_polys(polys: &[SvgPolygonInner]) -> Vec<SvgPolygonInner> {
    log_status("join polys");
    log_status(&serde_json::to_string(polys).unwrap_or_default());
    use geo::BooleanOps;
    let mut first = match polys.get(0) {
        Some(s) => vec![s.round_to_3dec().correct_winding_order_cloned()],
        None => return Vec::new(),
    };
    let mut other = polys.iter().skip(1).filter_map(|t| {
        let t = t.round_to_3dec().correct_winding_order_cloned();
        if t.is_zero_area() {
            None
        } else {
            Some(t)
        }
    }).collect::<Vec<_>>();
    if other.is_empty() {
        return first;
    }
    for s in other.iter_mut() {
        *s = s.round_to_3dec();
        for q in first.iter() {
            s.correct_almost_touching_points(q, 0.05, true);
        }
        let a = translate_to_geo_poly_special(&first);
        let b = translate_to_geo_poly_special_shared(&[&s]);
        let join = a.union(&b);
        first = translate_from_geo_poly(&join);
    }

    log_status("done!");
    first
}

pub fn intersect_polys(a: &SvgPolygonInner, b: &SvgPolygonInner) -> Vec<SvgPolygonInner> {
    use geo::BooleanOps;

    log_status("intersect polys");
    log_status(&serde_json::to_string(&a).unwrap_or_default());
    log_status(&serde_json::to_string(&b).unwrap_or_default());

    let mut a = a.round_to_3dec();
    let mut b = b.round_to_3dec();
    a.correct_winding_order();
    b.correct_winding_order();

    if a.is_zero_area() {
        return Vec::new();
    }
    if b.is_zero_area() {
        return Vec::new();
    }

    if a.equals(&b) {
        return vec![a];
    }

    let a = translate_to_geo_poly_special(&[a]);
    let b = translate_to_geo_poly_special(&[b]);
    let intersect = a.boolean_op(&b, geo::OpType::Intersection);
    let mut s = translate_from_geo_poly(&intersect);

    for q in s.iter_mut() {
        q.correct_winding_order();
    }

    log_status("intersect polys done");
    s
}
