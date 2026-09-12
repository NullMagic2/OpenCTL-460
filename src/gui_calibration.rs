// Included inside app so this page uses the same native layout and settings helpers.
const CAL_RECORD: i32 = 800;
const CAL_CANCEL: i32 = 801;
const CAL_RESTART: i32 = 802;
const CAL_APPLY: i32 = 803;
const CAL_SAVE: i32 = 804;
const CAL_LOAD: i32 = 805;
const CAL_IMPORT: i32 = 806;
const CAL_EXPORT: i32 = 807;
const CAL_LIST: i32 = 808;
const CAL_STATUS: i32 = 809;
const CAL_METER: i32 = 810;
const CAL_PAD: i32 = 811;
const CAL_CLEAR: i32 = 818;
const CAL_NOTICE: u32 = WM_APP + 21;
const CAL_PROGRESS: i32 = 812;
const CAL_UNDO: i32 = 813;
const CAL_SUMMARY: i32 = 814;
const CAL_HEADING: i32 = 815;

#[derive(Default)]
struct CalibrationPage {
    session: ctl460_rust::calibration::Session,
    stream: Option<ctl460_rust::ipc::Mapping>,
    sequence: u64,
    owner: u32,
    draft: Option<Shape>,
    notice: Option<String>,
    continue_stage: Option<usize>,
    undo: Option<(Shape, Shape)>,
}
impl State {
    fn calibration_controls(&mut self, w: HWND) -> Result<(), String> {
        let before = Self::children(w);
        for (id, rect) in [
            (820, [226, 240, 380, 428]),
            (821, [634, 240, 332, 428]),
            (822, [226, 682, 784, 44]),
        ] {
            add(w, "CTL460SunkenPanel", "", id, rect, WS_CLIPSIBLINGS)?;
            unsafe {
                SetWindowPos(
                    control(w, id),
                    HWND_BOTTOM,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        }
        label(w, "Calibrate pressure", 240, 148, 740)?;
        add(w,"STATIC","Draw light, medium, then comfortably firm strokes in the square.\r\nKeep the driver running. Only strokes inside the square are recorded during each 8-second stage.",816,[240,183,740,52],0)?;
        add(
            w,
            "STATIC",
            "1 of 3: Light strokes",
            CAL_HEADING,
            [240, 252, 345, 28],
            0,
        )?;
        add(w,"STATIC","Draw at least two strokes, lifting between them.\r\nUse comfortable pressure; never press hard.",817,[240,291,345,65],0)?;
        add(
            w,
            "STATIC",
            "Ready. Click Record light when you want to begin.",
            CAL_STATUS,
            [240, 371, 345, 85],
            0,
        )?;
        add(
            w,
            "msctls_progress32",
            "Calibration progress",
            CAL_PROGRESS,
            [240, 464, 345, 18],
            0,
        )?;
        message(w, CAL_PROGRESS, PBM_SETRANGE32, 0, 8000);
        add(
            w,
            "STATIC",
            "Raw pressure: -- / 1023",
            CAL_METER,
            [240, 494, 345, 26],
            0,
        )?;
        add(
            w,
            "BUTTON",
            "Record light",
            CAL_RECORD,
            [240, 555, 170, 32],
            WS_TABSTOP,
        )?;
        add(
            w,
            "BUTTON",
            "Cancel stage",
            CAL_CANCEL,
            [422, 555, 163, 32],
            WS_TABSTOP,
        )?;
        add(
            w,
            "BUTTON",
            "Start over",
            CAL_RESTART,
            [240, 599, 170, 32],
            WS_TABSTOP,
        )?;
        label(w, "Calibration strokes", 650, 250, 205)?;
        add(
            w,
            "BUTTON",
            "Clear",
            CAL_CLEAR,
            [865, 630, 85, 32],
            WS_TABSTOP,
        )?;
        add(
            w,
            "CTL460CalibrationCanvas",
            "Calibration drawing square",
            CAL_PAD,
            [650, 284, 300, 300],
            0,
        )?;
        add(w, "STATIC", "", CAL_SUMMARY, [650, 588, 300, 40], 0)?;
        add(
            w,
            "BUTTON",
            "Apply calibration",
            CAL_APPLY,
            [650, 630, 170, 32],
            WS_TABSTOP,
        )?;
        add(
            w,
            "BUTTON",
            "Undo apply",
            CAL_UNDO,
            [422, 599, 163, 32],
            WS_TABSTOP,
        )?;
        combo(w, CAL_LIST, &["Current pressure"], 240, 690, 255)?;
        for (id, title, x, width) in [
            (CAL_LOAD, "Load", 507, 80),
            (CAL_SAVE, "Save as...", 599, 118),
            (CAL_IMPORT, "Import...", 729, 120),
            (CAL_EXPORT, "Export...", 861, 129),
        ] {
            add(w, "BUTTON", title, id, [x, 690, width, 28], WS_TABSTOP)?;
        }
        self.record_page(w, 6, &before);
        self.calibration_list(w);
        self.calibration_preview(w);
        self.calibration_buttons(w);
        Ok(())
    }
    fn calibration_list(&self, w: HWND) {
        message(w, CAL_LIST, CB_RESETCONTENT, 0, 0);
        for name in ["Current pressure"].into_iter().chain(
            self.pressure_library
                .presets
                .iter()
                .map(|p| p.name.as_str()),
        ) {
            message(w, CAL_LIST, CB_ADDSTRING, 0, wide(name).as_ptr() as isize);
        }
        message(w, CAL_LIST, CB_SETCURSEL, 0, 0);
    }
    fn calibration_preview(&self, w: HWND) {
        calibration_canvas::lift(control(w, CAL_PAD));
    }
    fn calibration_buttons(&self, w: HWND) {
        let cal = &self.calibration;
        let busy = cal.session.active;
        unsafe {
            EnableWindow(
                control(w, CAL_RECORD),
                i32::from(!busy && cal.session.stage < 3),
            );
            EnableWindow(control(w, CAL_CANCEL), i32::from(busy));
            for id in [CAL_SAVE, CAL_LOAD, CAL_IMPORT, CAL_EXPORT, CAL_LIST] {
                EnableWindow(control(w, id), i32::from(!busy));
            }
            EnableWindow(
                control(w, CAL_APPLY),
                i32::from(!busy && cal.draft.is_some()),
            );
            EnableWindow(
                control(w, CAL_UNDO),
                i32::from(
                    !busy
                        && cal.undo.as_ref().is_some_and(|(_, applied)| {
                            *applied == Shape::from_config(&self.config)
                        }),
                ),
            );
        }
        if cal.session.stage < 3 {
            text(
                w,
                CAL_RECORD,
                &format!("Record {}", ["light", "medium", "firm"][cal.session.stage]),
            );
            text(
                w,
                CAL_HEADING,
                &format!(
                    "{} of 3: {} strokes",
                    cal.session.stage + 1,
                    ctl460_rust::calibration::STAGES[cal.session.stage]
                ),
            );
        } else {
            text(w, CAL_HEADING, "Calibration complete");
        }
    }
    fn calibration_arm(&mut self, w: HWND) -> Result<(), String> {
        use ctl460_rust::ipc::Mapping;
        let stream = Mapping::open_reader()
            .map_err(|_| "Start the driver before recording pressure.".to_string())?;
        if !stream.active() {
            return Err("The driver is not responding. Start it, then try again.".into());
        }
        if self.calibration.session.stage >= 3 {
            return Ok(());
        }
        self.calibration.sequence = stream.latest();
        self.calibration.owner = stream.metadata().0;
        self.calibration.stream = Some(stream);
        self.calibration.session.arm(unsafe { GetTickCount64() });
        calibration_canvas::clear(control(w, CAL_PAD));
        calibration_canvas::recording(control(w, CAL_PAD), true);
        message(w, CAL_PROGRESS, PBM_SETPOS, 0, 0);
        text(w,CAL_STATUS,"Draw inside the square. The timer begins with your first stroke; strokes elsewhere are ignored.");
        self.calibration_buttons(w);
        Ok(())
    }
    fn calibration_poll(&mut self, w: HWND) {
        use ctl460_rust::{
            calibration::STAGE_MS,
            ipc::{Mapping, CAPACITY},
        };
        let visible = message(w, PAGE, TCM_GETCURSEL, 0, 0) == 6;
        if !visible && !self.calibration.session.active {
            return;
        }
        let now = unsafe { GetTickCount64() };
        if self.calibration.stream.is_none() {
            self.calibration.stream = Mapping::open_reader().ok();
        }
        let mut pad_rect = RECT::default();
        unsafe {
            GetWindowRect(control(w, CAL_PAD), &mut pad_rect);
        }
        let pad_available =
            visible && unsafe { GetForegroundWindow() == w && IsWindowEnabled(w) != 0 };
        let response = self
            .calibration
            .draft
            .clone()
            .unwrap_or_else(|| Shape::from_config(&self.config));
        let mut pad_config = self.config.clone();
        response.apply(&mut pad_config);
        let cal = &mut self.calibration;
        let mut fault = None;
        if let Some(stream) = &cal.stream {
            let latest = stream.latest();
            if !stream.active() {
                text(w, CAL_METER, "Raw pressure: -- / 1023");
                fault = Some("Driver disconnected. Start it and retry this stage.");
            } else {
                if let Some(pen) = stream.read(latest) {
                    text(w, CAL_METER, &format!("Raw pressure: {} / 1023", pen.raw));
                } else {
                    text(w, CAL_METER, "Raw pressure: -- / 1023");
                }
                if cal.session.active {
                    if stream.metadata().0 != cal.owner || latest < cal.sequence {
                        fault = Some("Driver restarted. Please record this stage again.");
                    } else if latest - cal.sequence > CAPACITY as u64 {
                        fault = Some("Recording was interrupted. Please retry this stage.");
                    } else {
                        for seq in cal.sequence + 1..=latest {
                            if let Some(pen) = stream.read(seq) {
                                let local = ctl460_rust::calibration_pad::local_position(
                                    pen.x,
                                    pen.y,
                                    unsafe {
                                        (
                                            GetSystemMetrics(SM_CXSCREEN),
                                            GetSystemMetrics(SM_CYSCREEN),
                                        )
                                    },
                                    (pad_rect.left, pad_rect.top, pad_rect.right, pad_rect.bottom),
                                );
                                let (x, y) = local.unwrap_or((-1., -1.));
                                let touching = pad_available
                                    && local.is_some()
                                    && pen.in_range()
                                    && pen.flags & 1 != 0
                                    && pen.flags & 6 == 0;
                                cal.session.sample(now, pen.raw, touching);
                                calibration_canvas::feed(
                                    control(w, CAL_PAD),
                                    x,
                                    y,
                                    pad_config.curve_at(pen.raw as f64),
                                    touching,
                                );
                            } else {
                                fault =
                                    Some("A pressure sample was missed. Please retry this stage.");
                                break;
                            }
                        }
                        cal.sequence = latest;
                    }
                }
            }
        } else {
            text(w, CAL_METER, "Raw pressure: -- / 1023");
        }
        if let Some(reason) = fault {
            if cal.session.active {
                cal.session.cancel();
                calibration_canvas::recording(control(w, CAL_PAD), false);
                text(w, CAL_STATUS, reason);
            }
            cal.stream = None;
        }
        if let Some(start) = cal.session.started.filter(|_| cal.session.active) {
            let elapsed = now.saturating_sub(start).min(STAGE_MS);
            message(w, CAL_PROGRESS, PBM_SETPOS, elapsed as usize, 0);
            text(
                w,
                CAL_STATUS,
                &format!(
                    "Recording {} strokes: {} seconds left.\r\nLift between strokes.",
                    ctl460_rust::calibration::STAGES[cal.session.stage].to_lowercase(),
                    (STAGE_MS - elapsed).div_ceil(1000)
                ),
            );
        }
        self.calibration_finish_stage(w, now);
        self.calibration_buttons(w);
    }
    fn calibration_finish_stage(&mut self, w: HWND, now: u64) {
        let cal = &mut self.calibration;
        if let Some(result) = cal.session.tick(now) {
            calibration_canvas::recording(control(w, CAL_PAD), false);
            match result {
                Err(e) => {
                    text(w, CAL_STATUS, &e);
                    cal.continue_stage = None;
                    cal.notice = Some(e);
                    unsafe {
                        PostMessageW(w, CAL_NOTICE, 0, 0);
                    }
                }
                Ok(None) => {
                    text(
                        w,
                        CAL_STATUS,
                        "Stage captured. Follow the popup to continue.",
                    );
                    cal.continue_stage = Some(cal.session.stage);
                    cal.notice = Some(format!(
                        "Stage complete. Next: {} strokes.\r\n\r\n{}",
                        ctl460_rust::calibration::STAGES[cal.session.stage],
                        ctl460_rust::calibration::stage_prompt(cal.session.stage)
                    ));
                    unsafe {
                        PostMessageW(w, CAL_NOTICE, 0, 0);
                    }
                }
                Ok(Some(shape)) => {
                    cal.draft = Some(shape);
                    cal.notice=Some("Calibration complete. All three pressure levels were captured. Click Apply calibration to use the response, or Save as to keep a named profile.".into());
                    unsafe {
                        PostMessageW(w, CAL_NOTICE, 0, 0);
                    }
                    text(w,CAL_STATUS,"Calibration complete. Apply calibration to use it, or Save as a named profile.");
                    text(
                        w,
                        CAL_SUMMARY,
                        &format!(
                            "Light / medium / firm: {} / {} / {}\r\nCalibration ready to apply.",
                            cal.session.anchors[0], cal.session.anchors[1], cal.session.anchors[2]
                        ),
                    );
                }
            }
            self.calibration_preview(w);
        }
        self.calibration_buttons(w);
    }
    fn calibration_apply(&mut self, w: HWND, shape: Shape) -> Result<(), String> {
        ctl460_rust::calibration::validate_shape(&shape)?;
        self.save(w)?;
        let previous = Shape::from_config(&self.config);
        shape.apply(&mut self.config);
        self.curve_fields(w);
        if let Err(e) = self.save(w) {
            previous.apply(&mut self.config);
            self.curve_fields(w);
            return Err(e);
        }
        self.calibration.undo = Some((previous, shape.clone()));
        self.pressure_draft = shape;
        self.pressure_list(w);
        text(w,CAL_STATUS,"Pressure curve saved. Lift the pen to apply it, then try light-to-firm strokes. Undo apply restores the previous curve.");
        self.calibration_buttons(w);
        Ok(())
    }
    fn calibration_command(&mut self, w: HWND, id: i32) -> Result<(), String> {
        match id {
            CAL_RECORD => return self.calibration_arm(w),
            CAL_CLEAR => calibration_canvas::clear(control(w, CAL_PAD)),
            CAL_CANCEL => {
                self.calibration.session.cancel();
                calibration_canvas::recording(control(w, CAL_PAD), false);
                text(w,CAL_STATUS,"Stage cancelled. Your pressure settings are unchanged; record this stage again when ready.");
            }
            CAL_RESTART => {
                self.calibration.continue_stage = None;
                self.calibration.notice = None;
                self.calibration.session = Default::default();
                calibration_canvas::recording(control(w, CAL_PAD), false);
                calibration_canvas::clear(control(w, CAL_PAD));
                self.calibration.draft = None;
                text(
                    w,
                    CAL_STATUS,
                    "Ready. Click Record light when you want to begin.",
                );
                text(w, CAL_SUMMARY, "");
                message(w, CAL_PROGRESS, PBM_SETPOS, 0, 0);
                self.calibration_preview(w);
            }
            CAL_APPLY => {
                if let Some(shape) = self.calibration.draft.clone() {
                    return self.calibration_apply(w, shape);
                }
            }
            CAL_UNDO => {
                if let Some((previous, applied)) = self.calibration.undo.clone() {
                    if Shape::from_config(&self.config) != applied {
                        return Err(
                            "Pressure was edited since Apply. Load a saved profile instead.".into(),
                        );
                    }
                    self.calibration_apply(w, previous)?;
                    self.calibration.undo = None;
                    text(w, CAL_STATUS, "Previous pressure curve restored.");
                }
            }
            CAL_LOAD => {
                let index = choice(w, CAL_LIST);
                self.calibration.draft = Some(if index == 0 {
                    Shape::from_config(&self.config)
                } else {
                    self.pressure_library
                        .presets
                        .get(index - 1)
                        .ok_or("Choose a saved profile.")?
                        .shape
                        .clone()
                });
                text(
                    w,
                    CAL_STATUS,
                    "Profile loaded. Click Apply calibration to use it.",
                );
                text(w, CAL_SUMMARY, "Loaded pressure profile. Not applied yet.");
                self.calibration_preview(w);
            }
            _ => {}
        }
        self.calibration_buttons(w);
        Ok(())
    }
    fn calibration_save(&mut self, w: HWND, name: &str) -> Result<(), String> {
        let shape = self
            .calibration
            .draft
            .clone()
            .unwrap_or_else(|| Shape::from_config(&self.config));
        let mut library = self.pressure_library.clone();
        library.add(name, shape)?;
        library.save(&self.settings.join("pressure-profiles.toml"))?;
        self.pressure_library = library;
        self.pressure_list(w);
        self.calibration_list(w);
        text(
            w,
            CAL_STATUS,
            "Named pressure profile saved. You can load it here or from the Pen tab.",
        );
        Ok(())
    }
}
