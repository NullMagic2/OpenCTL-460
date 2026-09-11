//! Synthetic regression fixtures for writing heuristics, geometric fidelity, and pressure settings.
//! These measure algorithm behavior, not handwriting classification accuracy on real people.
use ctl460_rust::{
    config::Config, engine::Engine, handwriting::HandwritingDetector, protocol::Sample,
    stroke::StrokeFilter,
};

fn pen(x: u16, y: u16) -> Sample {
    Sample {
        x,
        y,
        pressure: 400,
        tip: true,
        in_range: true,
        position_valid: true,
        ..Default::default()
    }
}
fn loop_stroke(d: &mut HandwritingDetector, start: f64, size: f64) {
    for i in 0..=40 {
        let a = f64::from(i) * std::f64::consts::TAU / 40.0;
        d.observe(
            (4000.0 + size * a.cos()) as u16,
            (4000.0 + size * a.sin()) as u16,
            true,
            start + f64::from(i) * 8.0,
        );
    }
    d.observe(4000, 4000, false, start + 328.0);
}

#[test]
fn repeated_small_curves_are_evidence_not_single_strokes() {
    let mut d = HandwritingDetector::default();
    loop_stroke(&mut d, 0.0, 200.0);
    assert!(!d.likely());
    loop_stroke(&mut d, 400.0, 200.0);
    assert!(!d.likely());
    loop_stroke(&mut d, 800.0, 200.0);
    assert!(d.likely());
    d.observe(0, 0, false, 4000.0);
    assert!(!d.likely());
    assert_eq!(d.score(), 0.0);
}

#[test]
fn large_drawings_dots_and_stationary_jitter_do_not_trigger_writing() {
    for size in [0.0, 3.0, 2000.0] {
        let mut d = HandwritingDetector::default();
        for i in 0..8 {
            loop_stroke(&mut d, f64::from(i) * 400.0, size);
        }
        assert!(!d.likely(), "size {size}");
    }
}

#[test]
fn handwriting_filter_bounds_error_on_loops_corners_and_reversals() {
    let mut f = StrokeFilter::default();
    let mut points = Vec::new();
    for i in 0..=80 {
        let a = f64::from(i) * std::f64::consts::TAU / 80.0;
        points.push((
            (1000.0 + 60.0 * a.cos()) as u16,
            (1000.0 + 60.0 * a.sin()) as u16,
        ));
    }
    points.extend([
        (1000, 1000),
        (1200, 1000),
        (1200, 1200),
        (1000, 1200),
        (1000, 1000),
        (1200, 1000),
        (1000, 1000),
    ]);
    for (x, y) in points {
        let (fx, fy) = f.handwriting(x, y, 8.0);
        assert!((f64::from(fx) - f64::from(x)).hypot(f64::from(fy) - f64::from(y)) <= 12.71);
    }
}

#[test]
fn writing_reduces_stationary_jitter_without_moving_first_point() {
    let mut f = StrokeFilter::default();
    assert_eq!(f.handwriting(1000, 1000, 8.0), (1000, 1000));
    let mut error = 0.0;
    for i in 0..100 {
        let x = if i % 2 == 0 { 1005 } else { 995 };
        let (fx, _) = f.handwriting(x, 1000, 8.0);
        error += (f64::from(fx) - 1000.0).powi(2);
    }
    assert!(error < 100.0 * 25.0 * 0.5);
}

#[test]
fn manual_writing_keeps_taps_and_lifts_and_disables_artistic_tilt() {
    let mut e = Engine::new(Config {
        handwriting_mode: "on".into(),
        virtual_tilt: true,
        ..Config::default()
    })
    .unwrap();
    e.push(pen(1200, 1300), 0.0).unwrap();
    let f = e.tick(0.0);
    assert!(f.handwriting && f.contact && f.pressure > 0.0);
    assert!(!f.virtual_tilt);
    assert_eq!((f.x, f.y), (1200, 1300));
    e.push(
        Sample {
            tip: false,
            pressure: 0,
            ..pen(1250, 1350)
        },
        1.0,
    )
    .unwrap();
    let f = e.tick(1.0);
    assert!(!f.contact);
    assert_eq!(f.pressure, 0.0);
    e.push(pen(6000, 7000), 8.0).unwrap();
    assert_eq!((e.tick(8.0).x, e.tick(8.0).y), (6000, 7000));
}

