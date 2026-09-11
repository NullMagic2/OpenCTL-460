//! Loads and validates a commented TOML pressure profile before device access.
//! Unknown keys and unsafe/nonfinite values fail with readable configuration errors.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub floor: u16,
    pub ceiling: u16,
    pub gamma: f64,
    pub pressure_nodes: Vec<crate::pressure_profiles::Node>,
    pub gain: f64,
    pub smoothing_ms: f64,
    pub responsiveness: f64,
    pub interpolation_ms: f64,
    pub press_on: u16,
    pub press_off: u16,
    pub output_hz: u32,
    pub stale_ms: u32,
    pub stroke_smoothing: bool,
    pub stroke_min_cutoff_hz: f64,
    pub stroke_beta: f64,
    /// Independent artistic controls, in percent. Zero preserves older profiles.
    pub streamline_amount: f64,
    pub streamline_pressure: f64,
    pub stabilization_amount: f64,
    pub motion_filter_amount: f64,
    pub motion_filter_expression: f64,
    /// Small-motion damping in hover and contact, independent of artistic smoothing.
    pub wobble_reduction: f64,
    pub preserve_aspect: bool,
    pub left_handed: bool,
    pub virtual_tilt: bool,
    pub tilt_max_degrees: f64,
    /// Extra double-tap tolerance in primary-screen pixels; zero disables assistance.
    pub double_click_distance: u32,
    pub button1: String,
    pub button2: String,
    pub backend: String,
    /// Compatibility report framing, changeable live while the pen is lifted.
    pub legacy_reports: bool,
    /// off = drawing settings; on = manual handwriting; auto = experimental stroke heuristic.
    pub handwriting_mode: String,
    /// legacy preserves existing profiles; other choices replace all contact
    /// position stages during handwriting. Pressure and hover remain independent.
    pub handwriting_filter: String,
    /// None retains the old independent position settings. Some(0..100)
    /// replaces them with one filter for every contact stroke. 30 = Light.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pen_control: Option<f64>,
    /// Remember an explicit choice of the older controls across GUI restarts.
    pub keep_advanced_smoothing: bool,
    /// Additional directional correction, independent of the base and artistic filters.
    pub circle_smoothing: f64,
    /// Protect deliberate corners and sustained small curves from combined smoothing.
    pub preserve_corners: bool,
    /// Final contact displacement bound in tablet millimetres; zero disables the guard.
    pub max_smoothing_distance_mm: f64,
    /// Restored UI applies the individual artistic sliders after the base filter.
    pub independent_line_controls: bool,
    /// Accept obsolete settings without restoring their speed-driven pressure behavior.
    #[serde(skip_serializing, deserialize_with = "ignore_legacy_speed_pressure")]
    pub speed_pressure: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            floor: 0,
            // Default pressure is a straight diagonal across the complete sensor range.
            ceiling: 1023,
            gamma: 1.0,
            pressure_nodes: Vec::new(),
            gain: 1.0,
            smoothing_ms: 6.0,
            responsiveness: 12.0,
            interpolation_ms: 4.0,
            press_on: 3,
            press_off: 1,
            output_hz: 250,
            stale_ms: 100,
            stroke_smoothing: true,
            stroke_min_cutoff_hz: 16.0,
            stroke_beta: 0.4,
            streamline_amount: 0.0,
            streamline_pressure: 0.0,
            stabilization_amount: 0.0,
            motion_filter_amount: 0.0,
            motion_filter_expression: 0.0,
            wobble_reduction: 40.0,
            preserve_aspect: true,
            left_handed: false,
            virtual_tilt: false,
            tilt_max_degrees: 50.0,
            double_click_distance: 0,
            button1: "Right click".into(),
            button2: "None".into(),
            backend: "ink".into(),
            legacy_reports: false,
            handwriting_mode: "off".into(),
            handwriting_filter: "legacy".into(),
            pen_control: None,
            keep_advanced_smoothing: false,
            circle_smoothing: 0.0,
            preserve_corners: true,
            max_smoothing_distance_mm: 1.0,
            independent_line_controls: false,
            speed_pressure: false,
        }
    }
}

impl Config {
    /// GUI migration preserves the saved base feel and exposes independent sliders.
    pub fn restore_line_controls(&mut self) {
        if let Some(amount) = self.pen_control {
            if amount > 30.0 {
                self.circle_smoothing = 100.0 * (amount - 30.0) / 70.0;
                self.pen_control = Some(30.0);
            }
        }
        self.independent_line_controls = true;
        self.keep_advanced_smoothing = true;
    }
    /// GUI-only upgrade: keep explicit custom/experimental paths intact.
    /// Standard legacy profiles adopt this release's default position filter.
    pub fn upgrade_pen_control(&mut self) {
        if self.keep_advanced_smoothing {
            return;
        }
        let standard_legacy = self.handwriting_filter == "legacy"
            && self.handwriting_mode != "auto"
            && self.streamline_amount == 0.0
            && self.stabilization_amount == 0.0
            && self.motion_filter_amount == 0.0
            && self.motion_filter_expression == 0.0;
        let light = self.handwriting_filter == "minimal" && self.handwriting_mode == "on";
        if self.pen_control.is_none() && self.stroke_smoothing && (standard_legacy || light) {
            self.pen_control = Some(30.0);
        }
    }
    pub fn simple_default() -> Self {
        Self {
            pen_control: Some(30.0),
            handwriting_mode: "on".into(),
            handwriting_filter: "minimal".into(),
            ..Self::default()
        }
    }
    /// Drawing presets must not change where the tablet maps or the user's shortcut assignments.
    pub fn drawing_profile(&self, mut preset: Self) -> Self {
        preset.double_click_distance = self.double_click_distance;
        preset.left_handed = self.left_handed;
        preset.preserve_aspect = self.preserve_aspect;
        preset.button1 = self.button1.clone();
        preset.button2 = self.button2.clone();
        preset.backend = self.backend.clone();
        preset.legacy_reports = self.legacy_reports;
        preset.pen_control = self.pen_control;
        preset.keep_advanced_smoothing = self.keep_advanced_smoothing;
        preset.circle_smoothing = self.circle_smoothing;
        preset.preserve_corners = self.preserve_corners;
        preset.max_smoothing_distance_mm = self.max_smoothing_distance_mm;
        preset.independent_line_controls = self.independent_line_controls;
        preset
    }
    pub fn parse(text: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        config.validate()?;
        Ok(config)
    }

