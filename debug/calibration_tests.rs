use ctl460_rust::{
    calibration::{self, Session, STAGE_MS, WAIT_MS},
    config::Config,
    pressure_profiles::Shape,
};

fn strokes(s: &mut Session, start: u64, raw: i32) {
    for stroke in 0..3 {
        for i in 0..150 {
            // Brief, outlying touchdown/lift values must not move the body median.
            let p = if !(20..130).contains(&i) { 1023 } else { raw };
            s.sample(start + stroke * 1000 + i * 4, p, true);
        }
        s.sample(start + stroke * 1000 + 700, 0, false);
    }
}
#[test]
fn waits_for_contact_not_hover_or_button_only_reports() {
    let mut s = Session::default();
    s.arm(100);
    s.sample(500, 0, false);
    s.sample(600, 800, false);
    s.sample(700, 0, true);
    assert!(s.started.is_none());
    assert!(s.tick(60_000).is_none());
    strokes(&mut s, 70_000, 200);
    assert!(s.tick(70_000 + STAGE_MS - 1).is_none());
    assert!(s.tick(70_000 + STAGE_MS).unwrap().unwrap().is_none());
    assert_eq!(s.anchors, vec![200]);
}
#[test]
fn absent_input_times_out_without_changing_stages() {
    let mut s = Session::default();
    s.arm(12);
    assert!(s.tick(12 + WAIT_MS).unwrap().is_err());
    assert!(!s.active);
    assert_eq!(s.stage, 0);
    assert!(s.anchors.is_empty());
}
#[test]
fn guided_capture_builds_bounded_monotone_response_with_headroom() {
    let mut s = Session::default();
    let mut shape = None;
    for (i, raw) in [200, 410, 780].into_iter().enumerate() {
        let t = i as u64 * 20_000;
        s.arm(t);
        strokes(&mut s, t + 100, raw);
        shape = s.tick(t + 100 + STAGE_MS).unwrap().unwrap();
    }
    let shape = shape.unwrap();
    assert_eq!(s.stage, 3);
    assert!(!s.active);
    let mut c = Config::default();
    shape.apply(&mut c);
    c.validate().unwrap();
    for raw in 0..1023 {
        assert!(c.curve(raw) <= c.curve(raw + 1));
    }
    assert_eq!(c.curve(0), 0.);
    assert_eq!(c.curve(1023), 1.);
    assert!((c.curve(780) - 0.9).abs() < 1e-10);
    assert!(c.curve(900) > 0.9 && c.curve(900) < 1.);
}
#[test]
fn ambiguous_stage_can_be_retried_without_losing_previous_measurements() {
    let mut s = Session::default();
    s.arm(0);
    strokes(&mut s, 10, 200);
    s.tick(8010).unwrap().unwrap();
    s.arm(10_000);
    strokes(&mut s, 10_010, 220);
    assert!(s.tick(18_010).unwrap().is_err());
    assert_eq!(s.stage, 1);
    assert_eq!(s.anchors, vec![200]);
    s.arm(20_000);
    strokes(&mut s, 20_010, 400);
    s.tick(28_010).unwrap().unwrap();
    assert_eq!(s.anchors, vec![200, 400]);
}
#[test]
fn too_few_strokes_and_cancel_never_create_a_profile() {
    let mut s = Session::default();
    s.arm(0);
    for t in 0..300 {
        s.sample(t * 4, 200, true);
    }
    assert!(s.tick(STAGE_MS).unwrap().is_err());
    assert!(s.anchors.is_empty());
    s.arm(10_000);
    strokes(&mut s, 10_010, 200);
    s.cancel();
    assert!(s.tick(30_000).is_none());
    assert_eq!(s.stage, 0);
}
#[test]
fn invalid_sensor_values_do_not_start_capture() {
    let mut s = Session::default();
    s.arm(0);
    for raw in [-1, 0, 2, 1024, i32::MAX] {
        s.sample(100, raw, true);
    }
    assert!(s.started.is_none());
}
#[test]
fn saturated_or_reversed_calibration_is_rejected() {
    for anchors in [
        vec![],
        vec![10, 400, 800],
        vec![200, 210, 800],
        vec![400, 200, 800],
        vec![200, 400, 1023],
    ] {
        assert!(calibration::suggest(&anchors).is_err());
    }
}
#[test]
fn portable_profile_roundtrips_and_only_changes_pressure() {
    let shape = calibration::suggest(&[200, 410, 780]).unwrap();
    let imported = calibration::import(&calibration::export(&shape).unwrap()).unwrap();
    assert_eq!(shape, imported);
    let mut c = Config::simple_default();
    c.button1 = "Precision Hold".into();
    c.pen_control = Some(47.);
    let previous = toml::to_string(&c).unwrap();
    imported.apply(&mut c);
    Shape::from_config(&Config::parse(&previous).unwrap()).apply(&mut c);
    assert_eq!(toml::to_string(&c).unwrap(), previous);
}
#[test]
fn user_settings_are_rejected_even_with_pressure_fields() {
    let mut c = Config::simple_default();
    c.pen_control = Some(81.);
    let shape = calibration::suggest(&[210, 420, 800]).unwrap();
    shape.apply(&mut c);
    assert!(calibration::import(&toml::to_string(&c).unwrap()).is_err());
}
#[test]
fn malformed_unknown_or_oversized_profiles_are_rejected() {
    let valid = calibration::export(&calibration::suggest(&[200, 400, 800]).unwrap()).unwrap();
    assert!(calibration::import(&valid.replace("version = 1", "version = 2")).is_err());
    assert!(calibration::import(&valid.replace("gain = 1.0", "gain = nan")).is_err());
    assert!(calibration::import("random = 5").is_err());
    assert!(calibration::import("").is_err());
    assert!(calibration::import(&" ".repeat(65_537)).is_err());
}

