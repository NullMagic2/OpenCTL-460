//! Replays identical rotated ovals and shallow diagonals through the complete Engine.
use ctl460_rust::{config::Config, engine::Engine, protocol::Sample};
use std::{fs, io::Write, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let out = PathBuf::from(args.get(1).ok_or("Output directory required")?);
    fs::create_dir_all(&out)?;
    let saved = if let Some(p) = args.get(2) {
        Config::parse(&fs::read_to_string(p)?)?
    } else {
        Config {
            pen_control: Some(30.0),
            streamline_amount: 50.0,
            stabilization_amount: 20.0,
            circle_smoothing: 29.0,
            handwriting_mode: "auto".into(),
            ..Default::default()
        }
    };
    for mode in ["saved", "motion"] {
        for shape in ["oval", "line"] {
            let mut file = fs::File::create(out.join(format!("{mode}_{shape}.csv")))?;
            writeln!(
                file,
                "angle,i,ideal_x,ideal_y,raw_x,raw_y,out_x,out_y,pressure,contact"
            )?;
            for degrees in [0.0f64, 15.0, 30.0, 45.0, 60.0, 90.0] {
                let angle = degrees.to_radians();
                let mut c = saved.clone();
                if mode == "motion" {
                    c.streamline_amount = 0.0;
                    c.stabilization_amount = 0.0;
                    c.motion_filter_amount = 75.0;
                    c.circle_smoothing = 0.0;
                }
                let mut e = Engine::new(c)?;
                for i in 0..720 {
                    let t = i as f64 * std::f64::consts::TAU / 360.0;
                    let (x, y) = if shape == "oval" {
                        (2300.0 * t.cos(), 650.0 * t.sin())
                    } else {
                        (-2600.0 + i as f64 * 7.0, 150.0 * (i as f64 / 100.0).sin())
                    };
                    let noise = 8.0 * (i as f64 * 1.9).sin();
                    let rot = |x: f64, y: f64| {
                        (
                            7300.0 + x * angle.cos() - y * angle.sin(),
                            4600.0 + x * angle.sin() + y * angle.cos(),
                        )
                    };
                    let ideal = rot(x, y);
                    let raw = rot(x, y + noise);
                    let time = i as f64 * 8.0;
                    e.push(
                        Sample {
                            x: raw.0.round() as u16,
                            y: raw.1.round() as u16,
                            pressure: 500,
                            tip: true,
                            in_range: true,
                            position_valid: true,
                            ..Default::default()
                        },
                        time,
                    )?;
                    let p = e.tick(time);
                    writeln!(
                        file,
                        "{degrees},{i},{},{},{},{},{},{},{},{}",
                        ideal.0,
                        ideal.1,
                        raw.0.round(),
                        raw.1.round(),
                        p.x,
                        p.y,
                        p.pressure,
                        p.contact
                    )?;
                }
            }
        }
    }
    Ok(())
}
