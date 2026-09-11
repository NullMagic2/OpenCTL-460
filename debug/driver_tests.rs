//! Hardware-free regression tests for decoding, pressure, stroke geometry, HID and pointer edges.
//! Fixtures are synthetic protocol examples, never represented as captures from a physical tablet.

use ctl460_rust::{
    config::Config,
    engine::{Engine, Frame},
    hid,
    lifecycle::{transition, Kind},
    protocol::{self, DecodeError, ReportFormat, Sample},
    stroke::{map_to_screen, StrokeFilter},
};

#[test]
fn tablet_mode_requires_confirmation_and_retries_transient_failures() {
    let mut replies = [Err("busy".into()), Ok(vec![2, 0]), Ok(vec![2, 2])].into_iter();
    assert!(ctl460_rust::tablet_mode::initialize(|| replies.next().unwrap()).is_ok());
    assert!(replies.next().is_none());
    let mut attempts = 0;
    let result = ctl460_rust::tablet_mode::initialize(|| {
        attempts += 1;
        Ok(vec![2])
    });
    assert_eq!(attempts, 3);
    assert!(result.unwrap_err().contains("unexpected mode response"));
    assert!(ctl460_rust::tablet_mode::initialize(|| Ok(vec![3, 2])).is_err());
}

fn linear() -> Config {
    Config {
        // Disable timing filters to isolate pressure calibration and interpolation fixtures.
        ceiling: 1023,
        gamma: 1.0,
        smoothing_ms: 0.0,
        interpolation_ms: 0.0,
        stroke_smoothing: false,
        ..Default::default()
    }
}
fn sample(pressure: u16) -> Sample {
    Sample {
        x: 1000,
        y: 1000,
        pressure,
        in_range: true,
        position_valid: true,
        tip: pressure > 0,
        ..Default::default()
    }
}
fn packet(flags: u8, x: u16, y: u16, p: u16) -> [u8; 9] {
    let [xl, xh] = x.to_le_bytes();
    let [yl, yh] = y.to_le_bytes();
    let [pl, ph] = p.to_le_bytes();
    [2, flags, xl, xh, yl, yh, pl, ph, 0]
}

