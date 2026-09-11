use ctl460_rust::protocol::Sample;
pub fn pen(x: f64, y: f64) -> Sample {
    Sample {
        x: x.round() as u16,
        y: y.round() as u16,
        pressure: 400,
        tip: true,
        in_range: true,
        position_valid: true,
        ..Sample::default()
    }
}
pub struct Trace {
    pub name: &'static str,
    pub samples: Vec<(f64, Sample)>,
    pub truth: Vec<(f64, f64)>,
}
pub fn fixtures() -> Vec<Trace> {
    let mut all = Vec::new();
    for name in [
        "stationary",
        "straight",
        "small_circle",
        "reverse_circle",
        "corner",
        "reversal",
        "figure_eight",
        "slow_turn",
        "noisy_circle",
        "noisy_ellipse",
        "noisy_straight",
    ] {
        let mut trace = Trace {
            name,
            samples: Vec::new(),
            truth: Vec::new(),
        };
        let mut t = 0.0;
        for i in 0..400 {
            let f = i as f64;
            let a = f * std::f64::consts::TAU / 100.0;
            let (x, y) = match name {
                "stationary" => (5000.0, 4000.0),
                "straight" => (3000.0 + f * 5.0, 4000.0),
                "small_circle" => (5000.0 + 40.0 * a.cos(), 4000.0 + 40.0 * a.sin()),
                "reverse_circle" => (5000.0 + 40.0 * a.cos(), 4000.0 - 40.0 * a.sin()),
                "corner" => {
                    if i < 200 {
                        (3000.0 + f * 5.0, 4000.0)
                    } else {
                        (3995.0, 4000.0 + (f - 199.0) * 5.0)
                    }
                }
                "reversal" => (
                    3000.0 + (if i < 200 { f } else { 398.0 - f }) * 15.0,
                    4000.0,
                ),
                "figure_eight" => (5000.0 + 70.0 * a.sin(), 4000.0 + 40.0 * (2.0 * a).sin()),
                "noisy_circle" => (5000.0 + 60.0 * a.cos(), 4000.0 + 60.0 * a.sin()),
                "noisy_ellipse" => (5000.0 + 100.0 * a.cos(), 4000.0 + 40.0 * a.sin()),
                "noisy_straight" => (3000.0 + f * 5.0, 4000.0),
                _ => {
                    let a = f * std::f64::consts::TAU / 200.0;
                    (5000.0 + 40.0 * a.cos(), 4000.0 + 40.0 * a.sin())
                }
            };
            let noise = if name == "stationary" {
                if i % 2 == 0 {
                    5.0
                } else {
                    -5.0
                }
            } else {
                0.0
            };
            let alternating = if i % 2 == 0 { 4.0 } else { -4.0 };
            let noisy = match name {
                "noisy_circle" | "noisy_ellipse" => {
                    (x + alternating * a.cos(), y + alternating * a.sin())
                }
                "noisy_straight" => (x, y + alternating),
                _ => (x + noise, y),
            };
            trace.samples.push((t, pen(noisy.0, noisy.1)));
            trace.truth.push((x, y));
            t += [6.0, 8.0, 10.0, 8.0][i % 4];
        }
        all.push(trace);
    }
    all
}
