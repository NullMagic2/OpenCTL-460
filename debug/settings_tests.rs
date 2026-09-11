//! Regression tests for safe CTL-460 collection selection and graphical pressure editing.
use ctl460_rust::{
    config::Config,
    device_selection::{automatic_index, is_pen_collection},
    pressure_editor::{drag, Handle},
};

#[cfg(windows)]
#[test]
fn handwriting_status_keeps_manual_enabled_when_pen_is_idle() {
    use ctl460_rust::{ipc::PenData, status::handwriting_summary};
    let manual = PenData::handwriting_mode_flags("on");
    assert_eq!(
        handwriting_summary(manual),
        "Handwriting: On (manual); stroke assistance idle"
    );
    assert_eq!(
        handwriting_summary(manual | 32),
        "Handwriting: On (manual); stroke assistance active"
    );
    let auto = PenData::handwriting_mode_flags("auto");
    assert!(handwriting_summary(auto | (85 << 8))
        .contains("Auto; stroke assistance idle; evidence 85/100"));
    assert!(handwriting_summary(auto | 32 | (85 << 8)).contains("stroke assistance active"));
    assert_eq!(
        handwriting_summary(PenData::handwriting_mode_flags("off")),
        "Handwriting: Off"
    );
    assert!(handwriting_summary(0).contains("mode unavailable"));
}

#[test]
fn aspect_mapping_reaches_all_edges_without_stretching_the_center() {
    use ctl460_rust::stroke::{map_to_screen, map_to_virtual_screen};
    for (width, height) in [(1920, 1080), (3440, 1440), (1200, 1920), (1600, 1000)] {
        for aspect in [false, true] {
            assert_eq!(map_to_screen(0, 0, width, height, aspect), (0, 0));
            assert_eq!(
                map_to_screen(14720, 9200, width, height, aspect),
                (width - 1, height - 1)
            );
            assert_eq!(map_to_virtual_screen(0, 0, width, height, aspect), (0, 0));
            assert_eq!(
                map_to_virtual_screen(14720, 9200, width, height, aspect),
                (14720, 9200)
            );
        }
        let center = map_to_screen(7360, 4600, width, height, true);
        let nearby = map_to_screen(7460, 4700, width, height, true);
        assert!(((nearby.0 - center.0) - (nearby.1 - center.1)).abs() <= 1);
        assert!((center.0 - width / 2).abs() <= 1 && (center.1 - height / 2).abs() <= 1);
    }
    assert_eq!(map_to_screen(0, 0, 1, 1, true), (0, 0));
}

#[test]
fn pencil_preset_keeps_current_mapping_and_shortcuts() {
    let current = Config {
        preserve_aspect: false,
        left_handed: true,
        backend: "hid".into(),
        button1: "Undo (Ctrl+Z)".into(),
        ..Config::default()
    };
    let preset = Config::parse(include_str!("../profiles/pencil_experimental.toml")).unwrap();
    let applied = current.drawing_profile(preset);
    assert!(!applied.preserve_aspect);
    assert!(applied.left_handed);
    assert_eq!(applied.backend, "hid");
    assert_eq!(applied.button1, current.button1);
    assert!(applied.virtual_tilt);
}