#[test]
fn native_report_full_range_and_buttons() {
    let s = protocol::decode(&packet(0xef, 14720, 9200, 1023), ReportFormat::Native9).unwrap();
    assert_eq!((s.x, s.y, s.pressure), (14720, 9200, 1023));
    assert!(s.tip && s.barrel && s.second_button && s.eraser && s.in_range);
}
#[test]
fn release_ignores_stale_payload() {
    let s = protocol::decode(&packet(0, 65535, 65535, 65535), ReportFormat::Native9).unwrap();
    assert!(!s.in_range && !s.tip);
    assert_eq!(s.pressure, 0);
}
#[test]
fn malformed_reports_are_rejected_without_panics() {
    for length in 0..128 {
        let bytes = vec![255; length];
        assert!(protocol::decode(&bytes, ReportFormat::Native9).is_err());
    }
    assert_eq!(
        protocol::decode(&packet(0xe1, 14721, 0, 0), ReportFormat::Native9),
        Err(DecodeError::Bounds)
    );
    assert_eq!(
        protocol::decode(&packet(0xe1, 0, 0, 1024), ReportFormat::Native9),
        Err(DecodeError::Bounds)
    );
}
#[test]
fn legacy_wrapper_requires_explicit_format() {
    let mut bytes = vec![0];
    bytes.extend(packet(0xe1, 4, 5, 6));
    bytes.push(0);
    assert!(protocol::decode(&bytes, ReportFormat::Native9).is_err());
    assert_eq!(
        protocol::decode(&bytes, ReportFormat::Wacom11)
            .unwrap()
            .pressure,
        6
    );
}
#[test]
fn not_ready_cannot_draw() {
    let s = protocol::decode(&packet(0xc1, 100, 100, 500), ReportFormat::Native9).unwrap();
    assert!(!s.tip);
    assert_eq!(s.pressure, 0);
}
#[test]
fn monotonic_curve_for_every_sensor_value() {
    for gamma in [0.2, 0.85, 1.0, 4.0] {
        let c = Config {
            gamma,
            ..Config::default()
        };
        let mut previous = 0.0;
        for raw in 0..=1023 {
            let p = c.curve(raw);
            assert!(p >= previous && p <= 1.0);
            previous = p;
        }
    }
}
#[test]
fn calibration_and_gain_are_bounded() {
    let c = Config {
        floor: 10,
        ceiling: 800,
        gain: 2.0,
        ..Config::default()
    };
    assert_eq!(c.curve(0), 0.0);
    assert_eq!(c.curve(10), 0.0);
    assert_eq!(c.curve(1023), 1.0);
    assert!(c.curve(400) > linear().curve(400));
}
#[test]
fn output_is_exactly_4098_values_with_endpoints() {
    let values: std::collections::HashSet<_> = (0..=4097)
        .map(|n| {
            Frame {
                pressure: f64::from(n) / 4097.0,
                ..Default::default()
            }
            .virtual_pressure()
        })
        .collect();
    assert_eq!(values.len(), 4098);
    assert!(values.contains(&0) && values.contains(&4097));
}
#[test]
fn simple_remapping_cannot_invent_sensor_levels() {
    let values: std::collections::HashSet<_> = (0..=1023)
        .map(|n| {
            Frame {
                pressure: f64::from(n) / 1023.0,
                ..Default::default()
            }
            .virtual_pressure()
        })
        .collect();
    assert_eq!(values.len(), 1024);
}
#[test]
fn interpolation_produces_fractional_intermediate_values() {
    let mut e = Engine::new(Config {
        interpolation_ms: 8.0,
        ..linear()
    })
    .unwrap();
    e.push(sample(3), 0.0).unwrap();
    e.push(sample(512), 8.0).unwrap();
    let a = e.tick(10.0);
    let b = e.tick(12.0);
    let c = e.tick(16.0);
    assert!(0.0 < a.pressure && a.pressure < b.pressure && b.pressure < c.pressure);
    assert!((b.pressure - (515.0 / 1023.0) * 0.5).abs() < 1e-10);
}
#[test]
fn release_bypasses_all_smoothing() {
    let mut e = Engine::new(Config::default()).unwrap();
    e.push(sample(900), 0.0).unwrap();
    e.tick(8.0);
    e.push(
        Sample {
            tip: false,
            ..sample(800)
        },
        9.0,
    )
    .unwrap();
    let f = e.tick(9.0);
    assert!(!f.contact);
    assert_eq!(f.pressure, 0.0);
    assert_eq!(e.tick(12.0).pressure, 0.0);
}
#[test]
fn proximity_loss_and_timeout_release() {
    let mut e = Engine::new(linear()).unwrap();
    e.push(sample(900), 0.0).unwrap();
    assert!(e.tick(99.0).contact);
    assert!(!e.tick(100.0).contact);
    e.push(sample(900), 200.0).unwrap();
    e.push(Sample::default(), 201.0).unwrap();
    let f = e.tick(201.0);
    assert!(!f.in_range);
    assert_eq!(f.pressure, 0.0);
}
#[test]
fn hysteresis_prevents_threshold_chatter() {
    let mut e = Engine::new(linear()).unwrap();
    for (t, p, expected) in [
        (0.0, 2, false),
        (1.0, 3, true),
        (2.0, 2, true),
        (3.0, 1, false),
        (4.0, 2, false),
    ] {
        e.push(sample(p), t).unwrap();
        assert_eq!(e.tick(t).contact, expected);
    }
}
#[test]
fn hover_does_not_create_contact_even_with_pressure() {
    let mut e = Engine::new(linear()).unwrap();
    e.push(
        Sample {
            tip: false,
            ..sample(1023)
        },
        0.0,
    )
    .unwrap();
    assert!(!e.tick(0.0).contact);
    assert_eq!(e.tick(0.0).pressure, 0.0);
}
#[test]
fn stale_stroke_state_does_not_leak_to_next_stroke() {
    let mut e = Engine::new(Config {
        interpolation_ms: 8.0,
        ..linear()
    })
    .unwrap();
    e.push(sample(1000), 0.0).unwrap();
    e.tick(8.0);
    e.push(sample(5), 200.0).unwrap();
    assert!((e.tick(200.0).pressure - 5.0 / 1023.0).abs() < 1e-10);
}
#[test]
fn invalid_config_is_rejected() {
    for text in [
        "gamma = nan",
        "gain = inf",
        "floor = 20\nceiling = 20",
        "output_hz = 0",
        "press_on = 1\npress_off = 2",
        "interpolation_ms = -1",
        "unknown = 4",
        "stroke_min_cutoff_hz = 0",
    ] {
        assert!(Config::parse(text).is_err(), "{text}");
    }
}
#[test]
fn invalid_report_time_is_rejected() {
    let mut e = Engine::new(linear()).unwrap();
    e.push(sample(10), 10.0).unwrap();
    for t in [9.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(e.push(sample(10), t).is_err());
    }
}
#[test]
fn pen_down_up_exit_has_valid_order() {
    let down = Frame {
        in_range: true,
        contact: true,
        pressure: 0.5,
        ..Default::default()
    };
    let events = transition(Frame::default(), down);
    assert_eq!(events[0].kind, Kind::Down);
    assert!(events[0].new_pointer);
    let events = transition(down, Frame::default());
    assert_eq!(
        events.iter().map(|e| e.kind).collect::<Vec<_>>(),
        [Kind::Up, Kind::Exit]
    );
    assert!(events
        .iter()
        .all(|e| !e.frame.contact && e.frame.pressure == 0.0));
}
#[test]
fn tool_flip_ends_old_stroke_first() {
    let down = Frame {
        in_range: true,
        contact: true,
        pressure: 0.5,
        ..Default::default()
    };
    let events = transition(
        down,
        Frame {
            eraser: true,
            ..down
        },
    );
    assert_eq!(
        events.iter().map(|e| e.kind).collect::<Vec<_>>(),
        [Kind::Up, Kind::Exit, Kind::Down]
    );
    assert!(events[2].new_pointer);
}
#[test]
fn hover_move_after_release_uses_original_lift_location() {
    let down = Frame {
        x: 100,
        y: 100,
        in_range: true,
        contact: true,
        ..Default::default()
    };
    let events = transition(
        down,
        Frame {
            x: 900,
            contact: false,
            ..down
        },
    );
    assert_eq!(events[0].kind, Kind::Up);
    assert_eq!(events[0].frame.x, 100);
    assert_eq!(events[1].kind, Kind::Update);
    assert_eq!(events[1].frame.x, 900);
}
#[test]
fn hid_report_matches_virtual_range_and_release() {
    let f = Frame {
        x: 14720,
        y: 9200,
        pressure: 1.0,
        in_range: true,
        contact: true,
        ..Default::default()
    };
    assert_eq!(hid::encode(f), [1, 17, 128, 57, 240, 35, 1, 16, 0, 0]);
    assert_eq!(
        &hid::encode(Frame {
            in_range: false,
            ..f
        })[6..8],
        &[0, 0]
    );
    assert!(hid::REPORT_DESCRIPTOR
        .windows(3)
        .any(|w| w == [0x26, 0x01, 0x10]));
}
#[test]
fn full_ink_range_is_separate_from_virtual_range() {
    let f = Frame {
        pressure: 1.0,
        ..Default::default()
    };
    assert_eq!(f.ink_pressure(), 1024);
    assert_eq!(f.virtual_pressure(), 4097);
}
#[test]
fn aspect_mapping_preserves_circles() {
    let a = map_to_screen(1000, 1000, 1920, 1080, true);
    let b = map_to_screen(2000, 2000, 1920, 1080, true);
    assert!(((b.0 - a.0) - (b.1 - a.1)).abs() <= 1);
    assert_eq!(map_to_screen(14720, 9200, 1920, 1080, false), (1919, 1079));
}
#[test]
fn stroke_filter_reduces_stationary_jitter() {
    let mut f = StrokeFilter::default();
    let mut filtered_energy = 0.0;
    for i in 0..200 {
        let x = if i % 2 == 0 { 1002 } else { 998 };
        let out = f.update(x, 1000, 8.0, 16.0, 0.4);
        if i > 20 {
            filtered_energy += (f64::from(out.0) - 1000.0).powi(2);
        }
    }
    assert!(filtered_energy < 179.0 * 4.0);
}
#[test]
fn adaptive_stroke_filter_tracks_fast_motion_better() {
    let mut adaptive = StrokeFilter::default();
    let mut fixed = StrokeFilter::default();
    adaptive.update(0, 0, 8.0, 16.0, 0.4);
    fixed.update(0, 0, 8.0, 16.0, 0.0);
    let a = adaptive.update(1000, 1000, 8.0, 16.0, 0.4);
    let b = fixed.update(1000, 1000, 8.0, 16.0, 0.0);
    assert!(a.0 > b.0 && a.0 <= 1000);
}
#[test]
fn reset_keeps_separate_letters_disconnected() {
    let mut f = StrokeFilter::default();
    f.update(100, 100, 8.0, 16.0, 0.4);
    f.reset();
    assert_eq!(f.update(9000, 8000, 8.0, 16.0, 0.4), (9000, 8000));
}
#[test]
fn smoothing_never_overshoots_pressure_bounds() {
    let mut e = Engine::new(Config::default()).unwrap();
    let mut seed = 123u32;
    for i in 0..10000 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        e.push(sample((seed % 1024) as u16), f64::from(i) * 8.0)
            .unwrap();
        let f = e.tick(f64::from(i) * 8.0 + 4.0);
        assert!((0.0..=1.0).contains(&f.pressure));
    }
}