    /// Erase is held, not toggled; either assigned side button can activate it.
    pub fn button_eraser(&self, states: [bool; 2]) -> bool {
        states
            .into_iter()
            .zip([&self.button1, &self.button2])
            .any(|(pressed, action)| pressed && action == "Erase (hold)")
    }
    pub fn validate(&self) -> Result<(), String> {
        if !self.circle_smoothing.is_finite() || !(0.0..=100.0).contains(&self.circle_smoothing) {
            return Err("circle_smoothing must be finite and within 0..100".into());
        }
        if self
            .pen_control
            .is_some_and(|v| !v.is_finite() || !(0.0..=100.0).contains(&v))
        {
            return Err("pen_control must be finite and within 0..100".into());
        }
        if !["legacy", "raw", "minimal", "responsive"].contains(&self.handwriting_filter.as_str()) {
            return Err(
                "handwriting_filter must be 'legacy', 'raw', 'minimal', or 'responsive'".into(),
            );
        }
        if !crate::pressure_profiles::valid(&self.pressure_nodes) {
            return Err("Pressure nodes must increase in input, never decrease in output, and retain (0,0)/(1,1) endpoints; maximum 16 nodes.".into());
        }
        if self.double_click_distance > 40 {
            return Err("double_click_distance must be 0..40 screen pixels".into());
        }
        if !["off", "on", "auto"].contains(&self.handwriting_mode.as_str()) {
            return Err("handwriting_mode must be 'off', 'on', or 'auto'".into());
        }
        if self.backend != "ink" && self.backend != "hid" {
            return Err("backend must be 'ink' or 'hid'".into());
        }
        if self.floor >= self.ceiling || self.ceiling > 1023 {
            return Err("Require 0 <= floor < ceiling <= 1023".into());
        }
        if self.press_off >= self.press_on || self.press_on > 1023 {
            return Err("Require press_off < press_on <= 1023".into());
        }
        for (name, value, low, high) in [
            (
                "max_smoothing_distance_mm",
                self.max_smoothing_distance_mm,
                0.0,
                5.0,
            ),
            ("wobble_reduction", self.wobble_reduction, 0.0, 100.0),
            ("streamline_amount", self.streamline_amount, 0.0, 100.0),
            ("streamline_pressure", self.streamline_pressure, 0.0, 100.0),
            (
                "stabilization_amount",
                self.stabilization_amount,
                0.0,
                100.0,
            ),
            (
                "motion_filter_amount",
                self.motion_filter_amount,
                0.0,
                100.0,
            ),
            (
                "motion_filter_expression",
                self.motion_filter_expression,
                0.0,
                100.0,
            ),
            ("gamma", self.gamma, 0.2, 4.0),
            ("gain", self.gain, 0.1, 3.0),
            ("smoothing_ms", self.smoothing_ms, 0.0, 30.0),
            ("responsiveness", self.responsiveness, 0.0, 100.0),
            ("interpolation_ms", self.interpolation_ms, 0.0, 16.0),
            (
                "stroke_min_cutoff_hz",
                self.stroke_min_cutoff_hz,
                1.0,
                120.0,
            ),
            ("stroke_beta", self.stroke_beta, 0.0, 10.0),
            ("tilt_max_degrees", self.tilt_max_degrees, 0.0, 60.0),
        ] {
            if !value.is_finite() || !(low..=high).contains(&value) {
                return Err(format!("{name} must be finite and within {low}..={high}"));
            }
        }
        if !(60..=500).contains(&self.output_hz) || !(30..=1000).contains(&self.stale_ms) {
            return Err("output_hz must be 60..=500; stale_ms must be 30..=1000".into());
        }
        crate::button_actions::validate(&self.button1)?;
        crate::button_actions::validate(&self.button2)?;
        Ok(())
    }

    /// Maps measured force into brush pressure; increasing gain never increases sensor resolution.
    pub fn curve(&self, raw: u16) -> f64 {
        self.curve_at(f64::from(raw))
    }

    /// Continuous evaluation lets the graph render between sensor steps without jagged quantization.
    pub fn curve_at(&self, raw: f64) -> f64 {
        let unit =
            ((raw - f64::from(self.floor)) / f64::from(self.ceiling - self.floor)).clamp(0.0, 1.0);
        let shaped = if self.pressure_nodes.is_empty() {
            unit.powf(self.gamma)
        } else {
            crate::pressure_profiles::evaluate(&self.pressure_nodes, unit)
        };
        (shaped * self.gain).clamp(0.0, 1.0)
    }
}

fn ignore_legacy_speed_pressure<'de, D: serde::Deserializer<'de>>(
    input: D,
) -> Result<bool, D::Error> {
    bool::deserialize(input).map(|_| false)
}

pub const BUTTON_ACTIONS: &[&str] = &[
    "None",
    "Right click",
    "Erase (hold)",
    "Undo (Ctrl+Z)",
    "Redo (Ctrl+Y)",
    "Eraser (E)",
    "Brush (B)",
    "Pan (hold Space)",
    "Left click",
    "Middle click",
    "Double click",
];
