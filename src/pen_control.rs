//! One causal position filter for drawing and writing. 30 exactly retains the
//! Light smoothing baseline; higher values damp sideways noise more strongly.
//! No curve fitting, prediction, pressure synthesis or post-lift continuation.
use crate::protocol::{MAX_X, MAX_Y};

/// Only the additional directional correction beyond Light is added to the
/// selected base path. This avoids applying Light's forward lag a second time.
/// Individual artistic filters keep their own, separately chosen displacement.
#[derive(Default)]
pub struct CircleControl {
    light: PenControl,
    steady: PenControl,
}
impl CircleControl {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn correction(&mut self, x: u16, y: u16, dt_ms: f64, amount: f64) -> (i32, i32) {
        if !amount.is_finite() || amount <= 0.0 {
            self.reset();
            return (0, 0);
        }
        let light = self.light.update(x, y, dt_ms, 30.0);
        let steady = self
            .steady
            .update(x, y, dt_ms, 30.0 + 0.7 * amount.min(100.0));
        let dx = f64::from(steady.0) - f64::from(light.0);
        let dy = f64::from(steady.1) - f64::from(light.1);
        let scale = (12.0 / dx.hypot(dy).max(12.0)).min(1.0);
        ((dx * scale).round() as i32, (dy * scale).round() as i32)
    }
}

#[derive(Default)]
pub struct PenControl {
    raw: Option<(f64, f64)>,
    filtered: (f64, f64),
    velocity: (f64, f64),
    anchor: (f64, f64),
    direction: Option<(f64, f64)>,
    corner_ms: f64,
    radius: f64,
    turn_sign: f64,
    history: [(f64, f64); 8],
    history_count: usize,
    history_next: usize,
}
fn alpha(hz: f64, dt: f64) -> f64 {
    let r = std::f64::consts::TAU * hz * dt;
    r / (1.0 + r)
}
impl PenControl {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn update(&mut self, x: u16, y: u16, dt_ms: f64, amount: f64) -> (u16, u16) {
        let raw = (f64::from(x) / 100.0, f64::from(y) / 100.0);
        let amount = if amount.is_finite() {
            amount.clamp(0.0, 100.0)
        } else {
            30.0
        };
        let Some(previous) = self.raw else {
            self.raw = Some(raw);
            self.filtered = raw;
            self.anchor = raw;
            self.radius = f64::INFINITY;
            return (x, y);
        };
        if !dt_ms.is_finite() || dt_ms <= 0.0 || dt_ms >= 100.0 {
            self.reset();
            return self.update(x, y, 4.0, amount);
        }
        let dt = dt_ms / 1000.0;
        let da = alpha(10.0, dt);
        self.velocity.0 += da * ((raw.0 - previous.0) / dt - self.velocity.0);
        self.velocity.1 += da * ((raw.1 - previous.1) / dt - self.velocity.1);
        let speed = self.velocity.0.hypot(self.velocity.1);
        let base = alpha(40.0 + 0.7 * speed, dt);
        let extra = ((amount - 30.0) / 70.0).max(0.0);
        self.history[self.history_next] = raw;
        self.history_next = (self.history_next + 1) % 8;
        self.history_count = (self.history_count + 1).min(8);
        let oldest = if self.history_count == 8 {
            self.history_next
        } else {
            0
        };
        let first = self.history[oldest];
        let mut travel = 0.0;
        for i in 1..self.history_count {
            let a = self.history[(oldest + i - 1) % 8];
            let b = self.history[(oldest + i) % 8];
            travel += (b.0 - a.0).hypot(b.1 - a.1);
        }
        let coherence = if travel > 0.0 {
            (raw.0 - first.0).hypot(raw.1 - first.1) / travel
        } else {
            0.0
        };
        let net_travel = (raw.0 - first.0).hypot(raw.1 - first.1);
        let confidence = if net_travel > 0.12 {
            1.0
        } else {
            ((coherence - 0.2) / 0.4).clamp(0.0, 1.0)
        };
        self.corner_ms = (self.corner_ms - dt_ms).max(0.0);
        let step = (raw.0 - self.anchor.0, raw.1 - self.anchor.1);
        let length = step.0.hypot(step.1);
        if length >= 0.08 {
            let direction = (step.0 / length, step.1 / length);
            if let Some(old) = self.direction {
                let signed_angle = (old.0 * direction.1 - old.1 * direction.0)
                    .atan2(old.0 * direction.0 + old.1 * direction.1);
                let angle = signed_angle.abs();
                if angle > 0.65 && coherence > 0.85 || angle > 2.5 && length >= 0.12 {
                    self.corner_ms = 24.0;
                }
                self.radius = if angle > 0.02 && signed_angle * self.turn_sign > 0.0 {
                    length / angle
                } else {
                    f64::INFINITY
                };
                self.turn_sign = if angle > 0.02 {
                    signed_angle.signum()
                } else {
                    0.0
                };
            }
            self.direction = Some(direction);
            self.anchor = raw;
        }
        let delta = (raw.0 - self.filtered.0, raw.1 - self.filtered.1);
        if amount <= 30.0 {
            let a = if amount == 30.0 {
                base
            } else {
                1.0 - (1.0 - base) * amount / 30.0
            };
            self.filtered.0 += a * delta.0;
            self.filtered.1 += a * delta.1;
        } else {
            let normal_alpha = if self.corner_ms > 0.0 {
                base
            } else {
                alpha(40.0 / (1.0 + 4.0 * extra) + (0.7 - 0.5 * extra) * speed, dt)
            };
            if speed > 1.0 {
                let tangent = (self.velocity.0 / speed, self.velocity.1 / speed);
                let along = delta.0 * tangent.0 + delta.1 * tangent.1;
                let across = -delta.0 * tangent.1 + delta.1 * tangent.0;
                let along_alpha = normal_alpha + (base - normal_alpha) * confidence;
                self.filtered.0 +=
                    along_alpha * along * tangent.0 - normal_alpha * across * tangent.1;
                self.filtered.1 +=
                    along_alpha * along * tangent.1 + normal_alpha * across * tangent.0;
                // Curvature and the original report bound sideways displacement.
                // This prevents strong damping from shrinking small loops.
                let error = (self.filtered.0 - raw.0, self.filtered.1 - raw.1);
                let normal = -error.0 * tangent.1 + error.1 * tangent.0;
                let limit = (self.radius * 0.06).clamp(0.015, 0.08);
                let correction = normal - normal.clamp(-limit, limit);
                self.filtered.0 += correction * tangent.1;
                self.filtered.1 -= correction * tangent.0;
            } else {
                self.filtered.0 += normal_alpha * delta.0;
                self.filtered.1 += normal_alpha * delta.1;
            }
            let error = (self.filtered.0 - raw.0, self.filtered.1 - raw.1);
            let distance = error.0.hypot(error.1);
            let limit = if self.corner_ms > 0.0 { 0.03 } else { 0.10 };
            if distance > limit {
                self.filtered = (
                    raw.0 + error.0 * limit / distance,
                    raw.1 + error.1 * limit / distance,
                );
            }
        }
        self.raw = Some(raw);
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
