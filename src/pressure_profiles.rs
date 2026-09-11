//! Smooth monotone pressure nodes and a named, user-owned preset library.
//! The built-in Default is generated from Config and never stored as an editable library entry.
use crate::config::Config;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub x: f64,
    pub y: f64,
}
pub fn valid(nodes: &[Node]) -> bool {
    nodes.is_empty()
        || ((2..=16).contains(&nodes.len())
            && nodes.first() == Some(&Node { x: 0.0, y: 0.0 })
            && nodes.last() == Some(&Node { x: 1.0, y: 1.0 })
            && nodes.iter().all(|p| {
                p.x.is_finite()
                    && p.y.is_finite()
                    && (0.0..=1.0).contains(&p.x)
                    && (0.0..=1.0).contains(&p.y)
            })
            && nodes
                .windows(2)
                .all(|p| p[1].x - p[0].x >= 0.001 - 1e-12 && p[1].y >= p[0].y))
}
/// Weighted harmonic tangents (PCHIP) preserve monotonicity without pressure overshoot.
pub fn evaluate(nodes: &[Node], x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let i = nodes
        .windows(2)
        .position(|p| x <= p[1].x)
        .unwrap_or(nodes.len() - 2);
    let slope = |j: usize| (nodes[j + 1].y - nodes[j].y) / (nodes[j + 1].x - nodes[j].x);
    let tangent = |j: usize| {
        if j == 0 {
            return slope(0);
        }
        if j == nodes.len() - 1 {
            return slope(j - 1);
        }
        let a = slope(j - 1);
        let b = slope(j);
        if a == 0.0 || b == 0.0 {
            return 0.0;
        }
        let h0 = nodes[j].x - nodes[j - 1].x;
        let h1 = nodes[j + 1].x - nodes[j].x;
        let w0 = 2.0 * h1 + h0;
        let w1 = h1 + 2.0 * h0;
        (w0 + w1) / (w0 / a + w1 / b)
    };
    let a = nodes[i];
    let b = nodes[i + 1];
    let h = b.x - a.x;
    let t = (x - a.x) / h;
    ((2.0 * t.powi(3) - 3.0 * t * t + 1.0) * a.y
        + (t.powi(3) - 2.0 * t * t + t) * h * tangent(i)
        + (-2.0 * t.powi(3) + 3.0 * t * t) * b.y
        + (t.powi(3) - t * t) * h * tangent(i + 1))
    .clamp(a.y, b.y)
}
pub fn seed(config: &mut Config) {
    seed_at(config, 0.5);
}
/// Restore a straight, full-range response without changing other pen settings.
pub fn reset_linear(config: &mut Config) {
    config.floor = 0;
    config.ceiling = 1023;
    config.gamma = 1.0;
    config.gain = 1.0;
    config.pressure_nodes.clear();
    seed(config);
}
/// Convert only the three existing graph handles; never add hidden sampling nodes.
pub fn seed_at(config: &mut Config, response: f64) {
    if config.pressure_nodes.is_empty() {
        let response = if response.is_finite() {
            response.clamp(0.001, 0.999)
        } else {
            0.5
        };
        config.pressure_nodes = [0.0, response, 1.0]
            .into_iter()
            .map(|x| Node {
                x,
                y: x.powf(config.gamma),
            })
            .collect();
    }
}
pub fn add(config: &mut Config, x: f64) -> Option<usize> {
    if config.pressure_nodes.len() >= 16 || !x.is_finite() {
        return None;
    }
    seed(config);
    let x = x.clamp(0.001, 0.999);
    if config
        .pressure_nodes
        .iter()
        .any(|p| (p.x - x).abs() < 0.001)
    {
        return None;
    }
    let y = evaluate(&config.pressure_nodes, x);
    let i = config.pressure_nodes.partition_point(|p| p.x < x);
    config.pressure_nodes.insert(i, Node { x, y });
    Some(i)
}
pub fn remove(config: &mut Config, i: usize) -> bool {
    if i == 0 || i + 1 >= config.pressure_nodes.len() {
        return false;
    }
    config.pressure_nodes.remove(i);
    true
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shape {
    pub floor: u16,
    pub ceiling: u16,
    pub gamma: f64,
    pub gain: f64,
    pub nodes: Vec<Node>,
}
impl Shape {
    pub fn from_config(c: &Config) -> Self {
        Self {
            floor: c.floor,
            ceiling: c.ceiling,
            gamma: c.gamma,
            gain: c.gain,
            nodes: c.pressure_nodes.clone(),
        }
    }
    pub fn apply(&self, c: &mut Config) {
        c.floor = self.floor;
        c.ceiling = self.ceiling;
        c.gamma = self.gamma;
        c.gain = self.gain;
        c.pressure_nodes = self.nodes.clone();
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    pub name: String,
    pub shape: Shape,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Library {
    pub presets: Vec<Preset>,
}
impl Library {
    pub fn add(&mut self, name: &str, shape: Shape) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty()
            || name.chars().count() > 64
            || name.chars().any(char::is_control)
            || ["default", "unsaved copy"].contains(&name.to_lowercase().as_str())
        {
            return Err("Use a name of 1-64 characters other than Default or Unsaved copy.".into());
        }
        if self.presets.len() >= 128
            || self
                .presets
                .iter()
                .any(|p| p.name.to_lowercase() == name.to_lowercase())
        {
            return Err(
                "Choose a new name; existing presets are never overwritten (maximum 128).".into(),
            );
        }
        let mut c = Config::default();
        shape.apply(&mut c);
        c.validate()?;
        self.presets.push(Preset {
            name: name.into(),
            shape,
        });
        Ok(())
    }
    pub fn remove(&mut self, name: &str) -> Result<(), String> {
        let i = self
            .presets
            .iter()
            .position(|p| p.name == name)
            .ok_or("Only saved custom presets can be deleted")?;
        self.presets.remove(i);
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e.to_string()),
        };
        let parsed: Self =
            toml::from_str(&text).map_err(|e| format!("Pressure preset library: {e}"))?;
        let mut result = Self::default();
        for p in parsed.presets {
            result.add(&p.name, p.shape)?;
        }
        Ok(result)
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let data = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        let temporary = path.with_extension("tmp");
        std::fs::write(
            &temporary,
            format!("# User-created pressure presets. Default is built in.\n{data}"),
        )
        .map_err(|e| e.to_string())?;
        std::fs::rename(temporary, path).map_err(|e| e.to_string())
    }
}