#[test]
fn a_short_dot_has_pressure_on_the_first_report() {
    let mut e = Engine::new(Config::default()).unwrap();
    e.push(sample(80), 0.0).unwrap();
    let f = e.tick(0.0);
    assert!(f.contact && f.ink_pressure() > 0);
    e.push(Sample::default(), 1.0).unwrap();
    assert!(!e.tick(1.0).contact);
}

#[test]
fn all_shipped_profiles_validate_and_roundtrip() {
    for text in [
        include_str!("../profiles/natural.toml"),
        include_str!("../profiles/handwriting.toml"),
        include_str!("../profiles/raw.toml"),
        include_str!("../profiles/pencil_experimental.toml"),
        include_str!("../profiles/smooth_inking.toml"),
    ] {
        let c = Config::parse(text).unwrap();
        let saved = toml::to_string(&c).unwrap();
        assert_eq!(Config::parse(&saved).unwrap().button1, c.button1);
    }
}

#[test]
fn left_handed_mode_rotates_both_axes() {
    let mut e = Engine::new(Config {
        left_handed: true,
        ..linear()
    })
    .unwrap();
    e.push(sample(100), 0.0).unwrap();
    let f = e.tick(0.0);
    assert_eq!((f.x, f.y), (13720, 8200));
}

#[test]
fn simulated_tilt_is_opt_in_and_bounded() {
    for enabled in [false, true] {
        let mut e = Engine::new(Config {
            virtual_tilt: enabled,
            ..linear()
        })
        .unwrap();
        for i in 0..50 {
            e.push(
                Sample {
                    x: 1000 + i * 100,
                    ..sample(100)
                },
                f64::from(i) * 8.0,
            )
            .unwrap();
            let f = e.tick(f64::from(i) * 8.0);
            assert_eq!(f.virtual_tilt, enabled);
            assert!(f.tilt_x.abs() <= 50 && f.tilt_y.abs() <= 50);
            if !enabled {
                assert_eq!((f.tilt_x, f.tilt_y), (0, 0));
            }
        }
    }
}

