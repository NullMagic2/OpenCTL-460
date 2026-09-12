//! Turns real reports into continuous pressure with a bounded causal ramp.
//! Ramp interpolation estimates intermediate values over time; it invents no sensor bits.
//! Release, proximity loss and stale input bypass smoothing to avoid stuck ink.

use crate::{
    config::Config,
    handwriting::HandwritingDetector,
    line_smoothing::LineSmoothing,
    protocol::{Sample, MAX_X, MAX_Y},
    stroke::StrokeFilter,
    wobble::WobbleFilter,
    writing_filter::WritingFilter,
};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Frame {
    pub x: u16,
    pub y: u16,
    pub pressure: f64,
    pub in_range: bool,
    pub contact: bool,
    pub barrel: bool,
    pub eraser: bool,
    pub tilt_x: i32,
    pub tilt_y: i32,
    pub virtual_tilt: bool,
    pub handwriting: bool,
    /// Heuristic evidence 0..100, never a probability of correctly recognized writing.
    pub handwriting_score: u8,
}

impl Frame {
    pub fn ink_pressure(self) -> u32 {
        (self.pressure.clamp(0.0, 1.0) * 1024.0).round() as u32
    }

    /// Exactly 4,098 diagnostic/future-HID values. Ink injection uses 0..=1024.
    pub fn virtual_pressure(self) -> u16 {
        (self.pressure.clamp(0.0, 1.0) * 4097.0).round() as u16
    }
}

pub struct Engine {
    config: Config,
    frame: Frame,
    last_report: Option<f64>,
    filtered: f64,
    last_target: f64,
    release_drop: f64,
    ramp_from: f64,
    ramp_to: f64,
    ramp_start: f64,
    stroke: StrokeFilter,
    pen_control: crate::pen_control::PenControl,
    straight: crate::straight_assist::StraightAssist,
    circle_control: crate::pen_control::CircleControl,
    writing_filter: WritingFilter,
    wobble: WobbleFilter,
    line: LineSmoothing,
    release_settling: crate::endpoint_settling::ReleaseSettling,
    guard: crate::smoothing_guard::SmoothingGuard,
    handwriting: HandwritingDetector,
    ramp_ms: f64,
}

impl Engine {
    pub fn new(config: Config) -> Result<Self, String> {
        config.validate()?;
        Ok(Self {
            ramp_ms: config.interpolation_ms,
            config,
            frame: Frame::default(),
            last_report: None,
            filtered: 0.0,
            last_target: 0.0,
            release_drop: 0.0,
            ramp_from: 0.0,
            ramp_to: 0.0,
            ramp_start: 0.0,
            stroke: StrokeFilter::default(),
            pen_control: crate::pen_control::PenControl::default(),
            straight: crate::straight_assist::StraightAssist::default(),
            circle_control: crate::pen_control::CircleControl::default(),
            writing_filter: WritingFilter::default(),
            wobble: WobbleFilter::default(),
            line: LineSmoothing::default(),
            release_settling: Default::default(),
            guard: crate::smoothing_guard::SmoothingGuard::default(),
            handwriting: HandwritingDetector::default(),
        })
    }

    /// Keep an ongoing stroke unchanged. The caller retries after contact and held
    /// buttons end, so mapping, force and filter state change as one transaction.
    pub fn reconfigure(&mut self, config: Config) -> Result<bool, String> {
        config.validate()?;
        if self.frame.contact {
            return Ok(false);
        }
        self.release();
        self.wobble.reset();
        if self.config.handwriting_mode != config.handwriting_mode {
            self.handwriting.reset();
            self.frame.handwriting_score = 0;
        }
        self.ramp_ms = config.interpolation_ms;
        self.config = config;
        Ok(true)
    }

    fn pressure_at(&self, now_ms: f64) -> f64 {
        let fraction = if self.ramp_ms == 0.0 {
            1.0
        } else {
            ((now_ms - self.ramp_start) / self.ramp_ms).clamp(0.0, 1.0)
        };
        self.ramp_from + (self.ramp_to - self.ramp_from) * fraction
    }

    fn release(&mut self) {
        self.frame.contact = false;
        self.frame.pressure = 0.0;
        self.frame.handwriting = false;
        self.frame.virtual_tilt = false;
        self.filtered = 0.0;
        self.last_target = 0.0;
        self.release_drop = 0.0;
        self.ramp_from = 0.0;
        self.ramp_to = 0.0;
        self.stroke.reset();
        self.pen_control.reset();
        self.release_settling.reset();
        self.straight.reset();
        self.circle_control.reset();
        self.writing_filter.reset();
        self.line.reset();

        self.guard.reset();
        self.frame.tilt_x = 0;
        self.frame.tilt_y = 0;
    }

