//! Offline comparison of synthetic geometry or --trace raw reports. Never opens hardware.
use ctl460_rust::{config::Config, engine::Engine, protocol::Sample};
use std::{
    fs,
    io::{BufWriter, Write},
    path::PathBuf,
};
#[path = "writing_fixtures.rs"]
mod fixtures;

fn captured(path: &std::path::Path) -> Result<(Config, fixtures::Trace), String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 50_000_000 {
        return Err("Trace exceeds 50 MB limit".into());
    }
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    if !text.starts_with("# CTL460 trace v1;") {
        return Err("Expected a raw CTL460 trace v1; old mapped observer recordings cannot be replayed as raw input".into());
    }
    if !text.trim_end().ends_with("# complete; dropped_reports=0") {
        return Err(
            "Trace is still open, incomplete, or lost reports; stop the recording before replay"
                .into(),
        );
    }
    let mut profile = String::new();
    let mut reading = false;
    let mut initial = None;
    let mut trace = fixtures::Trace {
        name: "recorded",
        samples: Vec::new(),
        truth: Vec::new(),
    };
    let mut previous = 0;
    for line in text.lines() {
        if line == "# config-begin" {
            reading = true;
            profile.clear();
            continue;
        }
        if line == "# config-end" {
            let c = Config::parse(&profile)?;
            let serialized = toml::to_string(&c).map_err(|e| e.to_string())?;
            if let Some((_, old)) = &initial {
                if old != &serialized {
                    return Err("Settings changed during recording; record a fixed-settings trace for this comparison".into());
                }
            } else {
                initial = Some((c, serialized));
            }
            reading = false;
            continue;
        }
        if reading {
            profile.push_str(line.strip_prefix("# ").ok_or("Malformed profile marker")?);
            profile.push('\n');
            continue;
        }
        if line.starts_with('#') || line.starts_with("sequence,") || line.is_empty() {
            continue;
        }
        let v: Vec<_> = line.split(',').collect();
        if v.len() != 19 {
            return Err("Incomplete trace row".into());
        }
        let sequence = v[0].parse::<u64>().map_err(|e| e.to_string())?;
        if sequence != previous + 1 {
            return Err("Trace has missing reports; refusing a misleading replay".into());
        }
        previous = sequence;
        let value = |i: usize| v[i].parse::<u16>().map_err(|e| e.to_string());
        let flag = |i: usize| -> Result<bool, String> {
            match v[i] {
                "0" => Ok(false),
                "1" => Ok(true),
                _ => Err("Invalid trace flag".into()),
            }
        };
        let sample = Sample {
            x: value(3)?,
            y: value(4)?,
            pressure: value(5)?,
            in_range: flag(6)?,
            position_valid: flag(7)?,
            tip: flag(8)?,
            barrel: flag(12)?,
            second_button: flag(10)?,
            eraser: flag(13)?,
        };
        trace
            .samples
            .push((v[1].parse::<f64>().map_err(|e| e.to_string())?, sample));
    }
    if reading || trace.samples.is_empty() {
        return Err("Empty or incomplete raw trace".into());
    }
    Ok((initial.ok_or("Trace lacks initial settings")?.0, trace))
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut output = None;
    let mut input = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" => {
                output = Some(PathBuf::from(
                    args.next().ok_or("Missing output directory")?,
                ))
            }
            "--trace" => input = Some(PathBuf::from(args.next().ok_or("Missing raw trace path")?)),
            _ => {
                return Err("Usage: ctl460-writing-replay --out DIRECTORY [--trace RAW.csv]".into())
            }
        }
    }
    let output = output.ok_or("Specify --out DIRECTORY")?;
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let (base, traces) = if let Some(path) = input {
        let (c, t) = captured(&path)?;
        (c, vec![t])
    } else {
        (
            Config {
                handwriting_mode: "on".into(),
                wobble_reduction: 60.0,
                ..Config::default()
            },
            fixtures::fixtures(),
        )
    };
    let mut points =
        BufWriter::new(fs::File::create(output.join("points.csv")).map_err(|e| e.to_string())?);
    writeln!(
        points,
        "case,mode,time_ms,raw_x,raw_y,filtered_x,filtered_y,pressure,contact"
    )
    .map_err(|e| e.to_string())?;
    let mut summary=String::from("case,mode,contact_samples,max_displacement_mm,rms_known_shape_error_mm,mean_x_lag_mm,loop_area_ratio\n");
    for trace in traces {
        for mode in [
            "legacy",
            "raw",
            "minimal",
            "responsive",
            "control30",
            "control65",
            "control100",
        ] {
            let mut c = base.clone();
            c.handwriting_mode = "on".into();
            c.handwriting_filter = if mode.starts_with("control") {
                "minimal"
            } else {
                mode
            }
            .into();
            c.pen_control = mode
                .strip_prefix("control")
                .map(|v| v.parse::<f64>().unwrap());
            c.stroke_smoothing = true;
            let mut engine = Engine::new(c)?;
            let (mut maximum, mut error, mut lag, mut n) = (0.0_f64, 0.0, 0.0, 0);
            let mut loop_points = Vec::new();
            let mut raw_points = Vec::new();
            for (i, (t, s)) in trace.samples.iter().enumerate() {
                engine.push(*s, *t)?;
                let f = engine.tick(*t);
                writeln!(
                    points,
                    "{},{mode},{t:.6},{},{},{},{},{:.9},{}",
                    trace.name,
                    s.x,
                    s.y,
                    f.x,
                    f.y,
                    f.pressure,
                    u8::from(f.contact)
                )
                .map_err(|e| e.to_string())?;
                if !f.contact || !s.position_valid {
                    continue;
                }
                // Compare in the same physical orientation when left-handed mapping is enabled.
                let raw = if base.left_handed {
                    (14720.0 - f64::from(s.x), 9200.0 - f64::from(s.y))
                } else {
                    (f64::from(s.x), f64::from(s.y))
                };
                let actual = (f64::from(f.x), f64::from(f.y));
                maximum = maximum.max((actual.0 - raw.0).hypot(actual.1 - raw.1) / 100.0);
                lag += (raw.0 - actual.0) / 100.0;
                if let Some(truth) = trace.truth.get(i) {
                    error +=
                        ((actual.0 - truth.0).powi(2) + (actual.1 - truth.1).powi(2)) / 10000.0;
                }
                n += 1;
                if i >= 100 {
                    loop_points.push(actual);
                    raw_points.push(raw);
                }
            }
            fn area(points: &[(f64, f64)]) -> f64 {
                points
                    .iter()
                    .zip(points.iter().cycle().skip(1))
                    .take(points.len())
                    .map(|(a, b)| a.0 * b.1 - a.1 * b.0)
                    .sum::<f64>()
                    * 0.5
            }
            let radius = if ["small_circle", "reverse_circle"].contains(&trace.name) {
                format!("{:.6}", area(&loop_points) / area(&raw_points))
            } else {
                String::new()
            };
            let shape = if trace.truth.is_empty() || n == 0 {
                String::new()
            } else {
                format!("{:.6}", (error / n as f64).sqrt())
            };
            let xlag = if trace.name == "straight" && n > 0 {
                format!("{:.6}", lag / n as f64)
            } else {
                String::new()
            };
            summary.push_str(&format!(
                "{},{mode},{n},{maximum:.6},{shape},{xlag},{radius}\n",
                trace.name
            ));
        }
    }
    points.flush().map_err(|e| e.to_string())?;
    fs::write(output.join("metrics.csv"), &summary).map_err(|e| e.to_string())?;
    print!("{summary}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