#[test]
fn tilt_holds_at_rest_and_resets_between_strokes() {
    let mut t = ctl460_rust::tilt::VirtualTilt::default();
    t.update(100, 100, 0.2, 8.0, 50.0);
    let angle = t.update(500, 100, 0.2, 8.0, 50.0);
    assert_ne!(angle, (0, 0));
    assert_eq!(t.update(500, 100, 0.2, 8.0, 50.0), angle);
    t.reset();
    assert_eq!(t.update(500, 100, 0.2, 8.0, 50.0), (0, 0));
}

#[test]
fn stroke_reversal_does_not_flip_tilt_hemisphere() {
    let mut t = ctl460_rust::tilt::VirtualTilt::default();
    t.update(100, 100, 0.2, 8.0, 50.0);
    let a = t.update(500, 100, 0.2, 8.0, 50.0);
    let b = t.update(100, 100, 0.2, 8.0, 50.0);
    assert_eq!(a.1.signum(), b.1.signum());
}

#[test]
fn arbitrary_shortcut_strings_are_rejected() {
    assert!(Config::parse("button1 = 'Execute arbitrary program'").is_err());
    for action in ctl460_rust::config::BUTTON_ACTIONS {
        let c = Config {
            button1: action.to_string(),
            ..Config::default()
        };
        assert!(c.validate().is_ok());
    }
}