#[test]
fn tilt_does_not_move_the_reported_tip_and_apis_share_screen_position() {
    use ctl460_rust::{
        engine::Engine,
        protocol::Sample,
        stroke::{map_to_screen, map_to_virtual_screen},
    };
    let config = Config::default();
    let mut upright = Engine::new(config.clone()).unwrap();
    let mut tilted = Engine::new(Config {
        virtual_tilt: true,
        ..config
    })
    .unwrap();
    for i in 0..50 {
        let sample = Sample {
            x: 2000 + i * 31,
            y: 3000 + i * 7,
            pressure: 400,
            tip: true,
            in_range: true,
            position_valid: true,
            ..Default::default()
        };
        upright.push(sample, i as f64 * 4.0).unwrap();
        tilted.push(sample, i as f64 * 4.0).unwrap();
        let a = upright.tick(i as f64 * 4.0);
        let b = tilted.tick(i as f64 * 4.0);
        assert_eq!((a.x, a.y), (b.x, b.y));
        for (width, height) in [(1920, 1080), (3840, 2160), (1200, 1920)] {
            for aspect in [false, true] {
                let pixel = map_to_screen(b.x, b.y, width, height, aspect);
                let virtual_point = map_to_virtual_screen(b.x, b.y, width, height, aspect);
                let through_wintab =
                    map_to_screen(virtual_point.0, virtual_point.1, width, height, false);
                assert!(
                    (pixel.0 - through_wintab.0).abs() <= 1
                        && (pixel.1 - through_wintab.1).abs() <= 1
                );
            }
        }
    }
}
#[test]
fn mouse_first_enumeration_selects_pen_and_rejects_ambiguity() {
    assert_eq!(
        automatic_index(&[(0, 1, 2), (0, 13, 1), (1, 0xff00, 1)]).unwrap(),
        1
    );
    assert_eq!(
        automatic_index(&[(1, 0xff00, 1), (0, 13, 1), (0, 1, 2)]).unwrap(),
        1
    );
    assert!(!is_pen_collection(0, 1, 2));
    assert!(automatic_index(&[(0, 1, 2), (1, 0xff00, 1)]).is_err());
    assert!(automatic_index(&[(0, 13, 1), (0, 13, 1)]).is_err());
    assert!(automatic_index(&[]).is_err());
}
#[test]
fn dragged_response_matches_actual_driver_curve_and_survives_save() {
    let mut c = Config {
        floor: 10,
        ceiling: 850,
        gain: 1.25,
        ..Config::default()
    };
    drag(&mut c, Handle::Response, 512.0 / 1023.0, 0.85);
    assert!((c.curve(512) - 0.85).abs() < 0.00001);
    let saved = Config::parse(&toml::to_string(&c).unwrap()).unwrap();
    assert_eq!(saved.curve(512), c.curve(512));
    let mut previous = 0.0;
    for raw in 0..=1023 {
        let p = c.curve(raw);
        assert!(p >= previous && p <= 1.0);
        previous = p;
    }
}
#[test]
fn endpoint_drags_never_cross_or_break_calibration() {
    let mut c = Config::default();
    for input in [-10.0, 0.0, 0.25, 1.0, 10.0] {
        drag(&mut c, Handle::Zero, input, 0.0);
        c.validate().unwrap();
        drag(&mut c, Handle::Full, 1.0 - input, 1.0);
        c.validate().unwrap();
        assert_eq!(c.curve(c.floor), 0.0);
    }
}

#[test]
fn node_endpoints_drag_the_input_range_and_keep_the_shape_valid() {
    use ctl460_rust::pressure_profiles::{reset_linear, Shape};
    let mut c = Config::default();
    reset_linear(&mut c);
    let nodes = c.pressure_nodes.clone();
    drag(&mut c, Handle::Node(2), 0.8, 0.7);
    assert_eq!(c.ceiling, 818);
    drag(&mut c, Handle::Node(0), 0.1, 0.2);
    assert_eq!(c.floor, 102);
    assert_eq!(c.pressure_nodes, nodes);
    assert_eq!(c.curve(c.floor), 0.0);
    assert_eq!(c.curve(c.ceiling), 1.0);
    c.validate().unwrap();
    let restored = Config::parse(&toml::to_string(&c).unwrap()).unwrap();
    assert_eq!(Shape::from_config(&c), Shape::from_config(&restored));
    drag(&mut c, Handle::Node(2), -1.0, 1.0);
    drag(&mut c, Handle::Node(0), 2.0, 0.0);
    c.validate().unwrap();
    assert!(c.floor < c.ceiling);
}

