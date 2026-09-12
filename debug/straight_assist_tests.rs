//! Directional assistance regressions: normal-mode integration and geometric safeguards.
use ctl460_rust::{
    config::Config, engine::Engine, protocol::Sample, straight_assist::StraightAssist,
};
use std::f64::consts::TAU;
fn transform(p: (f64, f64), angle: f64) -> (f64, f64) {
    (
        7000.0 + p.0 * angle.cos() - p.1 * angle.sin(),
        4500.0 + p.0 * angle.sin() + p.1 * angle.cos(),
    )
}
#[test]
fn straight_wander_is_reduced_equally_in_all_directions_without_forward_lag() {
    for angle in [0.0, 0.3, TAU / 4.0, TAU / 2.0, 3.0 * TAU / 4.0] {
        let mut assist = StraightAssist::default();
        let (mut before, mut after, mut longitudinal) = (0.0, 0.0, 0.0);
        for i in 0..800 {
            let along = i as f64 * 4.0 - 1600.0;
            let across = 6.0 * (i as f64 * TAU / 40.0).sin();
            let raw = transform((along, across), angle);
            let out = assist.update(raw, 0.5);
            let normal = (-(out.0 - 7000.0) * angle.sin() + (out.1 - 4500.0) * angle.cos()).abs();
            if i > 100 {
                before += across * across;
                after += normal * normal;
                longitudinal +=
                    ((out.0 - raw.0) * angle.cos() + (out.1 - raw.1) * angle.sin()).abs();
            }
        }
        eprintln!(
            "angle={angle:.3} RMS ratio={:.3} mean forward change={:.3}",
            (after / before).sqrt(),
            longitudinal / 699.0
        );
        assert!(after < before * 0.65);
        assert!(longitudinal / 699.0 < 0.1);
    }
}
#[test]
fn clean_circles_ellipses_and_spirals_keep_their_shape() {
    for kind in 0..3 {
        let mut assist = StraightAssist::default();
        let mut maximum: f64 = 0.0;
        for i in 0..1800 {
            let a = i as f64 * TAU / 720.0;
            let radius = if kind == 2 {
                300.0 + i as f64 * 0.5
            } else {
                800.0
            };
            let raw = (
                7000.0 + radius * a.cos(),
                4500.0 + radius * if kind == 1 { 2.0 * a.sin() } else { a.sin() },
            );
            let p = assist.update(raw, 0.5);
            maximum = maximum.max((p.0 - raw.0).hypot(p.1 - raw.1));
        }
        assert!(maximum < 1e-8, "kind {kind}, error {maximum}");
    }
}
#[test]
fn stationary_reports_corners_and_reversals_cannot_lock_or_accumulate_drift() {
    let mut assist = StraightAssist::default();
    let mut raw = (0.0, 0.0);
    let mut out = raw;
    for i in 0..200 {
        raw = (i as f64 * 4.0, 6.0 * (i as f64 * TAU / 40.0).sin());
        out = assist.update(raw, 0.5);
    }
    for _ in 0..10000 {
        assert_eq!(assist.update(raw, 0.5), out);
    }
    for i in 1..100 {
        let p = (raw.0, raw.1 + i as f64 * 4.0);
        let out = assist.update(p, 0.5);
        assert!((out.0 - p.0).hypot(out.1 - p.1) <= 12.001);
        if i > 25 {
            assert_eq!(out, p);
        }
    }
    assist.reset();
    assert_eq!(assist.update((100.0, 100.0), 0.5), (100.0, 100.0));
    for i in (0..400).chain((0..400).rev()) {
        let p = (100.0, i as f64);
        assert_eq!(assist.update(p, 0.5), p);
    }
}
#[test]
fn normal_mode_preserves_pressure_hover_raw_bypass_and_stroke_starts() {
    let c = Config {
        pen_control: Some(30.0),
        handwriting_mode: "off".into(),
        ..Default::default()
    };
    let mut e = Engine::new(c.clone()).unwrap();
    let mut raw_engine = Engine::new(Config {
        pen_control: Some(0.0),
        ..c
    })
    .unwrap();
    for i in 0..500 {
        let sample = Sample {
            x: 4000 + i * 4,
            y: (4500.0 + 6.0 * (i as f64 * TAU / 40.0).sin()).round() as u16,
            pressure: 400,
            tip: true,
            in_range: true,
            position_valid: true,
            ..Default::default()
        };
        e.push(sample, i as f64 * 8.0).unwrap();
        raw_engine.push(sample, i as f64 * 8.0).unwrap();
        let a = e.tick(i as f64 * 8.0);
        let b = raw_engine.tick(i as f64 * 8.0);
        assert_eq!(a.pressure, b.pressure);
        assert!(a.contact);
        assert_eq!((b.x, b.y), (sample.x, sample.y));
        if i == 0 {
            assert_eq!((a.x, a.y), (sample.x, sample.y));
        }
    }
    e.push(
        Sample {
            x: 8000,
            y: 6000,
            in_range: true,
            position_valid: true,
            ..Default::default()
        },
        4000.0,
    )
    .unwrap();
    assert!(!e.tick(4000.0).contact);
    e.push(
        Sample {
            x: 8100,
            y: 6100,
            pressure: 400,
            tip: true,
            in_range: true,
            position_valid: true,
            ..Default::default()
        },
        4008.0,
    )
    .unwrap();
    assert_eq!((e.tick(4008.0).x, e.tick(4008.0).y), (8100, 6100));
}

