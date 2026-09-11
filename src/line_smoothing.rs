//! Independent causal filters for flowing strokes and spatial noise suppression.
//! StreamLine follows distance, Stabilization averages more at higher speed, and Motion
//! Filtering attenuates short spatial wavelengths with bounded corner behavior.
//! Fixed storage bounds processing and memory; callers reset at every stroke boundary.
use crate::config::Config;

#[derive(Default)]
pub struct LineSmoothing {
    previous: Option<(f64, f64)>,
    low: (f64, f64),
    low_twice: (f64, f64),
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
            self.low = raw;
            self.low_twice = raw;
            self.stream = raw;
            self.remember(raw, dt_ms);
            return (x, y);
        };
        let delta = (raw.0 - previous.0, raw.1 - previous.1);
        let distance = delta.0.hypot(delta.1);
        self.previous = Some(raw);
        let amount = config.motion_filter_amount / 100.0;
        let mut point = raw;
        if amount > 1e-6 && distance > 0.0 {
            // Two cascaded spatial low-pass states, integrated exactly along
            // the measured segment. Distance, not report count or elapsed time,
            // sets the response. The compensated combination retains straight
            // travel while rejecting short-wavelength coordinate noise.
            let length = 200.0 * amount * amount;
            let ratio = distance / length;
            let decay = (-ratio).exp();
            let axis = |raw: f64, previous: f64, low: f64, twice: f64| {
                let lag = (raw - previous) / ratio;
                let a = low - previous + lag;
                let b = twice - previous + 2.0 * lag;
                (
                    raw - lag + a * decay,
                    raw - 2.0 * lag + (b + a * ratio) * decay,
                )
            };
            let (lx, tx) = axis(raw.0, previous.0, self.low.0, self.low_twice.0);
            let (ly, ty) = axis(raw.1, previous.1, self.low.1, self.low_twice.1);
            self.low = (lx, ly);
            self.low_twice = (tx, ty);
            // Compensation can ring at a reversal. Clamp to the interval from
            // the previous output to this real endpoint, then bound total
            // displacement. No future point or post-release report is created.
            let candidate = (
                (2.0 * lx - tx).clamp(self.motion.0.min(raw.0), self.motion.0.max(raw.0)),
                (2.0 * ly - ty).clamp(self.motion.1.min(raw.1), self.motion.1.max(raw.1)),
            );
            let correction = (candidate.0 - raw.0, candidate.1 - raw.1);
            let error = correction.0.hypot(correction.1);
            let limit = 80.0 * amount * amount;
            let expression = config.motion_filter_expression / 100.0 * (1.0 - amount);
            let scale = if error > 0.0 {
                (limit / error).min(1.0)
            } else {
                1.0
            };
            point = (
                raw.0 + correction.0 * scale * (1.0 - expression),
                raw.1 + correction.1 * scale * (1.0 - expression),
            );
        } else if amount > 1e-6 {
            point = self.motion;
        } else {
            self.low = raw;
            self.low_twice = raw;
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
