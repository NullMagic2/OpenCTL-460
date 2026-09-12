//! End-to-end shape/smoothing tests using synthetic pen reports; no device input.
use ctl460_rust::{
    config::Config,
    engine::{Engine, Frame},
    lifecycle::lift_before_hover,
    precision::{PrecisionBounds, PrecisionHold},
    protocol::Sample,
    shape_assist::ShapeAssist,
};
fn sample(x: u16, y: u16, p: u16) -> Sample {
    Sample {
        x,
        y,
        pressure: p,
        in_range: true,
        position_valid: true,
        tip: p > 0,
        ..Default::default()
    }
}
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
fn config() -> Config {
    Config {
        handwriting_mode: "off".into(),
        pen_control: Some(30.0),
        independent_line_controls: true,
        close_shapes: true,
        ..Default::default()
    }
}
fn circle(reverse: bool, radius: f64, count: u16) -> Vec<(u16, u16)> {
    (0..=count)
        .map(|i| {
            let a = f64::from(i) * std::f64::consts::TAU / f64::from(count)
                * if reverse { -1.0 } else { 1.0 };
            (
                (3000.0 + radius * a.cos()).round() as u16,
                (4500.0 + radius * a.sin()).round() as u16,
            )
        })
        .collect()
}
#[test]
fn closure_snaps_clockwise_and_counterclockwise_near_loops_exactly() {
    let mut time = 0.0;
    for reverse in [false, true] {
        let mut a = ShapeAssist::default();
        let c = config();
        let points = circle(reverse, 300.0, 240);
        let start = points[0];
        let mut out = Frame::default();
        for &(x, y) in &points[..240] {
            out = a.apply(frame(x, y), &c, {
                time += 8.0;
                time
            });
        }
        for _ in 0..24 {
            out = a.apply(frame(points[239].0, points[239].1), &c, {
                time += 8.0;
                time
            });
        }
        assert!(a.snapped());
        assert_eq!((out.x, out.y), start);
        assert_eq!(out.pressure, 0.5);
    }
}
#[test]
fn closure_stays_off_for_open_hooks_taps_short_strokes_and_disabled_mode() {
    let mut time = 0.0;
    for path in [
        vec![(3000, 4500); 100],
        vec![(3000, 4500), (3020, 4500), (3001, 4500)],
        vec![
            (3000, 4500),
            (4000, 4500),
            (4000, 4600),
            (3010, 4600),
            (3001, 4501),
        ],
    ] {
        let mut a = ShapeAssist::default();
        for (x, y) in path {
            let f = frame(x, y);
            assert_eq!(
                a.apply(f, &config(), {
                    time += 8.0;
                    time
                }),
                f
            );
        }
    }
    let mut a = ShapeAssist::default();
    let c = Config {
        close_shapes: false,
        ..config()
    };
    for (x, y) in circle(false, 300.0, 240) {
        let f = frame(x, y);
        assert_eq!(
            a.apply(f, &c, {
                time += 8.0;
                time
            }),
            f
        );
    }
}
#[test]
fn handwriting_eraser_barrel_and_lift_clear_closure_and_marker_state() {
    let mut time = 0.0;
    for excluded in [
        Frame {
            handwriting: true,
            ..frame(3100, 4500)
        },
        Frame {
            eraser: true,
            ..frame(3100, 4500)
        },
        Frame {
            barrel: true,
            ..frame(3100, 4500)
        },
        Frame {
            contact: false,
            pressure: 0.0,
            ..frame(3100, 4500)
        },
        Frame::default(),
    ] {
        let mut a = ShapeAssist::default();
        a.apply(frame(3000, 4500), &config(), {
            time += 8.0;
            time
        });
        assert!(a.start().is_some());
        assert_eq!(
            a.apply(excluded, &config(), {
                time += 8.0;
                time
            }),
            excluded
        );
        assert!(a.start().is_none());
        assert!(!a.snapped());
        let next = frame(9000, 5000);
        assert_eq!(
            a.apply(next, &config(), {
                time += 8.0;
                time
            }),
            next
        );
        assert_eq!(a.start(), Some((9000, 5000)));
    }
}
#[test]
fn snapped_endpoint_has_hysteresis_and_releases_on_deliberate_movement() {
    let mut time = 0.0;
    let mut a = ShapeAssist::default();
    let c = config();
    for (x, y) in circle(false, 300.0, 240) {
        a.apply(frame(x, y), &c, {
            time += 8.0;
            time
        });
    }
    for _ in 0..24 {
        a.apply(frame(3300, 4500), &c, {
            time += 8.0;
            time
        });
    }
    assert!(a.snapped());
    for x in [3303, 3308, 3295] {
        let f = a.apply(frame(x, 4500), &c, {
            time += 8.0;
            time
        });
        assert_eq!((f.x, f.y), (3300, 4500));
    }
    let f = a.apply(frame(3450, 4500), &c, {
        time += 8.0;
        time
    });
    assert!(!a.snapped());
    assert_eq!(f.x, 3450);
}
#[test]
fn precision_closure_marker_and_lift_share_the_same_emitted_start() {
    let mut time = 0.0;
    let c = Config {
        button1: "Precision Hold".into(),
        precision_gain: [50.0; 2],
        ..config()
    };
    let mut p = PrecisionHold::default();
    p.buttons(&c, [true, false]);
    p.buttons(&c, [false, false]);
    let b = PrecisionBounds::full();
    p.apply(
        Frame {
            contact: false,
            ..frame(3000, 4500)
        },
        b,
    );
    let mut a = ShapeAssist::default();
    let mut start = None;
    let mut last = Frame::default();
    for (x, y) in circle(false, 600.0, 360) {
        last = a.apply(p.apply(frame(x, y), b), &c, {
            time += 8.0;
            time
        });
        if start.is_none() {
            start = Some((last.x, last.y));
        }
    }
    for _ in 0..24 {
        last = a.apply(p.apply(frame(3600, 4500), b), &c, {
            time += 8.0;
            time
        });
    }
    assert_eq!(a.start(), start);
    assert_eq!(Some((last.x, last.y)), start);
    let hover = a.apply(
        p.apply(
            Frame {
                contact: false,
                pressure: 0.0,
                ..frame(5000, 5500)
            },
            b,
        ),
        &c,
        {
            time += 8.0;
            time
        },
    );
    let up = lift_before_hover(last, hover).unwrap();
    assert_eq!(Some((up.x, up.y)), start);
    assert_eq!(up.pressure, 0.0);
    assert!(!up.contact);
}
#[test]
fn endpoint_settles_in_contact_without_pressure_changes_or_lift_tail() {
    for (stream, stabilize, motion) in [
        (100.0, 0.0, 0.0),
        (0.0, 100.0, 0.0),
        (0.0, 0.0, 100.0),
        (100.0, 100.0, 100.0),
    ] {
        let c = Config {
            streamline_amount: stream,
            stabilization_amount: stabilize,
            motion_filter_amount: motion,
            ..config()
        };
        let mut e = Engine::new(c.clone()).unwrap();
        let mut baseline = Engine::new(Config {
            endpoint_settling: false,
            ..c
        })
        .unwrap();
        for i in 0..100 {
            for v in [&mut e, &mut baseline] {
                v.push(sample(2000 + i * 30, 4500, 500), f64::from(i) * 8.0)
                    .unwrap();
            }
        }
        let mut last = e.tick(792.0);
        for i in 1..=80 {
            let t = 792.0 + f64::from(i) * 8.0;
            let s = sample(4970, 4500, 500);
            e.push(s, t).unwrap();
            baseline.push(s, t).unwrap();
            let f = e.tick(t);
            assert_eq!(f.pressure, baseline.tick(t).pressure);
            assert!(f.x >= last.x && f.x <= 4970, "endpoint reversed/overshot");
            last = f;
        }
        assert!(4970 - last.x <= 2, "did not settle: {}", last.x);
        e.push(sample(6000, 5000, 0), 1440.0).unwrap();
        let hover = e.tick(1440.0);
        let up = lift_before_hover(last, hover).unwrap();
        assert_eq!((up.x, up.y), (last.x, last.y));
        assert!(!up.contact);
        assert_eq!(up.pressure, 0.0);
        assert!(!e.tick(1448.0).contact);
    }
}
fn roughness(points: &[(u16, u16)]) -> f64 {
    points[100..]
        .windows(3)
        .map(|p| {
            let ax = f64::from(p[1].0) - f64::from(p[0].0);
            let ay = f64::from(p[1].1) - f64::from(p[0].1);
            let bx = f64::from(p[2].0) - f64::from(p[1].0);
            let by = f64::from(p[2].1) - f64::from(p[1].1);
            (ax * by - ay * bx).atan2(ax * bx + ay * by).abs()
        })
        .sum::<f64>()
        / (points.len() - 102) as f64
}
fn replay(amount: f64, legacy: bool, shape: &str) -> Vec<(u16, u16)> {
    replay_filter(amount, legacy, shape, "stream")
}
fn replay_filter(amount: f64, legacy: bool, shape: &str, filter: &str) -> Vec<(u16, u16)> {
    // Reproduce a saved Light base, including the old GUI-independent flag.
    let mut c = config();
    c.streamline_amount = if filter == "stream" || filter == "all" {
        amount
    } else {
        0.0
    };
    c.stabilization_amount = if filter == "stabilize" || filter == "all" {
        amount
    } else {
        0.0
    };
    c.motion_filter_amount = if filter == "motion" || filter == "all" {
        amount
    } else {
        0.0
    };
    c.flowing_smoothing = !legacy;
    c.independent_line_controls = legacy;
    c.endpoint_settling = false;
    let mut e = Engine::new(c).unwrap();
    (0..720)
        .map(|i| {
            let a = f64::from(i) * std::f64::consts::TAU / 360.0;
            let noise = 60.0 * (f64::from(i) * 1.9).sin();
            let (x, y) = if shape == "line" {
                (1800 + i * 15, (4500.0 + noise).round() as u16)
            } else if shape == "circle" {
                (
                    (6000.0 + (1800.0 + noise) * a.cos()).round() as u16,
                    (4500.0 + (1800.0 + noise) * a.sin()).round() as u16,
                )
            } else {
                (
                    1800 + i * 15,
                    (4500.0 + 900.0 * (f64::from(i) / 100.0).sin() + noise).round() as u16,
                )
            };
            e.push(sample(x, y, 500), f64::from(i) * 8.0).unwrap();
            let f = e.tick(f64::from(i) * 8.0);
            (f.x, f.y)
        })
        .collect()
}
#[test]
fn full_pipeline_streamline_visibly_smooths_curves_at_high_settings() {
    for shape in ["circle", "s_curve"] {
        let raw = replay(0.0, false, shape);
        let medium = replay(50.0, false, shape);
        let high = replay(100.0, false, shape);
        let old = replay(100.0, true, shape);
        let a = roughness(&raw);
        let b = roughness(&medium);
        let c = roughness(&high);
        println!(
            "{shape}: turn roughness 0%={a:.6},50%={b:.6},100%={c:.6},legacy100%={:.6}",
            roughness(&old)
        );
        assert!(b < a * 0.4);
        assert!(c < b * 0.8);
        assert!(c < roughness(&old) * 0.3);
        if shape == "circle" {
            let mean = high[360..]
                .iter()
                .map(|p| (f64::from(p.0) - 6000.0).hypot(f64::from(p.1) - 4500.0))
                .sum::<f64>()
                / 360.0;
            assert!(mean > 1500.0 && mean < 1900.0, "circle collapsed: {mean}");
        }
    }
}
#[test]
fn settings_validate_preserve_and_roundtrip_drawing_aids() {
    let c = config();
    let d = Config::parse(&toml::to_string(&c).unwrap())
        .unwrap()
        .drawing_profile(Config::default());
    assert!(d.close_shapes && d.show_start_marker && d.endpoint_settling && d.flowing_smoothing);
    assert_eq!(d.closure_radius_mm, 0.6);
    for bad in ["nan", "inf", "0.0", "2.1"] {
        assert!(Config::parse(&format!("closure_radius_mm={bad}")).is_err());
    }
}
#[test]
fn actual_hover_to_contact_starts_at_the_measured_point() {
    let mut e = Engine::new(Config {
        wobble_reduction: 100.0,
        streamline_amount: 100.0,
        ..config()
    })
    .unwrap();
    for i in 0..20 {
        e.push(sample(3000 + i, 4500, 0), f64::from(i) * 8.0)
            .unwrap();
    }
    e.push(sample(3020, 4500, 500), 160.0).unwrap();
    let f = e.tick(160.0);
    assert_eq!((f.x, f.y), (3020, 4500));
    assert!(f.pressure > 0.0);
}