#[test]
fn automatic_mode_switches_only_on_next_pen_down() {
    let mut e = Engine::new(Config {
        handwriting_mode: "auto".into(),
        streamline_amount: 80.0,
        ..Config::default()
    })
    .unwrap();
    let mut artistic_writing_samples = 0;
    for s in 0..4 {
        let t = f64::from(s) * 344.0;
        for i in 0..=40 {
            let a = f64::from(i) * std::f64::consts::TAU / 40.0;
            let now = t + f64::from(i) * 8.0;
            e.push(
                pen(
                    (4000.0 + 200.0 * a.cos()) as u16,
                    (4000.0 + 200.0 * a.sin()) as u16,
                ),
                now,
            )
            .unwrap();
            assert_eq!(e.tick(now).handwriting, s >= 3);
            let frame = e.tick(now);
            if frame.handwriting && i > 5 {
                let dx = f64::from(frame.x) - (4000.0 + 200.0 * a.cos());
                let dy = f64::from(frame.y) - (4000.0 + 200.0 * a.sin());
                if dx.hypot(dy) > 20.0 {
                    artistic_writing_samples += 1;
                }
            }
        }
        e.push(
            Sample {
                tip: false,
                pressure: 0,
                ..pen(4200, 4000)
            },
            t + 328.0,
        )
        .unwrap();
    }
    assert!(
        artistic_writing_samples > 10,
        "auto handwriting must retain StreamLine"
    );
}

#[test]
fn pressure_controls_reject_invalid_ranges_and_have_expected_effects() {
    for invalid in [
        "floor=950\nceiling=900",
        "gain=0.0",
        "gain=3.1",
        "press_on=3\npress_off=3",
        "handwriting_mode='recognize'",
    ] {
        assert!(Config::parse(invalid).is_err());
    }
    let c = Config {
        floor: 100,
        ceiling: 800,
        gain: 1.0,
        gamma: 1.0,
        ..Config::default()
    };
    assert_eq!(c.curve(100), 0.0);
    assert_eq!(c.curve(800), 1.0);
    assert!((c.curve(450) - 0.5).abs() < 1e-10);
    let boosted = Config {
        gain: 1.5,
        ..c.clone()
    };
    assert!(boosted.curve(450) > c.curve(450));
    assert_eq!(boosted.curve(1023), 1.0);
}

#[test]
fn tight_handwriting_loops_keep_their_radius_and_reset_between_letters() {
    use ctl460_rust::stroke::StrokeFilter;
    for radius in [30.0, 40.0, 60.0] {
        let mut filter = StrokeFilter::default();
        for i in 0..=96 {
            let angle = f64::from(i) * std::f64::consts::TAU / 24.0;
            let raw = (
                (5000.0 + radius * angle.cos()).round() as u16,
                (5000.0 + radius * angle.sin()).round() as u16,
            );
            let point = filter.handwriting(raw.0, raw.1, 8.0);
            let error = (f64::from(point.0) - f64::from(raw.0))
                .hypot(f64::from(point.1) - f64::from(raw.1));
            assert!(
                error <= 12.8,
                "must retain the handwriting displacement bound"
            );
            if i >= 8 {
                assert!(
                    error <= radius * 0.14 + 1.0,
                    "tight loop error {error}, radius {radius}"
                );
                let actual_radius =
                    (f64::from(point.0) - 5000.0).hypot(f64::from(point.1) - 5000.0);
                assert!(actual_radius >= radius * 0.88, "loop must not collapse");
            }
        }
        filter.reset();
        assert_eq!(filter.handwriting(8000, 2000, 8.0), (8000, 2000));
    }
}
