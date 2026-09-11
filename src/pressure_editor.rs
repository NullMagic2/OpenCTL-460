//! Converts graph gestures into the same calibrated pressure curve used by the live feeder.
//! Endpoint drags retain a valid calibration range; response drags fit the gamma curve to the pointer.
use crate::config::Config;

#[derive(Clone, Copy)]
pub enum Handle {
    Zero,
    Node(usize),
    Response,
    Full,
}

pub fn drag(config: &mut Config, handle: Handle, input: f64, output: f64) {
    if !input.is_finite() || !output.is_finite() {
        return;
    }
    let raw = (input.clamp(0.0, 1.0) * 1023.0).round() as u16;
    match handle {
        Handle::Node(i) => {
            // End nodes adjust the calibrated input range, just like the legacy graph handles.
            // Their normalized output remains zero/full so the curve stays monotone and valid.
            if i >= config.pressure_nodes.len() {
                return;
            }
            if i == 0 {
                config.floor = raw.min(config.ceiling - 1);
                return;
            }
            if i + 1 == config.pressure_nodes.len() {
                config.ceiling = raw.clamp(config.floor + 1, 1023);
                return;
            }
            let nodes = &mut config.pressure_nodes;
            let x = ((input * 1023.0 - f64::from(config.floor))
                / f64::from(config.ceiling - config.floor))
            .clamp(
                nodes[i - 1].x + 0.001,
                (nodes[i + 1].x - 0.001).max(nodes[i - 1].x + 0.001),
            );
            let y = (output / config.gain).clamp(nodes[i - 1].y, nodes[i + 1].y);
            nodes[i] = crate::pressure_profiles::Node { x, y };
        }
        Handle::Zero => config.floor = raw.min(config.ceiling - 1),
        Handle::Full => config.ceiling = raw.clamp(config.floor + 1, 1023),
        Handle::Response => {
            let x = ((f64::from(raw) - f64::from(config.floor))
                / f64::from(config.ceiling - config.floor))
            .clamp(0.001, 0.999);
            let y = (output / config.gain).clamp(0.000001, 0.999999);
            config.gamma = (y.ln() / x.ln()).clamp(0.2, 4.0);
        }
    }
}