#[test]
fn every_drawing_preset_starts_with_a_full_range_diagonal() {
    // Drawing/handwriting selection must not reintroduce the former shortened, curved baseline.
    for text in [
        include_str!("../profiles/natural.toml"),
        include_str!("../profiles/handwriting.toml"),
        include_str!("../profiles/pencil_experimental.toml"),
        include_str!("../profiles/smooth_inking.toml"),
        include_str!("../profiles/raw.toml"),
    ] {
        let c = Config::parse(text).unwrap();
        for raw in 0..=1023 {
            assert!((c.curve(raw) - f64::from(raw) / 1023.0).abs() < 1e-12);
        }
    }
}
#[test]
fn edge_and_nonfinite_gestures_remain_finite_and_bounded() {
    let mut c = Config::default();
    for input in [0.0, 1.0, -3.0, 10.0, f64::NAN, f64::INFINITY] {
        for output in [0.0, 1.0, -3.0, 10.0, f64::NAN] {
            drag(&mut c, Handle::Response, input, output);
            c.validate().unwrap();
        }
    }
}

#[test]
fn eraser_assignment_is_momentary_and_can_use_either_button() {
    let config = Config {
        button1: "Erase (hold)".into(),
        button2: "Erase (hold)".into(),
        ..Config::default()
    };
    config.validate().unwrap();
    assert!(!config.button_eraser([false, false]));
    assert!(config.button_eraser([true, false]));
    assert!(config.button_eraser([false, true]));
    assert!(config.button_eraser([true, true]));
    let shortcut = Config {
        button1: "Eraser (E)".into(),
        ..Config::default()
    };
    assert!(!shortcut.button_eraser([true, false]));
}

#[test]
fn double_click_distance_round_trips_and_survives_presets() {
    let c = Config::parse("double_click_distance = 24").unwrap();
    assert_eq!(
        Config::parse(&toml::to_string(&c).unwrap())
            .unwrap()
            .double_click_distance,
        24
    );
    assert_eq!(
        c.drawing_profile(Config::default()).double_click_distance,
        24
    );
    assert_eq!(Config::default().double_click_distance, 0);
    assert!(Config::parse("double_click_distance = 41").is_err());
}

#[test]
fn double_tap_assistance_anchors_only_nearby_quick_second_taps() {
    use ctl460_rust::{double_click::DoubleClick, engine::Frame};
    let frame = |x, down| Frame {
        x,
        y: 2000,
        in_range: true,
        contact: down,
        pressure: if down { 0.5 } else { 0.0 },
        ..Frame::default()
    };
    let mut taps = DoubleClick::default();
    let first = frame(1000, true);
    assert_eq!(taps.apply(first, (100, 200), 0, 20, 500), first);
    taps.apply(frame(1000, false), (100, 200), 50, 20, 500);
    assert_eq!(
        taps.apply(frame(1100, true), (110, 200), 150, 20, 500).x,
        1000
    );
    let lifted = taps.apply(frame(1100, false), (110, 200), 200, 20, 500);
    assert_eq!(lifted.x, 1000);
    assert!(!lifted.contact);
    assert_eq!(lifted.pressure, 0.0);
    // Consumed pair: third tap begins freely.
    assert_eq!(
        taps.apply(frame(1120, true), (112, 200), 250, 20, 500).x,
        1120
    );
    taps.apply(frame(1120, false), (112, 200), 300, 20, 500);
    assert_eq!(
        taps.apply(frame(1130, true), (113, 200), 900, 20, 500).x,
        1130
    );
}

#[test]
fn double_tap_assistance_releases_on_drag_and_never_modifies_eraser_or_off() {
    use ctl460_rust::{double_click::DoubleClick, engine::Frame};
    let mut taps = DoubleClick::default();
    let pen = Frame {
        x: 1000,
        y: 2000,
        in_range: true,
        contact: true,
        pressure: 0.6,
        ..Frame::default()
    };
    taps.apply(pen, (100, 200), 0, 20, 500);
    taps.apply(
        Frame {
            contact: false,
            ..pen
        },
        (100, 200),
        50,
        20,
        500,
    );
    taps.apply(Frame { x: 1100, ..pen }, (110, 200), 100, 20, 500);
    let drag = Frame { x: 1600, ..pen };
    assert_eq!(taps.apply(drag, (160, 200), 120, 20, 500), drag);
    let eraser = Frame {
        eraser: true,
        ..pen
    };
    assert_eq!(taps.apply(eraser, (100, 200), 130, 20, 500), eraser);
    assert_eq!(taps.apply(pen, (100, 200), 140, 0, 500), pen);
    assert_eq!(
        taps.apply(
            Frame {
                in_range: false,
                ..pen
            },
            (100, 200),
            150,
            20,
            500
        )
        .x,
        1000
    );
}

