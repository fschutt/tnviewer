use crate::{nas::{translate_geoline, translate_to_geo_poly, SvgLine, SvgPoint, SvgPolygon}, pdf::{FlurstueckeInPdfSpace, GebaeudeInPdfSpace, RissExtentReprojected}, ui::TextPlacement, uuid_wasm::log_status};

pub struct OptimizedTextPlacement {
    pub rect: SvgLine,
    pub original: TextPlacement,
    pub optimized: TextPlacement,
}

pub const LABEL_HEIGHT_M: f64 = 10.0;
pub const LABEL_WIDTH_M: f64 = 10.0;

pub fn optimize_labels(
    flurstuecke: &FlurstueckeInPdfSpace,
    gebaeude: &GebaeudeInPdfSpace,
    avoid_areas_in_pdf_space: &[SvgPolygon],
    initial_text_pos: &[TextPlacement],
    riss_extent: &RissExtentReprojected,
    tolerance: f64,
) -> Vec<OptimizedTextPlacement> {

    let width_pixels = (riss_extent.width_m() / tolerance).ceil() as usize;
    let height_pixels = (riss_extent.height_m() / tolerance).ceil() as usize;

    let mut overlap_boolmap = match render_overlap_boolmap(
        flurstuecke,
        gebaeude,
        avoid_areas_in_pdf_space, 
        width_pixels, 
        height_pixels
    ) {
        Some(s) => s,
        None => return initial_text_pos.iter().map(|s| {
            OptimizedTextPlacement {
                rect: svg_label_pos_to_line(&s.pos),
                optimized: s.clone(),
                original: s.clone(),
            }
        }).collect(),
    };

    let maxiterations = 2;
    let mut initial_text_pos_clone = initial_text_pos.to_vec();
    for tp in initial_text_pos_clone.iter_mut() {
        let mut textpos_totry = vec![tp.pos];
        let mut textpos_found = None;
        'outer: for _ in 0..maxiterations {
            let mut newaccum_pos = Vec::new();
            for newpostotry in textpos_totry.iter() {
                if !label_overlaps_feature(
                    newpostotry,
                    &overlap_boolmap,
                    riss_extent,
                    tolerance
                ) {
                    // mark region as occupied
                    textpos_found = Some(*newpostotry);
                    paint_label_onto_map(
                        newpostotry,
                        &mut overlap_boolmap,
                        riss_extent,
                        tolerance
                    );
                    break 'outer;
                } else {
                    newaccum_pos.append(&mut gen_new_points(newpostotry));
                }
            }
            if !newaccum_pos.is_empty() {
                textpos_totry = newaccum_pos;
            }

        }
        if let Some(found) = textpos_found {
            tp.pos = found;
        }
    }

    log_status(&format!("OK rendered map! {width_pixels} x {height_pixels} pixels"));

    initial_text_pos.iter().zip(initial_text_pos_clone.iter()).map(|(old, new)| {
        OptimizedTextPlacement {
            rect: svg_label_pos_to_line(&new.pos),
            optimized: new.clone(),
            original: old.clone(),
        }
    }).collect()
}

fn gen_new_points(p: &SvgPoint) -> Vec<SvgPoint> {
    let label_height_half = LABEL_HEIGHT_M / 2.0;
    let label_width_half = LABEL_HEIGHT_M / 2.0;
    let xpos = vec![
        -LABEL_WIDTH_M,
        -label_width_half,
        0.0,
        label_width_half,
        LABEL_WIDTH_M,
    ];
    let ypos = vec![
        -LABEL_HEIGHT_M,
        -label_height_half,
        0.0,
        label_height_half,
        LABEL_HEIGHT_M,
    ];
    xpos.iter().flat_map(|xshift| ypos.iter().filter_map(|yshift| {
        if *xshift == 0.0 && *yshift == 0.0 {
            None
        } else {
            Some(p.translate(*xshift, *yshift))
        }
    })).collect()
}

