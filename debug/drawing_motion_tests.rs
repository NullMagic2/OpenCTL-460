//! Direction, precision-speed, and continuous-spiral regressions.
use ctl460_rust::{
    config::Config,
    engine::{Engine, Frame},
    line_smoothing::constrain_motion,
    precision::{output_bounds, PrecisionBounds, PrecisionHold},
    protocol::Sample,
    shape_assist::ShapeAssist,
    stroke::map_to_screen,
};
use std::f64::consts::TAU;
fn frame(x: u16, y: u16) -> Frame {
    Frame {
        x,
        y,
        pressure: 0.5,
        contact: true,
        in_range: true,
        ..Default::default()
    }
}
fn rotate(p: (f64, f64), a: f64) -> (f64, f64) {
    (p.0 * a.cos() - p.1 * a.sin(), p.0 * a.sin() + p.1 * a.cos())
}
fn enable(p: &mut PrecisionHold, gain: f64) {
    let c = Config {
        button1: "Precision Hold".into(),
        precision_gain: [gain * 100.0; 2],
        ..Default::default()
    };
    p.buttons(&c, [true, false]);
    p.buttons(&c, [false, false]);
}
#[test]
fn reversal_guard_is_invariant_under_arbitrary_rotation_and_translation() {
    for (old, raw, candidate) in [
        ((0.0, 0.0), (100.0, 5.0), (60.0, -20.0)),
        ((0.0, 0.0), (100.0, 0.0), (110.0, 4.0)),
        ((20.0, 30.0), (20.0, 30.0), (40.0, 50.0)),
        ((80.0, -40.0), (-20.0, 60.0), (120.0, 90.0)),
    ] {
        let expected = constrain_motion(old, raw, candidate);
        for deg in 0..360 {
            let a = (deg as f64).to_radians();
            let transform = |p| {
                let p = rotate(p, a);
                (p.0 + 7000.0, p.1 + 4500.0)
            };
            let out = constrain_motion(transform(old), transform(raw), transform(candidate));
            let expected = transform(expected);
            assert!((out.0 - expected.0).hypot(out.1 - expected.1) < 1e-8);
        }
    }
}
#[test]
fn full_pipeline_shallow_ovals_keep_their_contour_and_reduce_following_error() {
    for degrees in [0.0f64, 15.0, 30.0, 45.0, 60.0, 90.0] {
        let angle = degrees.to_radians();
        let c = Config {
            pen_control: Some(30.0),
            streamline_amount: 50.0,
            stabilization_amount: 20.0,
            circle_smoothing: 29.0,
            handwriting_mode: "auto".into(),
            ..Default::default()
        };
        let mut e = Engine::new(c).unwrap();
        let (mut error, mut contour) = (0.0, 0.0);
        for i in 0..720 {
            let t = i as f64 * TAU / 360.0;
            let ideal = rotate((2300.0 * t.cos(), 650.0 * t.sin()), angle);
            let raw = rotate(
                (
                    2300.0 * t.cos(),
                    650.0 * t.sin() + 8.0 * (i as f64 * 1.9).sin(),
                ),
                angle,
            );
            e.push(
                Sample {
                    x: (7300.0 + raw.0).round() as u16,
                    y: (4600.0 + raw.1).round() as u16,
                    pressure: 500,
                    tip: true,
                    in_range: true,
                    position_valid: true,
                    ..Default::default()
                },
                i as f64 * 8.0,
            )
            .unwrap();
            let f = e.tick(i as f64 * 8.0);
            if i >= 360 {
                let p = (f64::from(f.x) - 7300.0, f64::from(f.y) - 4600.0);
                error += (p.0 - ideal.0).hypot(p.1 - ideal.1);
                let p = rotate(p, -angle);
                let q = (p.0 / 2300.0).hypot(p.1 / 650.0);
                contour += (q - 1.0).abs() * p.0.hypot(p.1) / q;
            }
        }
        assert!(
            error / 360.0 < 150.0,
            "{degrees} degrees: following error {}",
            error / 360.0
        );
        assert!(
            contour / 360.0 < 10.0,
            "{degrees} degrees: oval distortion {}",
            contour / 360.0
        );
    }
}
#[test]
fn precision_has_constant_scale_across_full_vertical_travel_both_directions() {
    let b = PrecisionBounds::full();
    for gain in [0.1, 0.25, 0.5, 0.9, 1.0] {
        let mut p = PrecisionHold::default();
        enable(&mut p, gain);
        assert_eq!(p.apply(frame(7360, 4600), b), frame(7360, 4600));
        for y in (0..=9200).step_by(10).chain((0..=9200).step_by(10).rev()) {
            let f = p.apply(frame(7360, y), b);
            assert_eq!(f.x, 7360);
            assert_eq!(
                f.y,
                (4600.0 + (f64::from(y) - 4600.0) * gain).round() as u16
            );
            assert_eq!(f.pressure, 0.5);
        }
    }
}
#[test]
fn precision_tall_ovals_and_diagonals_scale_equally_far_from_the_anchor() {
    for gain in [0.1, 0.5, 0.9] {
        for degrees in [0.0f64, 30.0, 45.0, 90.0] {
            let mut p = PrecisionHold::default();
            enable(&mut p, gain);
            let b = PrecisionBounds::full();
            let mut start = None;
            for i in 0..=720 {
                let a = i as f64 * TAU / 360.0;
                let q = rotate((650.0 * a.cos(), 3200.0 * a.sin()), degrees.to_radians());
                let raw = frame((7300.0 + q.0).round() as u16, (4600.0 + q.1).round() as u16);
                let anchor = *start.get_or_insert((raw.x, raw.y));
                let out = p.apply(raw, b);
                assert_eq!(
                    out.x,
                    (f64::from(anchor.0) + (f64::from(raw.x) - f64::from(anchor.0)) * gain).round()
                        as u16
                );
                assert_eq!(
                    out.y,
                    (f64::from(anchor.1) + (f64::from(raw.y) - f64::from(anchor.1)) * gain).round()
                        as u16
                );
            }
        }
    }
}
#[test]
fn precision_hover_is_reduced_and_each_stroke_starts_at_the_visible_target() {
    for (w, h) in [(1920, 1080), (1080, 1920), (5120, 1440)] {
        let b = output_bounds(w, h, true);
        let mut p = PrecisionHold::default();
        enable(&mut p, 0.1);
        for (x, y) in [(0, 0), (14720, 9200), (7000, 4500), (14720, 0), (0, 9200)] {
            let hover = Frame {
                contact: false,
                pressure: 0.0,
                ..frame(x, y)
            };
            let visible = p.apply(hover, b);
            assert!(p.active());
            let down = p.apply(frame(x, y), b);
            assert_eq!(
                map_to_screen(down.x, down.y, w, h, true),
                map_to_screen(visible.x, visible.y, w, h, true)
            );
            p.apply(frame(x.saturating_add(20).min(14720), y), b);
        }
    }
}
fn closure_config() -> Config {
    Config {
        close_shapes: true,
        ..Default::default()
    }
}
fn near_loop(a: &mut ShapeAssist, c: &Config, dt: f64) -> (f64, Frame) {
    let mut end = frame(3500, 4500);
    let mut time = 0.0;
    for i in 0..355 {
        let t = i as f64 * TAU / 360.0;
        end = frame(
            (3000.0 + 500.0 * t.cos()).round() as u16,
            (4500.0 + 500.0 * t.sin()).round() as u16,
        );
        assert_eq!(a.apply(end, c, time), end);
        time += dt;
    }
    (time, end)
}
#[test]
fn moving_spirals_repeated_loops_and_connected_loops_are_bit_exact() {
    for dt in [2.0, 8.0, 16.0] {
        for kind in 0..4 {
            let mut a = ShapeAssist::default();
            let c = closure_config();
            for i in 0..1800 {
                let t = i as f64 * TAU / 360.0;
                let radius = match kind {
                    1 => 800.0 - i as f64 * 0.3,
                    2 => 250.0 + i as f64 * 0.3,
                    _ => 500.0,
                };
                let center = if kind == 3 {
                    2500.0 + i as f64 * 3.0
                } else {
                    5000.0
                };
                let f = frame(
                    (center + radius * t.cos()).round() as u16,
                    (4500.0 + radius * t.sin()).round() as u16,
                );
                assert_eq!(a.apply(f, &c, i as f64 * dt), f, "kind {kind}, report {i}");
                assert!(!a.snapped());
            }
        }
    }
}
#[test]
fn closure_requires_measured_pause_then_eases_to_start_and_releases_on_motion() {
    for dt in [2.0, 8.0, 16.0] {
        let mut a = ShapeAssist::default();
        let c = closure_config();
        let (mut time, end) = near_loop(&mut a, &c, dt);
        let start = a.start().unwrap();
        let initial =
            (f64::from(end.x) - f64::from(start.0)).hypot(f64::from(end.y) - f64::from(start.1));
        let mut previous = initial;
        let pause_start = time;
        for _ in 0..=(200.0 / dt) as usize {
            let out = a.apply(end, &c, time);
            if time - pause_start < 64.0 {
                assert_eq!(out, end);
            }
            let distance = (f64::from(out.x) - f64::from(start.0))
                .hypot(f64::from(out.y) - f64::from(start.1));
            assert!(distance <= previous + 0.8);
            previous = distance;
            assert_eq!(out.pressure, end.pressure);
            time += dt;
        }
        assert!(a.snapped());
        assert!(previous < 0.1);
        let resume = frame(end.x + 12, end.y);
        assert_eq!(a.apply(resume, &c, time), resume);
        assert!(!a.snapped());
    }
}
#[test]
fn repeated_timer_frames_and_report_gaps_cannot_trigger_closure() {
    let mut a = ShapeAssist::default();
    let c = closure_config();
    let (mut time, end) = near_loop(&mut a, &c, 8.0);
    for _ in 0..2000 {
        assert_eq!(a.apply(end, &c, time), end);
    }
    assert!(!a.snapped());
    for _ in 0..20 {
        time += 100.0;
        assert_eq!(a.apply(end, &c, time), end);
    }
    assert!(!a.snapped());
}
#[test]
fn brief_pass_or_pause_followed_by_motion_lift_or_tool_change_cancels_snap() {
    let c = closure_config();
    for stop in [
        Frame {
            contact: false,
            pressure: 0.0,
            ..frame(3500, 4500)
        },
        Frame {
            eraser: true,
            ..frame(3500, 4500)
        },
        Frame {
            handwriting: true,
            ..frame(3500, 4500)
        },
        Frame::default(),
    ] {
        let mut a = ShapeAssist::default();
        let (mut time, end) = near_loop(&mut a, &c, 8.0);
        for _ in 0..8 {
            assert_eq!(a.apply(end, &c, time), end);
            time += 8.0;
        }
        assert_eq!(a.apply(stop, &c, time), stop);
        assert!(!a.snapped());
        assert!(a.start().is_none());
    }
}