#[test]
fn kernel_wire_validator_rejects_malformed_and_out_of_range_reports() {
    use ctl460_rust::wire::{valid_report, REPORT_LEN};
    let valid = hid::encode(Frame {
        contact: true,
        in_range: true,
        x: 14720,
        y: 9200,
        pressure: 1.0,
        tilt_x: 60,
        tilt_y: -60,
        virtual_tilt: true,
        ..Default::default()
    });
    assert!(valid_report(&valid));
    for length in 0..=REPORT_LEN + 2 {
        if length != REPORT_LEN {
            assert!(!valid_report(&vec![0; length]));
        }
    }
    for (offset, value) in [
        (0, 2),
        (1, 0x80),
        (1, 1),
        (2, 255),
        (5, 255),
        (7, 17),
        (8, 61),
        (9, 195),
    ] {
        let mut bad = valid;
        bad[offset] = value;
        assert!(!valid_report(&bad), "offset {offset}, value {value}");
    }
    let mut bad = valid;
    bad[1] = 16;
    assert!(!valid_report(&bad), "hover cannot have pressure");
    assert!(valid_report(&hid::encode(Frame::default())));
}

#[cfg(windows)]
#[test]
fn status_shows_measured_and_virtual_capabilities_and_live_values() {
    use ctl460_rust::ipc::{Mapping, PenData};
    let writer =
        Mapping::create_writer().expect("Stop the normal feeder before running debug tests");
    writer.set_metadata(1, 250, 42);
    std::thread::sleep(std::time::Duration::from_millis(1100));
    assert!(!writer.active());
    writer.heartbeat();
    assert!(writer.active());
    assert_eq!(writer.latest(), 0, "waiting must not invent WinTab samples");
    assert!(ctl460_rust::status::summary().contains("waiting for tablet input"));
    writer.publish(PenData {
        raw: 512,
        pressure: 2048,
        flags: 9 | 32 | (85 << 8) | PenData::handwriting_mode_flags("auto"),
        ..Default::default()
    });
    let status = ctl460_rust::status::summary();
    for expected in [
        "1,024 measured",
        "4,098",
        "Virtual HID",
        "512/1023 -> 2048/4097",
        "Handwriting: Auto; stroke assistance active",
        "85/100",
        "42 us",
    ] {
        assert!(status.contains(expected), "missing {expected}: {status}");
    }
    drop(writer);
    assert!(ctl460_rust::status::summary().contains("stopped"));
}

#[test]
fn shared_lift_keeps_hid_and_wintab_endpoint_before_hover_or_exit() {
    use ctl460_rust::lifecycle::lift_before_hover;
    let contact = Frame {
        x: 1234,
        y: 2345,
        in_range: true,
        contact: true,
        pressure: 0.3,
        barrel: true,
        ..Frame::default()
    };
    for in_range in [true, false] {
        let hover = Frame {
            x: 4000,
            y: 5000,
            in_range,
            ..Frame::default()
        };
        let lift = lift_before_hover(contact, hover).unwrap();
        assert_eq!((lift.x, lift.y), (1234, 2345));
        assert!(!lift.contact && lift.pressure == 0.0 && !lift.barrel && lift.in_range);
        let packet = hid::encode(lift);
        assert_eq!(u16::from_le_bytes([packet[2], packet[3]]), 1234);
        assert_eq!(u16::from_le_bytes([packet[4], packet[5]]), 2345);
        assert_eq!(packet[1] & 1, 0);
        assert_eq!(&packet[6..8], &[0, 0]);
        assert!(
            lift_before_hover(lift, hover).is_none(),
            "lift is emitted only once"
        );
        assert!(
            lift_before_hover(hover, contact).is_none(),
            "touchdown stays immediate"
        );
    }
    assert!(lift_before_hover(contact, contact).is_none());
}
