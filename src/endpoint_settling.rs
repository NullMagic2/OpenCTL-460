//! Recognize a slowing, lightening pen while it is still in contact.
//! This only relaxes positional lag; it never changes pressure or adds reports.
#[derive(Default)]
pub struct ReleaseSettling {
    previous: Option<(f64, f64)>,
    pressure: u16,
    peak_pressure: u16,
    peak_speed: f64,
    falling_ms: f64,
}
impl ReleaseSettling {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn update(&mut self, x: u16, y: u16, pressure: u16, dt_ms: f64) -> f64 {
        if !dt_ms.is_finite() || dt_ms <= 0.0 || dt_ms >= 100.0 {
            self.reset();
            return 0.0;
        }
        let p = (f64::from(x), f64::from(y));
        let previous = self.previous.replace(p);
        self.peak_pressure = self.peak_pressure.max(pressure);
        if pressure < self.pressure {
            self.falling_ms += dt_ms.clamp(0.0, 32.0);
        } else if pressure > self.pressure.saturating_add(8) {
            self.falling_ms = 0.0;
        }
        self.pressure = pressure;
        let Some(previous) = previous else {
            return 0.0;
        };
        let speed = (p.0 - previous.0).hypot(p.1 - previous.1) / dt_ms;
        self.peak_speed = (self.peak_speed * (-dt_ms / 80.0).exp()).max(speed);
        if self.falling_ms < 16.0 || self.peak_pressure < 64 || self.peak_speed < 0.1 {
            return 0.0;
        }
        let lightening =
            ((0.65 - f64::from(pressure) / f64::from(self.peak_pressure)) / 0.35).clamp(0.0, 1.0);
        let slowing = ((0.65 - speed / self.peak_speed) / 0.65).clamp(0.0, 1.0);
        lightening * slowing
    }
}
