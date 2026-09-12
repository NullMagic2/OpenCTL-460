//! Synthetic geometry, button, lifecycle and config tests; no physical input is injected.
use ctl460_rust::{
    config::Config,
    engine::{Engine, Frame},
    lifecycle::lift_before_hover,
    precision::{output_bounds, PrecisionBounds, PrecisionHold},
    protocol::{Sample, MAX_X, MAX_Y},
    stroke::map_to_screen,
};
fn frame(x: u16, y: u16, contact: bool) -> Frame {
    Frame {
        x,
        y,
        in_range: true,
        contact,
        pressure: if contact { 0.4 } else { 0.0 },
        ..Default::default()
    }
}
fn config(gain: f64) -> Config {
    Config {
        button1: "Precision Hold".into(),
        button2: "Precision Hold".into(),
        precision_gain: [gain * 100.0; 2],
        ..Default::default()
    }
}
fn toggle(p: &mut PrecisionHold, c: &Config) {
    p.buttons(c, [true, false]);
    p.buttons(c, [false, false]);
}

fn activated(gain: f64, at: (u16, u16), bounds: PrecisionBounds) -> PrecisionHold {
    let mut p = PrecisionHold::default();
    toggle(&mut p, &config(gain));
    assert_eq!(
        p.apply(frame(at.0, at.1, false), bounds),
        frame(at.0, at.1, false)
    );
    p
}
#[test]
fn config_backward_compatibility_validation_and_presets() {
    assert_eq!(Config::parse("").unwrap().precision_gain, [50.0; 2]);
    let c = config(0.37);
    let saved = toml::to_string(&c).unwrap();
    let c = Config::parse(&saved)
        .unwrap()
        .drawing_profile(Config::default());
    assert_eq!(c.precision_gain, [37.0; 2]);
    assert_eq!(c.button_precision([true, false]), Some(0.37));
    assert_eq!(Config::default().button_precision([true, true]), None);
    for g in [f64::NAN, f64::INFINITY, 9.9, 100.1] {
        let mut c = c.clone();
        c.precision_gain[0] = g;
        assert!(c.validate().is_err());
    }
}

#[test]
fn native_button_reports_reduce_processed_strokes_with_drawing_filters_enabled() {
    use ctl460_rust::protocol::{decode, ReportFormat};
    for gain in [0.8, 0.5, 0.1] {
        let c = Config {
            streamline_amount: 30.0,
            stabilization_amount: 20.0,
            pen_control: Some(30.0),
            circle_smoothing: 29.0,
            flowing_smoothing: true,
            independent_line_controls: true,
            ..config(gain)
        };
        let mut engine = Engine::new(c.clone()).unwrap();
        let mut p = PrecisionHold::default();
        let b = output_bounds(1920, 1080, true);
        let mut anchor = Frame::default();
        for i in 0..103u16 {
            // Click in hover, release, then draw a diagonal through real decoding
            // and smoothing. Precision consumes the original side-button states.
            let x = 3000 + i.saturating_sub(2) * 30;
            let y = 2500 + i.saturating_sub(2) * 20;
            let mut bytes = [0u8; 9];
            bytes[0] = 2;
            bytes[1] = 0xe0
                | if i == 0 {
                    2
                } else if i >= 2 {
                    1
                } else {
                    0
                };
            bytes[2..4].copy_from_slice(&x.to_le_bytes());
            bytes[4..6].copy_from_slice(&y.to_le_bytes());
            bytes[6..8].copy_from_slice(&(if i >= 2 { 400u16 } else { 0 }).to_le_bytes());
            let mut sample = decode(&bytes, ReportFormat::Native9).unwrap();
            p.buttons(&c, [sample.barrel, sample.second_button]);
            sample.barrel = false;
            engine.push(sample, f64::from(i) * 4.0).unwrap();
            let original = engine.tick(f64::from(i) * 4.0);
            let out = p.apply(original, b);
            if i < 2 {
                assert_eq!(out, original);
                if i == 1 {
                    p.reconfigure(&c);
                }
            } else {
                if i == 2 {
                    anchor = original;
                }
                assert!(out.contact);
                assert_eq!(
                    out.x,
                    (f64::from(anchor.x) + f64::from(original.x - anchor.x) * gain).round() as u16
                );
                assert_eq!(
                    out.y,
                    (f64::from(anchor.y) + f64::from(original.y - anchor.y) * gain).round() as u16
                );
                assert_eq!(out.pressure, original.pressure);
            }
        }
    }
}
#[test]
fn each_button_toggles_once_on_press_and_never_on_release_or_repeats() {
    for i in 0..2 {
        let mut p = PrecisionHold::default();
        let mut c = config(0.5);
        c.precision_gain = [25.0, 50.0];
        let mut states = [false; 2];
        states[i] = true;
        for _ in 0..10 {
            p.buttons(&c, states);
            assert!(p.enabled());
        }
        p.buttons(&c, [false; 2]);
        assert!(p.enabled());
        p.buttons(&c, states);
        assert!(!p.enabled());
    }
    let mut p = PrecisionHold::default();
    p.buttons(&config(0.5), [true, true]);
    assert!(p.enabled());
}
#[test]
fn inactive_and_zero_reduction_are_bit_exact_over_sensor_grid() {
    for aspect in [false, true] {
        let b = output_bounds(1920, 1080, aspect);
        let mut off = PrecisionHold::default();
        let mut on = activated(1.0, (7360, 4600), b);
        for x in (0..=MAX_X).step_by(32) {
            for y in (0..=MAX_Y).step_by(32) {
                let f = frame(x, y, true);
                assert_eq!(off.apply(f, b), f);
                assert_eq!(on.apply(f, b), f);
            }
        }
    }
}
#[test]
fn precision_binding_emits_no_keyboard_or_mouse_events() {
    use ctl460_rust::button_actions::{Bindings, InputCode, InputSink};
    struct NoInput;
    impl InputSink for NoInput {
        fn held(&self, _: InputCode) -> bool {
            false
        }
        fn send(&mut self, _: InputCode, _: bool) -> Result<(), String> {
            panic!("precision injected a key/click")
        }
    }
    let mut b = Bindings::new(["Precision Hold".into(), "Precision Hold".into()]);
    for s in [[true, false], [true, true], [false, true], [false, false]] {
        b.update(&mut NoInput, s).unwrap();
    }
}

