use crate::nas::translate_from_geo_poly;
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

    let first = vec![original.round_to_3dec().correct_winding_order_cloned()];
    let to_subtract = subtract.iter().filter_map(|s| {
        let s = s.round_to_3dec().correct_winding_order_cloned();
        if s.is_zero_area() {
            None
        } else {
            Some(s)
        }
    }).collect::<Vec<_>>();

    let a = translate_to_geo_poly_special(&first);
    let b = translate_to_geo_poly_special(&to_subtract);
    let join = a.difference(&b);
    let s = translate_from_geo_poly(&join)
    .into_iter()
    .filter_map(|s| if s.is_zero_area() { 
        None 
    } else { 
        Some(s.round_to_3dec().correct_winding_order_cloned()) 
    })
    .collect();

    log_status("subtract from poly done!");
    s
}

pub fn join_polys(polys: &[SvgPolygonInner]) -> Vec<SvgPolygonInner> {
    log_status("join polys");
    log_status(&serde_json::to_string(polys).unwrap_or_default());
    use geo::BooleanOps;
    let first = match polys.get(0) {
        Some(s) => vec![s.round_to_3dec().correct_winding_order_cloned()],
        None => return Vec::new(),
    };
    let other = polys.iter().skip(1).filter_map(|t| {
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
    let a = translate_to_geo_poly_special(&first);
    let b = translate_to_geo_poly_special(&other);
    let join = a.union(&b);
    let s = translate_from_geo_poly(&join);
    log_status("done!");
    s
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
