//! Behavioral tests for artistic smoothing, pressure response, and stroke isolation.
//! Synthetic paths establish invariants; they do not establish perceptual parity with other drawing applications.
use ctl460_rust::{
    config::Config, engine::Engine, line_smoothing::LineSmoothing, protocol::Sample,
};

fn pen(x: u16, y: u16, pressure: u16) -> Sample {
    Sample {
        x,
        y,
        pressure,
        in_range: true,
        position_valid: true,
        tip: pressure > 0,
        ..Default::default()
    }
}
fn path(c: &Config, dt: f64, step: u16) -> Vec<(u16, u16)> {
    let mut filter = LineSmoothing::default();
    (0..100)
        .map(|i| {
            filter.update(
                1000 + i * step,
                if i < 10 { 1000 } else { 1000 + (i % 2) * 30 },
                dt,
                c,
            )
        })
        .collect()
}
fn wiggle(points: &[(u16, u16)]) -> u32 {
    points[20..]
        .windows(2)
        .map(|p| p[1].1.abs_diff(p[0].1) as u32)
        .sum()
}
#[test]
fn zero_controls_preserve_every_sample_and_old_profiles_load() {
    let c = Config::parse("stroke_smoothing = false").unwrap();
    assert_eq!(c.streamline_amount, 0.0);
    let p = path(&c, 4.0, 20);
    for (i, point) in p.iter().enumerate() {
        assert_eq!(
            *point,
            (
                1000 + i as u16 * 20,
                if i < 10 {
                    1000
                } else {
                    1000 + (i as u16 % 2) * 30
                }
            )
        );
    }
}
#[test]
fn independent_controls_reduce_wobble() {
    let raw = wiggle(&path(&Config::default(), 4.0, 20));
    for c in [
        Config {
            streamline_amount: 70.0,
            ..Config::default()
        },
        Config {
            stabilization_amount: 70.0,
            ..Config::default()
        },
        Config {
            motion_filter_amount: 70.0,
            ..Config::default()
        },
    ] {
        assert!(wiggle(&path(&c, 4.0, 20)) < raw / 2);
    }
}
#[test]
fn motion_filter_is_pace_independent_and_expression_restores_variation() {
    let mut c = Config {
        motion_filter_amount: 60.0,
        ..Config::default()
    };
    let filtered = path(&c, 2.0, 20);
    assert_eq!(filtered, path(&c, 30.0, 20));
    c.motion_filter_expression = 100.0;
    assert!(wiggle(&path(&c, 2.0, 20)) > wiggle(&filtered));
    c.motion_filter_amount = 100.0;
    let high = path(&c, 2.0, 20);
    c.motion_filter_expression = 0.0;
    assert_eq!(high, path(&c, 2.0, 20));
}
#[test]
fn stabilization_averages_more_samples_at_higher_speed() {
    let c = Config {
        stabilization_amount: 100.0,
        ..Config::default()
    };
    let straight = |step: u16| {
        let mut f = LineSmoothing::default();
        (0..100)
            .map(|i| f.update(1000 + i * step, 1000, 4.0, &c))
            .collect::<Vec<_>>()
    };
    let slow = straight(2);
    let fast = straight(80);
    // Normalize lag by distance per report: this measures the averaging window.
    let slow_lag = (1198 - slow[99].0) as f64 / 2.0;
    let fast_lag = (8920 - fast[99].0) as f64 / 80.0;
    assert!(fast_lag > slow_lag + 1.0, "slow={slow_lag} fast={fast_lag}");
}
#[test]
fn dots_releases_proximity_and_new_strokes_never_inherit_smoothing() {
    let c = Config {
        streamline_amount: 100.0,
        streamline_pressure: 100.0,
        stabilization_amount: 100.0,
        motion_filter_amount: 100.0,
        ..Config::default()
    };
    let mut e = Engine::new(c).unwrap();
    e.push(pen(1000, 1000, 400), 0.0).unwrap();
    assert!(e.tick(0.0).pressure > 0.0);
    e.push(pen(3000, 3000, 800), 4.0).unwrap();
    e.push(pen(3000, 3000, 0), 8.0).unwrap();
    assert!(!e.tick(8.0).contact);
    assert_eq!(e.tick(8.0).pressure, 0.0);
    e.push(pen(8000, 7000, 400), 12.0).unwrap();
    assert_eq!((e.tick(12.0).x, e.tick(12.0).y), (8000, 7000));
    assert!(!e.tick(200.0).contact);
    e.push(pen(2000, 2000, 400), 204.0).unwrap();
    assert_eq!((e.tick(204.0).x, e.tick(204.0).y), (2000, 2000));
}
#[test]
fn pressure_and_position_controls_apply_in_both_drawing_and_handwriting() {
    let base = Config {
        stroke_smoothing: false,
        smoothing_ms: 0.0,
        interpolation_ms: 0.0,
        ..Config::default()
    };
    let smooth = Config {
        streamline_pressure: 100.0,
        streamline_amount: 100.0,
        motion_filter_amount: 100.0,
        stabilization_amount: 100.0,
        ..base.clone()
    };
    let mut raw = Engine::new(base.clone()).unwrap();
    let mut filtered = Engine::new(smooth.clone()).unwrap();
    for engine in [&mut raw, &mut filtered] {
        engine.push(pen(1000, 1000, 100), 0.0).unwrap();
        engine.push(pen(1100, 1050, 900), 4.0).unwrap();
    }
    assert!(filtered.tick(4.0).pressure < raw.tick(4.0).pressure);
    let mut writing = Engine::new(Config {
        handwriting_mode: "on".into(),
        ..smooth
    })
    .unwrap();
    writing.push(pen(1000, 1000, 100), 0.0).unwrap();
    writing.push(pen(1100, 1050, 900), 4.0).unwrap();
    assert_eq!(writing.tick(4.0).pressure, filtered.tick(4.0).pressure);
    assert!(writing.tick(4.0).x < 1100);
    assert_eq!(
        (writing.tick(4.0).x, writing.tick(4.0).y),
        (filtered.tick(4.0).x, filtered.tick(4.0).y)
    );
}
#[test]
fn all_percentages_reject_nonfinite_and_out_of_range_values() {
    for key in [
        "streamline_amount",
        "streamline_pressure",
        "stabilization_amount",
        "motion_filter_amount",
        "motion_filter_expression",
    ] {
        for value in ["-1.0", "100.1", "nan", "inf"] {
            assert!(Config::parse(&format!("{key} = {value}")).is_err());
        }
    }
}

