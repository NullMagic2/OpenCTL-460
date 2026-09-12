//! Constant precision reduction in hover and contact, with lift-to-reposition clutching.
//! A single scalar scales both axes; there is no distance-dependent acceleration.
use crate::{
    config::Config,
    engine::Frame,
    protocol::{MAX_X, MAX_Y},
};

#[derive(Clone, Copy, Debug)]
pub struct PrecisionBounds {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
}
impl PrecisionBounds {
    pub fn full() -> Self {
        Self {
            min_x: 0.0,
            max_x: f64::from(MAX_X),
            min_y: 0.0,
            max_y: f64::from(MAX_Y),
        }
    }
    fn clamp(self, p: (f64, f64)) -> (f64, f64) {
        (
            p.0.clamp(self.min_x, self.max_x),
            p.1.clamp(self.min_y, self.max_y),
        )
    }
}
#[derive(Clone, Copy)]
struct Anchor {
    source: (f64, f64),
    output: (f64, f64),
    gain: f64,
}
#[derive(Default)]
pub struct PrecisionHold {
    requested: Option<(usize, f64)>,
    previous_buttons: [bool; 2],
    previous_contact: bool,
    anchor: Option<Anchor>,
    previous_input: Option<(f64, f64)>,
    position: Option<(f64, f64)>,
}
impl PrecisionHold {
    /// Apply live settings at a stroke boundary.
    pub fn reconfigure(&mut self, config: &Config) {
        // Live edits are applied after lift. Retain the toggle, but refresh its
        // gain from the activating button; removing that binding disables it.
        self.requested = self.requested.and_then(|(button, _)| {
            let mut selected = [false; 2];
            selected[button] = true;
            config.button_precision(selected).map(|gain| (button, gain))
        });
        self.anchor = None;
        self.previous_input = None;
        self.previous_contact = false;
    }
    pub fn active(&self) -> bool {
        self.anchor.is_some()
    }
    pub fn enabled(&self) -> bool {
        self.requested.is_some()
    }
    /// Button release never toggles; timer repeats and simultaneous presses toggle once.
    pub fn buttons(&mut self, config: &Config, states: [bool; 2]) {
        let rising = std::array::from_fn(|i| states[i] && !self.previous_buttons[i]);
        if let Some(gain) = config.button_precision(rising) {
            self.requested = if self.requested.is_some() {
                None
            } else {
                // Simultaneous presses use the strongest reduction, as before.
                let button = (0..2)
                    .find(|&i| {
                        rising[i] && {
                            let mut selected = [false; 2];
                            selected[i] = true;
                            config.button_precision(selected) == Some(gain)
                        }
                    })
                    .expect("a precision rising edge supplied the gain");
                Some((button, gain))
            };
        }
        self.previous_buttons = states;
    }
    /// Hover and contact share one mapping. Toggle changes during a stroke wait
    /// for lift. Leaving proximity keeps the cursor position and clears the input
    /// anchor, so moving the lifted pen provides full-screen reach without acceleration.
    pub fn apply(&mut self, mut frame: Frame, bounds: PrecisionBounds) -> Frame {
        if !frame.in_range {
            self.anchor = None;
            self.previous_input = None;
            self.previous_contact = false;
            self.previous_buttons = [false; 2];
            return frame;
        }
        let raw = (f64::from(frame.x), f64::from(frame.y));
        if !self.previous_contact || !frame.contact {
            match self.requested {
                Some((_, gain)) if self.anchor.is_none_or(|a| a.gain != gain) => {
                    self.anchor = Some(Anchor {
                        source: self.previous_input.unwrap_or(raw),
                        output: bounds.clamp(self.position.unwrap_or(raw)),
                        gain,
                    });
                }
                None => self.anchor = None,
                _ => (),
            }
        }
        self.previous_contact = frame.contact;
        let mut output = raw;
        if let Some(a) = self.anchor {
            if a.gain != 1.0 {
                let scaled = (
                    a.output.0 + (raw.0 - a.source.0) * a.gain,
                    a.output.1 + (raw.1 - a.source.1) * a.gain,
                );
                output = bounds.clamp(scaled);
                // Rebase at a screen edge so reversing immediately moves inward.
                if output != scaled {
                    self.anchor = Some(Anchor {
                        source: raw,
                        output,
                        gain: a.gain,
                    });
                }
                frame.x = output.0.round().clamp(0.0, f64::from(MAX_X)) as u16;
                frame.y = output.1.round().clamp(0.0, f64::from(MAX_Y)) as u16;
            }
        }
        self.previous_input = Some(raw);
        self.position = Some(output);
        frame
    }
}
/// Tablet-coordinate rectangle that actually maps to visible screen pixels.
/// Aspect-preserving mode crops one tablet dimension; using the raw sensor
/// limits there would create a dead band that feels like an invisible edge.
pub fn output_bounds(width: i32, height: i32, aspect: bool) -> PrecisionBounds {
    if !aspect {
        return PrecisionBounds::full();
    }

    let w = f64::from((width - 1).max(1));
    let h = f64::from((height - 1).max(1));
    let scale = (w / f64::from(MAX_X)).max(h / f64::from(MAX_Y));
    let ox = (w - scale * f64::from(MAX_X)) / 2.0;
    let oy = (h - scale * f64::from(MAX_Y)) / 2.0;
    PrecisionBounds {
        min_x: ((0.0 - ox) / scale).clamp(0.0, f64::from(MAX_X)),
        max_x: ((w - ox) / scale).clamp(0.0, f64::from(MAX_X)),
        min_y: ((0.0 - oy) / scale).clamp(0.0, f64::from(MAX_Y)),
        max_y: ((h - oy) / scale).clamp(0.0, f64::from(MAX_Y)),
    }
}
