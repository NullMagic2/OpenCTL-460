//! Exports real Engine outputs for a reproducible synthetic curve comparison.
use ctl460_rust::{config::Config, engine::Engine, protocol::Sample};
use std::{fs, io::Write, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Provide an output directory")?,
    );
    fs::create_dir_all(&out)?;
    for shape in ["line", "circle", "s_curve"] {
        let base = Config {
            handwriting_mode: "off".into(),
            pen_control: Some(30.0),
            independent_line_controls: true,
            endpoint_settling: false,
            ..Default::default()
        };
        let settings = [
            ("zero", 0., 0., 0., true),
            ("legacy", 100., 0., 0., false),
            ("half", 50., 0., 0., true),
            ("full", 100., 0., 0., true),
            ("stab50", 0., 50., 0., true),
            ("stab100", 0., 100., 0., true),
            ("motion50", 0., 0., 50., true),
            ("motion100", 0., 0., 100., true),
            ("all50", 50., 50., 50., true),
            ("all100", 100., 100., 100., true),
        ];
        let mut engines: Vec<_> = settings
            .iter()
            .map(|&(_, stream, stab, motion, flowing)| {
                Engine::new(Config {
                    streamline_amount: stream,
                    stabilization_amount: stab,
                    motion_filter_amount: motion,
                    flowing_smoothing: flowing,
                    ..base.clone()
                })
            })
            .collect::<Result<_, _>>()?;
        let mut file = fs::File::create(out.join(format!("{shape}.csv")))?;
        write!(file, "t,raw_x,raw_y")?;
        for (name, ..) in settings {
            write!(file, ",{name}_x,{name}_y")?;
        }
        writeln!(file)?;
        for i in 0..720u16 {
            let a = f64::from(i) * std::f64::consts::TAU / 360.0;
            let n = 60.0 * (f64::from(i) * 1.9).sin();
            let (x, y) = if shape == "line" {
                (1800 + i * 15, (4500.0 + n).round() as u16)
            } else if shape == "circle" {
                (
                    (6000.0 + (1800.0 + n) * a.cos()).round() as u16,
                    (4500.0 + (1800.0 + n) * a.sin()).round() as u16,
                )
            } else {
                (
                    1800 + i * 15,
                    (4500.0 + 900.0 * (f64::from(i) / 100.0).sin() + n).round() as u16,
                )
            };
            let t = f64::from(i) * 8.0;
            write!(file, "{t},{x},{y}")?;
            for e in &mut engines {
                e.push(
                    Sample {
                        x,
                        y,
                        pressure: 500,
                        tip: true,
                        in_range: true,
                        position_valid: true,
                        ..Default::default()
                    },
                    t,
                )?;
                let p = e.tick(t);
                write!(file, ",{},{}", p.x, p.y)?;
            }
            writeln!(file)?;
        }
    }
    Ok(())
}
