use std::{collections::BTreeMap, f64::consts::PI};

use ndarray::Axis;
use web_sys::console::log_1;

use crate::{nas::{translate_geoline, translate_to_geo_poly, SplitNasXml, SvgLine, SvgPoint, SvgPolygon}, pdf::{Flurstuecke, FlurstueckeInPdfSpace, Gebaeude, GebaeudeInPdfSpace, RissConfig, RissExtentReprojected}, ui::{AenderungenIntersection, TextPlacement}, uuid_wasm::{js_random, log_status, uuid}};

pub struct OptimizedTextPlacement {
    pub original: TextPlacement,
    pub optimized: TextPlacement,
}

impl OptimizedTextPlacement {
    pub fn get_line(&self) -> Option<(SvgPoint, SvgPoint)> {
        if crate::nas::point_is_in_polygon(&self.optimized.pos, &self.optimized.poly) {
            return None; 
        }

        let (a, b) = (self.optimized.pos, self.optimized.ref_pos);
        Some((a, b))
    }
}

pub const LABEL_HEIGHT_M: f64 = 13.0;
pub const LABEL_WIDTH_M: f64 = 20.0;
pub const LABEL_WIDTH_PER_CHAR_M: f64 = 5.0;

pub struct OptimizeConfig {
    tolerance: f64,
    riss_config: RissConfig,
    riss_extent: RissExtentReprojected,
    width_pixels: usize,
    height_pixels: usize,
    one_px_x_in_m: f64,
    one_px_y_in_m: f64,
}

#[derive(Debug, Clone)]
pub struct Pixel {
    pub x: usize,
    pub y: usize,
}

impl OptimizeConfig {

    pub fn new(
        riss_config: &RissConfig,
        riss_extent: &RissExtentReprojected,
        tolerance: f64,
    ) -> Self {
        let how_many_pixels_x = (riss_config.width_mm as f64 / tolerance).round() as usize;
        let how_many_pixels_y = (riss_config.height_mm  as f64 / tolerance).round() as usize;
        let one_px_x_in_m = riss_extent.width_m() / how_many_pixels_x as f64;
        let one_px_y_in_m = riss_extent.height_m() / how_many_pixels_y as f64;
        Self { 
            tolerance, 
            riss_config: riss_config.clone(), 
            riss_extent: riss_extent.clone(), 
            one_px_x_in_m: one_px_x_in_m, 
            one_px_y_in_m: one_px_y_in_m,
            width_pixels: how_many_pixels_x,
            height_pixels: how_many_pixels_y,
        }
    }

    pub fn point_to_pixel(&self, point: &SvgPoint) -> Pixel {
        let ts = self.translate_svg_point_to_pixel_space(point);
        Pixel { 
            x: ts.x.round() as usize, 
            y: ts.y.round() as usize, 
        }
    }

    pub fn pixel_to_point(&self, pixel: &Pixel) -> SvgPoint {
        SvgPoint { 
            x: (pixel.x as f64 * self.one_px_x_in_m) + self.riss_extent.min_x, 
            y: self.riss_extent.max_y - (pixel.y as f64 * self.one_px_y_in_m), 
        }
    }

    pub fn label_height_pixel(&self) -> usize {
        (LABEL_HEIGHT_M / self.one_px_y_in_m).ceil() as usize
    }

    pub fn label_width_pixel(&self, lw_meter: f64) -> usize {
        (lw_meter / self.one_px_x_in_m).ceil() as usize
    }

    fn translate_svg_point_to_pixel_space(&self, point: &SvgPoint) -> SvgPoint {
        SvgPoint { 
            x: (point.x - self.riss_extent.min_x) / self.one_px_x_in_m, 
            y: (self.riss_extent.max_y - point.y) / self.one_px_y_in_m, 
        }
    }