#[test]
fn strong_smoothed_loop_settles_and_closes_through_engine_and_precision() {
    let mut time = 0.0;
    let c = Config {
        streamline_amount: 100.0,
        button1: "Precision Hold".into(),
        ..config()
    };
    let mut e = Engine::new(c.clone()).unwrap();
    let mut p = PrecisionHold::default();
    p.buttons(&c, [true, false]);
    p.buttons(&c, [false, false]);
    let b = PrecisionBounds::full();
    p.apply(
        Frame {
            contact: false,
            ..frame(3000, 4500)
        },
        b,
    );
    let mut a = ShapeAssist::default();
    let points = circle(false, 1000.0, 360);
    let mut first = None;
    let mut out = Frame::default();
    for (i, &(x, y)) in points
        .iter()
        .chain(std::iter::repeat(&points[0]).take(100))
        .enumerate()
    {
        let t = i as f64 * 8.0;
        e.push(sample(x, y, 500), t).unwrap();
        out = a.apply(p.apply(e.tick(t), b), &c, {
            time += 8.0;
            time
        });
        if first.is_none() {
            first = Some((out.x, out.y));
        }
    }
    assert!(a.snapped());
    assert_eq!(Some((out.x, out.y)), first);
    assert!(out.contact && out.pressure > 0.0);
}
#[cfg(windows)]
#[test]
fn marker_is_click_through_nonactivating_expires_and_destroys_its_window() {
    use std::{thread, time::Duration};
    use windows_sys::Win32::{Foundation::*, UI::WindowsAndMessaging::*};
    unsafe extern "system" fn find(w: HWND, l: LPARAM) -> i32 {
        unsafe {
            let mut pid = 0;
            GetWindowThreadProcessId(w, &mut pid);
            let mut name = [0u16; 64];
            let n = GetClassNameW(w, name.as_mut_ptr(), 64);
            if pid == std::process::id()
                && String::from_utf16_lossy(&name[..n as usize]) == "OpenCTLStrokeStartMarker"
            {
                *(l as *mut HWND) = w;
                return 0;
            }
            1
        }
    }
    let marker = ctl460_rust::start_marker::StartMarker::new().unwrap();
    let mut window: HWND = std::ptr::null_mut();
    unsafe {
        EnumWindows(Some(find), &mut window as *mut HWND as LPARAM);
    }
    assert!(!window.is_null());
    let foreground = unsafe { GetForegroundWindow() };
    marker.show(Some((200, 200)), foreground as isize);
    thread::sleep(Duration::from_millis(60));
    unsafe {
        assert_eq!(GetForegroundWindow(), foreground);
        assert_ne!(IsWindowVisible(window), 0);
        let styles = GetWindowLongW(window, GWL_EXSTYLE) as u32;
        assert_eq!(
            styles & (WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW),
            WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW
        );
        assert_eq!(
            SendMessageW(window, WM_NCHITTEST, 0, 0),
            HTTRANSPARENT as LRESULT
        );
    }
    thread::sleep(Duration::from_millis(280));
    assert_eq!(unsafe { IsWindowVisible(window) }, 0);
    drop(marker);
    assert_eq!(unsafe { IsWindow(window) }, 0);
}

