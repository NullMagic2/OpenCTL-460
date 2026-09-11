//! Raw spatial evidence protects deliberate turns and bounds the final combined
//! contact displacement. No pressure, hover, prediction or extra reports are used.
#[derive(Default)]
pub struct SmoothingGuard {
    anchor: Option<(f64, f64)>,
    direction: Option<(f64, f64)>,
    straight: u8,
    turn_sign: f64,
    turns: u8,
    allowance: f64,
}
impl SmoothingGuard {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    /// Returns true for a deliberate sharp turn after a straight approach.
    pub fn observe(&mut self, x: u16, y: u16, corners: bool, limit_mm: f64) -> bool {
        let raw = (f64::from(x), f64::from(y));
        let maximum = limit_mm * 100.0;
        let Some(anchor) = self.anchor else {
            self.anchor = Some(raw);
            self.allowance = maximum;
            return false;
        };
        let delta = (raw.0 - anchor.0, raw.1 - anchor.1);
        let length = delta.0.hypot(delta.1);
        // Accumulate physical travel; sensor-sized changes cannot trigger corners.
        if length < 12.0 {
            return false;
        }
        self.anchor = Some(raw);
        self.allowance = (self.allowance + 0.5 * length).min(maximum);
        let direction = (delta.0 / length, delta.1 / length);
        let mut corner = false;
        if let Some(old) = self.direction {
            let signed = (old.0 * direction.1 - old.1 * direction.0)
                .atan2(old.0 * direction.0 + old.1 * direction.1);
            let angle = signed.abs();
            corner = corners && self.straight >= 3 && angle > 0.75;
            if angle < 0.12 {
                self.straight = self.straight.saturating_add(1);
            } else {
                self.straight = 0;
            }
            if angle > 0.08 && signed * self.turn_sign > 0.0 {
                self.turns = self.turns.saturating_add(1);
            } else {
                self.turns = 0;
            }
            self.turn_sign = if angle > 0.08 { signed.signum() } else { 0.0 };
            if corners && self.turns >= 2 {
                // Sustained curvature protects hooks and loops. Alternating
                // deviations do not count as a sustained deliberate turn.
                self.allowance = self.allowance.min((0.12 * length / angle).max(8.0));
            }
            if corner {
                self.allowance = self.allowance.min(8.0);
            }
        }
        self.direction = Some(direction);
        corner
    }
    /// Euclidean bound across every earlier position stage, plus <=0.0071 mm rounding.
    pub fn constrain(&self, raw: (u16, u16), filtered: (u16, u16)) -> (u16, u16) {
        let d = (
            f64::from(filtered.0) - f64::from(raw.0),
            f64::from(filtered.1) - f64::from(raw.1),
        );
        let length = d.0.hypot(d.1);
        if length <= self.allowance || length == 0.0 {
            return filtered;
        }
        let scale = self.allowance / length;
        (
            (f64::from(raw.0) + d.0 * scale).round() as u16,
            (f64::from(raw.1) + d.1 * scale).round() as u16,
        )
    }
}
