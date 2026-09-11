//! Exercises button actions with a fake input sink; never sends real mouse or keyboard events.
use ctl460_rust::{button_actions::*, config::Config};

#[test]
fn double_click_does_not_recheck_stale_async_state_between_queued_clicks() {
    struct Lagging {
        queries: std::cell::Cell<usize>,
        events: Vec<(InputCode, bool)>,
    }
    impl InputSink for Lagging {
        fn held(&self, _: InputCode) -> bool {
            let n = self.queries.get();
            self.queries.set(n + 1);
            n > 0 // Simulate delayed processing of the first click's release.
        }
        fn send(&mut self, code: InputCode, up: bool) -> Result<(), String> {
            self.events.push((code, up));
            Ok(())
        }
    }
    let mut input = Lagging {
        queries: std::cell::Cell::new(0),
        events: Vec::new(),
    };
    let mut bindings = Bindings::new(["Double click".into(), "None".into()]);
    bindings.update(&mut input, [true, false]).unwrap();
    assert_eq!(
        input.events,
        [
            (InputCode::LeftMouse, false),
            (InputCode::LeftMouse, true),
            (InputCode::LeftMouse, false),
            (InputCode::LeftMouse, true)
        ]
    );
}

#[derive(Default)]
struct Fake {
    held: Vec<InputCode>,
    events: Vec<(InputCode, bool)>,
    fail_down: Option<InputCode>,
}
impl InputSink for Fake {
    fn held(&self, code: InputCode) -> bool {
        self.held.contains(&code)
    }
    fn send(&mut self, code: InputCode, up: bool) -> Result<(), String> {
        if !up && self.fail_down == Some(code) {
            return Err("injected failure".into());
        }
        self.events.push((code, up));
        if up {
            self.held.retain(|c| *c != code);
        } else {
            self.held.push(code);
        }
        Ok(())
    }
}
#[test]
fn validates_custom_chords_and_preserves_them_across_drawing_profiles() {
    assert_eq!(shortcut("ctrl+Shift+z").unwrap(), [0x11, 0x10, 0x5a]);
    for chord in ["F24", "Space", "Alt+Left", "Win+Shift+S", "Ctrl+1"] {
        assert!(shortcut(chord).is_ok());
    }
    for chord in [
        "",
        "Ctrl",
        "Ctrl+",
        "Ctrl+Ctrl+Z",
        "A+B",
        "F25",
        "cmd.exe",
        "Ctrl+Z;whoami",
    ] {
        assert!(shortcut(chord).is_err(), "{chord}");
    }
    let config = Config {
        button1: "Shortcut: Ctrl+Shift+Z".into(),
        button2: "Erase (hold)".into(),
        ..Config::default()
    };
    let saved = toml::to_string(&config).unwrap();
    let restored = Config::parse(&saved)
        .unwrap()
        .drawing_profile(Config::default());
    assert_eq!(restored.button1, config.button1);
    assert!(restored.button_eraser([false, true]));
}
#[test]
fn shortcut_fires_once_and_preserves_physical_modifiers() {
    let mut input = Fake {
        held: vec![InputCode::Key(0x11)],
        ..Fake::default()
    };
    let mut bindings = Bindings::new(["Shortcut: Ctrl+Z".into(), "None".into()]);
    bindings.update(&mut input, [true, false]).unwrap();
    bindings.update(&mut input, [true, false]).unwrap();
    bindings.update(&mut input, [false, false]).unwrap();
    bindings.release_all(&mut input).unwrap();
    assert_eq!(
        input.events,
        [(InputCode::Key(0x5a), false), (InputCode::Key(0x5a), true)]
    );
    assert_eq!(input.held, [InputCode::Key(0x11)]);
}
#[test]
fn shared_mouse_holds_release_only_after_both_switches_and_on_shutdown() {
    let mut input = Fake::default();
    let mut bindings = Bindings::new(["Left click".into(), "Left click".into()]);
    for state in [[true, false], [true, true], [false, true]] {
        bindings.update(&mut input, state).unwrap();
    }
    assert_eq!(input.events, [(InputCode::LeftMouse, false)]);
    bindings.update(&mut input, [false, false]).unwrap();
    assert_eq!(input.events.last(), Some(&(InputCode::LeftMouse, true)));
    let mut bindings = Bindings::new(["Middle click".into(), "Pan (hold Space)".into()]);
    bindings.update(&mut input, [true, true]).unwrap();
    bindings.release_all(&mut input).unwrap();
    assert!(input.held.is_empty());
}
#[test]
fn double_click_is_one_pair_per_press_and_eraser_never_presses_e() {
    let mut input = Fake::default();
    let mut bindings = Bindings::new(["Double click".into(), "Erase (hold)".into()]);
    bindings.update(&mut input, [true, true]).unwrap();
    bindings.update(&mut input, [true, true]).unwrap();
    assert_eq!(
        input.events,
        [
            (InputCode::LeftMouse, false),
            (InputCode::LeftMouse, true),
            (InputCode::LeftMouse, false),
            (InputCode::LeftMouse, true)
        ]
    );
    assert!(input.held.is_empty());
}
#[test]
fn failed_shortcut_unwinds_modifiers_and_mouse_already_held_is_preserved() {
    let mut input = Fake {
        fail_down: Some(InputCode::Key(0x5a)),
        ..Fake::default()
    };
    let mut bindings = Bindings::new(["Shortcut: Ctrl+Shift+Z".into(), "None".into()]);
    assert!(bindings.update(&mut input, [true, false]).is_err());
    assert!(input.held.is_empty());
    input.held.push(InputCode::LeftMouse);
    let mut bindings = Bindings::new(["Left click".into(), "None".into()]);
    bindings.update(&mut input, [true, false]).unwrap();
    bindings.release_all(&mut input).unwrap();
    assert_eq!(input.held, [InputCode::LeftMouse]);
}

#[test]
fn recorded_keys_round_trip_to_the_same_injected_virtual_keys() {
    let keys = [
        8, 9, 13, 27, 32, 0x21, 0x2e, 0x30, 0x39, 0x41, 0x5a, 0x60, 0x69, 0x6a, 0x6b, 0x6d, 0x6e,
        0x6f, 0x70, 0x87, 0xba, 0xbb, 0xbc, 0xbd, 0xbe, 0xbf, 0xc0, 0xdb, 0xdc, 0xdd, 0xde,
    ];
    for key in keys {
        for mask in 0..16 {
            let modifiers = std::array::from_fn(|i| mask & (1 << i) != 0);
            let chord = captured_shortcut(key, modifiers).unwrap();
            let mut expected: Vec<_> = [0x11, 0x10, 0x12, 0x5b]
                .into_iter()
                .zip(modifiers)
                .filter_map(|(key, held)| held.then_some(key))
                .collect();
            expected.push(key);
            assert_eq!(shortcut(&chord).unwrap(), expected, "{chord}");
        }
    }
    assert_eq!(
        captured_shortcut(0x5a, [true, true, false, false]).unwrap(),
        "Ctrl+Shift+Z"
    );
    for key in [0x10, 0x11, 0x12, 0x5b, 0xa0, 0xff] {
        assert!(captured_shortcut(key, [false; 4]).is_err());
    }
}
