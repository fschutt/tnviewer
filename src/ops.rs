use crate::nas::translate_from_geo_poly;
use crate::nas::SvgPolygonInner;
use crate::nas::translate_to_geo_poly;
use crate::uuid_wasm::log_status;


// only called in stage5 (subtracting overlapping Aenderungen)
pub fn subtract_from_poly(
    original: &SvgPolygonInner,
    subtract: &[&SvgPolygonInner],
) -> SvgPolygonInner {
    use geo::BooleanOps;
    let mut first = original.round_to_3dec();
    for i in subtract.iter() {
        let mut fi = first.round_to_3dec();
        let mut i = i.round_to_3dec();
        fi.correct_winding_order();
        if fi.equals(&i) {
            return SvgPolygonInner::default();
        }
        i.correct_almost_touching_points(&fi, 0.05, true);
        let i = i.round_to_3dec();
        if i.is_zero_area() {
            continue;
        }
        if fi.is_zero_area() {
            return SvgPolygonInner::default();
        }
        let a = translate_to_geo_poly(&fi);
        let b = translate_to_geo_poly(&i);
        let join = a.difference(&b);
        let s = translate_from_geo_poly(&join);
        log_status(&serde_json::to_string(&s).unwrap_or_default());
        let outer_rings = s.iter().flat_map(|or| or.outer_rings.clone()).collect::<Vec<_>>();
        let inner_rings = s.iter().flat_map(|or| or.inner_rings.clone()).collect::<Vec<_>>();
        let new = SvgPolygonInner {
            outer_rings: outer_rings,
            inner_rings: inner_rings,
        };
        log_status(&serde_json::to_string(&new).unwrap_or_default());
        first = new;
    }

    log_status("returning");
    log_status(&serde_json::to_string(&first).unwrap_or_default());

    first
}

pub fn join_polys(
    polys: &[SvgPolygonInner],
    _autoclean: bool,
    _debug: bool,
) -> Option<SvgPolygonInner> {
    use geo::BooleanOps;
    let mut first = match polys.get(0) {
        Some(s) => s.round_to_3dec(),
        None => return None,
    };
    for i in polys.iter().skip(1) {
        let i = i.round_to_3dec();
        if first.equals(&i) {
            continue;
        }
        if i.is_empty() {
            continue;
        }
        let mut fi = first.round_to_3dec();
        fi.correct_winding_order();
        let a = translate_to_geo_poly(&fi);
        let b = translate_to_geo_poly(&i);
        let join = a.union(&b);
        let s = translate_from_geo_poly(&join);
        let new = SvgPolygonInner {
            outer_rings: s
                .iter()
                .flat_map(|s| s.outer_rings.clone().into_iter())
                .collect(),
            inner_rings: s
                .iter()
                .flat_map(|s| s.inner_rings.clone().into_iter())
                .collect(),
        };
        first = new;
    }

    first.correct_winding_order();
    Some(first)
}


macro_rules! define_func {
    ($fn_name:ident, $op:expr) => {
        pub fn $fn_name(a: &SvgPolygonInner, b: &SvgPolygonInner) -> Vec<SvgPolygonInner> {
            use geo::BooleanOps;

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

            let a = translate_to_geo_poly(&a);
            let b = translate_to_geo_poly(&b);
            let intersect = a.boolean_op(&b, $op);
            let mut s = translate_from_geo_poly(&intersect);

            for q in s.iter_mut() {
                q.correct_winding_order();
            }

            s
        }
    };
}

define_func!(intersect_polys, geo::OpType::Intersection);