#[test]
fn measured_release_stays_responsive_even_with_strong_pressure_smoothing() {
    // A decreasing force sequence must reach each new coordinate without holding
    // the old thick width for an interpolation timer. Test writing and drawing.
    for mode in ["off", "on"] {
        for interval in [4.0, 8.0, 16.0] {
            let c = Config {
                handwriting_mode: mode.into(),
                smoothing_ms: 30.0,
                interpolation_ms: 12.0,
                streamline_pressure: 100.0,
                ..Config::default()
            };
            let mut engine = Engine::new(c.clone()).unwrap();
            engine.push(pen(1000, 1000, 900), 0.0).unwrap();
            let mut previous = engine.tick(0.0).pressure;
            for (i, raw) in [750, 500, 250, 80, 10].into_iter().enumerate() {
                let time = (i + 1) as f64 * interval;
                engine
                    .push(pen(1100 + i as u16 * 100, 1000, raw), time)
                    .unwrap();
                let output = engine.tick(time).pressure;
                let target = c.curve(raw);
                assert!(
                    output < previous,
                    "fall must be visible at the new position"
                );
                assert!(
                    output >= target,
                    "no invented pressure below the measured curve"
                );
                assert!(output - target <= (previous - target) * 0.14 + 1e-9);
                previous = output;
            }
            engine.push(pen(2000, 1000, 0), 6.0 * interval).unwrap();
            for time in [6.0 * interval, 7.0 * interval] {
                let frame = engine.tick(time);
                assert!(!frame.contact && frame.pressure == 0.0, "no post-lift tail");
            }
        }
    }
}

