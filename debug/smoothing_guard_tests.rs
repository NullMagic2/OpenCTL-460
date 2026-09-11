//! End-to-end checks for combined displacement, geometry, pressure and stroke isolation.
use ctl460_rust::{config::Config, engine::Engine, protocol::Sample};
fn sample(x: u16, y: u16, p: u16) -> Sample {
    Sample {
        x,
        y,
        pressure: p,
        tip: p > 0,
        in_range: true,
        position_valid: true,
        ..Default::default()
    }
}
fn strong() -> Config {
    Config {
        streamline_amount: 100.0,
        stabilization_amount: 100.0,
        motion_filter_amount: 100.0,
        circle_smoothing: 100.0,
        independent_line_controls: true,
        ..Config::default()
    }
}
#[test]
fn stacked_filters_obey_one_euclidean_limit_and_leave_pressure_identical() {
    for mode in ["off", "on", "auto"] {
        for dt in [2.0, 8.0, 16.0] {
            let c = Config {
                handwriting_mode: mode.into(),
                max_smoothing_distance_mm: 0.4,
                ..strong()
            };
            let mut guarded = Engine::new(c.clone()).unwrap();
            let mut original = Engine::new(Config {
                max_smoothing_distance_mm: 0.0,
                ..c
            })
            .unwrap();
            for i in 0..300 {
                let angle = i as f64 * 0.07;
                let x = (5000.0 + 600.0 * angle.cos()) as u16;
                let y = (4000.0 + 400.0 * angle.sin()) as u16;
                let p = sample(x, y, 200 + (i % 100) * 5);
                let time = i as f64 * dt;
                guarded.push(p, time).unwrap();
                original.push(p, time).unwrap();
                let a = guarded.tick(time);
                let b = original.tick(time);
                assert!(
                    (f64::from(a.x) - f64::from(x)).hypot(f64::from(a.y) - f64::from(y)) <= 40.71
                );
                assert_eq!(a.pressure, b.pressure);
                assert_eq!(a.contact, b.contact);
            }
        }
    }
}
#[test]
fn deliberate_corner_discards_old_path_without_ending_contact() {
    let mut a = Engine::new(strong()).unwrap();
    let mut b = Engine::new(Config {
        preserve_corners: false,
        ..strong()
    })
    .unwrap();
    for i in 0..30 {
        for e in [&mut a, &mut b] {
            e.push(sample(2000 + i * 20, 3000, 500), i as f64 * 8.0)
                .unwrap();
        }
    }
    for e in [&mut a, &mut b] {
        e.push(sample(2580, 3020, 500), 240.0).unwrap();
    }
    let p = a.tick(240.0);
    let q = b.tick(240.0);
    assert!(p.contact);
    assert_eq!((p.x, p.y), (2580, 3020));
    assert_ne!((p.x, p.y), (q.x, q.y));
    assert_eq!(p.pressure, q.pressure);
}
#[test]
fn protected_small_loop_retains_more_radius_than_unprotected_stack() {
    let measure = |enabled| {
        let mut e = Engine::new(Config {
            preserve_corners: enabled,
            ..strong()
        })
        .unwrap();
        let mut sum = 0.0;
        for i in 0..500 {
            let angle = i as f64 * 0.09;
            let x = (5000.0 + 60.0 * angle.cos()).round() as u16;
            let y = (4000.0 + 60.0 * angle.sin()).round() as u16;
            e.push(sample(x, y, 500), i as f64 * 8.0).unwrap();
            let p = e.tick(i as f64 * 8.0);
            if i > 200 {
                sum += (f64::from(p.x) - 5000.0).hypot(f64::from(p.y) - 4000.0);
            }
        }
        sum / 299.0
    };
    let protected = measure(true);
    let unprotected = measure(false);
    println!("loop radius: protected={protected} unprotected={unprotected}");
    assert!(protected > 50.0);
    assert!(protected > unprotected + 15.0);
}
#[test]
fn raw_bypass_hover_lift_and_next_stroke_are_preserved() {
    let mut e = Engine::new(Config {
        stroke_smoothing: false,
        wobble_reduction: 0.0,
        ..Config::default()
    })
    .unwrap();
    for (i, (x, y)) in [(1000, 2000), (1200, 2000), (1200, 2300), (1000, 2400)]
        .into_iter()
        .enumerate()
    {
        e.push(sample(x, y, 500), i as f64 * 8.0).unwrap();
        let p = e.tick(i as f64 * 8.0);
        assert_eq!((p.x, p.y), (x, y));
    }
    e.push(sample(1300, 2500, 0), 40.0).unwrap();
    assert!(!e.tick(40.0).contact);
    e.push(sample(8000, 7000, 500), 48.0).unwrap();
    let p = e.tick(48.0);
    assert_eq!((p.x, p.y), (8000, 7000));
    assert!(!e.tick(200.0).contact);
}
#[test]
fn settings_validate_roundtrip_and_presets_preserve_choices() {
    assert!(Config::parse("").unwrap().preserve_corners);
    for value in ["nan", "inf", "-0.1", "5.1"] {
        assert!(Config::parse(&format!("max_smoothing_distance_mm={value}")).is_err());
    }
    let c = Config::parse("preserve_corners=false\nmax_smoothing_distance_mm=0.3").unwrap();
    let d = Config::parse(&toml::to_string(&c).unwrap()).unwrap();
    assert!(!d.preserve_corners);
    assert_eq!(d.max_smoothing_distance_mm, 0.3);
    let p = c.drawing_profile(Config::default());
    assert!(!p.preserve_corners);
    assert_eq!(p.max_smoothing_distance_mm, 0.3);
}