#[test]
fn eraser_tool_transition_lifts_before_new_pressure_and_encodes_hid_flags() {
    use ctl460_rust::{
        engine::Frame,
        hid,
        lifecycle::{transition, Kind},
    };
    let pen = Frame {
        x: 1000,
        y: 2000,
        contact: true,
        in_range: true,
        pressure: 0.7,
        ..Frame::default()
    };
    let eraser = Frame {
        eraser: true,
        ..pen
    };
    let events = transition(pen, eraser);
    assert_eq!(
        events.iter().map(|e| e.kind).collect::<Vec<_>>(),
        [Kind::Up, Kind::Exit, Kind::Down]
    );
    assert!(!events[0].frame.eraser);
    assert_eq!(events[0].frame.pressure, 0.0);
    assert_eq!(hid::encode(eraser)[1] & 0x0c, 0x0c);
    assert_eq!(
        hid::encode(Frame {
            contact: false,
            ..eraser
        })[1]
            & 0x0c,
        0x04
    );
    assert_eq!(
        hid::encode(Frame {
            in_range: false,
            ..eraser
        })[1],
        0
    );
}

#[test]
fn pressure_nodes_are_smooth_monotone_and_editable_without_crossing() {
    use ctl460_rust::pressure_editor::{drag, Handle};
    use ctl460_rust::pressure_profiles::{add, remove, Node};
    let mut c = Config {
        floor: 0,
        ceiling: 1023,
        gain: 1.0,
        pressure_nodes: vec![
            Node { x: 0.0, y: 0.0 },
            Node { x: 0.2, y: 0.6 },
            Node { x: 0.5, y: 0.7 },
            Node { x: 1.0, y: 1.0 },
        ],
        ..Config::default()
    };
    c.validate().unwrap();
    let mut previous = 0.0;
    for i in 0..=10000 {
        let y = c.curve_at(f64::from(i) * 1023.0 / 10000.0);
        assert!((0.0..=1.0).contains(&y));
        assert!(y + 1e-12 >= previous);
        previous = y;
    }
    // Matching left/right derivatives at each interior node: no piecewise-linear corners.
    for n in &c.pressure_nodes[1..3] {
        let x = n.x * 1023.0;
        let h = 0.0001;
        let left = (c.curve_at(x) - c.curve_at(x - h)) / h;
        let right = (c.curve_at(x + h) - c.curve_at(x)) / h;
        assert!((left - right).abs() < 1e-6);
    }
    let i = add(&mut c, 0.35).unwrap();
    assert_eq!(c.pressure_nodes.len(), 5);
    drag(&mut c, Handle::Node(i), 1.0, 0.0);
    c.validate().unwrap();
    assert!(c.pressure_nodes[i].x < c.pressure_nodes[i + 1].x);
    assert!(!remove(&mut c, 0));
    assert!(remove(&mut c, i));
    c.validate().unwrap();
    c.pressure_nodes[1].y = 2.0;
    assert!(c.validate().is_err());
}

#[test]
fn adding_pressure_nodes_converts_only_visible_handles_then_adds_one() {
    use ctl460_rust::pressure_profiles::{add, seed_at};
    let mut c = Config::default();
    assert!(add(&mut c, f64::NAN).is_none());
    assert!(c.pressure_nodes.is_empty());
    seed_at(&mut c, 0.37);
    assert_eq!(c.pressure_nodes.len(), 3);
    assert_eq!(c.pressure_nodes[1].x, 0.37);
    assert_eq!(c.pressure_nodes[1].y, 0.37_f64.powf(c.gamma));
    let original = c.pressure_nodes.clone();
    for x in [0.6, 0.8, 0.2] {
        let count = c.pressure_nodes.len();
        add(&mut c, x).unwrap();
        assert_eq!(c.pressure_nodes.len(), count + 1);
        assert!(original.iter().all(|n| c.pressure_nodes.contains(n)));
        c.validate().unwrap();
    }
    let before = c.pressure_nodes.clone();
    assert!(add(&mut c, 0.6).is_none());
    assert_eq!(c.pressure_nodes, before);
    let mut legacy = Config::default();
    add(&mut legacy, 0.25).unwrap();
    assert_eq!(legacy.pressure_nodes.len(), 4);
}

