//! Offline trajectory tests: no input injection or live shared-memory writes.
use ctl460_rust::{config::Config, engine::Engine, protocol::Sample, wobble::WobbleFilter};

fn sample(x: u16, y: u16, contact: bool) -> Sample {
    Sample {
        x,
        y,
        pressure: if contact { 512 } else { 0 },
        tip: contact,
        position_valid: true,
        in_range: true,
        ..Sample::default()
    }
}

#[test]
fn held_pen_wobble_is_reduced_in_hover_and_contact_at_multiple_report_rates() {
    for hz in [100, 125, 200, 250] {
        for frequency in [6.0, 10.0, 12.0] {
            for contact in [false, true] {
                let mut engine = Engine::new(Config {
                    stroke_smoothing: false,
                    ..Config::default()
                })
                .unwrap();
                let (mut raw_energy, mut filtered_energy) = (0.0, 0.0);
                for i in 0..hz * 3 {
                    let t = f64::from(i) / f64::from(hz);
                    let x = (5000.0 + 12.0 * (std::f64::consts::TAU * frequency * t).sin()).round()
                        as u16;
                    engine.push(sample(x, 4500, contact), t * 1000.0).unwrap();
                    let frame = engine.tick(t * 1000.0);
                    assert_eq!(frame.contact, contact);
                    if i >= hz {
                        raw_energy += (f64::from(x) - 5000.0).powi(2);
                        filtered_energy += (f64::from(frame.x) - 5000.0).powi(2);
                    }
                }
                let ratio = (filtered_energy / raw_energy).sqrt();
                println!("{hz} Hz reports, {frequency} Hz wobble, contact={contact}: RMS ratio {ratio:.3}");
                assert!(ratio < 0.55, "{hz}/{frequency}/{contact}: {ratio}");
            }
        }
    }
}

#[test]
fn slow_motion_has_no_dead_zone_fast_motion_tracks_and_all_edges_are_reachable() {
    for strength in [40.0, 100.0] {
        let mut filter = WobbleFilter::default();
        filter.update(5000, 4500, 8.0, strength);
        let mut previous = 5000;
        for i in 1..200 {
            let raw = 5000 + i;
            let (x, y) = filter.update(raw, 4500, 8.0, strength);
            assert!(x >= previous && x <= raw);
            assert!(f64::from(raw - x) <= 0.3 * strength + 1.0);
            assert_eq!(y, 4500);
            previous = x;
        }
        for _ in 0..300 {
            previous = filter.update(5199, 4500, 8.0, strength).0;
        }
        assert_eq!(
            previous, 5199,
            "a tiny intentional move must eventually arrive"
        );
        assert_eq!(filter.update(5400, 4500, 8.0, strength), (5400, 4500));
        for (x, y) in [(0, 4500), (14720, 4500), (5000, 0), (5000, 9200)] {
            filter.reset();
            filter.update(x.clamp(1, 14719), y.clamp(1, 9199), 8.0, strength);
            assert_eq!(filter.update(x, y, 8.0, strength), (x, y));
        }
    }
}

#[test]
fn small_loops_stay_bounded_and_do_not_overshoot() {
    let mut filter = WobbleFilter::default();
    for i in 0..600 {
        let angle = f64::from(i) * std::f64::consts::TAU / 125.0;
        let raw = (
            (5000.0 + 50.0 * angle.cos()).round() as u16,
            (4500.0 + 50.0 * angle.sin()).round() as u16,
        );
        let p = filter.update(raw.0, raw.1, 8.0, 40.0);
        assert!(
            (f64::from(p.0) - f64::from(raw.0)).hypot(f64::from(p.1) - f64::from(raw.1)) <= 12.8
        );
        assert!((4950..=5050).contains(&p.0) && (4450..=4550).contains(&p.1));
    }
}

#[test]
fn hover_to_contact_is_exact_and_force_is_identical_with_combined_controls() {
    for mode in ["off", "on"] {
        let config = Config {
            handwriting_mode: mode.into(),
            streamline_amount: 20.0,
            stabilization_amount: 19.0,
            streamline_pressure: 40.0,
            virtual_tilt: true,
            ..Config::default()
        };
        let mut damped = Engine::new(config.clone()).unwrap();
        let mut original = Engine::new(Config {
            wobble_reduction: 0.0,
            ..config
        })
        .unwrap();
        for i in 0..500 {
            let mut s = sample(5000 + (i % 2) * 8, 4500, (100..400).contains(&i));
            if s.tip {
                s.pressure = 100 + (i % 100) * 8;
            }
            let t = f64::from(i) * 8.0;
            damped.push(s, t).unwrap();
            original.push(s, t).unwrap();
            let a = damped.tick(t + 4.0);
            let b = original.tick(t + 4.0);
            assert_eq!(
                (a.pressure, a.contact, a.in_range, a.tilt_x, a.tilt_y),
                (b.pressure, b.contact, b.in_range, b.tilt_x, b.tilt_y)
            );
            if i == 100 {
                assert_eq!(
                    (a.x, a.y),
                    (s.x, s.y),
                    "stroke start must not inherit hover lag"
                );
            }
        }
    }
}

#[test]
fn bypass_config_validation_and_proximity_reset() {
    let mut filter = WobbleFilter::default();
    for i in 0..100 {
        assert_eq!(
            filter.update(5000 + i % 9, 4500, 8.0, 0.0),
            (5000 + i % 9, 4500)
        );
    }
    assert_eq!(Config::parse("").unwrap().wobble_reduction, 40.0);
    assert_eq!(
        Config::parse(include_str!("../profiles/raw.toml"))
            .unwrap()
            .wobble_reduction,
        0.0
    );
    for invalid in ["-1", "101", "nan", "inf"] {
        assert!(Config::parse(&format!("wobble_reduction = {invalid}")).is_err());
    }
    let mut engine = Engine::new(Config::default()).unwrap();
    engine.push(sample(5000, 4500, false), 0.0).unwrap();
    engine.push(sample(5010, 4500, false), 8.0).unwrap();
    engine
        .push(
            Sample {
                in_range: false,
                position_valid: false,
                ..Sample::default()
            },
            16.0,
        )
        .unwrap();
    engine.push(sample(5005, 4500, false), 24.0).unwrap();
    assert_eq!(engine.tick(24.0).x, 5005);
    engine.push(sample(5010, 4500, false), 32.0).unwrap();
    engine.tick(200.0);
    engine.push(sample(5006, 4500, false), 208.0).unwrap();
    assert_eq!(engine.tick(208.0).x, 5006);
}
