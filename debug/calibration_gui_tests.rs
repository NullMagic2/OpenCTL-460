// Exercises native controls with a hidden test window and isolated files, never the user's GUI.
#[test]
fn calibration_preview_apply_undo_and_named_profiles_use_native_controls() {
    let settings =
        std::env::temp_dir().join(format!("openctl-calibration-test-{}", std::process::id()));
    fs::create_dir_all(&settings).unwrap();
    let config = Config::simple_default();
    let mut state = State {
        root: std::env::current_dir().unwrap(),
        settings: settings.clone(),
        recording: false,
        startup_directory: settings.join("startup"),
        pressure_draft: Shape::from_config(&config),
        config,
        button_shortcuts: Default::default(),
        last_saved: String::new(),
        settings_error: false,
        child: None,
        hid_session: false,
        event: null_mut(),
        closing: false,
        minimize_requested: false,
        auto_start: false,
        start_pending: false,
        startup_wait: ctl460_rust::startup_wait::StartupWait::new(std::time::Instant::now(), false),
        count: 0,
        paths: Vec::new(),
        pages: std::array::from_fn(|_| Vec::new()),
        font: null_mut(),
        bold_font: null_mut(),
        title_font: null_mut(),
        preview: Some(settings.join("unused.bmp")),
        preview_page: 6,
        layout: Vec::new(),
        pressure_library: Library::default(),
        calibration: Default::default(),
    };
    unsafe {
        let common = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_BAR_CLASSES | ICC_TAB_CLASSES | ICC_PROGRESS_CLASS,
        };
        assert_ne!(InitCommonControlsEx(&common), 0);
        visuals::register().unwrap();
        calibration_canvas::register().unwrap();
        let class = wide("OpenCTLCalibrationUnitTest");
        let instance = GetModuleHandleW(null());
        let wc = WNDCLASSW {
            lpfnWndProc: Some(DefWindowProcW),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            ..Default::default()
        };
        assert_ne!(RegisterClassW(&wc), 0);
        let w = CreateWindowExW(
            0,
            class.as_ptr(),
            wide("Calibration unit test").as_ptr(),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            1040,
            890,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        assert!(!w.is_null());
        state.controls(w).unwrap();
        state.save(w).unwrap();
        let before = fs::read_to_string(settings.join("settings.toml")).unwrap();
        assert_eq!(message(w, PAGE, TCM_GETITEMCOUNT, 0, 0), 7);
        assert_eq!(message(w, CAL_LIST, CB_GETCOUNT, 0, 0), 1);
        assert_ne!(IsWindowEnabled(control(w, CAL_PAD)), 0);
        let mut square = RECT::default();
        GetClientRect(control(w, CAL_PAD), &mut square);
        assert_eq!(square.right, square.bottom);
        let pad = control(w, CAL_PAD);
        calibration_canvas::feed(pad, 0.2, 0.2, 0.5, true);
        assert_eq!(
            calibration_canvas::sample_count(pad),
            0,
            "Idle pad must not record"
        );
        calibration_canvas::recording(pad, true);
        calibration_canvas::feed(pad, 0.2, 0.2, 0.2, true);
        calibration_canvas::feed(pad, 0.7, 0.7, 0.9, true);
        calibration_canvas::feed(pad, 1.2, 0.8, 0.9, true);
        assert_eq!(calibration_canvas::sample_count(pad), 2);
        calibration_canvas::recording(pad, false);
        calibration_canvas::feed(pad, 0.8, 0.8, 0.5, true);
        assert_eq!(
            calibration_canvas::sample_count(pad),
            2,
            "Finished pad must not record"
        );
        state.calibration_command(w, CAL_CLEAR).unwrap();
        assert_eq!(calibration_canvas::sample_count(pad), 0);
        // Exercise actual page completion handling, including the medium/firm popup handoff.
        for (stage, raw) in [180, 420, 780].into_iter().enumerate() {
            let now = stage as u64 * 10_000;
            state.calibration.session.arm(now);
            for stroke in 0..3 {
                for i in 0..150 {
                    state
                        .calibration
                        .session
                        .sample(now + stroke * 1000 + i * 4, raw, true);
                }
                state
                    .calibration
                    .session
                    .sample(now + stroke * 1000 + 700, 0, false);
            }
            state.calibration_finish_stage(w, now + 8_000);
            assert_eq!(state.calibration.session.stage, stage + 1);
            let notice = state.calibration.notice.take().unwrap();
            if stage < 2 {
                assert_eq!(state.calibration.continue_stage.take(), Some(stage + 1));
                assert!(notice.contains(if stage == 0 {
                    "Medium strokes"
                } else {
                    "Firm strokes"
                }));
            } else {
                assert!(notice.contains("Calibration complete"));
                assert!(state.calibration.continue_stage.is_none());
                assert!(state.calibration.draft.is_some());
            }
        }
        state.calibration_command(w, CAL_RESTART).unwrap();
        // Right-column controls share the drawing square's left/right edges.
        let mut bounds = RECT::default();
        GetWindowRect(control(w, CAL_PAD), &mut bounds);
        for id in [CAL_SUMMARY, CAL_APPLY] {
            let mut c = RECT::default();
            GetWindowRect(control(w, id), &mut c);
            assert_eq!(c.left, bounds.left);
        }
        for id in [CAL_CLEAR] {
            let mut c = RECT::default();
            GetWindowRect(control(w, id), &mut c);
            assert_eq!(c.right, bounds.right);
        }
        assert!(!read(w, CAL_METER).contains('\n'));
        let shape = ctl460_rust::calibration::suggest(&[210, 420, 780]).unwrap();
        state.calibration.draft = Some(shape.clone());
        state.calibration_preview(w);
        state.calibration_buttons(w);
        assert_eq!(
            fs::read_to_string(settings.join("settings.toml")).unwrap(),
            before
        );
        assert_ne!(IsWindowEnabled(control(w, CAL_APPLY)), 0);
        state.calibration_save(w, "Test pressure").unwrap();
        assert_eq!(
            Library::load(&settings.join("pressure-profiles.toml"))
                .unwrap()
                .presets[0]
                .shape,
            shape
        );
        assert_eq!(message(w, CAL_LIST, CB_GETCOUNT, 0, 0), 2);
        // Saving a draft does not activate it.
        assert_eq!(
            fs::read_to_string(settings.join("settings.toml")).unwrap(),
            before
        );
        state.calibration.draft = None;
        message(w, CAL_LIST, CB_SETCURSEL, 1, 0);
        state.calibration_command(w, CAL_LOAD).unwrap();
        assert_eq!(state.calibration.draft, Some(shape.clone()));
        assert_eq!(
            fs::read_to_string(settings.join("settings.toml")).unwrap(),
            before
        );
        state.calibration_command(w, CAL_APPLY).unwrap();
        let after =
            Config::parse(&fs::read_to_string(settings.join("settings.toml")).unwrap()).unwrap();
        assert_eq!(Shape::from_config(&after), shape);
        let mut nonpressure = after.clone();
        Shape::from_config(&Config::parse(&before).unwrap()).apply(&mut nonpressure);
        assert_eq!(
            toml::to_string(&nonpressure).unwrap(),
            toml::to_string(&Config::parse(&before).unwrap()).unwrap()
        );
        state.calibration_command(w, CAL_UNDO).unwrap();
        assert_eq!(
            fs::read_to_string(settings.join("settings.toml")).unwrap(),
            before
        );
        assert_eq!(IsWindowEnabled(control(w, CAL_UNDO)), 0);
        // Editing other fields since Apply must prevent an obsolete Undo from overwriting them.
        state.calibration_command(w, CAL_APPLY).unwrap();
        state.config.gain = 0.9;
        assert!(state.calibration_command(w, CAL_UNDO).is_err());
        KillTimer(w, 1);
        KillTimer(w, 4);
        DestroyWindow(w);
        UnregisterClassW(class.as_ptr(), instance);
    }
    fs::remove_file(settings.join("settings.toml")).unwrap();
    fs::remove_file(settings.join("pressure-profiles.toml")).unwrap();
    fs::remove_dir(settings).unwrap();
}
