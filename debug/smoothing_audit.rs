//! Replays synthetic strokes through the real processing engine; no tablet access or injection.
//! CSV separates geometric wobble, coordinate lag and pressure bias so delay alone cannot pass.
use ctl460_rust::{config::Config, engine::Engine, protocol::Sample};
fn main() {
    println!("mode,streamline,motion,stabilization,pressure,wavelength_mm,lateral_rms_units,lag_units,mean_pressure_raw_equivalent");
    for mode in ["off", "on"] {
        for (stream, motion, stab, pressure) in [
            (0., 0., 0., 0.),
            (80., 0., 0., 0.),
            (0., 80., 0., 0.),
            (0., 0., 80., 0.),
            (80., 80., 0., 0.),
            (80., 0., 80., 0.),
            (0., 80., 80., 0.),
            (80., 80., 80., 0.),
            (0., 0., 0., 80.),
        ] {
            for wavelength in [200.0, 1000.0] {
                let mut e = Engine::new(Config {
                    handwriting_mode: mode.into(),
                    streamline_amount: stream,
                    motion_filter_amount: motion,
                    stabilization_amount: stab,
                    streamline_pressure: pressure,
                    ..Config::default()
                })
                .unwrap();
                let (mut yy, mut lag, mut force, mut n) = (0., 0., 0., 0.);
                for i in 0..800u16 {
                    let x = 2000 + i * 12;
                    let y = (4500.0
                        + 60.0 * (f64::from(i * 12) * std::f64::consts::TAU / wavelength).sin())
                    .round() as u16;
                    let raw = if i % 2 == 0 { 488 } else { 512 };
                    let t = f64::from(i) * 8.;
                    e.push(
                        Sample {
                            x,
                            y,
                            pressure: raw,
                            tip: true,
                            in_range: true,
                            position_valid: true,
                            ..Sample::default()
                        },
                        t,
                    )
                    .unwrap();
                    let f = e.tick(t + 4.);
                    if i >= 200 {
                        yy += (f64::from(f.y) - 4500.).powi(2);
                        lag += f64::from(x) - f64::from(f.x);
                        force += f.pressure * 1023.;
                        n += 1.;
                    }
                }
                println!(
                    "{mode},{stream},{motion},{stab},{pressure},{},{:.3},{:.3},{:.3}",
                    wavelength / 100.,
                    (yy / n).sqrt(),
                    lag / n,
                    force / n
                );
            }
        }
    }
}