    pub fn polygon_to_pixel_space(&self, poly: &SvgPolygon) -> SvgPolygon {
        SvgPolygon {
            outer_rings: poly.outer_rings.iter().map(|p| self.line_to_pixel_space(p)).collect(),
            inner_rings: poly.inner_rings.iter().map(|p| self.line_to_pixel_space(p)).collect(),
        }
    }

    pub fn line_to_pixel_space(&self, line: &SvgLine) -> SvgLine {
        SvgLine {
            points: line.points.iter().map(|p| self.translate_svg_point_to_pixel_space(p)).collect()
        }
    }
}

fn render_boolmap(
    map: &ndarray::Array2<bool>
) -> Vec<String> {
    map.axis_iter(Axis(0))
    .map(|row| {
        row.iter().map(|a| if *a { '■' } else { '□' }).collect::<String>()
    })
    .collect()
}

pub fn optimize_labels(
    flurstuecke: &SplitNasXml,
    splitflaechen: &[AenderungenIntersection],
    gebaeude: &Gebaeude,
    avoid_areas_in_pdf_space: &[SvgPolygon],
    initial_text_pos: &[TextPlacement],
    config: &OptimizeConfig,
) -> Vec<OptimizedTextPlacement> {

    let initial = initial_text_pos.iter().map(|s| {
        OptimizedTextPlacement {
            optimized: s.clone(),
            original: s.clone(),
        }
    }).collect();

    let background_boolmap = match render_stage1_overlap_boolmap(
        flurstuecke,
        splitflaechen,
        gebaeude,
        avoid_areas_in_pdf_space, 
        config,
    ) {
        Some(s) => s,
        None => return initial,
    };

    let mut label_boolmap = match initialize_empty_boolmap(config) {
        Some(s) => s,
        None => return initial,
    };

    let mut lines_boolmap = match initialize_empty_boolmap(config) {
        Some(s) => s,
        None => return initial,
    };

    let maxiterations = 20;
    let maxpoints_per_iter = 50;
    let random_number_cache = (0..(maxiterations * maxpoints_per_iter))
    .map(|_| (js_random(), js_random(), js_random()))
    .collect::<Vec<_>>();

    let mut initial_text_pos_clone = initial_text_pos.to_vec();
    initial_text_pos_clone.sort_by(|a, b| a.area.cmp(&b.area)); // label small areas first
    let mut modifications = BTreeMap::new();

    for (i, tp) in initial_text_pos_clone.iter().enumerate() {
        
        let mut textpos_totry = vec![tp.pos];
        let mut textpos_found = Vec::new();
        let tp_width = tp.kuerzel.chars().count() as f64 * LABEL_WIDTH_PER_CHAR_M + 2.5;
        
        let tp_triangles = tp.poly.get_triangle_points();
        let mut taken_nearest_points = Vec::new();

        for i in 0..maxiterations {

            for newpostotry in textpos_totry.iter() {

                let label_overlaps_background_feature = label_overlaps_feature(
                    newpostotry,
                    &background_boolmap,
                    &config,
                    tp_width,
                );

                let label_overlaps_other_label = label_overlaps_feature(
                    newpostotry,
                    &label_boolmap,
                    &config,
                    tp_width,
                );

                let label_overlaps_other_line = label_overlaps_feature(
                    newpostotry,
                    &lines_boolmap,
                    &config,
                    tp_width,
                );

                let nearest_point = tp_triangles.iter()
                .filter_map(|t| {
                    if taken_nearest_points.iter().any(|q: &SvgPoint| q.equals(t)) {
                        None
                    } else {
                        Some(*t)
                    }
                })
                .min_by_key(|s| {
                    (s.dist(newpostotry) * 1000.0) as usize
                }).unwrap_or(tp.pos);


                let line_will_overlap_other_label = test_line_will_intersect(
                    newpostotry,
                    &nearest_point,
                    &label_boolmap,
                    &config,
                ) as u64;

                let line_will_overlap_other_line = test_line_will_intersect(
                    newpostotry,
                    &nearest_point,
                    &lines_boolmap,
                    &config,
                ) as u64;

                let line_will_overlap_background = test_line_will_intersect(
                    newpostotry,
                    &nearest_point,
                    &background_boolmap,
                    &config,
                ) as u64;

                let distance = newpostotry.dist(&nearest_point);

                let penalty = if label_overlaps_background_feature || label_overlaps_other_label || label_overlaps_other_line {
                    u64::MAX
                } else {
                    (distance * 10.0).round() as u64 + 
                    (line_will_overlap_other_label * 1_000_000) +
                    (line_will_overlap_other_line * 10_000) +
                    (line_will_overlap_background * 1_000)
                };

                // log_status(&format!("{}: testing pos {newpostotry:?}: label_overlaps_background_feature = {label_overlaps_background_feature:?}, line_will_overlap_other_label = {line_will_overlap_other_label:?}, line_will_overlap_other_line = {line_will_overlap_other_line:?}, line_will_overlap_background = {line_will_overlap_background:?}, distance = {distance:?}", tp.kuerzel));

                textpos_found.push((penalty, *newpostotry, nearest_point));
                taken_nearest_points.push(nearest_point);
            }

            if !textpos_found.iter().any(|(penalty, pos, _)| *penalty < 5) {
                let np = gen_new_points(&tp.pos, i, maxpoints_per_iter, &random_number_cache);
                textpos_totry = np;
            } else {
                break;
            }
        }

        textpos_found.sort_by(|a, b| a.0.cmp(&b.0));

        let (least_penalty, newpos, newtargetpos) = textpos_found
        .first().cloned()
        .unwrap_or((u64::MAX, tp.pos, tp.pos));

        paint_label_onto_map(
            &newpos,
            &mut label_boolmap,
            &config,
            tp_width,
        );

        paint_line_onto_map(
            &newpos,
            &newtargetpos,
            &mut lines_boolmap,
            &config,
        );

        modifications.insert(i, (newpos, newtargetpos));
    }

    initial_text_pos_clone.iter().enumerate().map(|(i, tp)| {
        let optimized_pos = modifications.get(&i).cloned().unwrap_or((tp.pos, tp.pos));
        OptimizedTextPlacement {
            optimized: TextPlacement { 
                kuerzel: tp.kuerzel.clone(), 
                status: tp.status.clone(), 
                pos: optimized_pos.0, // TODO
                ref_pos: optimized_pos.1,
                area: tp.area.clone(),
                poly: tp.poly.clone()
            },
            original: tp.clone(),
        }
    }).collect()
}

