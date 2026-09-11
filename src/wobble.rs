//! Small-motion damping shared by hover, Ink and WinTab before artistic shaping.
//! Causal and bounded: no prediction, dead zone, pressure changes or buffered release.
use crate::protocol::{MAX_X, MAX_Y};

#[derive(Default)]
pub struct WobbleFilter {
    previous: Option<(f64, f64)>,
    filtered: (f64, f64),
}

impl WobbleFilter {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// 100 tablet units/mm. Zero bypasses; 40% limits added displacement to 0.12 mm.
    pub fn update(&mut self, x: u16, y: u16, dt_ms: f64, amount: f64) -> (u16, u16) {
        let raw = (f64::from(x), f64::from(y));
        let previous = self.previous.replace(raw);
        let strength = amount / 100.0;
        if let Some(previous) =
            previous.filter(|_| strength > 0.0 && dt_ms > 0.0 && dt_ms.is_finite())
        {
            let dt = dt_ms / 1000.0;
            let speed = (raw.0 - previous.0).hypot(raw.1 - previous.1) / (100.0 * dt);
            // Small alternating movement stays damped. Deliberate motion smoothly
            // opens the filter, reaching exact tracking at 80 mm/s.
            let motion = ((speed - 15.0) / 65.0).clamp(0.0, 1.0);
            let follow = motion * motion * (3.0 - 2.0 * motion);
            let cutoff = 24.0 / (1.0 + 16.0 * strength);
            let a = 1.0 - (-2.0 * std::f64::consts::PI * cutoff * dt).exp();
            let a = a + (1.0 - a) * follow;
            self.filtered.0 += a * (raw.0 - self.filtered.0);
            self.filtered.1 += a * (raw.1 - self.filtered.1);
            let delta = (self.filtered.0 - raw.0, self.filtered.1 - raw.1);
            let distance = delta.0.hypot(delta.1);
            let limit = 30.0 * strength * (1.0 - follow);
            if distance > limit {
                self.filtered = (
                    raw.0 + delta.0 * limit / distance,
                    raw.1 + delta.1 * limit / distance,
                );
            }
            // Reach every physical edge exactly, without an asymptotic last-pixel stall.
            if x == 0 || x == MAX_X {
                self.filtered.0 = raw.0;
            }
            if y == 0 || y == MAX_Y {
                self.filtered.1 = raw.1;
            }
        } else {
            self.filtered = raw;
        }
        (
            self.filtered.0.round() as u16,
            self.filtered.1.round() as u16,
        )
    }
}
