use ctl460_rust::{
    config::Config, engine::Engine, protocol::Sample, writing_filter::WritingFilter,
};
#[path = "writing_fixtures.rs"]
mod fixtures;
use fixtures::pen;
fn config(mode: &str) -> Config {
    Config {
        handwriting_mode: "on".into(),
        handwriting_filter: mode.into(),
        wobble_reduction: 60.0,
        ..Config::default()
    }
}

#[test]
fn total_displacement_is_bounded_through_the_real_engine() {
    for trace in fixtures::fixtures() {
        let mut c = config("responsive");
        c.flowing_smoothing = false;
        c.endpoint_settling = false;
        c.streamline_amount = 100.0;
        c.stabilization_amount = 100.0;
        c.motion_filter_amount = 100.0;
        let mut e = Engine::new(c).unwrap();
        for (t, s) in trace.samples {
            e.push(s, t).unwrap();
            let f = e.tick(t);
            let error = (f64::from(f.x) - f64::from(s.x)).hypot(f64::from(f.y) - f64::from(s.y));
            assert!(error <= 7.91, "{}: {error}", trace.name);
        }
    }
}

#[test]
fn hover_lift_timeout_tool_flip_and_mode_change_do_not_drag_new_strokes() {
    for mode in ["raw", "minimal", "responsive"] {
        let mut e = Engine::new(config(mode)).unwrap();
        for i in 0..20 {
            e.push(
                Sample {
                    tip: false,
                    pressure: 0,
                    ..pen(1000.0 + i as f64, 1000.0)
                },
                i as f64 * 8.0,
            )
            .unwrap();
        }
        e.push(pen(1100.0, 1100.0), 160.0).unwrap();
        assert_eq!((e.tick(160.0).x, e.tick(160.0).y), (1100, 1100));
        assert!(!e.reconfigure(config("legacy")).unwrap());
        e.push(
            Sample {
                tip: false,
                pressure: 0,
                ..pen(1110.0, 1100.0)
            },
            168.0,
        )
        .unwrap();
        assert!(!e.tick(168.0).contact);
        assert_eq!(e.tick(168.0).pressure, 0.0);
        e.push(pen(1300.0, 1200.0), 176.0).unwrap();
        assert_eq!((e.tick(176.0).x, e.tick(176.0).y), (1300, 1200));
        assert!(!e.tick(276.0).contact);
        e.push(pen(5000.0, 3000.0), 280.0).unwrap();
        assert_eq!((e.tick(280.0).x, e.tick(280.0).y), (5000, 3000));
        e.push(
            Sample {
                eraser: true,
                ..pen(5100.0, 3100.0)
            },
            288.0,
        )
        .unwrap();
        assert_eq!((e.tick(288.0).x, e.tick(288.0).y), (5100, 3100));
    }
}

#[test]
fn raw_mode_is_exact_and_pressure_is_identical_between_position_modes() {
    let mut engines: Vec<_> = ["legacy", "raw", "minimal", "responsive"]
        .map(|m| Engine::new(config(m)).unwrap())
        .into();
    for i in 0..200 {
        let s = Sample {
            pressure: if i % 13 == 0 { 0 } else { 100 + i },
            tip: i % 13 != 0,
            ..pen(4000.0 + f64::from(i) * 7.0, 3000.0 + f64::from(i % 9))
        };
        let t = f64::from(i) * 8.0;
        let frames: Vec<_> = engines
            .iter_mut()
            .map(|e| {
                e.push(s, t).unwrap();
                e.tick(t + 2.0)
            })
            .collect();
        for f in &frames {
            assert_eq!(f.pressure, frames[0].pressure);
            assert_eq!(f.contact, frames[0].contact);
        }
        if s.tip {
            assert_eq!((frames[1].x, frames[1].y), (s.x, s.y));
        }
    }
}

#[test]
fn prototype_reduces_small_stationary_noise_and_preserves_small_loops() {
    for trace in fixtures::fixtures() {
        if !["stationary", "small_circle", "reverse_circle"].contains(&trace.name) {
            continue;
        }
        let mut e = Engine::new(config("responsive")).unwrap();
        let mut error = 0.0;
        let mut n = 0.0;
        for ((t, s), truth) in trace.samples.into_iter().zip(trace.truth) {
            e.push(s, t).unwrap();
            let f = e.tick(t);
            if t < 160.0 {
                continue;
            }
            error += (f64::from(f.x) - truth.0).powi(2) + (f64::from(f.y) - truth.1).powi(2);
            n += 1.0;
            if trace.name != "stationary" {
                let radius = (f64::from(f.x) - 5000.0).hypot(f64::from(f.y) - 4000.0);
                assert!(radius >= 37.0, "loop radius {radius}");
            }
        }
        if trace.name == "stationary" {
            assert!((error / n).sqrt() < 3.0);
        }
    }
}

