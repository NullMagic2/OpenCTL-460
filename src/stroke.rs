//! Adaptive One Euro coordinate filtering and aspect-correct screen mapping.
//! The speed signal is in millimetres/second (100 CTL-460 units per millimetre).
//! Faster strokes raise the cutoff to reduce lag; no prediction or corner overshoot.

use crate::protocol::{MAX_X, MAX_Y};

#[derive(Default)]
pub struct StrokeFilter {
    previous_raw: Option<(f64, f64)>,
    filtered: (f64, f64),
    velocity: (f64, f64),
    writing_anchor: Option<(f64, f64)>,
    writing_direction: Option<(f64, f64)>,
    writing_budget: f64,
}

fn alpha(cutoff: f64, dt: f64) -> f64 {
    let r = 2.0 * std::f64::consts::PI * cutoff * dt;
    r / (1.0 + r)
}

impl StrokeFilter {
    /// Bound displacement to 0.12 mm for writing. This causal correction adds no look-ahead
    /// delay, never extrapolates beyond the raw point, and keeps tight turns close to the pen.
    pub fn handwriting(&mut self, x: u16, y: u16, dt_ms: f64) -> (u16, u16) {
        let raw = (f64::from(x) / 100.0, f64::from(y) / 100.0);
        // Ignore sub-0.04 mm direction noise. Estimated bend radius tightens the
        // allowed lag to 10% of the radius, preserving small loops and reversals.
        if let Some(anchor) = self.writing_anchor {
            self.writing_budget = (self.writing_budget
                + if dt_ms.is_finite() {
                    dt_ms.clamp(0.0, 50.0)
                } else {
                    0.0
                } * 0.002)
                .min(0.12);
            let step = (raw.0 - anchor.0, raw.1 - anchor.1);
            let length = step.0.hypot(step.1);
            if length >= 0.04 {
                let direction = (step.0 / length, step.1 / length);
                if let Some(previous) = self.writing_direction {
                    let angle = (previous.0 * direction.1 - previous.1 * direction.0)
                        .atan2(previous.0 * direction.0 + previous.1 * direction.1)
                        .abs();
                    if angle > 0.001 {
                        self.writing_budget = self
                            .writing_budget
                            .min((0.1 * length / angle).clamp(0.02, 0.12));
                    }
                }
                self.writing_direction = Some(direction);
                self.writing_anchor = Some(raw);
            }
        } else {
            self.writing_anchor = Some(raw);
            self.writing_budget = 0.12;
        }
        self.update(x, y, dt_ms, 20.0, 0.5);
        let d = (self.filtered.0 - raw.0, self.filtered.1 - raw.1);
        let distance = d.0.hypot(d.1);
        if distance > self.writing_budget {
            self.filtered = (
                raw.0 + d.0 * self.writing_budget / distance,
                raw.1 + d.1 * self.writing_budget / distance,
            );
        }
        (
            (self.filtered.0 * 100.0).round() as u16,
            (self.filtered.1 * 100.0).round() as u16,
        )
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn update(&mut self, x: u16, y: u16, dt_ms: f64, min_cutoff: f64, beta: f64) -> (u16, u16) {
        let raw = (f64::from(x) / 100.0, f64::from(y) / 100.0);
        if let Some(previous) = self.previous_raw {
            if dt_ms > 0.0 && dt_ms.is_finite() {
                let dt = dt_ms / 1000.0;
                let da = alpha(10.0, dt);
                self.velocity.0 += da * ((raw.0 - previous.0) / dt - self.velocity.0);
                self.velocity.1 += da * ((raw.1 - previous.1) / dt - self.velocity.1);
                let cutoff = min_cutoff + beta * self.velocity.0.hypot(self.velocity.1);
                let a = alpha(cutoff, dt);
                self.filtered.0 += a * (raw.0 - self.filtered.0);
                self.filtered.1 += a * (raw.1 - self.filtered.1);
            }
        } else {
            self.filtered = raw;
        }
        self.previous_raw = Some(raw);
        (
            (self.filtered.0 * 100.0)
                .round()
                .clamp(0.0, f64::from(MAX_X)) as u16,
            (self.filtered.1 * 100.0)
                .round()
                .clamp(0.0, f64::from(MAX_Y)) as u16,
        )
    }
}

/// Maps a centered tablet area to the full display while optionally preserving geometry.
pub fn map_to_virtual_screen(
    x: u16,
    y: u16,
    width: i32,
    height: i32,
    preserve_aspect: bool,
) -> (u16, u16) {
    let (px, py) = map_to_screen_precise(x, y, width, height, preserve_aspect);
    // Common normalized output coordinates keep HID, WinTab and the Windows cursor aligned.
    (
        (px / f64::from((width - 1).max(1)) * f64::from(MAX_X)).round() as u16,
        (py / f64::from((height - 1).max(1)) * f64::from(MAX_Y)).round() as u16,
    )
}

/// Cover the display using a centered tablet area; clamp unused tablet margins to its edges.
pub fn map_to_screen(x: u16, y: u16, width: i32, height: i32, preserve_aspect: bool) -> (i32, i32) {
    let (x, y) = map_to_screen_precise(x, y, width, height, preserve_aspect);
    (x.round() as i32, y.round() as i32)
}

/// Preserve subpixel geometry for HID/WinTab; round only for APIs that require whole pixels.
pub fn map_to_screen_precise(
    x: u16,
    y: u16,
    width: i32,
    height: i32,
    preserve_aspect: bool,
) -> (f64, f64) {
    let w = f64::from((width - 1).max(0));
    let h = f64::from((height - 1).max(0));
    let (sx, sy) = (w / f64::from(MAX_X), h / f64::from(MAX_Y));
    let (sx, sy) = if preserve_aspect {
        (sx.max(sy), sx.max(sy))
    } else {
        (sx, sy)
    };
    let ox = (w - sx * f64::from(MAX_X)) / 2.0;
    let oy = (h - sy * f64::from(MAX_Y)) / 2.0;
    (
        (ox + f64::from(x.min(MAX_X)) * sx).clamp(0.0, w),
        (oy + f64::from(y.min(MAX_Y)) * sy).clamp(0.0, h),
    )
}
