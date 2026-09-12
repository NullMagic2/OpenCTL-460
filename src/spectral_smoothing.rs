//! Bounded spatial resampling and zero-phase spectral filtering for live pen input.
//! Uniform distance sampling makes the response independent of drawing pace.
//! Mirrored padding supplies the unavailable side of the current endpoint.
use std::f64::consts::PI;
const N: usize = 512;
const HISTORY: usize = N / 2;
const STEP: f64 = 10.0;
type Point = (f64, f64);

/// In-place radix-2 transform. Real and imaginary parts can carry two coordinates.
pub fn transform(v: &mut [(f64, f64); N], inverse: bool) {
    let mut j = 0;
    for i in 1..N {
        let mut bit = N / 2;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            v.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= N {
        let angle = (if inverse { 2.0 } else { -2.0 }) * PI / len as f64;
        let root = (angle.cos(), angle.sin());
        for start in (0..N).step_by(len) {
            let mut w = (1.0, 0.0);
            for i in 0..len / 2 {
                let a = v[start + i];
                let b = v[start + i + len / 2];
                let b = (b.0 * w.0 - b.1 * w.1, b.0 * w.1 + b.1 * w.0);
                v[start + i] = (a.0 + b.0, a.1 + b.1);
                v[start + i + len / 2] = (a.0 - b.0, a.1 - b.1);
                w = (w.0 * root.0 - w.1 * root.1, w.0 * root.1 + w.1 * root.0);
            }
        }
        len *= 2;
    }
    if inverse {
        for p in v {
            p.0 /= N as f64;
            p.1 /= N as f64;
        }
    }
}

/// Driver-scale calibration. Percentages are normalized before calling.
pub fn cutoff(amount: f64, expression: f64) -> f64 {
    // A logarithmic frequency sweep gives useful control across the whole slider.
    // Expression opens the pass band at intermediate strengths; full strength wins.
    (0.5 * (-4.2 * amount).exp() * (1.0 + 2.0 * expression * (1.0 - amount))).min(0.499)
}

pub struct SpectralSmoothing {
    history: [Point; HISTORY],
    next: usize,
    count: usize,
    previous: Option<Point>,
    remainder: f64,
    response: [f64; N],
    settings: (f64, f64),
}
impl Default for SpectralSmoothing {
    fn default() -> Self {
        Self {
            history: [(0.0, 0.0); HISTORY],
            next: 0,
            count: 0,
            previous: None,
            remainder: 0.0,
            response: [1.0; N],
            settings: (-1.0, -1.0),
        }
    }
}
impl SpectralSmoothing {
    /// Once contact settling reaches its target, discard the old moving history.
    /// Otherwise a resumed stroke could stall against a pre-pause spectral tail.
    pub fn anchor(&mut self, raw: Point) {
        self.next = 0;
        self.count = 0;
        self.remainder = 0.0;
        self.previous = Some(raw);
        self.push(raw);
    }

    fn push(&mut self, p: Point) {
        self.history[self.next] = p;
        self.next = (self.next + 1) % HISTORY;
        self.count = (self.count + 1).min(HISTORY);
    }
    fn past(&self, age: usize) -> Point {
        self.history[(self.next + HISTORY - 1 - age.min(self.count - 1)) % HISTORY]
    }
    fn configure(&mut self, amount: f64, expression: f64) {
        if self.settings == (amount, expression) {
            return;
        }
        self.settings = (amount, expression);
        let frequency = cutoff(amount, expression);
        let bias = 1.0 + 7.0 * expression * (1.0 - amount);
        let half = (2.0 * bias / frequency).ceil().clamp(2.0, 255.0) as usize;
        let mut kernel = [(0.0, 0.0); N];
        for k in 0..=half {
            let x = k as f64;
            let window = 0.35875
                + 0.48829 * (PI * x / half as f64).cos()
                + 0.14128 * (2.0 * PI * x / half as f64).cos()
                + 0.01168 * (3.0 * PI * x / half as f64).cos();
            let value = if k == 0 {
                2.0 * frequency
            } else {
                (2.0 * PI * frequency * x).sin() / (PI * x)
            };
            kernel[k].0 = value * window;
            if k > 0 {
                kernel[N - k].0 = kernel[k].0;
            }
        }
        transform(&mut kernel, false);
        let dc = kernel[0].0.hypot(kernel[0].1).max(1e-12);
        for (gain, k) in self.response.iter_mut().zip(kernel) {
            *gain = (k.0.hypot(k.1) / dc).min(1.0);
        }
        self.response[0] = 1.0;
    }
    pub fn update(&mut self, raw: Point, amount: f64, expression: f64) -> Point {
        self.configure(amount, expression);
        let Some(previous) = self.previous.replace(raw) else {
            self.push(raw);
            return raw;
        };
        let delta = (raw.0 - previous.0, raw.1 - previous.1);
        let distance = delta.0.hypot(delta.1);
        if distance > 0.0 {
            let mut at = STEP - self.remainder;
            // Even a full tablet diagonal is bounded to fewer than 1800 insertions.
            while at <= distance {
                let f = at / distance;
                self.push((previous.0 + f * delta.0, previous.1 + f * delta.1));
                at += STEP;
            }
            self.remainder = (self.remainder + distance) % STEP;
        }
        if amount <= 1e-6 {
            return raw;
        }
        // Shift the fixed spatial history by the fractional last segment, keeping
        // adjacent reports continuous rather than stepping on the resampling grid.
        let blend = self.remainder / STEP;
        let mut signal = [(0.0, 0.0); N];
        signal[0] = raw;
        for age in 1..=HISTORY {
            let a = self.past(age);
            let b = self.past(age - 1);
            let p = (
                a.0 * (1.0 - blend) + b.0 * blend,
                a.1 * (1.0 - blend) + b.1 * blend,
            );
            signal[age] = p;
            if age < HISTORY {
                signal[N - age] = p;
            }
        }
        transform(&mut signal, false);
        for (p, gain) in signal.iter_mut().zip(self.response) {
            p.0 *= gain;
            p.1 *= gain;
        }
        transform(&mut signal, true);
        signal[0]
    }
}