#[test]
fn pressure_flags_and_no_tilt_survive_the_shared_transform() {
    let b = PrecisionBounds::full();
    let c = Config {
        virtual_tilt: true,
        ..config(0.5)
    };
    let mut engine = Engine::new(c).unwrap();
    let mut p = activated(0.5, (3000, 4000), b);
    for i in 0..200 {
        let sample = Sample {
            x: 3000 + i * 10,
            y: 4000,
            pressure: i * 4,
            tip: true,
            in_range: true,
            position_valid: true,
            ..Default::default()
        };
        engine.push(sample, f64::from(i) * 4.0).unwrap();
        let original = engine.tick(f64::from(i) * 4.0);
        let out = p.apply(original, b);
        assert_eq!(
            Frame {
                x: original.x,
                y: original.y,
                ..out
            },
            original
        );
        assert!(!out.virtual_tilt);
        assert_eq!((out.tilt_x, out.tilt_y), (0, 0));
        let report = ctl460_rust::hid::encode(out);
        assert_eq!(&report[8..10], &[0, 0]);
    }
}
#[test]
fn hover_and_contact_have_identical_constant_reduction_without_tip_jumps() {
    let b = PrecisionBounds::full();
    for percent in 10..=100 {
        let gain = percent as f64 / 100.0;
        let mut p = activated(gain, (7360, 4600), b);
        for contact in [false, true, false, true] {
            for (x, y) in [(8000, 6000), (12000, 8500), (3000, 1000), (7360, 4600)] {
                let out = p.apply(frame(x, y, contact), b);
                assert_eq!(
                    out.x,
                    (7360.0 + (f64::from(x) - 7360.0) * gain).round() as u16
                );
                assert_eq!(
                    out.y,
                    (4600.0 + (f64::from(y) - 4600.0) * gain).round() as u16
                );
                assert_eq!(out.contact, contact);
                assert_eq!(out.pressure, if contact { 0.4 } else { 0.0 });
                let same = p.apply(frame(x, y, !contact), b);
                assert_eq!((same.x, same.y), (out.x, out.y));
            }
        }
    }
}
#[test]
fn circles_close_and_preserve_heading_in_hover_and_contact() {
    let b = PrecisionBounds::full();
    for gain in [0.1, 0.25, 0.5, 0.9, 1.0] {
        for contact in [false, true] {
            let mut p = activated(gain, (3000, 4600), b);
            let mut first = None;
            for step in 0..=360 {
                let a = f64::from(step % 360).to_radians();
                let raw = frame(
                    (3000.0 + 800.0 * a.cos()).round() as u16,
                    (4600.0 + 800.0 * a.sin()).round() as u16,
                    contact,
                );
                let out = p.apply(raw, b);
                assert_eq!(
                    out.x,
                    (3000.0 + (f64::from(raw.x) - 3000.0) * gain).round() as u16
                );
                assert_eq!(
                    out.y,
                    (4600.0 + (f64::from(raw.y) - 4600.0) * gain).round() as u16
                );
                if step == 0 {
                    first = Some(out);
                }
                if step == 360 {
                    assert_eq!(Some(out), first);
                }
            }
        }
    }
}
#[test]
fn clutch_keeps_position_and_reaches_every_screen_corner_without_acceleration() {
    for (w, h) in [(1920, 1080), (1080, 1920), (5120, 1440)] {
        for aspect in [false, true] {
            let b = output_bounds(w, h, aspect);
            for gain in [0.1, 0.25, 0.5, 0.9] {
                for (tx, ty) in [(0, 0), (MAX_X, 0), (0, MAX_Y), (MAX_X, MAX_Y)] {
                    let mut p = activated(gain, (7360, 4600), b);
                    let mut last = frame(7360, 4600, false);
                    for _ in 0..24 {
                        p.apply(Frame::default(), b);
                        let enter = p.apply(frame(7360, 4600, false), b);
                        assert_eq!((enter.x, enter.y), (last.x, last.y));
                        last = p.apply(frame(tx, ty, false), b);
                    }
                    assert_eq!(
                        map_to_screen(last.x, last.y, w, h, aspect),
                        map_to_screen(tx, ty, w, h, aspect)
                    );
                    // At the clamped boundary the first reverse move must respond.
                    let rx = if tx == 0 { 100 } else { MAX_X - 100 };
                    let ry = if ty == 0 { 100 } else { MAX_Y - 100 };
                    let back = p.apply(frame(rx, ry, false), b);
                    assert!((i32::from(back.x) - i32::from(last.x)).abs() >= 9);
                    assert!((i32::from(back.y) - i32::from(last.y)).abs() >= 9);
                }
            }
        }
    }
}
#[test]
fn repeated_timer_frames_and_clutching_do_not_drift() {
    let b = PrecisionBounds::full();
    let mut p = activated(0.1, (7360, 4600), b);
    let end = p.apply(frame(7563, 4681, false), b);
    for _ in 0..10000 {
        assert_eq!(p.apply(frame(7563, 4681, false), b), end);
    }
    for _ in 0..100 {
        p.apply(Frame::default(), b);
        assert_eq!(p.apply(frame(4000, 3000, false), b), end);
    }
    let down = p.apply(frame(4000, 3000, true), b);
    assert_eq!((down.x, down.y), (end.x, end.y));
}
#[test]
fn live_settings_keep_toggle_update_correct_button_and_preserve_cursor() {
    let b = PrecisionBounds::full();
    for button in 0..2 {
        let mut c = config(0.5);
        let mut p = PrecisionHold::default();
        let mut states = [false; 2];
        states[button] = true;
        p.buttons(&c, states);
        p.buttons(&c, [false; 2]);
        p.apply(frame(2000, 4000, false), b);
        let end = p.apply(frame(3000, 5000, false), b);
        assert_eq!((end.x, end.y), (2500, 4500));
        c.streamline_amount = 30.0;
        p.reconfigure(&c);
        assert!(p.enabled());
        assert_eq!(p.apply(frame(3000, 5000, false), b), end);
        c.precision_gain[button] = 20.0;
        c.precision_gain[1 - button] = 90.0;
        p.reconfigure(&c);
        assert_eq!(p.apply(frame(3000, 5000, false), b), end);
        let reduced = p.apply(frame(4000, 6000, false), b);
        assert_eq!((reduced.x, reduced.y), (2700, 4700));
        if button == 0 {
            c.button1 = "Left click".into();
        } else {
            c.button2 = "Left click".into();
        }
        p.reconfigure(&c);
        assert!(!p.enabled());
        assert_eq!(
            p.apply(frame(4000, 6000, false), b),
            frame(4000, 6000, false)
        );
    }
    let mut off = PrecisionHold::default();
    off.reconfigure(&config(0.2));
    assert!(!off.enabled());
}
#[test]
fn contact_toggles_wait_for_lift_and_lift_keeps_the_last_ink_endpoint() {
    let b = PrecisionBounds::full();
    let c = config(0.25);
    let mut p = PrecisionHold::default();
    p.apply(frame(2000, 4000, true), b);
    toggle(&mut p, &c);
    assert_eq!(p.apply(frame(2400, 4000, true), b), frame(2400, 4000, true));
    let end = p.apply(frame(2400, 4000, false), b);
    assert_eq!(end.x, 2400);
    let hover = p.apply(frame(2800, 4000, false), b);
    assert_eq!(hover.x, 2500);
    let down = p.apply(frame(2800, 4000, true), b);
    assert_eq!(down.x, 2500);
    toggle(&mut p, &c);
    let end = p.apply(frame(3200, 4000, true), b);
    assert_eq!(end.x, 2600);
    let hover = p.apply(frame(3300, 4000, false), b);
    assert_eq!(hover.x, 3300);
    assert_eq!(lift_before_hover(end, hover).unwrap().x, 2600);
}

