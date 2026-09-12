use ctl460_rust::{
    config::Config, endpoint_settling::ReleaseSettling, engine::Engine, protocol::Sample,
};
fn sample(x: u16, pressure: u16) -> Sample {
    Sample {
        x,
        y: 4500,
        pressure,
        tip: pressure > 0,
        in_range: true,
        position_valid: true,
        ..Default::default()
    }
}
#[test]
fn finishing_a_line_reduces_endpoint_lag_without_changing_pressure_or_adding_ink() {
    let c = Config {
        pen_control: Some(30.0),
        streamline_amount: 30.0,
        stabilization_amount: 20.0,
        handwriting_mode: "auto".into(),
        ..Default::default()
    };
    let mut enabled = Engine::new(c.clone()).unwrap();
    let mut disabled = Engine::new(Config {
        endpoint_settling: false,
        ..c
    })
    .unwrap();
    let (mut t, mut x) = (0.0, 4000);
    let mut last = None;
    for (step, pressure) in std::iter::repeat_n((20, 600), 80).chain([
        (18, 580),
        (14, 540),
        (10, 440),
        (6, 320),
        (4, 220),
        (2, 150),
        (1, 80),
        (0, 40),
    ]) {
        x += step;
        t += 8.0;
        enabled.push(sample(x, pressure), t).unwrap();
        disabled.push(sample(x, pressure), t).unwrap();
        let a = enabled.tick(t);
        let b = disabled.tick(t);
        assert_eq!(a.pressure, b.pressure);
        assert!(a.contact && b.contact);
        assert_eq!(a.y, 4500);
        assert!(a.x <= x);
        if let Some(old) = last {
            assert!(a.x >= old);
        }
        last = Some(a.x);
    }
    let a = enabled.tick(t);
    let b = disabled.tick(t);
    eprintln!("finish lag before {} after {}", x - b.x, x - a.x);
    assert!(f64::from(x - a.x) < f64::from(x - b.x) * 0.7);
    enabled.push(sample(x + 500, 0), t + 8.0).unwrap();
    for tick in 0..100 {
        let f = enabled.tick(t + 8.0 + tick as f64);
        assert!(!f.contact);
        assert_eq!(f.pressure, 0.0);
    }
    enabled.push(sample(3000, 400), t + 200.0).unwrap();
    assert_eq!(enabled.tick(t + 200.0).x, 3000);
}
#[test]
fn changing_pressure_alone_or_speed_alone_cannot_trigger_finishing() {
    for changing_pressure in [false, true] {
        let mut filter = ReleaseSettling::default();
        let mut x = 1000;
        for i in 0..100 {
            x += if changing_pressure || i < 70 { 20 } else { 1 };
            let p = if changing_pressure && i >= 70 {
                600 - (i - 69) * 18
            } else {
                600
            };
            assert_eq!(filter.update(x, 3000, p, 8.0), 0.0);
        }
    }
}
#[test]
fn new_stroke_and_invalid_timing_discard_release_evidence() {
    let mut filter = ReleaseSettling::default();
    for i in 0..80 {
        filter.update(1000 + i * 20, 3000, 600, 8.0);
    }
    filter.update(2600, 3000, 400, 8.0);
    assert!(filter.update(2601, 3000, 100, 8.0) > 0.0);
    for bad in [0.0, 100.0, f64::NAN, f64::INFINITY] {
        assert_eq!(filter.update(2601, 3000, 10, bad), 0.0);
    }
    filter.reset();
    assert_eq!(filter.update(4000, 2000, 100, 8.0), 0.0);
}
