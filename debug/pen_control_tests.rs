use ctl460_rust::{
    config::Config, engine::Engine, pen_control::PenControl, protocol::Sample,
    writing_filter::WritingFilter,
};
#[path = "writing_fixtures.rs"]
mod fixtures;
fn config(amount: f64) -> Config {
    Config {
        pen_control: Some(amount),
        handwriting_mode: "on".into(),
        ..Config::default()
    }
}

#[test]
fn light_setting_matches_the_approved_filter_sample_for_sample() {
    for trace in fixtures::fixtures() {
        let mut old = WritingFilter::default();
        let mut new = PenControl::default();
        let mut last = None;
        for (t, s) in trace.samples {
            let dt = last.map_or(4.0, |old| t - old);
            last = Some(t);
            assert_eq!(
                new.update(s.x, s.y, dt, 30.0),
                old.update(s.x, s.y, dt, 60.0, false),
                "{}",
                trace.name
            );
        }
    }
}

#[test]
fn steadier_settings_reduce_noisy_curve_error_without_collapsing_clean_loops() {
    for trace in fixtures::fixtures() {
        if ![
            "noisy_circle",
            "noisy_ellipse",
            "noisy_straight",
            "stationary",
            "small_circle",
            "reverse_circle",
        ]
        .contains(&trace.name)
        {
            continue;
        }
        let mut errors = Vec::new();
        for amount in [30.0, 65.0, 100.0] {
            let mut engine = Engine::new(config(amount)).unwrap();
            let (mut squared, mut count) = (0.0, 0);
            for (i, ((t, s), truth)) in trace.samples.iter().zip(&trace.truth).enumerate() {
                engine.push(*s, *t).unwrap();
                let f = engine.tick(*t);
                if i < 100 {
                    continue;
                }
                squared += if trace.name == "noisy_straight" {
                    // Lateral noise, measured separately from longitudinal lag.
                    (f64::from(f.y) - truth.1).powi(2)
                } else {
                    (f64::from(f.x) - truth.0).powi(2) + (f64::from(f.y) - truth.1).powi(2)
                };
                count += 1;
                if ["small_circle", "reverse_circle"].contains(&trace.name) {
                    assert!((f64::from(f.x) - 5000.0).hypot(f64::from(f.y) - 4000.0) >= 37.0);
                }
            }
            errors.push((squared / f64::from(count)).sqrt() / 100.0);
        }
        println!(
            "{}: Light {:.5}, 65 {:.5}, 100 {:.5} mm RMS",
            trace.name, errors[0], errors[1], errors[2]
        );
        if trace.name.starts_with("noisy") || trace.name == "stationary" {
            // Ellipse fixture noise is radial, so it includes an along-stroke
            // component that this sideways filter intentionally preserves.
            let ratio = if trace.name == "noisy_ellipse" {
                0.95
            } else {
                0.90
            };
            assert!(
                errors[1] < errors[0] * ratio,
                "{} needs useful noise reduction: {errors:?}",
                trace.name
            );
            assert!(errors[2] < errors[0] * ratio, "{}: {errors:?}", trace.name);
        }
    }
}

#[test]
fn control_bounds_the_whole_pipeline_and_leaves_pressure_independent() {
    let mut c = config(100.0);
    c.streamline_amount = 100.0;
    c.stabilization_amount = 100.0;
    c.motion_filter_amount = 100.0;
    let mut strong = Engine::new(c.clone()).unwrap();
    c.pen_control = Some(0.0);
    let mut direct = Engine::new(c).unwrap();
    for (t, s) in fixtures::fixtures().remove(6).samples {
        strong.push(s, t).unwrap();
        direct.push(s, t).unwrap();
        let a = strong.tick(t + 1.0);
        let b = direct.tick(t + 1.0);
        assert_eq!(a.pressure, b.pressure);
        assert_eq!(a.contact, b.contact);
        assert_eq!((b.x, b.y), (s.x, s.y));
        assert!((f64::from(a.x) - f64::from(s.x)).hypot(f64::from(a.y) - f64::from(s.y)) <= 10.71);
    }
}