#[test]
fn ordinary_drawing_reduces_sideways_wander_and_has_no_upward_preference() {
    for vertical in [false, true] {
        let c = Config {
            pen_control: Some(30.0),
            handwriting_mode: "off".into(),
            ..Default::default()
        };
        let mut normal = Engine::new(c.clone()).unwrap();
        let mut mirrored = Engine::new(c).unwrap();
        let (mut before, mut after) = (0.0, 0.0);
        for i in 0..600 {
            let along = 3000 + i * 4;
            let across = (4500.0 + 6.0 * (i as f64 * TAU / 40.0).sin()).round() as u16;
            let (x, y) = if vertical {
                (across, along)
            } else {
                (along, across)
            };
            let sample = Sample {
                x,
                y,
                pressure: 400,
                tip: true,
                in_range: true,
                position_valid: true,
                ..Default::default()
            };
            let time = i as f64 * 8.0;
            normal.push(sample, time).unwrap();
            mirrored
                .push(
                    Sample {
                        y: 9200 - y,
                        ..sample
                    },
                    time,
                )
                .unwrap();
            let out = normal.tick(time);
            let reflected = mirrored.tick(time);
            assert!((i32::from(out.x) - i32::from(reflected.x)).abs() <= 1);
            assert!((i32::from(out.y) + i32::from(reflected.y) - 9200).abs() <= 1);
            if i > 100 {
                before += (f64::from(across) - 4500.0).powi(2);
                after += (f64::from(if vertical { out.x } else { out.y }) - 4500.0).powi(2);
            }
        }
        eprintln!(
            "normal mode vertical={vertical} RMS ratio={}",
            (after / before).sqrt()
        );
        assert!(after < 0.70 * before);
    }
}

#[test]
fn assistance_is_spatial_and_does_not_pull_a_line_toward_an_axis() {
    let mut dense = StraightAssist::default();
    let mut subdivided = StraightAssist::default();
    let mut previous: Option<(f64, f64)> = None;
    for i in 0..600 {
        let p = transform((i as f64 * 4.0, 6.0 * (i as f64 * TAU / 40.0).sin()), 0.71);
        let a = dense.update(p, 0.5);
        if let Some(q) = previous {
            for n in 1..4 {
                subdivided.update(
                    (
                        q.0 + (p.0 - q.0) * n as f64 / 4.0,
                        q.1 + (p.1 - q.1) * n as f64 / 4.0,
                    ),
                    0.5,
                );
            }
        }
        let b = subdivided.update(p, 0.5);
        // Subdivision may ease correction more frequently, but not alter the
        // spatial fit or introduce an accumulating directional displacement.
        assert!((a.0 - b.0).hypot(a.1 - b.1) < 2.0);
        previous = Some(p);
    }
}
