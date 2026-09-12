//! Independent causal filters for flowing strokes and spatial noise suppression.
//! StreamLine follows distance, Stabilization averages more at higher speed, and Motion
//! Filtering uses a windowed-sinc FFT response with bounded corner behavior.
//! Fixed storage bounds processing and memory; callers reset at every stroke boundary.
use crate::{config::Config, spectral_smoothing::SpectralSmoothing};

pub struct LineSmoothing {
    previous: Option<(f64, f64)>,
    spectral: SpectralSmoothing,
    low: (f64, f64),
    low_twice: (f64, f64),
    motion: (f64, f64),
    stream: (f64, f64),
    stream_twice: (f64, f64),
    stream_output: (f64, f64),
    history: [(f64, f64); 64],
    rest_anchor: Option<(f64, f64)>,
    rest_ms: f64,
    stabilization_speed: f64,
    intervals: [f64; 64],
    next: usize,
    count: usize,
}

impl Default for LineSmoothing {
    fn default() -> Self {
        Self {
            previous: None,
            spectral: SpectralSmoothing::default(),
            low: (0.0, 0.0),
            low_twice: (0.0, 0.0),
            motion: (0.0, 0.0),
            stream: (0.0, 0.0),
            stream_twice: (0.0, 0.0),
            stream_output: (0.0, 0.0),
            history: [(0.0, 0.0); 64],
            intervals: [0.0; 64],
            next: 0,
            count: 0,
            rest_anchor: None,
            rest_ms: 0.0,
            stabilization_speed: 0.0,
        }
    }
}