    pub fn push(&mut self, mut sample: Sample, now_ms: f64) -> Result<(), String> {
        if sample.position_valid && (sample.x > MAX_X || sample.y > MAX_Y) {
            return Err("Sample coordinates out of range".into());
        }
        if sample.pressure > 1023 {
            return Err("Sample pressure out of range".into());
        }
        if self.config.left_handed && sample.position_valid {
            sample.x = MAX_X - sample.x;
            sample.y = MAX_Y - sample.y;
        }
        if !now_ms.is_finite() || now_ms < 0.0 || self.last_report.is_some_and(|t| now_ms < t) {
            return Err("Report timestamps must be finite, nonnegative and monotonic".into());
        }
        // Expire the previous stroke before accepting a fresh report after a pause.
        self.tick(now_ms);
        let mut had_contact = self.frame.contact;
        let mut old_pressure = self.pressure_at(now_ms);
        let dt = self.last_report.map_or(4.0, |t| now_ms - t);
        self.last_report = Some(now_ms);
        if sample.position_valid {
            // A tool flip ends the old stroke before its pressure can affect the new tool.
            if self.frame.eraser != sample.eraser {
                self.release();
                self.wobble.reset();
                had_contact = false;
                old_pressure = 0.0;
            }
            self.frame.x = sample.x;
            self.frame.y = sample.y;
            self.frame.eraser = sample.eraser;
            if sample.in_range {
                (self.frame.x, self.frame.y) =
                    self.wobble
                        .update(sample.x, sample.y, dt, self.config.wobble_reduction);
            }
        }
        self.frame.in_range = sample.in_range && (sample.position_valid || self.frame.in_range);
        self.frame.barrel = self.frame.in_range && sample.barrel;
        let threshold = if self.frame.contact {
            self.config.press_off + 1
        } else {
            self.config.press_on
        };
        let contact = self.frame.in_range && sample.tip && sample.pressure >= threshold;
        if self.config.handwriting_mode == "auto" && (sample.position_valid || !contact) {
            self.handwriting.observe(
                if self.config.handwriting_filter == "legacy" {
                    self.frame.x
                } else {
                    sample.x
                },
                if self.config.handwriting_filter == "legacy" {
                    self.frame.y
                } else {
                    sample.y
                },
                contact,
                now_ms,
            );
            self.frame.handwriting_score = (self.handwriting.score() * 100.0).round() as u8;
        }
        if !contact {
            if !sample.in_range {
                self.wobble.reset();
                self.handwriting.reset();
                self.frame.handwriting_score = 0;
            }
            self.release();
            return Ok(());
        }
        if !had_contact {
            // The first contact lands on the measured point, not a lagging hover.
            if sample.position_valid {
                self.frame.x = sample.x;
                self.frame.y = sample.y;
                self.wobble.reset();
            }
            self.frame.handwriting = self.config.handwriting_mode == "on"
                || (self.config.handwriting_mode == "auto" && self.handwriting.likely());
            self.ramp_ms = if self.frame.handwriting {
                self.config.interpolation_ms.min(2.0)
            } else {
                self.config.interpolation_ms
            };
        }
        self.frame.contact = true;
        if sample.position_valid && self.config.max_smoothing_distance_mm > 0.0 {
            if self.guard.observe(
                sample.x,
                sample.y,
                self.config.preserve_corners,
                self.config.max_smoothing_distance_mm,
            ) {
                // Keep force/contact intact; discard only stale position history
                // at a deliberate corner so the old path cannot pull the pen back.
                if !self.config.flowing_smoothing {
                    self.line.reset();
                }
                self.stroke.reset();
                self.pen_control.reset();
                self.release_settling.reset();
                self.straight.reset();
                self.writing_filter.reset();
                self.circle_control.reset();
                self.wobble.reset();
                self.frame.x = sample.x;
                self.frame.y = sample.y;
            }
        }

        let single_writing_filter = self.config.pen_control.is_some()
            || self.frame.handwriting && self.config.handwriting_filter != "legacy";
        if sample.position_valid && self.config.pen_control.is_some() {
            (self.frame.x, self.frame.y) = if self.config.independent_line_controls
                && !self.config.stroke_smoothing
            {
                (sample.x, sample.y)
            } else {
                self.pen_control
                    .update(sample.x, sample.y, dt, self.config.pen_control.unwrap())
            };
        } else if sample.position_valid && single_writing_filter {
            // Start from original coordinates, bypassing hover wobble and all
            // artistic position stages. One displacement budget covers the path.
            (self.frame.x, self.frame.y) =
                if !self.config.stroke_smoothing || self.config.handwriting_filter == "raw" {
                    (sample.x, sample.y)
                } else {
                    self.writing_filter.update(
                        sample.x,
                        sample.y,
                        dt,
                        self.config.wobble_reduction,
                        self.config.handwriting_filter == "responsive",
                    )
                };
        } else if sample.position_valid && self.config.stroke_smoothing {
            let (x, y) = if self.frame.handwriting {
                self.stroke.handwriting(self.frame.x, self.frame.y, dt)
            } else {
                self.stroke.update(
                    self.frame.x,
                    self.frame.y,
                    dt,
                    self.config.stroke_min_cutoff_hz,
                    self.config.stroke_beta,
                )
            };
            self.frame.x = x;
            self.frame.y = y;
        }
        // Normal pen processing: learn only sufficiently straight contact paths.
        // Existing raw/off settings remain bypasses; Precision Hold is independent.
        if sample.position_valid {
            if self.config.stroke_smoothing
                && self.config.pen_control.is_none_or(|amount| amount > 0.0)
                && !self.frame.handwriting
                && !self.frame.eraser
                && !self.frame.barrel
            {
                let p = self
                    .straight
                    .update((f64::from(self.frame.x), f64::from(self.frame.y)), 0.5);
                self.frame.x = p.0.round().clamp(0.0, f64::from(MAX_X)) as u16;
                self.frame.y = p.1.round().clamp(0.0, f64::from(MAX_Y)) as u16;
            } else {
                self.straight.reset();
            }
        }
        if sample.position_valid
            && self.config.flowing_smoothing
            && self.config.max_smoothing_distance_mm > 0.0
        {
            (self.frame.x, self.frame.y) = self
                .guard
                .constrain((sample.x, sample.y), (self.frame.x, self.frame.y));
        }
        // Force determines width. Position and velocity never substitute pressure.
        let target = self.config.curve(sample.pressure);
        // User-selected artistic controls follow the adaptive drawing/handwriting
        // stage in both modes. Their extra displacement is deliberate and is not
        // constrained by the handwriting stage's own 0.12 mm jitter-filter bound.
        // In flowing mode the guard constrains only the base stage above.
        if sample.position_valid
            && (self.config.flowing_smoothing
                || !single_writing_filter
                || self.config.independent_line_controls)
        {
            let finishing = self
                .release_settling
                .update(sample.x, sample.y, sample.pressure, dt);
            let finishing =
                if self.config.endpoint_settling && !self.frame.eraser && !self.frame.barrel {
                    finishing
                } else {
                    0.0
                };
            (self.frame.x, self.frame.y) =
                self.line
                    .update_finishing(self.frame.x, self.frame.y, dt, &self.config, finishing);
        }
        if sample.position_valid && self.config.circle_smoothing > 0.0 {
            let (dx, dy) = self.circle_control.correction(
                sample.x,
                sample.y,
                dt,
                self.config.circle_smoothing,
            );
            self.frame.x = (i32::from(self.frame.x) + dx).clamp(0, i32::from(MAX_X)) as u16;
            self.frame.y = (i32::from(self.frame.y) + dy).clamp(0, i32::from(MAX_Y)) as u16;
        }
        if sample.position_valid
            && !self.config.flowing_smoothing
            && self.config.max_smoothing_distance_mm > 0.0
        {
            (self.frame.x, self.frame.y) = self
                .guard
                .constrain((sample.x, sample.y), (self.frame.x, self.frame.y));
        }
        let mut tau = (if self.frame.handwriting {
            self.config.smoothing_ms.min(3.0)
        } else {
            self.config.smoothing_ms
        }) / (1.0 + self.config.responsiveness * (target - self.filtered).abs())
            + 60.0 * (self.config.streamline_pressure / 100.0).powi(2);
        // Falling force must reach the current position promptly, even with strong
        // artistic pressure smoothing. Keep a short noise filter, but no extra ramp
        // from the previous width at the new release position. No synthetic tail is added.
        // A small alternating force fluctuation is jitter, not a pen release.
        // Only a meaningful or sustained measured fall takes the fast release path;
        // symmetric jitter uses the same filter in both directions and keeps its mean.
        if had_contact && target <= self.last_target {
            self.release_drop += self.last_target - target;
        } else {
            self.release_drop = 0.0;
        }
        self.last_target = target;
        let falling = had_contact && target < self.filtered && self.release_drop >= 0.03;
        if falling {
            tau = tau.min(2.0);
        }
        let alpha = if tau == 0.0 {
            1.0
        } else {
            1.0 - (-dt / tau).exp()
        };
        // A short tap must contain nonzero pressure even if it ends before the next timer tick.
        self.filtered = if had_contact {
            self.filtered + alpha * (target - self.filtered)
        } else {
            target
        };
        self.ramp_from = if falling {
            self.filtered
        } else if had_contact {
            old_pressure
        } else {
            target
        };
        self.ramp_to = self.filtered;
        self.ramp_start = now_ms;
        Ok(())
    }

    pub fn tick(&mut self, now_ms: f64) -> Frame {
        if !now_ms.is_finite()
            || self
                .last_report
                .is_none_or(|t| now_ms - t >= f64::from(self.config.stale_ms))
        {
            self.release();
            self.wobble.reset();
            self.handwriting.reset();
            self.frame.handwriting_score = 0;
            self.frame.in_range = false;
            self.frame.barrel = false;
        }
        self.frame.pressure = if self.frame.contact {
            self.pressure_at(now_ms)
        } else {
            0.0
        };
        self.frame
    }
}