#[test]
fn calibration_file_type_is_separate_from_user_settings() {
    assert_eq!(
        calibration::export_path(std::path::Path::new("My pencil")).unwrap(),
        std::path::PathBuf::from("My pencil.calibration_profile")
    );
    assert_eq!(
        calibration::export_path(std::path::Path::new("My pencil.calibration_profile")).unwrap(),
        std::path::PathBuf::from("My pencil.calibration_profile")
    );
    assert!(calibration::export_path(std::path::Path::new("settings.toml")).is_err());
    use std::path::Path;
    for name in ["Pencil.calibration_profile", "My brush.CALIBRATION_PROFILE"] {
        calibration::validate_profile_path(Path::new(name)).unwrap();
    }
    for name in [
        "settings.toml",
        "profile",
        "pencil.calibration_profile.toml",
        "pencil.cal",
    ] {
        assert!(calibration::validate_profile_path(Path::new(name)).is_err());
    }
    let valid = calibration::export(&calibration::suggest(&[200, 400, 800]).unwrap()).unwrap();
    assert!(!valid.contains("button1"));
    assert!(!valid.contains("streamline_amount"));
    assert!(calibration::import(&format!("button1 = \"None\"\n{valid}")).is_err());
}

#[test]
fn pad_breaks_strokes_on_lift_and_boundary_exit_and_bounds_memory() {
    use ctl460_rust::calibration_pad::{Pad, MAX_SEGMENTS};
    let mut p = Pad::default();
    assert!(p.feed(0.1, 0.2, 0.2, true));
    assert!(p.feed(0.2, 0.2, 0.8, true));
    assert!(!p.feed(1.1, 0.2, 0.8, true));
    assert!(p.feed(0.8, 0.2, 0.5, true));
    let segment = p.segments.back().unwrap();
    assert_eq!(segment.a.x, segment.b.x);
    assert!(!p.feed(0.8, 0.2, 0.5, false));
    p.feed(0.4, 0.4, 0.5, true);
    let segment = p.segments.back().unwrap();
    assert_eq!(segment.a.x, segment.b.x);
    assert!(!p.feed(f64::NAN, 0.2, 0.8, true));
    for _ in 0..MAX_SEGMENTS + 10 {
        p.feed(0.4, 0.4, 1.0, true);
    }
    assert_eq!(p.segments.len(), MAX_SEGMENTS);
    p.clear();
    assert!(p.segments.is_empty());
}

#[test]
fn feeder_coordinates_land_in_pad_and_light_advances_to_medium() {
    use ctl460_rust::{
        calibration_pad::local_position,
        protocol::{MAX_X, MAX_Y},
        stroke::map_to_virtual_screen,
    };
    for screen in [(1920, 1080), (3840, 2160)] {
        let pad = (
            screen.0 / 2 - 150,
            screen.1 / 2 - 150,
            screen.0 / 2 + 150,
            screen.1 / 2 + 150,
        );
        let mut session = Session::default();
        session.arm(0);
        for stroke in 0..3 {
            for i in 0..150 {
                let (x, y) = map_to_virtual_screen(
                    MAX_X / 2 + i as u16,
                    MAX_Y / 2,
                    screen.0,
                    screen.1,
                    true,
                );
                assert!(local_position(x as i32, y as i32, screen, pad).is_some());
                session.sample(stroke * 1000 + i * 4, 180, true);
            }
            session.sample(stroke * 1000 + 700, 0, false);
        }
        session.tick(STAGE_MS).unwrap().unwrap();
        assert_eq!(session.stage, 1);
        assert_eq!(session.anchors, vec![180]);
        assert!(calibration::stage_prompt(session.stage).starts_with("Medium strokes:"));
        assert!(local_position(0, 0, screen, pad).is_none());
        assert!(local_position(65535, 65535, screen, pad).is_none());
    }
}
