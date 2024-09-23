use crate::nas::{SvgLine, SvgPoint};

pub type AngleDegrees = f64;

pub struct PointOnLineConfig {
    pub symbol_width_m: f64,
    pub distance_on_line_m: f64,
}

pub fn generate_points_along_lines(config: &PointOnLineConfig, lines: &[SvgLine]) -> Vec<(SvgPoint, AngleDegrees)> {

    let symbol_width_half = config.symbol_width_m / 2.0;

    let mut target_points = Vec::new();

    for l in lines.iter() {

        let items = l.to_points_vec();

        let line_length = items.iter().map(|e| get_length(e)).sum();

        let mut points_for_line = Vec::new();
        let mut offset_on_path = 0.0;
        let mut sum_length_elements_so_far = 0.0;

        'outer: for e in items.iter() {

            let element_length = get_length(e);

            while offset_on_path < sum_length_elements_so_far + element_length {

                if offset_on_path + config.symbol_width_m > line_length {
                    break 'outer;
                }

                let point_on_path = offset_on_path -
                    sum_length_elements_so_far +
                    symbol_width_half;

                let current_t_on_element = get_t_at_offset(e, point_on_path);

                let current_angle = get_tangent_vector_at_t(e, current_t_on_element)
                    .rotate_90deg_ccw()
                    .angle_degrees();

                let point_x = get_x_at_t(e, current_t_on_element);
                let point_y = get_y_at_t(e, current_t_on_element);

                points_for_line.push((SvgPoint {
                    x: point_x,
                    y: point_y
                }, current_angle));

                offset_on_path += config.symbol_width_m + config.distance_on_line_m;
            }

            sum_length_elements_so_far += element_length;
        }

        target_points.extend(points_for_line.into_iter());
    }

    target_points
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
#[repr(C)]
pub struct SvgVector {
    pub x: f64,
    pub y: f64,
}

impl SvgVector {
    /// Returns the angle of the vector in degrees
    #[inline]
    pub fn angle_degrees(&self) -> f64 {
        //  y
        //  |  /
        //  | /
        //   / a)
        //   ___ x

        (-self.x).atan2(self.y).to_degrees()
    }

    #[inline]
    #[must_use = "returns a new vector"]
    pub fn normalize(&self) -> Self {
        let tangent_length = self.x.hypot(self.y);

        Self {
            x: self.x / tangent_length,
            y: self.y / tangent_length,
        }
    }

    /// Rotate the vector 90 degrees counter-clockwise
    #[must_use = "returns a new vector"]
    #[inline]
    pub fn rotate_90deg_ccw(&self) -> Self {
        Self {
            x: -self.y,
            y: self.x,
        }
    }
}

fn get_length((a, b): &(SvgPoint, SvgPoint)) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    dx.hypot(dy)
}

fn get_t_at_offset(ab: &(SvgPoint, SvgPoint), offset: f64) -> f64 {
    offset / get_length(ab)
}

fn get_tangent_vector_at_t((a, b): &(SvgPoint, SvgPoint), t: f64) -> SvgVector {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    SvgVector {
        x: dx,
        y: dy,
    }
    .normalize()
}

pub fn get_x_at_t((a, b): &(SvgPoint, SvgPoint), t: f64) -> f64 {
    a.x as f64 + (b.x as f64 - a.x as f64) * t
}

pub fn get_y_at_t((a, b): &(SvgPoint, SvgPoint), t: f64) -> f64 {
    a.y as f64 + (b.y as f64 - a.y as f64) * t
}