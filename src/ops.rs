use std::collections::BTreeMap;
use std::collections::BTreeSet;

use crate::nas::translate_from_geo_poly;
use crate::nas::translate_to_geo_poly_special_shared;
use crate::nas::SvgPoint;
use crate::nas::SvgPolygon;
use crate::nas::SvgPolygonInner;
use crate::nas::translate_to_geo_poly_special;
use crate::ui::dist_to_segment;
use crate::ui::Aenderungen;
use crate::ui::PolyNeu;
use crate::uuid_wasm::log_status;
// use crate::uuid_wasm::log_status;

// only called in stage5 (subtracting overlapping Aenderungen)

pub fn subtract_from_poly(
    original: &SvgPolygonInner,
    subtract: &[&SvgPolygonInner],
    debug: bool,
) -> Vec<SvgPolygonInner> {

    if subtract.is_empty() {
        return vec![original.clone()];
    }

    use geo::BooleanOps;
    if debug {
        log_status("subtract_from_poly");
        log_status(&serde_json::to_string(original).unwrap_or_default());
        log_status(&serde_json::to_string(subtract).unwrap_or_default());
    }

    let mut first = vec![original.round_to_3dec()];
    for i in subtract.iter() {
        let fi = first.iter().map(|s| s.round_to_3dec().correct_winding_order_cloned()).collect::<Vec<_>>();
        let mut i = i.round_to_3dec().correct_winding_order_cloned();
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


/*
fn insert_poly_points_from_near_polys(s: &[SvgPolygonInner], insert_all_points: bool) -> Vec<SvgPolygonInner> {
    const DST: f64 = 0.05;
    let mut snew = BTreeMap::new();
    for (qid, q) in s.iter().enumerate() {

        let mut q = q.clone();
        let q_rect = q.get_rect();
        let near_polys = s
            .iter()
            .enumerate()
            .filter(|(i, q)| *i != qid && q.get_rect().overlaps_rect(&q_rect))
            .map(|(i, it)| snew.get(&i).unwrap_or_else(|| it))
            .collect::<Vec<_>>();

        for n in near_polys.iter() {
            q.insert_points_from(n, DST, insert_all_points);
        } 

        snew.insert(qid, q);
    }
    snew.into_values().collect()
}
*/

fn merge_poly_points(s: &[SvgPolygonInner], original_pts: &[SvgPolygonInner]) -> Vec<SvgPolygonInner> {
    
    const DST: f64 = 0.1;

    let all_points_vec = original_pts.iter().flat_map(|s| s.get_all_points()).collect::<Vec<_>>();
    let max_items = all_points_vec.len().saturating_div(20).max(500).min(100);
    let ap_quadtree = quadtree_f32::QuadTree::new_with_max_items_per_quad(
        all_points_vec.iter().enumerate().map(|(i, s)| {
            (quadtree_f32::ItemId(i), quadtree_f32::Item::Rect(s.get_rect(DST)))
        }),
        max_items,
    );

    let mut s = s.to_vec();
    for p in s.iter_mut() {
        for o in p.outer_ring.points.iter_mut() {

            let mut near_points = ap_quadtree.get_ids_that_overlap(&o.get_rect(DST))
            .into_iter()
            .filter_map(|i| all_points_vec.get(i.0))
            .collect::<Vec<_>>();

            near_points.sort_by(|a, b| a.dist(&o).total_cmp(&b.dist(&o)));

            if let Some(first) = near_points.first() {
                if first.dist(o) < DST {
                    *o = SvgPoint {
                        x: first.x,
                        y: first.y,
                    };
                }
            }
        }
    }

    s
}

fn merge_poly_lines(s: &[SvgPolygonInner]) -> Vec<SvgPolygonInner> {
    
    let all_points_btree = s.iter().flat_map(|s| {
        crate::geograf::get_linecoords(s)
    }).collect::<BTreeSet<_>>();

    let all_points_btree = all_points_btree.into_iter().enumerate().map(|(i, ((sx, sy), (ex, ey)))| {
        (i, crate::nas::SvgLine {
            points: vec![crate::nas::SvgPoint { 
                x: sx as f64 / 1000.0, 
                y: sy as f64 / 1000.0, 
            }, crate::nas::SvgPoint { 
                x: ex as f64 / 1000.0, 
                y: ey as f64 / 1000.0, 
            }]
        })
    }).collect::<BTreeMap<_, _>>();

    let maxitems = all_points_btree.len().saturating_div(20).min(100).max(500);
    let ap_quadtree = quadtree_f32::QuadTree::new_with_max_items_per_quad(all_points_btree.iter().map(|(i, s)| {
        (quadtree_f32::ItemId(*i), quadtree_f32::Item::Rect(s.get_rect()))
    }), maxitems);

    const DST: f64 = 0.05;

    let mut s = s.to_vec();
    for p in s.iter_mut() {
        for o in p.outer_ring.points.iter_mut() {
            let mut near_lines = ap_quadtree.get_ids_that_overlap(&o.get_rect(DST))
            .into_iter()
            .filter_map(|q| all_points_btree.get(&q.0))
            .filter_map(|s| Some(dist_to_segment(*o, s.points.get(0)?.clone(), s.points.get(1)?.clone())))
            .collect::<Vec<_>>();
            near_lines.sort_by(|a, b| a.distance.total_cmp(&b.distance));
            if let Some(first) = near_lines.first() {
                *o = first.nearest_point;
            }
        }
    }

    s
}


pub fn join_polys_special_2(polys_orig: &[SvgPolygonInner], i: &SvgPolygonInner, debug: bool, insert_all_points: bool) -> Vec<SvgPolygonInner> {
    use geo::BooleanOps;

    let mut first = polys_orig.to_vec();

        let mut i = i.clone();

        for q in first.iter_mut() {
            i.insert_points_from(q, 0.1, insert_all_points);
            q.insert_points_from(&i, 0.1, insert_all_points);
            if i.is_completely_inside_of(q) {
                return first;
            }
        }

        if debug {
            log_status(&format!("joining first (insert_all_points: {insert_all_points:?}):"));
            log_status(&serde_json::to_string(&first).unwrap_or_default());
            log_status(&format!("joining second (insert_all_points: {insert_all_points:?}):"));
            log_status(&serde_json::to_string(&i).unwrap_or_default());
        }

        if i.is_zero_area() {
            return first;
        }

        let a = translate_to_geo_poly_special(&first);
        let b = translate_to_geo_poly_special_shared(&[&i]);
        let join = a.union(&b);
        first = translate_from_geo_poly(&join).iter().flat_map(crate::nas::cleanup_poly).collect::<Vec<_>>();

    first
}


pub fn join_polys_special(polys_orig: &[SvgPolygonInner], last: &SvgPolygonInner, debug: bool, insert_all_points: bool) -> Vec<SvgPolygonInner> {
    use geo::BooleanOps;
    let mut first = polys_orig.to_vec();

    // loop

        let mut i = last.clone();

        for q in first.iter_mut() {
            i.insert_points_from(q, 0.1, insert_all_points);
            q.insert_points_from(&i, 0.1, insert_all_points);
            if i.is_completely_inside_of(q) {
                continue;
            }
        }

        if debug {
            log_status(&format!("joining first (insert_all_points: {insert_all_points:?}):"));
            log_status(&serde_json::to_string(&first).unwrap_or_default());
            log_status(&format!("joining second (insert_all_points: {insert_all_points:?}):"));
            log_status(&serde_json::to_string(&i).unwrap_or_default());
        }

        let a = translate_to_geo_poly_special(&first);
        let b = translate_to_geo_poly_special_shared(&[&i]);
        let join = a.union(&b);
        first = translate_from_geo_poly(&join).iter().flat_map(crate::nas::cleanup_poly).collect::<Vec<_>>();

    first
}

pub fn join_polys(polys_orig: &[SvgPolygonInner], debug: bool, insert_all_points: bool) -> Vec<SvgPolygonInner> {
    use geo::BooleanOps;

    if polys_orig.len() < 2 {
        return polys_orig.to_vec();
    }

    if debug {
        log_status("join_polys");
        log_status(&serde_json::to_string(polys_orig).unwrap_or_default());
    }

    let mut polys = polys_orig.to_vec();
    polys.sort_by(|a, b| a.area_m2().abs().total_cmp(&b.area_m2().abs()));
    polys.dedup_by(|a, b| a.get_all_pointcoords_sorted() ==  b.get_all_pointcoords_sorted());


    if debug {
        log_status("2.5");
    }

    let polys = polys_orig.iter().flat_map(crate::nas::cleanup_poly).collect::<Vec<_>>();
    if debug {
        log_status("3.5");
    }
    // let polys = insert_poly_points_from_near_polys(&polys, insert_all_points);
    if debug {
        log_status("4.5");
    }
    let polys = merge_poly_points(&polys, &polys_orig);
    if debug {
        log_status("5.5");
    }
    let polys = polys.iter().flat_map(crate::nas::cleanup_poly).collect::<Vec<_>>();
    if debug {
        log_status("6.5");
    }
    let polys = merge_poly_lines(&
        polys.iter().map(|s| s.round_to_3dec()).collect::<Vec<_>>()
    ).into_iter().map(|s| s.round_to_3dec()).collect::<Vec<_>>();
    if debug {
        log_status("7.5");
    }
    let mut polys = merge_poly_points(&polys, &polys);
    if debug {
        log_status("8.5");
    }
    // let mut polys = insert_poly_points_from_near_polys(&polys, insert_all_points);
    if debug {
        log_status("9.5");
    }
    if debug {
        log_status("2");
    }

    polys.sort_by(|a, b| a.area_m2().abs().total_cmp(&b.area_m2().abs()));
    polys.dedup_by(|a, b| a.get_all_pointcoords_sorted() ==  b.get_all_pointcoords_sorted());
    polys.reverse(); // largest polys first
    polys.retain(|s| !s.is_zero_area());

    if debug {
        log_status("3");
    }

    let mut first = match polys.get(0) {
        Some(s) => vec![s.clone()],
        None => return Vec::new(),
    };

    for i in polys.iter().skip(1) {
        let mut i = i.clone();

        for q in first.iter_mut() {
            i.insert_points_from(q, 0.1, insert_all_points);
            q.insert_points_from(&i, 0.1, insert_all_points);
            if i.is_completely_inside_of(q) {
                continue;
            }
        }

        if debug {
            log_status(&format!("joining first (insert_all_points: {insert_all_points:?}):"));
            log_status(&serde_json::to_string(&first).unwrap_or_default());
            log_status(&format!("joining second (insert_all_points: {insert_all_points:?}):"));
            log_status(&serde_json::to_string(&i).unwrap_or_default());
        }

        if i.is_zero_area() {
            continue;
        }

        let a = translate_to_geo_poly_special(&first);
        let b = translate_to_geo_poly_special_shared(&[&i]);
        let join = a.union(&b);
        first = translate_from_geo_poly(&join).iter().flat_map(crate::nas::cleanup_poly).collect::<Vec<_>>();
    }

    first
}


pub fn join_polys_fast(polys: &[SvgPolygonInner], debug: bool, insert_all_points: bool) -> Vec<SvgPolygonInner> {
    use geo::BooleanOps;

    if debug {
        log_status("join_polys");
        log_status(&serde_json::to_string(polys).unwrap_or_default());
    }

    let mut polys = polys.iter().flat_map(crate::nas::cleanup_poly).collect::<Vec<_>>();
    polys.sort_by(|a, b| a.area_m2().abs().total_cmp(&b.area_m2().abs()));
    polys.dedup_by(|a, b| a.get_all_pointcoords_sorted() ==  b.get_all_pointcoords_sorted());
    let polys = merge_poly_lines(&
        polys.iter().map(|s| s.round_to_3dec()).collect::<Vec<_>>()
    ).into_iter().map(|s| s.round_to_3dec()).collect::<Vec<_>>();
    let polys = merge_poly_points(&polys, &polys);
    let mut polys = polys;
    polys.reverse(); // largest polys first
    polys.retain(|s| !s.is_zero_area());

    if polys.len() < 2 {
        return polys.to_vec();
    }

    let mut first = match polys.get(0) {
        Some(s) => vec![s.clone()],
        None => return Vec::new(),
    };

    for i in polys.iter().skip(1) {
        let mut i = i.clone();
        log_status(&serde_json::to_string(&i).unwrap_or_default());

        log_status("1");
        for q in first.iter_mut() {
            i.insert_points_from(q, 0.1, insert_all_points);
            q.insert_points_from(&i, 0.1, insert_all_points);
            if i.is_completely_inside_of(q) {
                continue;
            }
        }

        log_status("2");

        if i.is_zero_area() {
            continue;
        }

        log_status("3");

        let a = translate_to_geo_poly_special(&first);
        let b = translate_to_geo_poly_special_shared(&[&i]);
        let join = a.union(&b);
        log_status("joined!");
        first = translate_from_geo_poly(&join);
        log_status("merging lines...");
        first = merge_poly_lines(&
            first.iter().map(|s| s.round_to_3dec()).collect::<Vec<_>>()
        ).into_iter().map(|s| s.round_to_3dec()).collect::<Vec<_>>();
        log_status("merging points...");
        first = merge_poly_points(&first, &first);
        log_status("next poly!");
    }
    log_status("done!");
    first
}


pub fn intersect_polys(a: &SvgPolygonInner, b: &SvgPolygonInner, debug: bool) -> Vec<SvgPolygonInner> {
    use geo::BooleanOps;

    if debug {
        log_status("intersect polys");
        log_status(&serde_json::to_string(&a).unwrap_or_default());
        log_status(&serde_json::to_string(&b).unwrap_or_default());    
    }

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

    // log_status("intersect polys done");
    s
}