impl LineSmoothing {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Coordinates are tablet units (100/mm). Time affects Stabilization and optional contact-only endpoint settling.
    pub fn update(&mut self, x: u16, y: u16, dt_ms: f64, config: &Config) -> (u16, u16) {
        self.update_finishing(x, y, dt_ms, config, 0.0)
    }
    pub fn update_finishing(
        &mut self,
        x: u16,
        y: u16,
        dt_ms: f64,
        config: &Config,
        finishing: f64,
    ) -> (u16, u16) {
        let raw = (f64::from(x), f64::from(y));
        let rest = self.rest_anchor.get_or_insert(raw);
        if (raw.0 - rest.0).hypot(raw.1 - rest.1) > 8.0 {
            *rest = raw;
            self.rest_ms = 0.0;
        } else if dt_ms.is_finite() {
            self.rest_ms += dt_ms.clamp(0.0, 32.0);
        }
        let settling = config.endpoint_settling && (self.rest_ms > 40.0 || finishing > 0.0);
        let settle_alpha = if settling && dt_ms.is_finite() {
            let rest = if self.rest_ms > 40.0 {
                1.0 - (-dt_ms.clamp(0.0, 32.0) / 55.0).exp()
            } else {
                0.0
            };
            rest.max(finishing.clamp(0.0, 1.0) * (1.0 - (-dt_ms.clamp(0.0, 32.0) / 15.0).exp()))
        } else {
            0.0
        };
        let Some(previous) = self.previous else {
            self.spectral.update(
                raw,
                config.motion_filter_amount / 100.0,
                config.motion_filter_expression / 100.0,
            );
            self.previous = Some(raw);
            self.motion = raw;
            self.low = raw;
            self.low_twice = raw;
            self.stream = raw;
            self.stream_twice = raw;
            self.stream_output = raw;
            self.remember(raw, dt_ms);
            return (x, y);
        };
        let delta = (raw.0 - previous.0, raw.1 - previous.1);
        let distance = delta.0.hypot(delta.1);
        self.previous = Some(raw);
        let amount = config.motion_filter_amount / 100.0;
        let mut point = raw;
        if config.flowing_smoothing && amount > 1e-6 && distance > 0.0 {
            let candidate =
                self.spectral
                    .update(raw, amount, config.motion_filter_expression / 100.0);
            // A circular bound avoids axis locking through shallow diagonals.
            let candidate = constrain_motion(self.motion, raw, candidate);
            let correction = (candidate.0 - raw.0, candidate.1 - raw.1);
            let error = correction.0.hypot(correction.1);
            let scale = (200.0 * amount / error.max(1e-12)).min(1.0);
            point = (raw.0 + correction.0 * scale, raw.1 + correction.1 * scale);
        } else if amount > 1e-6 && distance > 0.0 {
            // Two cascaded spatial low-pass states, integrated exactly along
            // the measured segment. Distance, not report count or elapsed time,
            // sets the response. The compensated combination retains straight
            // travel while rejecting short-wavelength coordinate noise.
            let length = (if config.flowing_smoothing {
                400.0
            } else {
                200.0
            }) * amount
                * amount;
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
            let limit = (if config.flowing_smoothing {
                200.0
            } else {
                80.0
            }) * amount
                * amount;
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
        if settling {
            point = (
                point.0 + settle_alpha * (raw.0 - point.0),
                point.1 + settle_alpha * (raw.1 - point.1),
            );
            if config.flowing_smoothing && (point.0 - raw.0).hypot(point.1 - raw.1) < 1.0 {
                self.spectral.anchor(raw);
            }
            self.low = (
                self.low.0 + settle_alpha * (raw.0 - self.low.0),
                self.low.1 + settle_alpha * (raw.1 - self.low.1),
            );
            self.low_twice = (
                self.low_twice.0 + settle_alpha * (raw.0 - self.low_twice.0),
                self.low_twice.1 + settle_alpha * (raw.1 - self.low_twice.1),
            );
        }
        let previous_motion = self.motion;
        self.motion = point;
        let a = config.streamline_amount / 100.0;
        let length = if config.flowing_smoothing && a > 0.0 {
            // Convert a nonlinear per-control-point fraction to a spatial time
            // constant at a documented 0.25 mm reference step.
            let fraction = 1.0 - 0.96 * a.powf(0.125);
            -25.0 / (1.0 - fraction).ln()
        } else {
            250.0 * a * a
        };
        if length > 0.0 {
            let d = (point.0 - previous_motion.0, point.1 - previous_motion.1);
            let travel = d.0.hypot(d.1);
            if travel > 0.0 {
                // Integrate both spatial states over the complete measured segment.
                // A partial second-stage correction reduces forward lag and oval
                // shrinkage without predicting future points.
                let decay = (-travel / length).exp();
                let scale = length / travel;
                let old_stream = self.stream;
                let old_twice = self.stream_twice;
                let update = |end: f64, previous: f64, low: f64, twice: f64, delta: f64| {
                    let lag = delta * scale;
                    let a = low - previous + lag;
                    let b = twice - previous + 2.0 * lag;
                    (
                        end - lag + a * decay,
                        end - 2.0 * lag + (b + a * travel / length) * decay,
                    )
                };
                let x = update(point.0, previous_motion.0, old_stream.0, old_twice.0, d.0);
                let y = update(point.1, previous_motion.1, old_stream.1, old_twice.1, d.1);
                self.stream = (x.0, y.0);
                self.stream_twice = (x.1, y.1);
            }
            if settling {
                self.stream.0 += settle_alpha * (raw.0 - self.stream.0);
                self.stream.1 += settle_alpha * (raw.1 - self.stream.1);
                self.stream_twice.0 += settle_alpha * (raw.0 - self.stream_twice.0);
                self.stream_twice.1 += settle_alpha * (raw.1 - self.stream_twice.1);
            }
            point = if config.flowing_smoothing {
                let candidate = (
                    self.stream.0 + 0.4 * (self.stream.0 - self.stream_twice.0),
                    self.stream.1 + 0.4 * (self.stream.1 - self.stream_twice.1),
                );
                constrain_motion(self.stream_output, point, candidate)
            } else {
                self.stream
            };
        } else {
            self.stream = point;
            self.stream_twice = point;
        }
        self.stream_output = point;
        self.remember(point, dt_ms);
        if config.stabilization_amount > 0.0 && dt_ms.is_finite() && dt_ms > 0.0 {
            let measured_speed = distance * 10.0 / dt_ms; // mm/second
                                                          // Smooth the window's speed estimate so coordinate jitter cannot
                                                          // resize the averaging window on every report.
            let speed = if config.flowing_smoothing {
                self.stabilization_speed +=
                    (1.0 - (-dt_ms / 80.0).exp()) * (measured_speed - self.stabilization_speed);
                self.stabilization_speed
            } else {
                measured_speed
            };
            let strength = config.stabilization_amount / 100.0;
            let strength = if config.flowing_smoothing {
                strength.powf(2.2)
            } else {
                strength
            };
            let window_ms = strength
                * if config.flowing_smoothing {
                    8.0 + 72.0 * speed / (speed + 80.0)
                } else {
                    4.0 + 24.0 * speed / (speed + 80.0)
                };
            // Integrate the recent piecewise-linear path over the requested time.
            // Fractional report intervals matter: rounding to one sample used to
            // disable stabilization at common CTL-460 report rates and caused steps.
            let mut newer = point;
            let mut remaining = window_ms;
            let mut covered = 0.0;
            let mut area = (0.0, 0.0);
            for offset in 0..self.count.saturating_sub(1) {
                let index = (self.next + 63 - offset) % 64;
                let older = self.history[(index + 63) % 64];
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
        self.next = (self.next + 1) % 64;
        self.count = (self.count + 1).min(64);
    }
}

/// Bound a filtered point inside the disk with the previous output and measured
/// point as diameter endpoints. Unlike independent x/y clamps, the same geometry
/// applies at every heading, while a reversal cannot pass either endpoint.
pub fn constrain_motion(
    previous: (f64, f64),
    raw: (f64, f64),
    candidate: (f64, f64),
) -> (f64, f64) {
    let center = ((previous.0 + raw.0) * 0.5, (previous.1 + raw.1) * 0.5);
    let radius = (raw.0 - previous.0).hypot(raw.1 - previous.1) * 0.5;
    let d = (candidate.0 - center.0, candidate.1 - center.1);
    let length = d.0.hypot(d.1);
    if length <= radius {
        candidate
    } else if length > 0.0 {
        (
            center.0 + d.0 * radius / length,
            center.1 + d.1 * radius / length,
        )
    } else {
        center
    }
}
