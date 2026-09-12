//! Guided, raw-pressure calibration. No device access and no live configuration changes.
use crate::{
    config::Config,
    pressure_profiles::{Node, Shape},
};
use serde::{Deserialize, Serialize};

pub const STAGE_MS: u64 = 8_000;
pub const WAIT_MS: u64 = 120_000;
pub const STAGES: [&str; 3] = ["Light", "Medium", "Comfortably firm"];

#[derive(Default)]
pub struct Session {
    pub stage: usize,
    pub active: bool,
    pub started: Option<u64>,
    armed: u64,
    stroke: Vec<u16>,
    bodies: Vec<u16>,
    strokes: usize,
    pub anchors: Vec<u16>,
}
impl Session {
    pub fn arm(&mut self, now: u64) {
        self.active = true;
        self.started = None;
        self.armed = now;
        self.stroke.clear();
        self.bodies.clear();
        self.strokes = 0;
    }
    pub fn cancel(&mut self) {
        self.active = false;
        self.started = None;
        self.stroke.clear();
        self.bodies.clear();
    }
    pub fn sample(&mut self, now: u64, raw: i32, touching: bool) {
        if !self.active {
            return;
        }
        if touching && (3..=1023).contains(&raw) {
            self.started.get_or_insert(now);
            // Bound memory independently of the feeder's advertised cadence.
            if self.stroke.len() < 8_192 {
                self.stroke.push(raw as u16);
            }
        } else {
            self.end_stroke();
        }
    }
    fn end_stroke(&mut self) {
        if self.stroke.len() >= 20 {
            let trim = self.stroke.len() / 5;
            self.bodies
                .extend_from_slice(&self.stroke[trim..self.stroke.len() - trim]);
            self.strokes += 1;
        }
        self.stroke.clear();
    }
    /// None means still waiting/recording; Err keeps this stage available for retry.
    pub fn tick(&mut self, now: u64) -> Option<Result<Option<Shape>, String>> {
        if !self.active {
            return None;
        }
        let Some(start) = self.started else {
            if now.saturating_sub(self.armed) >= WAIT_MS {
                self.cancel();
                return Some(Err(
                    "No pen contact received. Check the driver, then record this stage again."
                        .into(),
                ));
            }
            return None;
        };
        if now.saturating_sub(start) < STAGE_MS {
            return None;
        }
        self.end_stroke();
        self.active = false;
        if self.strokes < 2 || self.bodies.len() < 100 {
            return Some(Err(
                "Draw at least two strokes during this stage, lifting between them. Please retry."
                    .into(),
            ));
        }
        self.bodies.sort_unstable();
        let anchor = self.bodies[self.bodies.len() / 2];
        if anchor < 20 || anchor > 980 || self.anchors.last().is_some_and(|last| anchor < last + 40)
        {
            return Some(Err("Pressure levels were too close or out of order. Retry with distinct, comfortable pressure; do not press hard.".into()));
        }
        self.anchors.push(anchor);
        self.stage += 1;
        Some(if self.stage == 3 {
            suggest(&self.anchors).map(Some)
        } else {
            Ok(None)
        })
    }
}
pub fn suggest(anchors: &[u16]) -> Result<Shape, String> {
    if anchors.len() != 3
        || anchors[0] < 20
        || anchors[2] > 980
        || anchors.windows(2).any(|a| a[1] < a[0].saturating_add(40))
    {
        return Err(
            "Three distinct light, medium and firm pressure measurements are required.".into(),
        );
    }
    let mut nodes = vec![Node { x: 0., y: 0. }];
    nodes.extend(
        anchors
            .iter()
            .zip([0.14, 0.45, 0.90])
            .map(|(&raw, y)| Node {
                x: raw as f64 / 1023.,
                y,
            }),
    );
    nodes.push(Node { x: 1., y: 1. });
    let shape = Shape {
        floor: 0,
        ceiling: 1023,
        gamma: 1.,
        gain: 1.,
        nodes,
    };
    validate_shape(&shape)?;
    Ok(shape)
}
pub fn validate_shape(shape: &Shape) -> Result<(), String> {
    let mut c = Config::default();
    shape.apply(&mut c);
    c.validate()
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileFile {
    pub version: u32,
    pub shape: Shape,
}
pub fn export(shape: &Shape) -> Result<String, String> {
    validate_shape(shape)?;
    toml::to_string_pretty(&ProfileFile {
        version: 1,
        shape: shape.clone(),
    })
    .map_err(|e| e.to_string())
}
/// Dedicated profiles contain pressure only; full user settings are never accepted here.
pub fn import(data: &str) -> Result<Shape, String> {
    if data.len() > 65_536 {
        return Err("Calibration profile is too large (maximum 64 KiB).".into());
    }
    let profile: ProfileFile = toml::from_str(data).map_err(|_| {
        "Choose a .calibration_profile file containing a pressure profile, not a user settings file.".to_string()
    })?;
    if profile.version != 1 {
        return Err("Unsupported calibration profile version.".into());
    }
    validate_shape(&profile.shape)?;
    Ok(profile.shape)
}
/// Enforce the separate file type even when a name is typed into the native picker.
pub fn validate_profile_path(path: &std::path::Path) -> Result<(), String> {
    if path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("calibration_profile"))
    {
        Ok(())
    } else {
        Err(
            "Use a name ending in .calibration_profile (for example, Pencil.calibration_profile)."
                .into(),
        )
    }
}

/// Append the complete extension ourselves: the legacy picker documents a three-character default limit.
pub fn export_path(path: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let path = if path.extension().is_none() {
        path.with_extension("calibration_profile")
    } else {
        path.to_path_buf()
    };
    validate_profile_path(&path)?;
    Ok(path)
}

pub fn stage_prompt(stage: usize) -> String {
    let instruction = match stage {
        0 => "Light strokes: use gentle tip contact.",
        1 => "Medium strokes: use your normal, comfortable drawing pressure.",
        _ => "Firm strokes: use comfortably firm pressure. Never press hard.",
    };
    format!("{instruction}\r\n\r\nClick OK, then draw at least two strokes inside the square, lifting between them. Recording starts with the first stroke and lasts 8 seconds. Strokes elsewhere are ignored.")
}