#[test]
fn release_response_preserves_force_control_across_motion_and_tilt() {
    // Repeat the same force ramp at different speeds and with simulated tilt.
    let response = |step: u16, tilt: bool| {
        let mut engine = Engine::new(Config {
            virtual_tilt: tilt,
            ..Config::default()
        })
        .unwrap();
        [100, 300, 700, 900, 700, 400, 150, 30, 0]
            .into_iter()
            .enumerate()
            .map(|(i, raw)| {
                let time = i as f64 * 8.0;
                engine
                    .push(pen(1000 + i as u16 * step, 1000, raw), time)
                    .unwrap();
                engine.tick(time).pressure
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(response(10, false), response(150, false));
    assert_eq!(response(10, false), response(150, true));
}

#[test]
fn artistic_sliders_change_manual_handwriting_with_adaptive_filter_enabled() {
    let response = |mut config: Config| {
        config.handwriting_mode = "on".into();
        config.stroke_smoothing = true;
        let mut engine = Engine::new(config).unwrap();
        (0..100)
            .map(|i| {
                let time = f64::from(i) * 4.0;
                engine
                    .push(pen(1000 + i * 20, 1000 + (i % 2) * 30, 500), time)
                    .unwrap();
                let f = engine.tick(time);
                assert!(f.handwriting);
                (f.x, f.y)
            })
            .collect::<Vec<_>>()
    };
    let base = response(Config::default());
    for c in [
        Config {
            streamline_amount: 70.0,
            ..Config::default()
        },
        Config {
            stabilization_amount: 70.0,
            ..Config::default()
        },
        Config {
            motion_filter_amount: 70.0,
            ..Config::default()
        },
    ] {
        assert!(wiggle(&response(c)) < wiggle(&base));
    }
    let c = Config {
        motion_filter_amount: 60.0,
        ..Config::default()
    };
    let filtered = response(c.clone());
    let expressive = response(Config {
        motion_filter_expression: 100.0,
        ..c
    });
    assert!(wiggle(&expressive) > wiggle(&filtered));
}

#[test]
fn pressure_slider_never_changes_coordinates_and_keeps_small_force_noise_centered() {
    for mode in ["off", "on"] {
        let base = Config {
            handwriting_mode: mode.into(),
            ..Config::default()
        };
        let mut a = Engine::new(base.clone()).unwrap();
        let mut b = Engine::new(Config {
            streamline_pressure: 80.0,
            ..base
        })
        .unwrap();
        let mut sum = 0.0;
        for i in 0..600u16 {
            let p = pen(
                2000 + i * 12,
                4500 + (i % 2) * 30,
                if i % 2 == 0 { 488 } else { 512 },
            );
            let time = f64::from(i) * 8.0;
            a.push(p, time).unwrap();
            b.push(p, time).unwrap();
            let raw = a.tick(time + 4.0);
            let smooth = b.tick(time + 4.0);
            assert_eq!((raw.x, raw.y), (smooth.x, smooth.y));
            if i >= 200 {
                sum += smooth.pressure * 1023.0;
            }
        }
        assert!(
            (sum / 400.0 - 500.0).abs() < 1.0,
            "pressure jitter must not bias strokes thin: {}",
            sum / 400.0
        );
    }
}
#[test]
fn stabilization_covers_fractional_report_intervals_without_a_one_sample_dead_zone() {
    // Equal physical speed, different report intervals: both should average the same
    // time span. Include a window shorter than one physical report interval.
    let lag = |dt: f64| {
        let c = Config {
            stabilization_amount: 80.0,
            ..Config::default()
        };
        let mut f = LineSmoothing::default();
        let mut output = (0, 0);
        for i in 0..100 {
            output = f.update((2000.0 + f64::from(i) * dt * 2.0) as u16, 4000, dt, &c);
        }
        2000.0 + 99.0 * dt * 2.0 - f64::from(output.0)
    };
    let baseline = lag(4.0);
    assert!(baseline > 5.0);
    for dt in [2.0, 8.0, 12.0, 16.0] {
        assert!((lag(dt) - baseline).abs() <= 1.0);
    }
}
#[test]
fn streamline_eighty_reduces_geometric_wobble_not_only_cursor_speed() {
    let c = Config {
        streamline_amount: 80.0,
        ..Config::default()
    };
    let mut f = LineSmoothing::default();
    let (mut raw_energy, mut smooth_energy) = (0.0, 0.0);
    for i in 0..700u16 {
        let x = 2000 + i * 12;
        let y = (4500.0 + 60.0 * (f64::from(i * 12) * std::f64::consts::TAU / 200.0).sin()).round()
            as u16;
        let p = f.update(x, y, 8.0, &c);
        if i > 200 {
            raw_energy += (f64::from(y) - 4500.0).powi(2);
            smooth_energy += (f64::from(p.1) - 4500.0).powi(2);
        }
    }
    // RMS measured perpendicular to the intended straight path; lag cannot pass this test.
    assert!(smooth_energy.sqrt() < raw_energy.sqrt() * 0.35);
}

// Evaluate perpendicular error against the intended centerline; cursor lag
// alone cannot pass this regression. The prior direction clip amplified these.
#[test]
fn motion_filter_rejects_short_wavelengths_and_limits_broad_curve_gain() {
    for (wavelength, maximum_ratio) in [
        (60.0, 0.25),
        (120.0, 0.50),
        (240.0, 0.80),
        (600.0, 1.12),
        (2000.0, 1.12),
    ] {
        let c = Config {
            motion_filter_amount: 70.0,
            ..Config::default()
        };
        let mut f = LineSmoothing::default();
        let (mut raw_energy, mut filtered_energy) = (0.0, 0.0);
        for i in 0..1000 {
            let x = 2000 + i * 10;
            let y = (4000.0 + 15.0 * (f64::from(i * 10) * std::f64::consts::TAU / wavelength).sin())
                .round() as u16;
            let point = f.update(x, y, 8.0, &c);
            if i >= 100 {
                raw_energy += (f64::from(y) - 4000.0).powi(2);
                filtered_energy += (f64::from(point.1) - 4000.0).powi(2);
            }
        }
        let ratio = (filtered_energy / raw_energy).sqrt();
        println!("wavelength={wavelength} units, RMS ratio={ratio:.4}");
        assert!(
            ratio < maximum_ratio,
            "wavelength={wavelength}, ratio={ratio}"
        );
    }
}

#[test]
fn motion_filter_keeps_millimetre_loops_and_has_a_bounded_error() {
    for radius in [100.0, 250.0, 1000.0] {
        let c = Config {
            flowing_smoothing: false,
            endpoint_settling: false,
            motion_filter_amount: 70.0,
            ..Config::default()
        };
        let mut f = LineSmoothing::default();
        let mut radii = 0.0;
        for i in 0..1000 {
            let angle = f64::from(i) * 2.0 * std::f64::consts::TAU / 999.0;
            let raw = (
                (6000.0 + radius * angle.cos()).round() as u16,
                (4000.0 + radius * angle.sin()).round() as u16,
            );
            let p = f.update(raw.0, raw.1, 4.0, &c);
            assert!(
                (f64::from(p.0) - f64::from(raw.0)).hypot(f64::from(p.1) - f64::from(raw.1))
                    <= 39.91
            );
            if i >= 500 {
                radii += (f64::from(p.0) - 6000.0).hypot(f64::from(p.1) - 4000.0);
            }
        }
        let ratio = radii / 500.0 / radius;
        println!("radius={radius} units, retained radius={ratio:.4}");
        assert!((ratio - 1.0).abs() < 0.08, "radius={radius}, ratio={ratio}");
    }
}

#[test]
fn motion_filter_cannot_ring_past_a_reversal_or_drift_at_rest() {
    let c = Config {
        flowing_smoothing: false,
        endpoint_settling: false,
        motion_filter_amount: 100.0,
        ..Config::default()
    };
    let mut f = LineSmoothing::default();
    let mut previous = f.update(1000, 1000, 4.0, &c);
    for x in (1010..=3000)
        .step_by(10)
        .chain((1000..=2990).step_by(10).rev())
    {
        let p = f.update(x, 1000, 4.0, &c);
        assert!(p.0 >= previous.0.min(x) && p.0 <= previous.0.max(x));
        assert_eq!(p.1, 1000);
        previous = p;
    }
    for _ in 0..100 {
        assert_eq!(f.update(1000, 1000, 4.0, &c), previous);
    }
}

#[test]
fn motion_filter_is_stable_at_tiny_valid_amounts() {
    for amount in [0.0, 1e-300, 1e-20, 0.00001] {
        let c = Config {
            motion_filter_amount: amount,
            ..Config::default()
        };
        let mut f = LineSmoothing::default();
        for (x, y) in [(1000, 2000), (1200, 2100), (1300, 2300), (1300, 2300)] {
            assert_eq!(f.update(x, y, 4.0, &c), (x, y));
        }
    }
}

#[test]
fn motion_filter_preserves_collinear_report_subdivision() {
    let c = Config {
        motion_filter_amount: 70.0,
        ..Config::default()
    };
    let sample = |step: usize| {
        let mut f = LineSmoothing::default();
        (0..=1000)
            .step_by(step)
            .map(|i| {
                let p = f.update(2000 + i as u16, 3000 + i as u16, step as f64, &c);
                (i, p)
            })
            .filter(|(i, _)| i % 20 == 0)
            .map(|(_, p)| p)
            .collect::<Vec<_>>()
    };
    assert_eq!(sample(1), sample(20));
}