#[test]
fn each_smoothing_control_and_combination_changes_lines_and_curves() {
    for shape in ["line", "circle", "s_curve"] {
        let off = replay_filter(0.0, false, shape, "all");
        let baseline = roughness(&off);
        for filter in ["stream", "stabilize", "motion", "all"] {
            let half = replay_filter(50.0, false, shape, filter);
            let full = replay_filter(100.0, false, shape, filter);
            let medium = roughness(&half);
            let high = roughness(&full);
            println!("{shape}/{filter}: off={baseline:.6},50%={medium:.6},100%={high:.6}");
            assert!(
                medium < baseline * 0.9,
                "{shape}/{filter}: midpoint has too little effect"
            );
            assert!(
                high < medium,
                "{shape}/{filter}: maximum must have a stronger effect"
            );
            assert_eq!(full[0], off[0], "first contact must stay exact");
            if shape == "circle" {
                let radius = full[360..]
                    .iter()
                    .map(|p| (f64::from(p.0) - 6000.0).hypot(f64::from(p.1) - 4500.0))
                    .sum::<f64>()
                    / 360.0;
                assert!(
                    radius > 1400.0 && radius < 2000.0,
                    "{filter}: circle collapsed or expanded: {radius}"
                );
            } else {
                assert!(
                    full.last().unwrap().0 > 11000,
                    "{filter}: stroke failed to traverse the canvas"
                );
            }
        }
    }
}
