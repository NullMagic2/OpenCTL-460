//! Live settings tests: atomic persistence, latest-update delivery, validation and stroke boundaries.
//! Uses isolated debug/generated files and synthetic reports; never opens the tablet or injects input.
use ctl460_rust::{
    config::Config,
    engine::Engine,
    live_config::{self, Update, Watcher},
    protocol::Sample,
};
use std::{
    fs,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

fn directory() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("debug/generated")
        .join(format!(
            "live-settings-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir_all(&path).unwrap();
    path
}
fn update(watcher: &Watcher) -> Update {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(update) = watcher.take() {
            return update;
        }
        assert!(
            Instant::now() < deadline,
            "live settings delivery timed out"
        );
        thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn watcher_loads_atomic_replacements_and_rejects_invalid_settings() {
    let dir = directory();
    let path = dir.join("settings.toml");
    live_config::write_atomic(&path, "gamma=1.0").unwrap();
    let watcher = Watcher::start(path.clone()).unwrap();
    assert_eq!(update(&watcher).config.gamma, 1.0);
    live_config::write_atomic(
        &path,
        "gamma=0.5\nhandwriting_mode='on'\nstreamline_amount=44.0",
    )
    .unwrap();
    let latest = update(&watcher);
    assert_eq!(latest.config.gamma, 0.5);
    assert_eq!(latest.config.streamline_amount, 44.0);
    assert_eq!(latest.config.handwriting_mode, "on");
    watcher.report(latest.revision, &Ok(()));
    thread::sleep(Duration::from_millis(220));
    let status = fs::read_to_string(dir.join("live-settings-status.txt")).unwrap();
    assert!(status.ends_with("Settings applied."));
    live_config::write_atomic(&path, "gamma=-8").unwrap();
    thread::sleep(Duration::from_millis(220));
    assert!(watcher.take().is_none());
    assert!(fs::read_to_string(dir.join("live-settings-status.txt"))
        .unwrap()
        .contains("not applied"));
    live_config::write_atomic(&path, "gamma=2.0").unwrap();
    assert_eq!(update(&watcher).config.gamma, 2.0);
    drop(watcher);
}
#[test]
fn latest_valid_edit_replaces_unconsumed_updates() {
    let dir = directory();
    let path = dir.join("settings.toml");
    live_config::write_atomic(&path, "gamma=1.0").unwrap();
    let watcher = Watcher::start(path.clone()).unwrap();
    let _ = update(&watcher);
    for gamma in [0.4, 0.8, 1.5] {
        live_config::write_atomic(&path, &format!("gamma={gamma}")).unwrap();
        thread::sleep(Duration::from_millis(160));
    }
    assert_eq!(update(&watcher).config.gamma, 1.5);
    assert!(watcher.take().is_none());
}
#[test]
fn config_changes_apply_after_lift_without_restarting_engine() {
    let old = Config {
        smoothing_ms: 0.0,
        interpolation_ms: 0.0,
        stroke_smoothing: false,
        ..Config::default()
    };
    let mut engine = Engine::new(old.clone()).unwrap();
    let sample = Sample {
        x: 1000,
        y: 2000,
        pressure: 512,
        tip: true,
        in_range: true,
        position_valid: true,
        ..Sample::default()
    };
    engine.push(sample, 0.0).unwrap();
    let before = engine.tick(0.0);
    let next = Config {
        gamma: 2.0,
        left_handed: true,
        handwriting_mode: "on".into(),
        streamline_amount: 60.0,
        ..old
    };
    assert!(!engine.reconfigure(next.clone()).unwrap());
    engine.push(sample, 8.0).unwrap();
    assert_eq!(
        engine.tick(8.0),
        before,
        "active stroke cannot change under the pen"
    );
    engine
        .push(
            Sample {
                tip: false,
                pressure: 0,
                ..sample
            },
            16.0,
        )
        .unwrap();
    assert!(engine.reconfigure(next.clone()).unwrap());
    assert!(!engine.tick(16.0).contact);
    engine.push(sample, 24.0).unwrap();
    let after = engine.tick(24.0);
    assert!(after.handwriting);
    assert_eq!(after.x, 14720 - 1000);
    assert_eq!(after.y, 9200 - 2000);
    assert_eq!(after.pressure, next.curve(512));
    assert!(after.pressure < before.pressure);
    assert!(engine
        .reconfigure(Config {
            gamma: -1.0,
            ..Config::default()
        })
        .is_err());
}
