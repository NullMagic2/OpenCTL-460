//! Frozen 0.3.3 position-filter baseline used only by the synthetic audit.
use crate::config::Config;

#[derive(Default)]
pub struct LineSmoothing {
    previous: Option<(f64, f64)>,
    direction: (f64, f64),
    motion: (f64, f64),
    stream: (f64, f64),
    history: [(f64, f64); 32],
    intervals: [f64; 32],
    next: usize,
    count: usize,
}

impl LineSmoothing {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Coordinates are tablet units (100/mm). Time affects only Stabilization.
    pub fn update(&mut self, x: u16, y: u16, dt_ms: f64, config: &Config) -> (u16, u16) {
        let raw = (f64::from(x), f64::from(y));
        let Some(previous) = self.previous else {
            self.previous = Some(raw);
            self.motion = raw;
            self.stream = raw;
            self.remember(raw, dt_ms);
            return (x, y);
        };
        let delta = (raw.0 - previous.0, raw.1 - previous.1);
        let distance = delta.0.hypot(delta.1);
        self.previous = Some(raw);
        let amount = config.motion_filter_amount / 100.0;
        let mut point = raw;
        if amount > 0.0 && distance > 0.0 {
            if self.direction == (0.0, 0.0) {
                self.direction = (delta.0 / distance, delta.1 / distance);
            }
            // A reversal starts a fresh direction, retaining deliberate corners and hooks.
            if delta.0 * self.direction.0 + delta.1 * self.direction.1 < -0.5 * distance {
                self.direction = (delta.0 / distance, delta.1 / distance);
            }
            let normal = (-self.direction.1, self.direction.0);
            let lateral = (raw.0 - self.motion.0) * normal.0 + (raw.1 - self.motion.1) * normal.1;
            let expression = config.motion_filter_expression / 100.0 * (1.0 - amount);
            let removed =
                lateral.clamp(-80.0 * amount * amount, 80.0 * amount * amount) * (1.0 - expression);
            point = (raw.0 - normal.0 * removed, raw.1 - normal.1 * removed);
            let a = 1.0 - (-distance / (25.0 + 175.0 * amount)).exp();
            let direction = (
                self.direction.0 * (1.0 - a) + delta.0 / distance * a,
                self.direction.1 * (1.0 - a) + delta.1 / distance * a,
            );
            let length = direction.0.hypot(direction.1);
            if length > 1e-9 {
                self.direction = (direction.0 / length, direction.1 / length);
            }
        } else if amount > 0.0 {
            point = self.motion;
        }
        let previous_motion = self.motion;
        self.motion = point;
        let length = 250.0 * (config.streamline_amount / 100.0).powi(2);
        if length > 0.0 {
            let d = (point.0 - previous_motion.0, point.1 - previous_motion.1);
            let travel = d.0.hypot(d.1);
            if travel > 0.0 {
                // Exact exponential integration along this segment reduces report-rate bias.
                let decay = (-travel / length).exp();
                let scale = length / travel;
                self.stream = (
                    point.0 - d.0 * scale
                        + (self.stream.0 - previous_motion.0 + d.0 * scale) * decay,
                    point.1 - d.1 * scale
                        + (self.stream.1 - previous_motion.1 + d.1 * scale) * decay,
                );
            }
            point = self.stream;
        } else {
            self.stream = point;
        }
        self.remember(point, dt_ms);
        if config.stabilization_amount > 0.0 && dt_ms.is_finite() && dt_ms > 0.0 {
            let speed = distance * 10.0 / dt_ms; // mm/second
            let window_ms =
                config.stabilization_amount / 100.0 * (4.0 + 24.0 * speed / (speed + 80.0));
            // Integrate the recent piecewise-linear path over the requested time.
            // Fractional report intervals matter: rounding to one sample used to
            // disable stabilization at common CTL-460 report rates and caused steps.
            let mut newer = point;
            let mut remaining = window_ms;
            let mut covered = 0.0;
            let mut area = (0.0, 0.0);
            for offset in 0..self.count.saturating_sub(1) {
                let index = (self.next + 31 - offset) % 32;
                let older = self.history[(index + 31) % 32];
                let span = self.intervals[index];
                if span > 0.0 {
                    let used = remaining.min(span);
                    let f = used / span;
                    let boundary = (
                        newer.0 + (older.0 - newer.0) * f,
                        newer.1 + (older.1 - newer.1) * f,
                    );
                    area.0 += (newer.0 + boundary.0) * 0.5 * used;
                    area.1 += (newer.1 + boundary.1) * 0.5 * used;
                    covered += used;
                    remaining -= used;
                    if remaining <= 0.0 {
                        break;
                    }
                }
                newer = older;
            }
            if covered > 0.0 {
                point = (area.0 / covered, area.1 / covered);
            }
        }
        (
            point.0.round().clamp(0.0, 14720.0) as u16,
            point.1.round().clamp(0.0, 9200.0) as u16,
        )
    }

    fn remember(&mut self, point: (f64, f64), dt_ms: f64) {
        self.intervals[self.next] = if dt_ms.is_finite() {
            dt_ms.max(0.0)
        } else {
            0.0
        };
        self.history[self.next] = point;
        self.next = (self.next + 1) % 32;
        self.count = (self.count + 1).min(32);
    }
}
