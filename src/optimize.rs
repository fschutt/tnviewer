use ndarray::Axis;
use web_sys::console::log_1;

use crate::{nas::{translate_geoline, translate_to_geo_poly, SvgLine, SvgPoint, SvgPolygon}, pdf::{Flurstuecke, FlurstueckeInPdfSpace, Gebaeude, GebaeudeInPdfSpace, RissConfig, RissExtentReprojected}, ui::{AenderungenIntersection, TextPlacement}, uuid_wasm::log_status};

pub struct OptimizedTextPlacement {
    pub rect: SvgLine,
    pub original: TextPlacement,
    pub optimized: TextPlacement,
}

pub const LABEL_HEIGHT_M: f64 = 5.0;
pub const LABEL_WIDTH_M: f64 = 10.0;


pub struct OptimizeConfig {
    tolerance: f64,
    riss_config: RissConfig,
    riss_extent: RissExtentReprojected,
    width_pixels: usize,
    height_pixels: usize,
    one_px_x_in_m: f64,
    one_px_y_in_m: f64,
}

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

    pub fn label_width_pixel(&self) -> usize {
        (LABEL_WIDTH_M / self.one_px_x_in_m).ceil() as usize
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
    flurstuecke: &Flurstuecke,
    splitflaechen: &[AenderungenIntersection],
    gebaeude: &Gebaeude,
    avoid_areas_in_pdf_space: &[SvgPolygon],
    initial_text_pos: &[TextPlacement],
    config: &OptimizeConfig,
) -> Vec<OptimizedTextPlacement> {

    let mut overlap_boolmap = match render_overlap_boolmap(
        flurstuecke,
        splitflaechen,
        gebaeude,
        avoid_areas_in_pdf_space, 
        config,
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

    log_status(&format!("label height in pixels: {}", config.label_height_pixel()));
    log_status(&format!("label width in pixels: {}", config.label_width_pixel()));

    let mut initial_text_pos_clone = initial_text_pos.to_vec();
    let maxiterations = 4;
    for tp in initial_text_pos_clone.iter_mut() {
        log_1(&"labeling 1".into());
        let mut textpos_totry = vec![tp.pos];
        let mut textpos_found = None;
        'outer: for _ in 0..maxiterations {
            log_1(&"loop".into());
            let mut newaccum_pos = Vec::new();
            for newpostotry in textpos_totry.iter() {
                if !label_overlaps_feature(
                    newpostotry,
                    &overlap_boolmap,
                    &config,
                ) {
                    log_1(&"mark found".into());
                    // mark region as occupied
                    textpos_found = Some(*newpostotry);
                    paint_label_onto_map(
                        newpostotry,
                        &mut overlap_boolmap,
                        &config,
                    );
                    break 'outer;
                } else {
                    log_1(&"generating new positions".into());
                    newaccum_pos.append(&mut gen_new_points(newpostotry));
                }
            }
            if !newaccum_pos.is_empty() {
                newaccum_pos.sort_by(|a, b| a.dist(&tp.pos).total_cmp(&b.dist(&tp.pos)));
                newaccum_pos.dedup_by(|a, b| a.equals(b));
                textpos_totry = newaccum_pos;
            }

        }
        if let Some(found) = textpos_found {
            log_1(&"pos found".into());
            tp.pos = found;
        }
    }

    log_status(&format!("OK rendered map! {} x {} pixels", config.width_pixels, config.height_pixels));

    initial_text_pos.iter().zip(initial_text_pos_clone.iter()).map(|(old, new)| {
        OptimizedTextPlacement {
            rect: svg_label_pos_to_line(&new.pos),
            optimized: new.clone(),
            original: old.clone(),
        }
    }).collect()
}

fn gen_new_points(p: &SvgPoint) -> Vec<SvgPoint> {
    let lpos = 10.0;
    let lpos_half = lpos / 2.0;
    let xpos = vec![
        -lpos,
        -lpos_half,
        0.0,
        lpos_half,
        lpos,
    ];
    let ypos = vec![
        -lpos,
        -lpos_half,
        0.0,
        lpos_half,
        lpos,
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

fn paint_label_onto_map(
    point: &SvgPoint,
    map: &mut ndarray::Array2<bool>,
    config: &OptimizeConfig,
) -> bool {

    let pixel = config.point_to_pixel(point);
    let label_height_px = config.label_height_pixel();
    let label_width_px = config.label_width_pixel();

    for y_test in 0..label_height_px {
        for x_test in 0..label_width_px {
            match map.get_mut((pixel.y + y_test, pixel.x + x_test)) {
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
) -> bool {

    let pixel = config.point_to_pixel(point);
    let label_height_px = config.label_height_pixel();
    let label_width_px = config.label_width_pixel();
    
    for y_test in 0..label_height_px {
        for x_test in 0..label_width_px {
            match map.get((pixel.y + y_test, pixel.x + x_test)) {
                Some(s) => if *s { return true; },
                _ => { },
            }
        }
    }
    
    false
}

fn render_overlap_boolmap(
    flurstuecke: &Flurstuecke,
    splitflaechen: &[AenderungenIntersection],
    gebaeude: &Gebaeude,
    do_not_overlap_areas: &[SvgPolygon],
    config: &OptimizeConfig
) -> Option<ndarray::Array2<bool>> {
    use geo_rasterize::BinaryBuilder;

    let mut r = BinaryBuilder::new()
    .width(config.width_pixels)
    .height(config.height_pixels)
    .build()
    .ok()?;
    
    for shape in do_not_overlap_areas.iter() {
        r.rasterize(&translate_to_geo_poly(&config.polygon_to_pixel_space(shape))).ok()?;
    }

    for flst in flurstuecke.flst.iter() {
        for line in flst.poly.outer_rings.iter() {
            r.rasterize(&translate_geoline(&config.line_to_pixel_space(line))).ok()?;
        }
        for line in flst.poly.inner_rings.iter() {
            r.rasterize(&translate_geoline(&config.line_to_pixel_space(line))).ok()?;
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