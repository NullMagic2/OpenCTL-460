use ctl460_rust::{config::Config, engine::Engine, pen_control::CircleControl};
#[path = "writing_fixtures.rs"]
mod fixtures;

fn baseline() -> Config {
    let mut c = Config::simple_default();
    c.restore_line_controls();
    c
}

#[test]
fn circles_reduce_noise_on_light_without_changing_pressure_or_contact() {
    for name in ["noisy_circle", "noisy_ellipse"] {
        let trace = fixtures::fixtures()
            .into_iter()
            .find(|t| t.name == name)
            .unwrap();
        let mut plain = Engine::new(baseline()).unwrap();
        let mut curved = Engine::new(Config {
            circle_smoothing: 50.0,
            ..baseline()
        })
        .unwrap();
        let (mut a_error, mut b_error) = (0.0, 0.0);
        for ((t, s), truth) in trace.samples.into_iter().zip(trace.truth) {
            plain.push(s, t).unwrap();
            curved.push(s, t).unwrap();
            let a = plain.tick(t);
            let b = curved.tick(t);
            assert_eq!(a.pressure, b.pressure);
            assert_eq!(a.contact, b.contact);
            assert!(
                (f64::from(a.x) - f64::from(b.x)).hypot(f64::from(a.y) - f64::from(b.y)) <= 12.71
            );
            a_error += (f64::from(a.x) - truth.0).powi(2) + (f64::from(a.y) - truth.1).powi(2);
            b_error += (f64::from(b.x) - truth.0).powi(2) + (f64::from(b.y) - truth.1).powi(2);
        }
        assert!(b_error < a_error * 0.85, "{name}: {a_error} -> {b_error}");
    }
}

#[test]
fn individual_sliders_remain_effective_with_saved_feel_and_circle_assistance() {
    for which in 0..3 {
        let c = Config {
            circle_smoothing: 50.0,
            ..baseline()
        };
        let mut altered = c.clone();
        match which {
            0 => altered.streamline_amount = 70.0,
            1 => altered.stabilization_amount = 70.0,
            _ => altered.motion_filter_amount = 70.0,
        }
        let mut plain = Engine::new(c).unwrap();
        let mut changed = Engine::new(altered).unwrap();
        let mut differences = 0;
        for (t, s) in fixtures::fixtures()
            .into_iter()
            .find(|t| t.name == "figure_eight")
            .unwrap()
            .samples
        {
            plain.push(s, t).unwrap();
            changed.push(s, t).unwrap();
            let a = plain.tick(t);
            let b = changed.tick(t);
            differences += usize::from((a.x, a.y) != (b.x, b.y));
            assert_eq!(a.pressure, b.pressure);
        }
        assert!(differences > 100, "slider {which} was bypassed");
    }
}

#[test]
fn curve_off_is_exactly_neutral_and_correction_resets() {
    let mut c = CircleControl::default();
    for (t, s) in fixtures::fixtures().remove(0).samples {
        assert_eq!(c.correction(s.x, s.y, t, 0.0), (0, 0));
    }
    for dt in [4.0, 8.0, 12.0] {
        let mut c = CircleControl::default();
        for (i, (_, s)) in fixtures::fixtures()
            .remove(6)
            .samples
            .into_iter()
            .enumerate()
        {
            let (dx, dy) = c.correction(s.x, s.y, dt, 100.0);
            assert!(f64::from(dx).hypot(f64::from(dy)) <= 12.71);
            if i == 0 {
                assert_eq!((dx, dy), (0, 0));
            }
        }
        c.reset();
        assert_eq!(c.correction(14000, 9000, dt, 100.0), (0, 0));
        assert_eq!(c.correction(100, 100, 101.0, 100.0), (0, 0));
    }
}

#[test]
fn migration_preserves_settings_and_separates_curve_strength_once() {
    let mut c = Config {
        pen_control: Some(89.0),
        streamline_amount: 43.0,
        gamma: 1.7,
        preserve_aspect: false,
        button1: "None".into(),
        ..Config::default()
    };
    c.restore_line_controls();
    assert_eq!(c.pen_control, Some(30.0));
    assert!((c.circle_smoothing - 100.0 * 59.0 / 70.0).abs() < 1e-9);
    assert_eq!(c.streamline_amount, 43.0);
    assert_eq!(c.gamma, 1.7);
    assert!(!c.preserve_aspect);
    assert_eq!(c.button1, "None");
    assert!(c.independent_line_controls);
    let serialized = toml::to_string(&c).unwrap();
    c.restore_line_controls();
    assert_eq!(serialized, toml::to_string(&c).unwrap());
    let parsed = Config::parse(&serialized).unwrap();
    let preset = parsed.drawing_profile(Config::default());
    assert_eq!(preset.circle_smoothing, c.circle_smoothing);
    assert!(preset.independent_line_controls);
    for text in [
        "circle_smoothing=-1",
        "circle_smoothing=101",
        "circle_smoothing=nan",
        "circle_smoothing=inf",
    ] {
        assert!(Config::parse(text).is_err());
    }
    assert_eq!(Config::parse("").unwrap().circle_smoothing, 0.0);
}

#[test]
fn lift_timeout_and_tool_change_do_not_leave_a_curve_offset() {
    let mut e = Engine::new(Config {
        circle_smoothing: 100.0,
        ..baseline()
    })
    .unwrap();
    for (t, s) in fixtures::fixtures().remove(6).samples.into_iter().take(100) {
        e.push(s, t).unwrap();
        e.tick(t);
    }
    assert!(!e.tick(10000.0).contact);
    e.push(fixtures::pen(500.0, 600.0), 10004.0).unwrap();
    let f = e.tick(10004.0);
    assert_eq!((f.x, f.y), (500, 600));
    let mut s = fixtures::pen(1000.0, 1100.0);
    s.eraser = true;
    e.push(s, 10008.0).unwrap();
    let f = e.tick(10008.0);
    assert_eq!((f.x, f.y), (1000, 1100));
    s.tip = false;
    s.pressure = 0;
    e.push(s, 10012.0).unwrap();
    assert_eq!(e.tick(10012.0).pressure, 0.0);
    assert!(e.reconfigure(baseline()).unwrap());
    e.push(fixtures::pen(2000.0, 2100.0), 10016.0).unwrap();
    let f = e.tick(10016.0);
    assert_eq!((f.x, f.y), (2000, 2100));
}