fn svg_label_pos_to_line(p: &SvgPoint) -> SvgLine {
    SvgLine {
        points: vec![
            *p,
            SvgPoint {
                x: p.x + LABEL_WIDTH_M,
                y: p.y,
            },
            SvgPoint {
                x: p.x + LABEL_WIDTH_M,
                y: p.y + LABEL_HEIGHT_M,
            },
            SvgPoint {
                x: p.x,
                y: p.y + LABEL_HEIGHT_M,
            },
            *p,
        ]
    }
}

// project point from pdf space to pixel space
fn project_point(
    p: &SvgPoint,
    riss_extent: &RissExtentReprojected,
    tolerance: f64,
) -> (usize, usize) {
    let x_to_right = p.x - riss_extent.min_x;
    let y_to_bottom = p.y - riss_extent.min_y;
    ((x_to_right / tolerance).round() as usize, (y_to_bottom / tolerance).round() as usize)
}

// project point from pixel space to pdf space
fn unproject_point(
    (x, y): (usize, usize),
    riss_extent: &RissExtentReprojected,
    tolerance: f64,
) -> SvgPoint {
    
    let width_pixels = (riss_extent.width_m() / tolerance).ceil() as usize;
    let height_pixels = (riss_extent.height_m() / tolerance).ceil() as usize;
    let one_px_x = riss_extent.width_m() / width_pixels as f64;
    let one_px_y = riss_extent.height_m() / height_pixels as f64;
    SvgPoint {
        x: riss_extent.min_x + (x as f64 * one_px_x),
        y: riss_extent.min_y + (y as f64 * one_px_y),
    }
}


fn paint_label_onto_map(
    point: &SvgPoint,
    map: &mut ndarray::Array2<bool>,
    riss_extent: &RissExtentReprojected,
    tolerance: f64,
) -> bool {

    let (x, y) = project_point(point, riss_extent, tolerance);
    let label_height_px = (LABEL_HEIGHT_M / tolerance).ceil() as usize;
    let label_width_px = (LABEL_WIDTH_M / tolerance).ceil() as usize;
    
    for y_test in y..(y + label_height_px) {
        for x_xest in x..(x + label_width_px) {
            match map.get_mut((y_test, x_xest)) {
                Some(s) => { *s = true; },
                _ => { },
            }
        }
    }
    
    false
}


fn label_overlaps_feature(
    point: &SvgPoint,
    map: &ndarray::Array2<bool>,
    riss_extent: &RissExtentReprojected,
    tolerance: f64,
) -> bool {

    let (x, y) = project_point(point, riss_extent, tolerance);
    let label_height_px = (LABEL_HEIGHT_M / tolerance).ceil() as usize;
    let label_width_px = (LABEL_WIDTH_M / tolerance).ceil() as usize;
    
    for y_test in y..(y + label_height_px) {
        for x_xest in x..(x + label_width_px) {
            match map.get((y_test, x_xest)) {
                Some(s) => if *s { return true; },
                _ => { },
            }
        }
    }
    
    false
}

fn render_overlap_boolmap(
    flurstuecke: &FlurstueckeInPdfSpace,
    gebaeude: &GebaeudeInPdfSpace,
    do_not_overlap_areas: &[SvgPolygon],
    width_pixels: usize,
    height_pixels: usize,
) -> Option<ndarray::Array2<bool>> {
    use geo_rasterize::BinaryBuilder;

    let mut r = BinaryBuilder::new().width(width_pixels).height(height_pixels).build().ok()?;
    for shape in do_not_overlap_areas.iter() {
        r.rasterize(&translate_to_geo_poly(shape)).ok()?;
    }
    for flst in flurstuecke.flst.iter() {
        for line in flst.poly.outer_rings.iter() {
            r.rasterize(&translate_geoline(line)).ok()?;
        }
        for line in flst.poly.inner_rings.iter() {
            r.rasterize(&translate_geoline(line)).ok()?;
        }
    }

    for area in gebaeude.gebaeude.iter() {
        r.rasterize(&translate_to_geo_poly(&area.poly)).ok()?;
    }

    let pixels = r.finish();
    
    Some(pixels)
}