fn gen_new_points(p: &SvgPoint, iteration: usize, maxpoints: usize, cache: &[(f64, f64, f64)]) -> Vec<SvgPoint> {
    (0..maxpoints).map(|i| {
        let random1 = cache[(iteration * maxpoints) + i];
        let t = 2.0 * PI * random1.0;
        let u = random1.1 + random1.2;
        let r = if u > 1.0 { 2.0 - u } else { u };
        let maxdst = (iteration + 1) as f64 * 4.0;
        let xshift = r * t.cos() * maxdst; 
        let yshift = r * t.sin() * maxdst; 
        p.translate(xshift, yshift)
    }).collect()
}

// returns how many lines this position will intersect
fn test_line_will_intersect(
    start: &SvgPoint,
    end: &SvgPoint,
    map: &ndarray::Array2<bool>,
    config: &OptimizeConfig,
) -> usize {
    use bresenham::Bresenham;
    
    let start = config.point_to_pixel(start);
    let end = config.point_to_pixel(end);

    let mut intersections = 0;
    for (x, y) in Bresenham::new((start.x as isize, start.y as isize), (end.x as isize, end.y as isize)) {
        match map.get((y.max(0) as usize, x.max(0) as usize)) {
            Some(s) => { 
                if *s { intersections += 1; } 
            },
            _ => { },
        }
    }

    intersections
}

