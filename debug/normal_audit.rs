//! Offline stage-by-stage oval geometry audit; never opens a device or injects input.
use ctl460_rust::{config::Config, engine::Engine, protocol::Sample};
use std::f64::consts::TAU;
fn main() {
    println!(
        "stage,steps,start,clockwise,mean_dx,mean_dy,center_dx,center_dy,width_ratio,height_ratio"
    );
    for stage in 0..5 {
        for steps in [64, 128, 360] {
            for start in [0.0, 0.25, 0.5, 0.75] {
                for direction in [-1.0, 1.0] {
                    let c = Config {
                        flowing_smoothing: true,
                        independent_line_controls: true,
                        endpoint_settling: true,
                        pen_control: Some(if stage > 0 { 30.0 } else { 0.0 }),
                        streamline_amount: if stage > 1 { 30.0 } else { 0.0 },
                        stabilization_amount: if stage > 2 { 20.0 } else { 0.0 },
                        circle_smoothing: if stage > 3 { 29.0 } else { 0.0 },
                        handwriting_mode: "off".into(),
                        ..Default::default()
                    };
                    let mut e = Engine::new(c).unwrap();
                    let mut sum = (0.0, 0.0);
                    let mut bounds = [
                        f64::INFINITY,
                        f64::INFINITY,
                        f64::NEG_INFINITY,
                        f64::NEG_INFINITY,
                    ];
                    for i in 0..=steps {
                        let a = TAU * (start + direction * f64::from(i) / f64::from(steps));
                        let x = (7300.0 + 800.0 * a.cos()).round() as u16;
                        let y = (4600.0 + 1500.0 * a.sin()).round() as u16;
                        e.push(
                            Sample {
                                x,
                                y,
                                pressure: 400,
                                tip: true,
                                in_range: true,
                                position_valid: true,
                                ..Default::default()
                            },
                            f64::from(i) * 8.0,
                        )
                        .unwrap();
                        let f = e.tick(f64::from(i) * 8.0);
                        sum.0 += f64::from(f.x) - f64::from(x);
                        sum.1 += f64::from(f.y) - f64::from(y);
                        bounds[0] = bounds[0].min(f64::from(f.x));
                        bounds[1] = bounds[1].min(f64::from(f.y));
                        bounds[2] = bounds[2].max(f64::from(f.x));
                        bounds[3] = bounds[3].max(f64::from(f.y));
                    }
                    println!(
                        "{stage},{steps},{start},{direction},{:.3},{:.3},{:.3},{:.3},{:.5},{:.5}",
                        sum.0 / f64::from(steps + 1),
                        sum.1 / f64::from(steps + 1),
                        (bounds[0] + bounds[2]) / 2.0 - 7300.0,
                        (bounds[1] + bounds[3]) / 2.0 - 4600.0,
                        (bounds[2] - bounds[0]) / 1600.0,
                        (bounds[3] - bounds[1]) / 3000.0
                    );
                }
            }
        }
    }
}