#[test]
fn reset_curve_restores_linear_full_range_and_preserves_other_settings() {
    use ctl460_rust::pressure_profiles::{reset_linear, seed};
    let mut c = Config {
        floor: 80,
        ceiling: 800,
        gamma: 0.3,
        gain: 1.7,
        left_handed: true,
        ..Config::default()
    };
    seed(&mut c);
    c.pressure_nodes[1].y = 0.9;
    let before = c.clone();
    reset_linear(&mut c);
    assert_eq!(c.pressure_nodes.len(), 3);
    let default = Config::default();
    for raw in 0..=1023 {
        assert!((c.curve(raw) - f64::from(raw) / 1023.0).abs() < 1e-12);
        assert!(
            (default.curve(raw) - c.curve(raw)).abs() < 1e-12,
            "Default and Reset must have the same linear response at {raw}"
        );
    }
    // Restoring only the pressure fields should recover every original setting.
    let mut restored = c.clone();
    ctl460_rust::pressure_profiles::Shape::from_config(&before).apply(&mut restored);
    assert_eq!(
        toml::to_string(&restored).unwrap(),
        toml::to_string(&before).unwrap()
    );
    c.validate().unwrap();
}

#[test]
fn named_pressure_presets_cannot_overwrite_default_and_persist() {
    use ctl460_rust::pressure_profiles::{Library, Shape};
    let shape = Shape::from_config(&Config::default());
    let mut library = Library::default();
    assert!(library.add("Default", shape.clone()).is_err());
    assert!(library.remove("Default").is_err());
    library.add("Soft pencil", shape.clone()).unwrap();
    assert!(library.add("soft PENCIL", shape.clone()).is_err());
    assert!(library.add(" ", shape.clone()).is_err());
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("debug/generated/preset-unit.toml");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    library.save(&path).unwrap();
    let restored = Library::load(&path).unwrap();
    assert_eq!(restored.presets[0].shape, shape);
    library.remove("Soft pencil").unwrap();
    library.save(&path).unwrap();
    assert!(Library::load(&path).unwrap().presets.is_empty());
}

#[test]
fn equal_measured_pressure_is_independent_of_speed_direction_and_tilt() {
    // Identical force histories must produce identical output at different drawing speeds.
    use ctl460_rust::{engine::Engine, protocol::Sample};
    for writing in ["off", "on"] {
        for tilt in [false, true] {
            let c = Config {
                handwriting_mode: writing.into(),
                virtual_tilt: tilt,
                ..Config::default()
            };
            let mut slow = Engine::new(c.clone()).unwrap();
            let mut fast = Engine::new(c).unwrap();
            for i in 0..100u16 {
                let s = Sample {
                    x: 3000 + i * 5,
                    y: 4000,
                    pressure: 40 + (i * 17) % 850,
                    tip: true,
                    in_range: true,
                    position_valid: true,
                    ..Default::default()
                };
                let now = f64::from(i) * 8.0;
                slow.push(s, now).unwrap();
                fast.push(
                    Sample {
                        x: 3000 + i * 80,
                        y: 2000 + i * 30,
                        ..s
                    },
                    now,
                )
                .unwrap();
                assert_eq!(
                    slow.tick(now).virtual_pressure(),
                    fast.tick(now).virtual_pressure()
                );
            }
        }
    }
}

