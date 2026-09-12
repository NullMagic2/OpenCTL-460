//! Optional closure of the current drawing stroke. Fixed state, no stroke recording.
//! All coordinates are after Precision Hold, so the marker and snap use emitted geometry.
use crate::{config::Config, engine::Frame};
#[derive(Default)]
pub struct ShapeAssist {
    start: Option<(u16, u16)>,
    previous: (f64, f64),
    direction: Option<(f64, f64)>,
    turn_anchor: (f64, f64),
    turn: f64,
    travel: f64,
    bounds: [f64; 4],
    armed: bool,
    snapped: bool,
    last_ms: Option<f64>,
    pause_anchor: Option<(f64, f64)>,
    pause_ms: f64,
    closure_from: Option<(f64, f64)>,
}
impl ShapeAssist {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn start(&self) -> Option<(u16, u16)> {
        self.start
    }
    pub fn snapped(&self) -> bool {
        self.snapped
    }
    pub fn apply(&mut self, mut frame: Frame, c: &Config, now_ms: f64) -> Frame {
        if !frame.in_range || !frame.contact || frame.eraser || frame.barrel || frame.handwriting {
            self.reset();
            return frame;
        }
        if !now_ms.is_finite() || now_ms < 0.0 || self.last_ms.is_some_and(|t| now_ms < t) {
            self.reset();
            return frame;
        }
        let dt = self.last_ms.replace(now_ms).map_or(0.0, |t| now_ms - t);
        // Only fresh, closely spaced input reports establish a deliberate pause.
        if dt > 40.0 {
            self.pause_anchor = None;
            self.pause_ms = 0.0;
            self.closure_from = None;
        }
        let dt = if dt <= 40.0 { dt } else { 0.0 };
        let p = (f64::from(frame.x), f64::from(frame.y));
        let Some(start) = self.start else {
            self.start = Some((frame.x, frame.y));
            self.previous = p;
            self.turn_anchor = p;
            self.bounds = [p.0, p.1, p.0, p.1];
            return frame;
        };
        let delta = (p.0 - self.previous.0, p.1 - self.previous.1);
        let step = delta.0.hypot(delta.1);
        let distance = (p.0 - f64::from(start.0)).hypot(p.1 - f64::from(start.1));
        self.travel += step;
        self.previous = p;
        self.bounds[0] = self.bounds[0].min(p.0);
        self.bounds[1] = self.bounds[1].min(p.1);
        self.bounds[2] = self.bounds[2].max(p.0);
        self.bounds[3] = self.bounds[3].max(p.1);
        // Direction uses accumulated travel to suppress sensor-sized angular noise.
        let d = (p.0 - self.turn_anchor.0, p.1 - self.turn_anchor.1);
        let n = d.0.hypot(d.1);
        if n >= 8.0 {
            let direction = (d.0 / n, d.1 / n);
            if let Some(old) = self.direction {
                self.turn += (old.0 * direction.1 - old.1 * direction.0)
                    .atan2(old.0 * direction.0 + old.1 * direction.1);
            }
            self.direction = Some(direction);
            self.turn_anchor = p;
        }
        let radius = c.closure_radius_mm * 100.0;
        self.armed |= distance > radius * 3.0;
        if !c.close_shapes {
            return frame;
        }
        if self.snapped
            && (distance > radius * 1.6
                || self
                    .pause_anchor
                    .is_some_and(|a| (p.0 - a.0).hypot(p.1 - a.1) > 8.0))
        {
            self.snapped = false;
            self.pause_anchor = None;
            self.pause_ms = 0.0;
            self.closure_from = None;
        }
        let loop_like = self.armed
            && self.travel > 600.0_f64.max(radius * 8.0)
            && self.turn.abs() > 3.5
            && self.bounds[2] - self.bounds[0] > radius * 2.0
            && self.bounds[3] - self.bounds[1] > radius * 2.0;
        if !self.snapped {
            if loop_like && distance < radius {
                let anchor = self.pause_anchor.get_or_insert(p);
                if (p.0 - anchor.0).hypot(p.1 - anchor.1) > 4.0 {
                    *anchor = p;
                    self.pause_ms = 0.0;
                    self.closure_from = None;
                } else {
                    self.pause_ms += dt;
                }
                // Moving through the start leaves coordinates untouched. Once a
                // pause is established, ease to the start instead of jumping.
                if self.pause_ms >= 80.0 {
                    let from = *self.closure_from.get_or_insert(p);
                    let t = ((self.pause_ms - 80.0) / 80.0).clamp(0.0, 1.0);
                    let weight = t * t * (3.0 - 2.0 * t);
                    frame.x = (from.0 + (f64::from(start.0) - from.0) * weight).round() as u16;
                    frame.y = (from.1 + (f64::from(start.1) - from.1) * weight).round() as u16;
                    self.snapped = t >= 1.0;
                }
            } else {
                self.pause_anchor = None;
                self.pause_ms = 0.0;
                self.closure_from = None;
            }
        }
        if self.snapped {
            frame.x = start.0;
            frame.y = start.1;
        }
        frame
    }
}
