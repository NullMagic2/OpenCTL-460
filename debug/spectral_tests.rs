//! Independent numerical checks and geometry invariants for spectral processing.
use ctl460_rust::spectral_smoothing::{cutoff, transform, SpectralSmoothing};
use ctl460_rust::{config::Config, line_smoothing::LineSmoothing};
use std::f64::consts::TAU;
#[test]
fn fft_matches_direct_dft_and_recovers_both_packed_axes() {
    let original = std::array::from_fn(|i| {
        (
            (i as f64 * 0.23).sin() + i as f64 / 512.0,
            (i as f64 * 0.71).cos(),
        )
    });
    let mut actual = original;
    transform(&mut actual, false);
    for k in [0, 1, 7, 31, 128, 255, 256, 511] {
        let mut expected = (0.0, 0.0);
        for (n, p) in original.iter().enumerate() {
            let angle = -TAU * k as f64 * n as f64 / 512.0;
            expected.0 += p.0 * angle.cos() - p.1 * angle.sin();
            expected.1 += p.0 * angle.sin() + p.1 * angle.cos();
        }
        assert!((actual[k].0 - expected.0).hypot(actual[k].1 - expected.1) < 1e-8);
    }
    transform(&mut actual, true);
    for (a, b) in actual.iter().zip(original) {
        assert!((a.0 - b.0).hypot(a.1 - b.1) < 1e-10);
    }
}
#[test]
fn spectral_filter_preserves_constant_positions_and_translation_rotation() {
    for amount in [0.01, 0.25, 0.5, 0.75, 1.0] {
        let mut a = SpectralSmoothing::default();
        for _ in 0..100 {
            let p = a.update((1300.0, 4100.0), amount, 0.5);
            assert!((p.0 - 1300.0).hypot(p.1 - 4100.0) < 1e-8);
        }
        let mut a = SpectralSmoothing::default();
        let mut b = SpectralSmoothing::default();
        for i in 0..400 {
            let raw = (
                3000.0 + i as f64 * 5.0,
                4000.0 + 100.0 * (i as f64 * 0.13).sin(),
            );
            let pa = a.update(raw, amount, 0.5);
            let pb = b.update((10000.0 - raw.1, raw.0 + 500.0), amount, 0.5);
            assert!((pb.0 - (10000.0 - pa.1)).hypot(pb.1 - (pa.0 + 500.0)) < 1e-6);
        }
    }
}
#[test]
fn spectral_grid_is_invariant_to_collinear_subdivision() {
    let run = |step: usize| {
        let mut f = SpectralSmoothing::default();
        (0..=1600)
            .step_by(step)
            .map(|i| {
                let p = f.update((2000.0 + i as f64, 3000.0 + i as f64), 0.7, 0.4);
                (i, p)
            })
            .filter(|(i, _)| i % 20 == 0)
            .map(|(_, p)| p)
            .collect::<Vec<_>>()
    };
    for (a, b) in run(1).iter().zip(run(20)) {
        assert!((a.0 - b.0).hypot(a.1 - b.1) < 1e-7);
    }
}
#[test]
fn every_slider_step_has_ordered_cutoff_and_expression_releases_mid_strength() {
    let mut previous = 1.0;
    for i in 0..=100 {
        let a = i as f64 / 100.0;
        let f = cutoff(a, 0.0);
        assert!(f > 0.0 && f <= previous);
        assert!(cutoff(a, 1.0) >= f);
        previous = f;
    }
    assert_eq!(cutoff(1.0, 0.0), cutoff(1.0, 1.0));
}
#[test]
fn tiny_amounts_and_zero_are_continuous_and_dots_stay_exact() {
    for amount in [0.0, 1e-300, 1e-20, 0.00001, 0.01, 1.0, 100.0] {
        let c = Config {
            streamline_amount: amount,
            stabilization_amount: amount,
            motion_filter_amount: amount,
            ..Default::default()
        };
        let mut f = LineSmoothing::default();
        for _ in 0..100 {
            assert_eq!(f.update(3300, 4100, 8.0, &c), (3300, 4100));
        }
    }
}
#[test]
fn spectral_reversals_stay_bounded_and_do_not_drift_when_settling_is_off() {
    let c = Config {
        motion_filter_amount: 100.0,
        endpoint_settling: false,
        ..Default::default()
    };
    let mut f = LineSmoothing::default();
    let mut previous = f.update(1000, 3000, 8.0, &c);
    for x in (1010..=5000)
        .step_by(10)
        .chain((1000..=4990).step_by(10).rev())
    {
        let p = f.update(x, 3000, 8.0, &c);
        assert!(p.0 >= x.min(previous.0) && p.0 <= x.max(previous.0));
        assert!(p.0.abs_diff(x) <= 200);
        assert_eq!(p.1, 3000);
        previous = p;
    }
    for _ in 0..100 {
        assert_eq!(f.update(1000, 3000, 8.0, &c), previous);
    }
}

#[test]
fn resuming_after_endpoint_settling_does_not_inherit_old_spectral_lag() {
    let c = Config {
        motion_filter_amount: 100.0,
        endpoint_settling: true,
        ..Default::default()
    };
    let mut moving = LineSmoothing::default();
    for i in 0..100 {
        moving.update(2000 + i * 30, 4500, 8.0, &c);
    }
    for _ in 0..100 {
        moving.update(4970, 4500, 8.0, &c);
    }
    let mut fresh = LineSmoothing::default();
    fresh.update(4970, 4500, 8.0, &c);
    for i in 1..=50 {
        let x = 4970 + i * 10;
        let a = moving.update(x, 4500, 8.0, &c);
        let b = fresh.update(x, 4500, 8.0, &c);
        assert_eq!(a, b, "stale history after rest at report {i}");
    }
}