#[test]
fn new_strokes_and_tool_changes_start_exactly_at_measured_position() {
    for amount in [0.0, 30.0, 65.0, 100.0] {
        let mut e = Engine::new(config(amount)).unwrap();
        e.push(
            Sample {
                tip: false,
                pressure: 0,
                ..fixtures::pen(1000.0, 1000.0)
            },
            0.0,
        )
        .unwrap();
        e.push(fixtures::pen(2000.0, 3000.0), 8.0).unwrap();
        assert_eq!((e.tick(8.0).x, e.tick(8.0).y), (2000, 3000));
        assert!(!e.reconfigure(config(50.0)).unwrap());
        e.push(
            Sample {
                tip: false,
                pressure: 0,
                ..fixtures::pen(2100.0, 3100.0)
            },
            16.0,
        )
        .unwrap();
        assert_eq!(e.tick(16.0).pressure, 0.0);
        assert!(e.reconfigure(config(amount)).unwrap());
        e.push(fixtures::pen(5000.0, 4000.0), 24.0).unwrap();
        assert_eq!((e.tick(24.0).x, e.tick(24.0).y), (5000, 4000));
        e.push(
            Sample {
                eraser: true,
                ..fixtures::pen(5100.0, 4500.0)
            },
            32.0,
        )
        .unwrap();
        assert_eq!((e.tick(32.0).x, e.tick(32.0).y), (5100, 4500));
        assert!(!e.tick(132.0).contact);
        e.push(fixtures::pen(6000.0, 5000.0), 140.0).unwrap();
        assert_eq!((e.tick(140.0).x, e.tick(140.0).y), (6000, 5000));
    }
}

#[test]
fn config_keeps_old_profiles_and_accepts_one_valid_control() {
    assert_eq!(Config::parse("").unwrap().pen_control, None);
    assert_eq!(Config::simple_default().pen_control, Some(30.0));
    for invalid in [
        "pen_control=-1",
        "pen_control=101",
        "pen_control=nan",
        "pen_control=inf",
    ] {
        assert!(Config::parse(invalid).is_err());
    }
    let c = config(65.0);
    assert_eq!(
        Config::parse(&toml::to_string(&c).unwrap())
            .unwrap()
            .pen_control,
        Some(65.0)
    );
    assert_eq!(c.drawing_profile(Config::default()).pen_control, Some(65.0));
}

#[test]
fn direction_changes_are_not_held_by_the_steady_filter() {
    for trace in fixtures::fixtures() {
        if !["corner", "reversal"].contains(&trace.name) {
            continue;
        }
        let mut e = Engine::new(config(100.0)).unwrap();
        for (i, (t, s)) in trace.samples.into_iter().enumerate() {
            e.push(s, t).unwrap();
            let f = e.tick(t);
            if i == 200 || i == 201 {
                let distance =
                    (f64::from(f.x) - f64::from(s.x)).hypot(f64::from(f.y) - f64::from(s.y));
                assert!(distance <= 10.71, "{}: {distance}", trace.name);
            }
        }
    }
}

#[test]
fn upgrade_preserves_custom_profiles_and_all_pressure_mapping_fields() {
    let mut old = Config {
        gamma: 1.4,
        left_handed: true,
        wobble_reduction: 60.0,
        button1: "Undo (Ctrl+Z)".into(),
        ..Config::default()
    };
    old.upgrade_pen_control();
    assert_eq!(old.pen_control, Some(30.0));
    assert_eq!(old.gamma, 1.4);
    assert!(old.left_handed);
    assert_eq!(old.button1, "Undo (Ctrl+Z)");
    for mut c in [
        Config {
            keep_advanced_smoothing: true,
            ..Config::default()
        },
        Config {
            streamline_amount: 20.0,
            ..Config::default()
        },
        Config {
            stroke_smoothing: false,
            ..Config::default()
        },
        Config {
            handwriting_mode: "auto".into(),
            ..Config::default()
        },
        Config {
            handwriting_filter: "raw".into(),
            ..Config::default()
        },
        config(65.0),
    ] {
        let before = toml::to_string(&c).unwrap();
        c.upgrade_pen_control();
        assert_eq!(toml::to_string(&c).unwrap(), before);
    }
}

#[test]
fn rotation_and_sampling_rates_preserve_bounds_and_clean_loop_sizes() {
    for dt in [4.0, 8.0, 12.0] {
        for radius in [25.0, 60.0, 300.0] {
            for amount in [30.0, 65.0, 100.0] {
                let mut a = PenControl::default();
                let mut b = PenControl::default();
                for i in 0..500 {
                    let angle = i as f64 * dt * std::f64::consts::TAU / 1000.0;
                    let x = (5000.0 + radius * angle.cos()).round() as u16;
                    let y = (4000.0 + radius * angle.sin()).round() as u16;
                    let p = a.update(x, y, dt, amount);
                    let q = b.update(9000 - y, x, dt, amount);
                    assert!((i32::from(p.0) - i32::from(q.1)).abs() <= 1);
                    assert!((i32::from(p.1) + i32::from(q.0) - 9000).abs() <= 1);
                    if amount > 30.0 {
                        assert!(
                            (f64::from(p.0) - f64::from(x)).hypot(f64::from(p.1) - f64::from(y))
                                <= 10.71
                        );
                    }
                    let actual = (f64::from(p.0) - 5000.0).hypot(f64::from(p.1) - 4000.0);
                    assert!(
                        (actual - radius).abs() <= radius * 0.06 + 1.0,
                        "dt={dt}, radius={radius}, actual={actual}"
                    );
                }
            }
        }
    }
}
