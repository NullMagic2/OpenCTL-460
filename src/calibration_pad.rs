//! Bounded preview ink, independent of the pressure measurement session.
#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub pressure: f64,
}
#[derive(Clone, Copy, Debug)]
pub struct Segment {
    pub a: Point,
    pub b: Point,
}
#[derive(Default)]
pub struct Pad {
    pub segments: std::collections::VecDeque<Segment>,
    last: Option<Point>,
}
pub const MAX_SEGMENTS: usize = 5000;
impl Pad {
    pub fn feed(&mut self, x: f64, y: f64, pressure: f64, contact: bool) -> bool {
        if !contact
            || !x.is_finite()
            || !y.is_finite()
            || !pressure.is_finite()
            || !(0.0..=1.0).contains(&x)
            || !(0.0..=1.0).contains(&y)
        {
            self.last = None;
            return false;
        }
        let p = Point {
            x,
            y,
            pressure: pressure.clamp(0., 1.),
        };
        self.segments.push_back(Segment {
            a: self.last.unwrap_or(p),
            b: p,
        });
        self.last = Some(p);
        if self.segments.len() > MAX_SEGMENTS {
            self.segments.pop_front();
        }
        true
    }
    pub fn clear(&mut self) {
        self.segments.clear();
        self.last = None;
    }
    pub fn lift(&mut self) {
        self.last = None;
    }
}

/// The shared stream uses MAX_X/MAX_Y coordinates mapped to the primary display,
/// exactly like the feeder, rather than the Win32 mouse API's 0..65535 range.
pub fn local_position(
    x: i32,
    y: i32,
    screen: (i32, i32),
    pad: (i32, i32, i32, i32),
) -> Option<(f64, f64)> {
    use crate::protocol::{MAX_X, MAX_Y};
    if !(0..=MAX_X as i32).contains(&x)
        || !(0..=MAX_Y as i32).contains(&y)
        || screen.0 < 2
        || screen.1 < 2
        || pad.2 - pad.0 < 2
        || pad.3 - pad.1 < 2
    {
        return None;
    }
    let sx = x as f64 / MAX_X as f64 * (screen.0 - 1) as f64;
    let sy = y as f64 / MAX_Y as f64 * (screen.1 - 1) as f64;
    let point = (
        (sx - pad.0 as f64) / (pad.2 - pad.0 - 1) as f64,
        (sy - pad.1 as f64) / (pad.3 - pad.1 - 1) as f64,
    );
    if (0.0..=1.0).contains(&point.0) && (0.0..=1.0).contains(&point.1) {
        Some(point)
    } else {
        None
    }
}
