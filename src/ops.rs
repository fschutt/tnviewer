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
    log_status("subtract_from_poly");
    log_status(&serde_json::to_string(original).unwrap_or_default());
    log_status(&serde_json::to_string(subtract).unwrap_or_default());
    let mut first = vec![original.round_to_3dec()];
    for i in subtract.iter() {
        let fi = first.iter().map(|s| s.round_to_3dec().correct_winding_order_cloned()).collect::<Vec<_>>();
        let mut i = i.round_to_3dec();
        if fi.iter().all(|s| s.equals(&i)) {
            return Vec::new();
        }
        for f in fi.iter() {
            i.correct_almost_touching_points(&f, 0.05, true);
        }
        let i = i.round_to_3dec();
        if i.is_zero_area() {
            continue;
        }
        if fi.iter().all(|s| s.is_zero_area()) {
            return Vec::new();
        }
        let a = translate_to_geo_poly_special(&fi);
        let b = translate_to_geo_poly_special_shared(&[&i]);
        let join = a.difference(&b);
        let s = translate_from_geo_poly(&join);
        first = s;
    }

    let s = first.iter().map(|s| s.correct_winding_order_cloned()).collect::<Vec<_>>();
    log_status("subtract_from_poly done");
    s
}


pub fn join_polys(polys: &[SvgPolygonInner]) -> Vec<SvgPolygonInner> {
    use geo::BooleanOps;
    log_status("join_polys");
    log_status(&serde_json::to_string(polys).unwrap_or_default());
    let first = match polys.get(0) {
        Some(s) => vec![s.clone()],
        None => return Vec::new(),
    };
    let a = translate_to_geo_poly_special(&first);
    let b = translate_to_geo_poly_special_shared(&polys.iter().skip(1).collect::<Vec<_>>());
    let join = a.union(&b);
    let s = translate_from_geo_poly(&join);
    let s = s.iter().map(|s| s.correct_winding_order_cloned()).collect::<Vec<_>>();
    log_status("join_polys done");
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