#[test]
fn circles_near_every_sensor_edge_keep_uniform_scale_in_hover_and_contact() {
    let b = PrecisionBounds::full();
    for contact in [false, true] {
        for gain in [0.1, 0.25, 0.5, 0.69, 0.9] {
            for (cx, cy) in [
                (550, 4600),
                (MAX_X - 550, 4600),
                (7360, 550),
                (7360, MAX_Y - 550),
                (550, 550),
                (MAX_X - 550, 550),
                (550, MAX_Y - 550),
                (MAX_X - 550, MAX_Y - 550),
            ] {
                let start = (cx + 450, cy);
                let mut p = activated(gain, (7360, 4600), b);
                p.apply(Frame::default(), b);
                let visible = p.apply(frame(start.0, start.1, false), b);
                let down = p.apply(frame(start.0, start.1, contact), b);
                assert_eq!((down.x, down.y), (visible.x, visible.y));
                for step in 0..=720 {
                    let a = f64::from(step % 360).to_radians();
                    let x = (f64::from(cx) + 450.0 * a.cos()).round() as u16;
                    let y = (f64::from(cy) + 450.0 * a.sin()).round() as u16;
                    let out = p.apply(frame(x, y, contact), b);
                    let expected = (
                        (f64::from(down.x) + (f64::from(x) - f64::from(start.0)) * gain).round()
                            as u16,
                        (f64::from(down.y) + (f64::from(y) - f64::from(start.1)) * gain).round()
                            as u16,
                    );
                    assert_eq!(
                        (out.x, out.y),
                        expected,
                        "gain={gain} circle={cx},{cy} step={step}"
                    );
                    assert_eq!(out.contact, contact);
                    assert_eq!(out.pressure, if contact { 0.4 } else { 0.0 });
                }
            }
        }
    }
}

#[test]
fn edge_motion_does_not_jump_on_contact_or_release() {
    let b = PrecisionBounds::full();
    for (x, y) in [
        (100, 100),
        (MAX_X - 100, 100),
        (100, MAX_Y - 100),
        (MAX_X - 100, MAX_Y - 100),
    ] {
        let mut p = activated(0.5, (7360, 4600), b);
        let hover = p.apply(frame(x, y, false), b);
        let down = p.apply(frame(x, y, true), b);
        assert_eq!((down.x, down.y), (hover.x, hover.y));
        let nx = if x < 7360 { x + 40 } else { x - 40 };
        let ny = if y < 4600 { y + 40 } else { y - 40 };
        let moved = p.apply(frame(nx, ny, true), b);
        assert_eq!(
            i32::from(moved.x) - i32::from(down.x),
            (i32::from(nx) - i32::from(x)) / 2
        );
        assert_eq!(
            i32::from(moved.y) - i32::from(down.y),
            (i32::from(ny) - i32::from(y)) / 2
        );
        let up = p.apply(frame(nx, ny, false), b);
        assert_eq!((up.x, up.y), (moved.x, moved.y));
        for _ in 0..100 {
            assert_eq!(p.apply(frame(nx, ny, false), b), up);
        }
    }
}