#[test]
fn invalid_settings_zero_strength_and_clock_discontinuities_are_handled() {
    assert_eq!(Config::parse("").unwrap().handwriting_filter, "legacy");
    assert!(Config::parse("handwriting_filter='guess'").is_err());
    let mut f = WritingFilter::default();
    for i in 0..50 {
        assert_eq!(
            f.update(1000 + i * 10, 2000, 8.0, 0.0, true),
            (1000 + i * 10, 2000)
        );
    }
    for dt in [0.0, 1000.0, f64::NAN] {
        assert_eq!(f.update(7000, 3000, dt, 60.0, true), (7000, 3000));
    }
}

#[test]
fn trace_preserves_raw_and_effective_reports_without_overwriting_files() {
    use ctl460_rust::trace::Trace;
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("debug/generated");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!(
        "trace-test-{}-{}.csv",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let c = config("responsive");
    let mut trace = Trace::start(&path, &c).unwrap();
    assert!(Trace::start(&path, &c).is_err());
    let raw = pen(1000.0, 2000.0);
    let effective = Sample {
        eraser: true,
        ..raw
    };
    let mut engine = Engine::new(c).unwrap();
    engine.push(effective, 8.0).unwrap();
    trace.record(raw, effective, 8.0, 8.1, engine.tick(8.0));
    trace.reconfigure(&config("raw"));
    drop(trace);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.trim_end().ends_with("# complete; dropped_reports=0"));
    assert!(text.contains("1,8.000000,8.100000,1000,2000,400,1,1,1,0,0,0,0,1,"));
    assert!(text.contains("handwriting_filter = \"raw\""));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn recorded_samples_replay_and_incomplete_or_missing_reports_are_rejected() {
    use ctl460_rust::trace::Trace;
    let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("debug/generated")
        .join(format!(
            "writing-replay-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("raw.csv");
    let c = config("legacy");
    let mut engine = Engine::new(c.clone()).unwrap();
    let mut trace = Trace::start(&path, &c).unwrap();
    for (t, s) in fixtures::fixtures().remove(2).samples {
        engine.push(s, t).unwrap();
        trace.record(s, s, t, t + 0.1, engine.tick(t));
    }
    drop(trace);
    let run = |input: &std::path::Path| {
        std::process::Command::new(env!("CARGO_BIN_EXE_ctl460-writing-replay"))
            .arg("--trace")
            .arg(input)
            .arg("--out")
            .arg(directory.join("replay"))
            .output()
            .unwrap()
    };
    let result = run(&path);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let metrics = std::fs::read_to_string(directory.join("replay/metrics.csv")).unwrap();
    assert!(metrics.contains("recorded,responsive,400,"));
    let original = std::fs::read_to_string(&path).unwrap();
    let corrupt = directory.join("corrupt.csv");
    std::fs::write(
        &corrupt,
        original.replace("# complete; dropped_reports=0", ""),
    )
    .unwrap();
    assert!(!run(&corrupt).status.success());
    std::fs::write(&corrupt, original.replace("\n2,6.000000", "\n3,6.000000")).unwrap();
    assert!(!run(&corrupt).status.success());
    std::fs::write(&corrupt, "elapsed_ms,owner,sequence,time,x,y,pressure").unwrap();
    assert!(!run(&corrupt).status.success());
}

#[test]
fn drawing_settings_and_left_handed_mapping_remain_consistent() {
    let mut c = config("legacy");
    c.handwriting_mode = "off".into();
    let mut original = Engine::new(c.clone()).unwrap();
    c.handwriting_filter = "responsive".into();
    let mut drawing = Engine::new(c).unwrap();
    let mut left = config("responsive");
    left.left_handed = true;
    let mut rotated = Engine::new(left).unwrap();
    let mut writing = Engine::new(config("responsive")).unwrap();
    for (t, s) in fixtures::fixtures().remove(6).samples {
        original.push(s, t).unwrap();
        drawing.push(s, t).unwrap();
        assert_eq!(original.tick(t), drawing.tick(t));
        rotated.push(s, t).unwrap();
        writing.push(s, t).unwrap();
        let a = writing.tick(t);
        let b = rotated.tick(t);
        assert!((i32::from(a.x) + i32::from(b.x) - 14720).abs() <= 1);
        assert!((i32::from(a.y) + i32::from(b.y) - 9200).abs() <= 1);
    }
}
