//! Experimental causal handwriting filter. All motion evidence uses unfiltered
//! tablet millimetres; only the final result is quantized. No predicted points.
use crate::protocol::{MAX_X, MAX_Y};

#[derive(Default)]
pub struct WritingFilter {
    raw: Option<(f64, f64)>,
    filtered: (f64, f64),
    velocity: (f64, f64),
    anchor: (f64, f64),
    direction: Option<(f64, f64)>,
    segment_ms: f64,
    segment_speed: f64,
    segment_length: f64,
    turn_sign: f64,
    allowance: f64,
}

fn alpha(hz: f64, seconds: f64) -> f64 {
    let r = std::f64::consts::TAU * hz * seconds;
    r / (1.0 + r)
}

impl WritingFilter {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// `strength` is 0..100. At 60 the total contact displacement bound is
    /// 0.072 mm, plus at most 0.0071 mm output rounding. Parameters are prototype
    /// choices to evaluate, not calibrated sensor noise or perceptual thresholds.
    pub fn update(
        &mut self,
        x: u16,
        y: u16,
        dt_ms: f64,
        strength: f64,
        responsive: bool,
    ) -> (u16, u16) {
        let raw = (f64::from(x) / 100.0, f64::from(y) / 100.0);
        let strength = if strength.is_finite() {
            strength.clamp(0.0, 100.0) / 100.0
        } else {
            0.0
        };
        let maximum = 0.12 * strength;
        let Some(previous) = self.raw else {
            self.raw = Some(raw);
            self.filtered = raw;
            self.anchor = raw;
            self.allowance = maximum;
            return (x, y);
        };
        // A discontinuous clock/report gap must not drag a new stroke from old history.
        if !dt_ms.is_finite() || dt_ms <= 0.0 || dt_ms >= 100.0 {
            self.reset();
            return self.update(x, y, 4.0, strength * 100.0, responsive);
        }
        let dt = dt_ms / 1000.0;
        let a = alpha(10.0, dt);
        self.velocity.0 += a * ((raw.0 - previous.0) / dt - self.velocity.0);
        self.velocity.1 += a * ((raw.1 - previous.1) / dt - self.velocity.1);
        self.allowance = (self.allowance + 0.001 * dt_ms).min(maximum);
        self.segment_ms += dt_ms;
        let step = (raw.0 - self.anchor.0, raw.1 - self.anchor.1);
        let length = step.0.hypot(step.1);
        // Accumulate actual travel before estimating directions. Tiny alternating
        // sensor changes must not repeatedly switch the filter into corner mode.
        if responsive && length >= 0.06 {
            let direction = (step.0 / length, step.1 / length);
            let speed = length * 1000.0 / self.segment_ms;
            if let Some(old) = self.direction {
                let cross = old.0 * direction.1 - old.1 * direction.0;
                let dot = old.0 * direction.0 + old.1 * direction.1;
                let angle = cross.atan2(dot).abs();
                let sustained_turn = angle > 0.08 && cross * self.turn_sign > 0.0;
                let reversal = dot < -0.5 && length >= 0.12 && self.segment_length >= 0.12;
                let slowing =
                    self.segment_speed > 5.0 && speed < self.segment_speed * 0.6 && dot > 0.5;
                if sustained_turn || reversal {
                    self.allowance = self.allowance.min((0.08 * length / angle).max(0.01));
                }
                if slowing {
                    self.allowance = self.allowance.min(0.02);
                }
                self.turn_sign = if angle > 0.08 { cross.signum() } else { 0.0 };
            }
            self.direction = Some(direction);
            self.segment_speed = speed;
            self.segment_length = length;
            self.segment_ms = 0.0;
            self.anchor = raw;
        }
        let cutoff = if responsive {
            24.0 + 24.0 * (1.0 - strength)
        } else {
            40.0
        } + 0.7 * self.velocity.0.hypot(self.velocity.1);
        let a = alpha(cutoff, dt);
        self.filtered.0 += a * (raw.0 - self.filtered.0);
        self.filtered.1 += a * (raw.1 - self.filtered.1);
        if responsive {
            let delta = (self.filtered.0 - raw.0, self.filtered.1 - raw.1);
            let distance = delta.0.hypot(delta.1);
            if distance > self.allowance {
                self.filtered = (
                    raw.0 + delta.0 * self.allowance / distance,
                    raw.1 + delta.1 * self.allowance / distance,
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