#[test]
fn measured_pressure_preserves_light_tips_full_range_and_deliberate_changes() {
    use ctl460_rust::{engine::Engine, protocol::Sample};
    let sample = |force| Sample {
        x: 4000,
        y: 4000,
        pressure: force,
        tip: force > 0,
        in_range: true,
        position_valid: true,
        ..Default::default()
    };
    let mut e = Engine::new(Config::default()).unwrap();
    // Each fresh dot uses actual force, not a synthesized midpoint or a startup envelope.
    for (i, force) in [4, 64, 500, 1023].into_iter().enumerate() {
        let now = i as f64 * 100.0;
        e.push(sample(force), now).unwrap();
        assert!((e.tick(now).pressure - f64::from(force) / 1023.0).abs() < 1e-12);
        e.push(sample(0), now + 8.0).unwrap();
        assert_eq!(e.tick(now + 8.0).pressure, 0.0);
    }
    // Deliberate pressure changes must work even with a completely stationary pen.
    for i in 0..30 {
        e.push(sample(200), 400.0 + f64::from(i) * 4.0).unwrap();
    }
    let light = e.tick(516.0).pressure;
    for i in 0..30 {
        e.push(sample(850), 520.0 + f64::from(i) * 4.0).unwrap();
    }
    assert!(e.tick(636.0).pressure > light + 0.5);
    for (i, force) in [400, 200, 80, 20, 4].into_iter().enumerate() {
        e.push(sample(force), 640.0 + i as f64 * 8.0).unwrap();
    }
    assert!(
        e.tick(676.0).pressure < 0.02,
        "Measured light contact did not taper before lift"
    );
    e.push(sample(0), 680.0).unwrap();
    assert_eq!(e.tick(680.0).pressure, 0.0);
}

#[test]
fn pressure_smoothing_reduces_small_force_jitter_and_old_settings_migrate() {
    use ctl460_rust::{engine::Engine, protocol::Sample};
    let c = Config::parse("speed_pressure = true").unwrap();
    assert!(!c.speed_pressure);
    assert!(!toml::to_string(&c).unwrap().contains("speed_pressure"));
    let mut e = Engine::new(c).unwrap();
    let mut minimum = 1.0f64;
    let mut maximum = 0.0f64;
    for i in 0..100 {
        let s = Sample {
            x: 4000,
            y: 4000,
            pressure: if i % 2 == 0 { 495 } else { 505 },
            tip: true,
            in_range: true,
            position_valid: true,
            ..Default::default()
        };
        let now = f64::from(i) * 4.0;
        e.push(s, now).unwrap();
        let p = e.tick(now + 4.0).pressure;
        if i > 10 {
            minimum = minimum.min(p);
            maximum = maximum.max(p);
        }
    }
    assert!(
        maximum - minimum < 0.8 * 10.0 / 1023.0,
        "Filter failed to reduce sensor jitter"
    );
}

#[test]
fn virtual_mapping_keeps_subpixel_motion_for_slow_smoothed_curves() {
    use ctl460_rust::stroke::{map_to_screen, map_to_screen_precise, map_to_virtual_screen};
    for (width, height) in [(1920, 1080), (3840, 2160), (1280, 1024)] {
        for aspect in [false, true] {
            let mut points = std::collections::BTreeSet::new();
            for x in 7200..=7240 {
                let (vx, vy) = map_to_virtual_screen(x, 4500, width, height, aspect);
                points.insert((vx, vy));
                let (px, py) = map_to_screen_precise(x, 4500, width, height, aspect);
                let rx = f64::from(vx) / 14720.0 * f64::from(width - 1);
                let ry = f64::from(vy) / 9200.0 * f64::from(height - 1);
                assert!((px - rx).abs() <= f64::from(width - 1) / 14720.0 / 2.0 + 1e-8);
                assert!((py - ry).abs() <= f64::from(height - 1) / 9200.0 / 2.0 + 1e-8);
                assert_eq!(
                    map_to_screen(x, 4500, width, height, aspect),
                    (px.round() as i32, py.round() as i32)
                );
            }
            assert!(
                points.len() >= 40,
                "slow movements were collapsed to whole pixels"
            );
        }
    }
}