fn paint_line_onto_map(
    start: &SvgPoint,
    end: &SvgPoint,
    map: &mut ndarray::Array2<bool>,
    config: &OptimizeConfig,
) {
    use bresenham::Bresenham;
    
    let start = config.point_to_pixel(start);
    let end = config.point_to_pixel(end);

    for (x, y) in Bresenham::new((start.x as isize, start.y as isize), (end.x as isize, end.y as isize)) {
        match map.get_mut((y.max(0) as usize, x.max(0) as usize)) {
            Some(s) => { *s = true; },
            _ => { },
        }
    }
}

fn paint_label_onto_map(
    point: &SvgPoint,
    map: &mut ndarray::Array2<bool>,
    config: &OptimizeConfig,
    tp_width: f64,
) -> bool {

    let pixel = config.point_to_pixel(point);
    let label_height_px = config.label_height_pixel();
    let label_width_px = config.label_width_pixel(tp_width);

    for y_test in 0..label_height_px {
        for x_test in 0..label_width_px {
            match map.get_mut((pixel.y.saturating_sub(y_test), pixel.x + x_test)) {
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
    config: &OptimizeConfig,
    tp_width: f64,
) -> bool {

    let pixel = config.point_to_pixel(point);
    let label_height_px = config.label_height_pixel();
    let label_width_px = config.label_width_pixel(tp_width);
    
    for y_test in 0..label_height_px {
        for x_test in 0..label_width_px {
            match map.get((pixel.y.saturating_sub(y_test), pixel.x + x_test)) {
                Some(s) => if *s { return true; },
                _ => { },
            }
        }
    }
    
    false
}

fn initialize_empty_boolmap(
    config: &OptimizeConfig
) -> Option<ndarray::Array2<bool>> {

    log_status(&format!("initializing empty boolmap {} x {}", config.width_pixels, config.height_pixels));
    let r = geo_rasterize::BinaryBuilder::new()
    .width(config.width_pixels)
    .height(config.height_pixels)
    .build()
    .ok()?;

    let pixels = r.finish();
    
    Some(pixels)
}

fn render_stage1_overlap_boolmap(
    flurstuecke: &SplitNasXml,
    splitflaechen: &[AenderungenIntersection],
    gebaeude: &Gebaeude,
    do_not_overlap_areas: &[SvgPolygon],
    config: &OptimizeConfig
) -> Option<ndarray::Array2<bool>> {

    let mut r = geo_rasterize::BinaryBuilder::new()
    .width(config.width_pixels)
    .height(config.height_pixels)
    .build()
    .ok()?;
    
    for shape in do_not_overlap_areas.iter() {
        r.rasterize(&translate_to_geo_poly(&config.polygon_to_pixel_space(shape))).ok()?;
    }

    for (k, v) in flurstuecke.flurstuecke_nutzungen.iter() {
        for tp in v.iter() {
            for line in tp.poly.outer_rings.iter() {
                r.rasterize(&translate_geoline(&config.line_to_pixel_space(line))).ok()?;
            }
            for line in tp.poly.inner_rings.iter() {
                r.rasterize(&translate_geoline(&config.line_to_pixel_space(line))).ok()?;
            }
        }
    }

    for flst in splitflaechen.iter() {
        for line in flst.poly_cut.outer_rings.iter() {
            r.rasterize(&translate_geoline(&config.line_to_pixel_space(line))).ok()?;
        }
        for line in flst.poly_cut.inner_rings.iter() {
            r.rasterize(&translate_geoline(&config.line_to_pixel_space(line))).ok()?;
        }
    }

    for area in gebaeude.gebaeude.iter() {
        r.rasterize(&translate_to_geo_poly(&config.polygon_to_pixel_space(&area.poly))).ok()?;
    }

    let pixels = r.finish();
    
    Some(pixels)
}