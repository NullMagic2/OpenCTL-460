//! Native Windows control panel for profiles, handedness, side-button actions and driver lifecycle.
//! Uses windows-sys (windows-rs), stores settings in LocalAppData, and requests
//! graceful worker shutdown through a named event. The GUI never injects pen or keyboard input.
#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    eprintln!("The control panel requires Windows 11.");
}

#[cfg(windows)]
#[path = "../gui_prompt.rs"]
mod prompt;
#[cfg(windows)]
#[path = "../gui_shortcut.rs"]
mod shortcut_prompt;
#[cfg(windows)]
#[path = "../gui_startup.rs"]
mod startup;
#[cfg(windows)]
#[path = "../gui_tray.rs"]
mod tray;
#[cfg(windows)]
#[path = "../gui_visuals.rs"]
mod visuals;

#[cfg(windows)]
mod app {
    use super::{prompt, shortcut_prompt, startup, tray, visuals};
    use ctl460_rust::pressure_profiles::{Library, Shape};
    use ctl460_rust::{
        config::{Config, BUTTON_ACTIONS},
        protocol::{PRODUCT_ID, VENDOR_ID},
    };
    use std::{
        ffi::c_void,
        fs,
        os::windows::process::CommandExt,
        path::PathBuf,
        process::{Child, Command, Stdio},
        ptr::{null, null_mut},
    };
    use windows_sys::Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::{LibraryLoader::GetModuleHandleW, Threading::*},
        UI::{
            Controls::*,
            HiDpi::*,
            Input::KeyboardAndMouse::{EnableWindow, IsWindowEnabled},
            WindowsAndMessaging::*,
        },
    };

    // Static-control styles from WinUser.h, omitted by this windows-sys feature set.
    const SS_CENTER: u32 = 1;
    const SS_CENTERIMAGE: u32 = 0x200;
    const DEVICE: i32 = 100;
    const PROFILE: i32 = 110;
    const START_WINDOWS: i32 = 225;
    const GAMMA: i32 = 120;
    const SMOOTH: i32 = 121;
    const RAMP: i32 = 122;
    const STROKE: i32 = 123;
    const TILT: i32 = 124;
    const LEFT: i32 = 125;
    const RIGHT: i32 = 215;
    const ASPECT: i32 = 126;
    const LEGACY: i32 = 127;
    const B1: i32 = 130;
    const B2: i32 = 131;
    const SHORTCUT1: i32 = 240;
    const SHORTCUT2: i32 = 241;
    const TILTMAX: i32 = 132;
    const STATUS: i32 = 150;
    const FLOOR: i32 = 160;
    const CEILING: i32 = 161;
    const GAIN: i32 = 162;
    const PRESS_ON: i32 = 163;
    const PRESS_OFF: i32 = 164;
    const WRITING: i32 = 165;
    const WRITING_FILTER: i32 = 167;
    const WRITING_HINT: i32 = 239;
    const PEN_SETTINGS_TITLE: i32 = 275;
    const BACKEND: i32 = 166;
    const PAGE: i32 = 170;
    const CURVE: i32 = 210;
    const SENSITIVITY: i32 = 211;
    const DOUBLE_DISTANCE: i32 = 218;
    const PREVIEW_CURVE: u32 = WM_APP + 20;
    // commctrl.h defines TBM_GETPOS as WM_USER; windows-sys omits this alias.
    const TBM_GETPOS: u32 = WM_USER;
    const LINE_IDS: [i32; 7] = [180, 181, 182, 183, 184, 185, 186];
    const PRESETS: &[(&str, &str)] = &[
        (
            "Natural drawing",
            include_str!("../../profiles/natural.toml"),
        ),
        (
            "Handwriting",
            include_str!("../../profiles/handwriting.toml"),
        ),
        (
            "No smoothing (diagnostic)",
            include_str!("../../profiles/raw.toml"),
        ),
        (
            "Pencil - simulated tilt",
            include_str!("../../profiles/pencil_experimental.toml"),
        ),
        (
            "Smooth inking",
            include_str!("../../profiles/smooth_inking.toml"),
        ),
    ];
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    fn control(window: HWND, id: i32) -> HWND {
        unsafe { GetDlgItem(window, id) }
    }
    fn message(window: HWND, id: i32, msg: u32, w: usize, l: isize) -> isize {
        unsafe { SendMessageW(control(window, id), msg, w, l) }
    }
    fn text(window: HWND, id: i32, value: &str) {
        // Avoid repainting labels and generating EN_CHANGE for unchanged drag values.
        if read(window, id) == value {
            return;
        }
        unsafe {
            SetWindowTextW(control(window, id), wide(value).as_ptr());
        }
    }
    fn read(window: HWND, id: i32) -> String {
        let mut buf = [0u16; 256];
        let n = unsafe { GetWindowTextW(control(window, id), buf.as_mut_ptr(), buf.len() as i32) };
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }
    fn checked(window: HWND, id: i32) -> bool {
        message(window, id, BM_GETCHECK, 0, 0) == BST_CHECKED as isize
    }
    /// Optical center of a single centered STATIC's visible text, in screen pixels.
    /// Font line boxes include blank ascent/descent space, so their center does
    /// not reliably line up with the trackbar channel on high-DPI displays.
    fn label_ink_center(window: HWND, id: i32) -> i32 {
        unsafe {
            let label = control(window, id);
            let mut rect = RECT::default();
            GetWindowRect(label, &mut rect);
            let fallback = (rect.top + rect.bottom) / 2;
            let dc = GetDC(label);
            if dc.is_null() {
                return fallback;
            }
            let font = SendMessageW(label, WM_GETFONT, 0, 0) as HFONT;
            let previous = SelectObject(dc, font as _);
            let mut metrics = TEXTMETRICW::default();
            let mut top = i32::MAX;
            let mut bottom = i32::MIN;
            if GetTextMetricsW(dc, &mut metrics) != 0 {
                let mut transform = MAT2::default();
                transform.eM11.value = 1;
                transform.eM22.value = 1;
                for ch in read(window, id).encode_utf16() {
                    let mut glyph = GLYPHMETRICS::default();
                    if GetGlyphOutlineW(
                        dc,
                        ch as u32,
                        GGO_METRICS,
                        &mut glyph,
                        0,
                        null_mut(),
                        &transform,
                    ) != GDI_ERROR as u32
                        && glyph.gmBlackBoxY > 0
                    {
                        top = top.min(-glyph.gmptGlyphOrigin.y);
                        bottom = bottom.max(-glyph.gmptGlyphOrigin.y + glyph.gmBlackBoxY as i32);
                    }
                }
            }
            SelectObject(dc, previous);
            ReleaseDC(label, dc);
            if top == i32::MAX {
                return fallback;
            }
            let baseline =
                rect.top + (rect.bottom - rect.top - metrics.tmHeight) / 2 + metrics.tmAscent;
            baseline + (top + bottom) / 2
        }
    }
    fn check(window: HWND, id: i32, yes: bool) {
        message(
            window,
            id,
            BM_SETCHECK,
            if yes {
                BST_CHECKED as usize
            } else {
                BST_UNCHECKED as usize
            },
            0,
        );
    }
    fn choice(window: HWND, id: i32) -> usize {
        message(window, id, CB_GETCURSEL, 0, 0).max(0) as usize
    }
    fn error(window: HWND, value: &str) {
        unsafe {
            MessageBoxW(
                window,
                wide(value).as_ptr(),
                wide("OpenCTL 460").as_ptr(),
                MB_OK | MB_ICONERROR,
            );
        }
    }

    // All HWND/HINSTANCE arguments in these wrappers originate from this process. Strings are
    // NUL-terminated and remain alive for each synchronous Win32 call; no raw pointer is retained.
    fn add(
        window: HWND,
        class: &str,
        label: &str,
        id: i32,
        rect: [i32; 4],
        style: u32,
    ) -> Result<(), String> {
        let [x, y, w, h] = rect;
        let child = unsafe {
            CreateWindowExW(
                0,
                wide(class).as_ptr(),
                wide(label).as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                x,
                y,
                w,
                h,
                window,
                id as usize as HMENU,
                GetModuleHandleW(null()),
                null(),
            )
        };
        if child.is_null() {
            return Err(format!("Cannot create control {id}"));
        }
        unsafe {
            SendMessageW(
                child,
                WM_SETFONT,
                GetStockObject(DEFAULT_GUI_FONT) as usize,
                1,
            );
        }
        Ok(())
    }
    fn label(window: HWND, title: &str, x: i32, y: i32, w: i32) -> Result<(), String> {
        add(window, "STATIC", title, 0, [x, y, w, 24], 0)
    }
    fn combo(window: HWND, id: i32, labels: &[&str], x: i32, y: i32, w: i32) -> Result<(), String> {
        add(
            window,
            "COMBOBOX",
            "",
            id,
            [x, y, w, 220],
            (if [B1, B2].contains(&id) {
                CBS_DROPDOWN
            } else {
                CBS_DROPDOWNLIST
            }) as u32
                | WS_TABSTOP
                | WS_VSCROLL,
        )?;
        if [B1, B2].contains(&id) {
            // Display a recorded chord without adding it as a selectable action.
            // Editing still goes through the dedicated capture button.
            let mut info = COMBOBOXINFO {
                cbSize: std::mem::size_of::<COMBOBOXINFO>() as u32,
                ..Default::default()
            };
            unsafe {
                if GetComboBoxInfo(control(window, id), &mut info) != 0 {
                    SendMessageW(info.hwndItem, EM_SETREADONLY, 1, 0);
                }
            }
        }
        for value in labels {
            let value = wide(value);
            message(window, id, CB_ADDSTRING, 0, value.as_ptr() as isize);
        }
        message(window, id, CB_SETCURSEL, 0, 0);
        Ok(())
    }
    struct State {
        root: PathBuf,
        settings: PathBuf,
        recording: bool,
        startup_directory: PathBuf,
        config: Config,
        button_shortcuts: [String; 2],
        last_saved: String,
        settings_error: bool,
        child: Option<Child>,
        hid_session: bool,
        event: HANDLE,
        closing: bool,
        minimize_requested: bool,
        auto_start: bool,
        start_pending: bool,
        count: usize,
        paths: Vec<String>,
        pages: [Vec<HWND>; 5],
        font: HFONT,
        bold_font: HFONT,
        title_font: HFONT,
        preview: Option<PathBuf>,
        preview_page: usize,
        layout: Vec<(HWND, RECT)>,
        pressure_library: Library,
        pressure_draft: Shape,
    }
    impl Drop for State {
        fn drop(&mut self) {
            if !self.title_font.is_null() {
                unsafe {
                    DeleteObject(self.title_font as _);
                }
            }
            if !self.bold_font.is_null() {
                unsafe {
                    DeleteObject(self.bold_font as _);
                }
            }
            if !self.font.is_null() {
                unsafe {
                    DeleteObject(self.font as _);
                }
            }
        }
    }
    impl State {
        // Page children retain their parent so commands, accessibility and tab order stay native.
        fn children(w: HWND) -> Vec<HWND> {
            let mut result = Vec::new();
            unsafe {
                let mut child = GetWindow(w, GW_CHILD);
                while !child.is_null() {
                    result.push(child);
                    child = GetWindow(child, GW_HWNDNEXT);
                }
            }
            result
        }
        fn record_page(&mut self, w: HWND, page: usize, before: &[HWND]) {
            self.pages[page] = Self::children(w)
                .into_iter()
                .filter(|h| !before.contains(h))
                .collect();
        }
        fn show_page(&self, w: HWND) {
            let selected = message(w, PAGE, TCM_GETCURSEL, 0, 0).max(0) as usize;
            unsafe {
                for (page, children) in self.pages.iter().enumerate() {
                    for child in children {
                        ShowWindow(*child, if page == selected { SW_SHOW } else { SW_HIDE });
                    }
                }
                InvalidateRect(w, null(), 1);
            }
        }
        fn line_labels(&self, w: HWND) {
            for id in LINE_IDS {
                text(
                    w,
                    id + 20,
                    &format!("{}%", message(w, id, TBM_GETPOS, 0, 0)),
                );
            }
        }
        fn writing_hint(&self, w: HWND) {
            text(
                w,
                WRITING_HINT,
                match choice(w, WRITING_FILTER) {
                    0 => "Line and circle sliders also apply.",
                    3 => "Wobble sets Responsive strength.",
                    4 => "Saved feel applies in both modes.",
                    _ => "Line and circle sliders also apply.",
                },
            );
        }
        fn edit(w: HWND, id: i32, title: &str, x: i32, y: i32, width: i32) -> Result<(), String> {
            label(w, title, x, y, width)?;
            add(
                w,
                "EDIT",
                "",
                id,
                [x, y + 25, width, 27],
                WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32,
            )
        }
        fn checkbox(
            w: HWND,
            id: i32,
            title: &str,
            x: i32,
            y: i32,
            width: i32,
        ) -> Result<(), String> {
            add(
                w,
                "BUTTON",
                title,
                id,
                [x, y, width, 28],
                BS_AUTOCHECKBOX as u32 | WS_TABSTOP,
            )
        }
        fn controls(&mut self, w: HWND) -> Result<(), String> {
            add(
                w,
                "STATIC",
                "Bamboo CTL-460 settings",
                222,
                [20, 24, 170, 58],
                0,
            )?;
            add(
                w,
                "STATIC",
                &format!("Version {}", ctl460_rust::VERSION),
                224,
                [20, 86, 170, 24],
                0,
            )?;
            add(w, "STATIC", "Tablet:", 221, [20, 124, 170, 24], SS_CENTER)?;
            combo(w, DEVICE, &[], 20, 158, 170)?;
            add(w, "BUTTON", "Refresh", 101, [20, 200, 170, 30], WS_TABSTOP)?;
            add(w, "STATIC", "Drawing profile", 223, [20, 274, 170, 24], 0)?;
            combo(
                w,
                PROFILE,
                &PRESETS.iter().map(|p| p.0).collect::<Vec<_>>(),
                20,
                308,
                170,
            )?;
            add(
                w,
                "STATIC",
                "Pen settings",
                PEN_SETTINGS_TITLE,
                [220, 24, 400, 24],
                0,
            )?;
            add(
                w,
                "BUTTON",
                "  Initialize when\r\n  Windows starts",
                START_WINDOWS,
                [20, 376, 175, 54],
                WS_TABSTOP | BS_AUTOCHECKBOX as u32 | BS_MULTILINE as u32 | BS_TOP as u32,
            )?;
            check(
                w,
                START_WINDOWS,
                self.startup_directory.join("OpenCTL 460.lnk").is_file(),
            );
            label(w, "Make the pen feel right for you.", 220, 52, 650)?;
            add(
                w,
                "SysTabControl32",
                "Settings pages",
                PAGE,
                [220, 90, 780, 35],
                WS_TABSTOP | TCS_FOCUSONBUTTONDOWN,
            )?;
            for (index, title) in ["Pen", "Buttons", "Mapping", "Line smoothing", "Advanced"]
                .iter()
                .enumerate()
            {
                let mut title = wide(title);
                let item = TCITEMW {
                    mask: TCIF_TEXT,
                    pszText: title.as_mut_ptr(),
                    ..Default::default()
                };
                message(
                    w,
                    PAGE,
                    TCM_INSERTITEMW,
                    index,
                    &item as *const TCITEMW as isize,
                );
            }
            add(w, "CTL460Separator", "", 220, [226, 728, 784, 2], 0)?;
            for (id, title, x, width) in [
                (141, "Start driver", 240, 240),
                (142, "Stop driver", 492, 240),
                (143, "Open log", 744, 240),
            ] {
                add(w, "BUTTON", title, id, [x, 742, width, 34], WS_TABSTOP)?;
            }
            add(
                w,
                "STATIC",
                "Stopped. Settings save automatically.",
                STATUS,
                [240, 782, 754, 38],
                0,
            )?;
            add(
                w,
                "STATIC",
                "Pressure levels: 1024 physical, 4098 software",
                151,
                [240, 826, 754, 24],
                0,
            )?;
            let before = Self::children(w);
            add(
                w,
                "CTL460SunkenPanel",
                "",
                300,
                [226, 134, 366, 446],
                WS_CLIPSIBLINGS,
            )?;
            add(
                w,
                "CTL460SunkenPanel",
                "",
                301,
                [594, 134, 416, 446],
                WS_CLIPSIBLINGS,
            )?;
            label(w, "Pen", 240, 146, 280)?;
            add(
                w,
                "CTL460PenPicture",
                "Pen illustration",
                214,
                [240, 184, 90, 350],
                0,
            )?;
            label(w, "Tip sensitivity", 350, 392, 220)?;
            add(
                w,
                "msctls_trackbar32",
                "Tip sensitivity",
                SENSITIVITY,
                [350, 424, 220, 32],
                WS_TABSTOP | TBS_AUTOTICKS,
            )?;
            message(w, SENSITIVITY, TBM_SETRANGE, 1, 100 << 16);
            message(w, SENSITIVITY, TBM_SETTICFREQ, 10, 0);
            label(w, "Soft", 350, 460, 100)?;
            label(w, "Firm", 532, 460, 44)?;
            label(w, "Double-click distance", 350, 486, 230)?;
            add(
                w,
                "CTL460DistanceRamp",
                "Smaller to larger distance",
                219,
                [350, 510, 220, 17],
                0,
            )?;
            add(
                w,
                "msctls_trackbar32",
                "Double-click distance",
                DOUBLE_DISTANCE,
                [350, 527, 220, 25],
                WS_TABSTOP | TBS_AUTOTICKS,
            )?;
            message(w, DOUBLE_DISTANCE, TBM_SETRANGE, 1, 40 << 16);
            message(w, DOUBLE_DISTANCE, TBM_SETTICFREQ, 10, 0);
            label(w, "Off / smaller", 350, 552, 130)?;
            label(w, "Larger", 522, 552, 58)?;
            label(w, "Pressure curve", 608, 146, 386)?;
            combo(w, 230, &["Default", "Unsaved copy"], 608, 176, 208)?;
            add(w, "BUTTON", "New", 232, [824, 176, 76, 28], WS_TABSTOP)?;
            add(w, "BUTTON", "Delete", 234, [908, 176, 86, 28], WS_TABSTOP)?;
            add(
                w,
                "CTL460PressureCurve",
                "Editable pressure curve",
                CURVE,
                [608, 208, 386, 262],
                WS_TABSTOP,
            )?;
            add(
                w,
                "STATIC",
                "Drag to edit a copy. Default stays unchanged.",
                235,
                [608, 476, 386, 24],
                0,
            )?;
            add(
                w,
                "BUTTON",
                "Add node",
                236,
                [608, 510, 185, 28],
                WS_TABSTOP,
            )?;
            add(
                w,
                "BUTTON",
                "Remove node",
                237,
                [809, 510, 185, 28],
                WS_TABSTOP,
            )?;
            add(
                w,
                "BUTTON",
                "Save as...",
                233,
                [809, 546, 185, 28],
                WS_TABSTOP,
            )?;
            add(
                w,
                "BUTTON",
                "Reset curve",
                212,
                [608, 546, 185, 28],
                WS_TABSTOP,
            )?;
            self.record_page(w, 0, &before);
            let before = Self::children(w);
            add(
                w,
                "CTL460SunkenPanel",
                "",
                302,
                [226, 134, 784, 300],
                WS_CLIPSIBLINGS,
            )?;
            add(
                w,
                "CTL460SunkenPanel",
                "",
                303,
                [226, 446, 784, 118],
                WS_CLIPSIBLINGS,
            )?;
            label(w, "Tablet orientation", 240, 154, 700)?;
            for (id, title, x, image_id) in [
                (LEFT, "Left-handed", 290, 216),
                (RIGHT, "Right-handed", 670, 217),
            ] {
                add(
                    w,
                    "CTL460PenPicture",
                    title,
                    image_id,
                    [x, 196, 260, 176],
                    0,
                )?;
                add(
                    w,
                    "BUTTON",
                    title,
                    id,
                    [x + 50, 385, 180, 28],
                    BS_RADIOBUTTON as u32 | WS_TABSTOP,
                )?;
            }
            Self::checkbox(
                w,
                ASPECT,
                "Keep circles and letters in proportion",
                240,
                462,
                700,
            )?;
            label(
                w,
                "Proportions use a centered tablet area to reach every screen edge.",
                240,
                510,
                700,
            )?;
            self.record_page(w, 2, &before);
            let before = Self::children(w);
            add(
                w,
                "CTL460SunkenPanel",
                "",
                304,
                [226, 134, 784, 482],
                WS_CLIPSIBLINGS,
            )?;
            add(
                w,
                "CTL460SunkenPanel",
                "",
                305,
                [226, 620, 784, 103],
                WS_CLIPSIBLINGS,
            )?;
            for (i, (title, help)) in [
                (
                    "StreamLine - Line shape",
                    "Smooth the path; higher values add lag.",
                ),
                (
                    "StreamLine - Pressure",
                    "Smooth thickness changes; not the line path.",
                ),
                (
                    "Stabilization - Amount",
                    "More averaging at higher drawing speeds.",
                ),
                ("Motion Filtering - Amount", "Reduce sideways wobble."),
                (
                    "Motion Filtering - Expression",
                    "Keep some natural variation.",
                ),
                (
                    "Wobble reduction",
                    "Steady hover; contact depends on base smoothing.",
                ),
                (
                    "Circles and curves",
                    "Reduce curve wobble; 0% turns assistance off.",
                ),
            ]
            .iter()
            .enumerate()
            {
                // Keep a full text-line gap after each description.
                let y = 144 + i as i32 * 68;
                let id = LINE_IDS[i];
                add(
                    w,
                    "STATIC",
                    title,
                    250 + i as i32,
                    [240, y, 350, 24],
                    SS_CENTERIMAGE,
                )?;
                label(w, help, 240, y + 27, 400)?;
                add(
                    w,
                    "msctls_trackbar32",
                    title,
                    id,
                    [650, y, 270, 32],
                    WS_TABSTOP | TBS_AUTOTICKS,
                )?;
                message(w, id, TBM_SETRANGE, 1, 100 << 16);
                message(w, id, TBM_SETTICFREQ, 10, 0);
                message(w, id, TBM_SETPAGESIZE, 0, 10);
                add(w, "STATIC", "0%", id + 20, [942, y, 58, 24], SS_CENTERIMAGE)?;
            }
            label(w, "Handwriting assistance", 240, 626, 340)?;
            combo(
                w,
                WRITING,
                &[
                    "Off - drawing settings",
                    "On - manual handwriting",
                    "Auto - experimental detection",
                ],
                650,
                624,
                350,
            )?;
            Self::checkbox(w, STROKE, "Adaptive base smoothing", 240, 656, 380)?;
            add(
                w,
                "STATIC",
                "Artistic sliders also apply to handwriting.",
                WRITING_HINT,
                [650, 656, 350, 32],
                0,
            )?;
            label(w, "Base smoothing", 240, 690, 395)?;
            combo(
                w,
                WRITING_FILTER,
                &[
                    "Original",
                    "Raw base - no smoothing",
                    "Light smoothing",
                    "Responsive - experimental",
                    "Saved 0.3.0 feel (drawing + writing)",
                ],
                650,
                688,
                350,
            )?;
            self.record_page(w, 3, &before);
            let before = Self::children(w);
            add(
                w,
                "CTL460SunkenPanel",
                "",
                306,
                [226, 134, 784, 242],
                WS_CLIPSIBLINGS,
            )?;
            add(
                w,
                "CTL460SunkenPanel",
                "",
                307,
                [226, 383, 784, 195],
                WS_CLIPSIBLINGS,
            )?;
            for (i, (id, title)) in [
                (FLOOR, "Pressure zero point (0-1022)"),
                (CEILING, "Full pressure at (1-1023)"),
                (GAMMA, "Response curve (gamma)"),
                (GAIN, "Pressure gain (0.1-3.0)"),
                (PRESS_ON, "Start ink at (1-1023)"),
                (PRESS_OFF, "Stop ink at (0-1022)"),
                (SMOOTH, "Pressure smoothing (ms)"),
                (RAMP, "Pressure interpolation (ms)"),
                (TILTMAX, "Simulated tilt limit (degrees)"),
            ]
            .iter()
            .enumerate()
            {
                Self::edit(
                    w,
                    *id,
                    title,
                    240 + (i % 3) as i32 * 254,
                    150 + (i / 3) as i32 * 78,
                    232,
                )?;
            }
            Self::checkbox(w, TILT, "Simulate tilt from stroke motion", 240, 397, 700)?;
            Self::checkbox(
                w,
                LEGACY,
                "Legacy 11-byte reports (only when required)",
                240,
                439,
                700,
            )?;
            label(w, "Windows pen output", 240, 499, 320)?;
            combo(
                w,
                BACKEND,
                &["Windows Ink (user mode)", "Virtual HID (installed service)"],
                650,
                496,
                350,
            )?;
            label(
                w,
                "Virtual HID uses the installed service; no permission prompt on startup.",
                240,
                542,
                760,
            )?;
            self.record_page(w, 4, &before);
            let before = Self::children(w);
            add(
                w,
                "CTL460SunkenPanel",
                "",
                308,
                [226, 134, 184, 446],
                WS_CLIPSIBLINGS,
            )?;
            label(w, "Pen buttons", 240, 146, 160)?;
            add(
                w,
                "CTL460PenPicture",
                "Pen illustration",
                243,
                [270, 185, 90, 350],
                0,
            )?;
            add(
                w,
                "BUTTON",
                "Reset settings",
                244,
                [240, 542, 156, 30],
                WS_TABSTOP,
            )?;
            let actions: Vec<&str> = BUTTON_ACTIONS
                .iter()
                .map(|a| match *a {
                    "Erase (hold)" => "Eraser (hold)",
                    "Undo (Ctrl+Z)" => "Undo",
                    "Redo (Ctrl+Y)" => "Redo",
                    "Eraser (E)" => "Eraser tool",
                    "Brush (B)" => "Brush",
                    "Pan (hold Space)" => "Pan (hold)",
                    other => other,
                })
                .collect();
            for (i, (id, shortcut_id, title)) in [
                (B1, SHORTCUT1, "Button 1 (upper switch)"),
                (B2, SHORTCUT2, "Button 2 (lower switch)"),
            ]
            .into_iter()
            .enumerate()
            {
                let y = 134 + i as i32 * 212;
                add(
                    w,
                    "CTL460SunkenPanel",
                    "",
                    309 + i as i32,
                    [420, y, 590, 202],
                    WS_CLIPSIBLINGS,
                )?;
                label(w, title, 438, y + 12, 545)?;
                combo(w, id, &actions, 438, y + 48, 548)?;
                label(w, "Keyboard shortcut", 438, y + 92, 200)?;
                add(
                    w,
                    "BUTTON",
                    "Set shortcut...",
                    shortcut_id,
                    [648, y + 88, 338, 30],
                    WS_TABSTOP,
                )?;
                add(w, "STATIC", "", 245 + i as i32, [438, y + 132, 548, 24], 0)?;
                label(
                    w,
                    "Shortcut runs once per press. Mouse clicks can be held.",
                    438,
                    y + 162,
                    548,
                )?;
            }
            label(
                w,
                "Eraser: hold the button while using the pen tip; release to draw.",
                420,
                560,
                590,
            )?;
            self.record_page(w, 1, &before);
            // Decoration is a sibling, not a container: keep its large rectangle behind inputs.
            unsafe {
                for id in 300..=310 {
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
            // Retain the original 96-DPI geometry to avoid cumulative rounding on monitor changes.
            unsafe {
                for child in Self::children(w) {
                    let mut r = RECT::default();
                    GetWindowRect(child, &mut r);
                    MapWindowPoints(null_mut(), w, &mut r as *mut RECT as *mut POINT, 2);
                    self.layout.push((child, r));
                }
            }
            self.scale_layout(w);
            self.fill(w);
            self.refresh(w)?;
            self.show_page(w);
            unsafe {
                SetTimer(w, 1, 150, None);
            }
            Ok(())
        }
        fn scale_layout(&mut self, w: HWND) {
            // Redraw native controls at monitor resolution rather than bitmap-stretching the window.
            unsafe {
                let dpi = GetDpiForWindow(w).max(96) as i32;
                for (kind, metric) in [(ICON_BIG, SM_CXICON), (ICON_SMALL, SM_CXSMICON)] {
                    let size = GetSystemMetricsForDpi(metric, dpi as u32);
                    // MAKEINTRESOURCEW(1) is an integer resource identifier, never dereferenced.
                    #[allow(clippy::manual_dangling_ptr)]
                    let icon = LoadImageW(
                        GetModuleHandleW(null()),
                        1usize as *const u16,
                        IMAGE_ICON,
                        size,
                        size,
                        LR_SHARED,
                    );
                    SendMessageW(w, WM_SETICON, kind as usize, icon as isize);
                }
                let px = |n: i32| (n * dpi + 48) / 96;
                let font = CreateFontW(
                    -px(16),
                    0,
                    0,
                    0,
                    400,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET as u32,
                    0,
                    0,
                    CLEARTYPE_QUALITY as u32,
                    0,
                    wide("Segoe UI").as_ptr(),
                );
                if font.is_null() {
                    return;
                }
                SendMessageW(w, WM_SETFONT, font as usize, 0);
                for &(child, r) in &self.layout {
                    SetWindowPos(
                        child,
                        null_mut(),
                        px(r.left),
                        px(r.top),
                        px(r.right - r.left),
                        px(r.bottom - r.top),
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                    SendMessageW(child, WM_SETFONT, font as usize, 1);
                }
                // Align the visible channel and percentage to the label's ink,
                // after native font sizing. Do not center their padded windows.
                for (i, id) in LINE_IDS.iter().enumerate() {
                    let slider = control(w, *id);
                    let mut channel = RECT::default();
                    SendMessageW(
                        slider,
                        TBM_GETCHANNELRECT,
                        0,
                        &mut channel as *mut RECT as isize,
                    );
                    let mut track = RECT::default();
                    GetWindowRect(slider, &mut track);
                    let center = label_ink_center(w, 250 + i as i32);
                    let mut origin = POINT {
                        x: track.left,
                        y: center - (channel.top + channel.bottom) / 2,
                    };
                    ScreenToClient(w, &mut origin);
                    SetWindowPos(
                        slider,
                        null_mut(),
                        origin.x,
                        origin.y,
                        0,
                        0,
                        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                    let value = control(w, id + 20);
                    let mut value_rect = RECT::default();
                    GetWindowRect(value, &mut value_rect);
                    let mut value_origin = POINT {
                        x: value_rect.left,
                        y: value_rect.top + center - label_ink_center(w, id + 20),
                    };
                    ScreenToClient(w, &mut value_origin);
                    SetWindowPos(
                        value,
                        null_mut(),
                        value_origin.x,
                        value_origin.y,
                        0,
                        0,
                        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
                if !self.font.is_null() {
                    DeleteObject(self.font as _);
                }
                self.font = font;
                let bold = CreateFontW(
                    -px(16),
                    0,
                    0,
                    0,
                    700,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET as u32,
                    0,
                    0,
                    CLEARTYPE_QUALITY as u32,
                    0,
                    wide("Segoe UI").as_ptr(),
                );
                if !bold.is_null() {
                    SendMessageW(control(w, 221), WM_SETFONT, bold as usize, 1);
                    SendMessageW(control(w, 223), WM_SETFONT, bold as usize, 1);
                    SendMessageW(control(w, PEN_SETTINGS_TITLE), WM_SETFONT, bold as usize, 1);

                    if !self.bold_font.is_null() {
                        DeleteObject(self.bold_font as _);
                    }
                    self.bold_font = bold;
                    let title = CreateFontW(
                        -px(18),
                        0,
                        0,
                        0,
                        700,
                        0,
                        0,
                        0,
                        DEFAULT_CHARSET as u32,
                        0,
                        0,
                        CLEARTYPE_QUALITY as u32,
                        0,
                        wide("Segoe UI").as_ptr(),
                    );
                    if !title.is_null() {
                        SendMessageW(control(w, 222), WM_SETFONT, title as usize, 1);
                        if !self.title_font.is_null() {
                            DeleteObject(self.title_font as _);
                        }
                        self.title_font = title;
                    }
                }
                InvalidateRect(w, null(), 1);
            }
        }
        fn pressure_controls(&self, w: HWND) {
            let editable = choice(w, 230) != 0;
            unsafe {
                // The first graph edit changes the working copy, never the stored Default preset.
                EnableWindow(control(w, CURVE), 1);
                for id in [FLOOR, CEILING, GAIN, SENSITIVITY, 236, 237, 233] {
                    EnableWindow(control(w, id), i32::from(editable));
                }
                EnableWindow(
                    control(w, GAMMA),
                    i32::from(editable && self.config.pressure_nodes.is_empty()),
                );
                EnableWindow(control(w, 234), i32::from(choice(w, 230) >= 2));
            }
            text(
                w,
                235,
                if editable {
                    "Drag nodes, or click the curve to add one."
                } else {
                    "Drag to edit a copy. Default stays unchanged."
                },
            );
        }
        fn pressure_list(&self, w: HWND) {
            message(w, 230, CB_RESETCONTENT, 0, 0);
            for name in ["Default", "Unsaved copy"].into_iter().chain(
                self.pressure_library
                    .presets
                    .iter()
                    .map(|p| p.name.as_str()),
            ) {
                message(w, 230, CB_ADDSTRING, 0, wide(name).as_ptr() as isize);
            }
            let shape = Shape::from_config(&self.config);
            let index = if shape == Shape::from_config(&Config::default()) {
                0
            } else {
                self.pressure_library
                    .presets
                    .iter()
                    .position(|p| p.shape == shape)
                    .map_or(1, |i| i + 2)
            };
            message(w, 230, CB_SETCURSEL, index, 0);
            self.pressure_controls(w);
        }
        fn save_pressure(&mut self, w: HWND, name: &str) -> Result<(), String> {
            let shape = Shape::from_config(&visuals::get(control(w, CURVE)));
            let mut library = self.pressure_library.clone();
            library.add(name, shape.clone())?;
            library.save(&self.settings.join("pressure-profiles.toml"))?;
            self.pressure_library = library;
            shape.apply(&mut self.config);
            self.pressure_list(w);
            message(
                w,
                230,
                CB_SETCURSEL,
                self.pressure_library.presets.len() + 1,
                0,
            );
            self.pressure_controls(w);
            self.save(w)
        }
        fn fill(&mut self, w: HWND) {
            self.pressure_list(w);
            message(
                w,
                DOUBLE_DISTANCE,
                TBM_SETPOS,
                1,
                self.config.double_click_distance as isize,
            );
            for (id, value) in LINE_IDS.into_iter().zip([
                self.config.streamline_amount,
                self.config.streamline_pressure,
                self.config.stabilization_amount,
                self.config.motion_filter_amount,
                self.config.motion_filter_expression,
                self.config.wobble_reduction,
                self.config.circle_smoothing,
            ]) {
                message(w, id, TBM_SETPOS, 1, value.round() as isize);
            }
            message(
                w,
                LEGACY,
                BM_SETCHECK,
                usize::from(self.config.legacy_reports),
                0,
            );
            self.line_labels(w);
            visuals::set(control(w, CURVE), &self.config);
            message(
                w,
                SENSITIVITY,
                TBM_SETPOS,
                1,
                ((self.config.gamma / 0.2).ln() / 20.0_f64.ln() * 100.0).round() as isize,
            );
            for (id, value) in [
                (FLOOR, self.config.floor.to_string()),
                (CEILING, self.config.ceiling.to_string()),
                (GAIN, self.config.gain.to_string()),
                (PRESS_ON, self.config.press_on.to_string()),
                (PRESS_OFF, self.config.press_off.to_string()),
            ] {
                text(w, id, &value);
            }
            message(
                w,
                WRITING,
                CB_SETCURSEL,
                ["off", "on", "auto"]
                    .iter()
                    .position(|v| *v == self.config.handwriting_mode)
                    .unwrap_or(0),
                0,
            );
            message(
                w,
                WRITING_FILTER,
                CB_SETCURSEL,
                if self.config.pen_control.is_some() {
                    4
                } else {
                    ["legacy", "raw", "minimal", "responsive"]
                        .iter()
                        .position(|v| *v == self.config.handwriting_filter)
                        .unwrap_or(0)
                },
                0,
            );
            self.writing_hint(w);
            message(
                w,
                BACKEND,
                CB_SETCURSEL,
                usize::from(self.config.backend == "hid"),
                0,
            );
            text(w, GAMMA, &self.config.gamma.to_string());
            text(w, SMOOTH, &self.config.smoothing_ms.to_string());
            text(w, RAMP, &self.config.interpolation_ms.to_string());
            text(w, TILTMAX, &self.config.tilt_max_degrees.to_string());
            for (id, on) in [
                (STROKE, self.config.stroke_smoothing),
                (TILT, self.config.virtual_tilt),
                (LEFT, self.config.left_handed),
                (RIGHT, !self.config.left_handed),
                (ASPECT, self.config.preserve_aspect),
            ] {
                check(w, id, on);
            }
            for (id, shortcut_id, value) in [
                (B1, SHORTCUT1, &self.config.button1),
                (B2, SHORTCUT2, &self.config.button2),
            ] {
                let custom = value.strip_prefix(ctl460_rust::button_actions::CUSTOM);
                message(
                    w,
                    id,
                    CB_SETCURSEL,
                    if custom.is_some() {
                        usize::MAX
                    } else {
                        BUTTON_ACTIONS.iter().position(|s| s == value).unwrap_or(0)
                    },
                    0,
                );
                if let Some(chord) = custom {
                    text(w, id, chord);
                }
                self.button_shortcuts[usize::from(shortcut_id == SHORTCUT2)] =
                    ctl460_rust::button_actions::action_shortcut(value)
                        .unwrap_or("")
                        .into();
            }
            self.button_controls(w);
        }
        fn button_controls(&mut self, w: HWND) {
            for (index, (id, shortcut_id)) in
                [(B1, SHORTCUT1), (B2, SHORTCUT2)].into_iter().enumerate()
            {
                let action = BUTTON_ACTIONS
                    .get(message(w, id, CB_GETCURSEL, 0, 0) as usize)
                    .copied();
                if let Some(action) = action {
                    self.button_shortcuts[index] =
                        ctl460_rust::button_actions::action_shortcut(action)
                            .unwrap_or("")
                            .into();
                }
                let enabled = action
                    .is_none_or(|a| ctl460_rust::button_actions::action_shortcut(a).is_some());
                unsafe {
                    EnableWindow(control(w, shortcut_id), i32::from(enabled));
                }
                text(
                    w,
                    245 + index as i32,
                    match action {
                        Some("Erase (hold)") => "Hold the pen button to erase; release it to draw.",
                        Some("Pan (hold Space)") => {
                            "Pan holds Space until you release the pen button."
                        }
                        _ if enabled => "Click the button, then press a key or key combination.",
                        _ => "Select a keyboard action to set its shortcut.",
                    },
                );
            }
        }
        fn button_value(&self, w: HWND, id: i32, shortcut_id: i32) -> Result<String, String> {
            if let Some(action) = BUTTON_ACTIONS.get(message(w, id, CB_GETCURSEL, 0, 0) as usize) {
                return Ok((*action).into());
            }
            let chord = self.button_shortcuts[usize::from(shortcut_id == SHORTCUT2)].clone();
            ctl460_rust::button_actions::shortcut(&chord)?;
            Ok(format!("{}{chord}", ctl460_rust::button_actions::CUSTOM))
        }
        fn refresh(&mut self, w: HWND) -> Result<(), String> {
            let api = hidapi::HidApi::new().map_err(|e| e.to_string())?;
            message(w, DEVICE, CB_RESETCONTENT, 0, 0);
            self.count = 0;
            self.paths.clear();
            for d in api.device_list().filter(|d| {
                d.vendor_id() == VENDOR_ID
                    && d.product_id() == PRODUCT_ID
                    && ctl460_rust::device_selection::is_pen_collection(
                        d.interface_number(),
                        d.usage_page(),
                        d.usage(),
                    )
            }) {
                let s = wide(&format!("CTL-460 pen {}", self.count + 1));
                message(w, DEVICE, CB_ADDSTRING, 0, s.as_ptr() as isize);
                self.count += 1;
                self.paths.push(d.path().to_string_lossy().into_owned());
            }
            if self.count == 0 {
                let s = wide("No pen connected");
                message(w, DEVICE, CB_ADDSTRING, 0, s.as_ptr() as isize);
            }
            message(w, DEVICE, CB_SETCURSEL, 0, 0);
            unsafe {
                EnableWindow(
                    control(w, 141),
                    i32::from(self.count > 0 && self.child.is_none()),
                );
            }
            Ok(())
        }
        fn save(&mut self, w: HWND) -> Result<(), String> {
            let mut config = self.config.clone();
            let values = LINE_IDS.map(|id| message(w, id, TBM_GETPOS, 0, 0) as f64);
            config.streamline_amount = values[0];
            config.streamline_pressure = values[1];
            config.stabilization_amount = values[2];
            config.motion_filter_amount = values[3];
            config.motion_filter_expression = values[4];
            config.wobble_reduction = values[5];
            config.circle_smoothing = values[6];
            let number = |id| {
                read(w, id)
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| "Enter a valid number in each pressure/tilt field".to_string())
            };
            config.gamma = number(GAMMA)?;
            let integer = |id| {
                read(w, id).trim().parse::<u16>().map_err(|_| {
                    "Pressure thresholds must be whole numbers from 0 to 1023".to_string()
                })
            };
            config.floor = integer(FLOOR)?;
            config.ceiling = integer(CEILING)?;
            config.gain = number(GAIN)?;
            config.press_on = integer(PRESS_ON)?;
            config.press_off = integer(PRESS_OFF)?;
            config.handwriting_mode = ["off", "on", "auto"][choice(w, WRITING).min(2)].into();
            if choice(w, WRITING_FILTER) < 4 {
                config.handwriting_filter =
                    ["legacy", "raw", "minimal", "responsive"][choice(w, WRITING_FILTER)].into();
            }
            config.backend = if choice(w, BACKEND) == 1 {
                "hid"
            } else {
                "ink"
            }
            .into();
            config.smoothing_ms = number(SMOOTH)?;
            config.interpolation_ms = number(RAMP)?;
            config.tilt_max_degrees = number(TILTMAX)?;
            config.stroke_smoothing = checked(w, STROKE);
            config.virtual_tilt = checked(w, TILT);
            config.double_click_distance = message(w, DOUBLE_DISTANCE, TBM_GETPOS, 0, 0) as u32;
            config.left_handed = checked(w, LEFT);
            config.preserve_aspect = checked(w, ASPECT);
            config.button1 = self.button_value(w, B1, SHORTCUT1)?;
            config.button2 = self.button_value(w, B2, SHORTCUT2)?;
            config.legacy_reports = checked(w, LEGACY);
            config.validate()?;
            let contents = format!(
                "# Saved control-panel settings for the CTL-460 Rust driver.\n{}",
                toml::to_string_pretty(&config).map_err(|e| e.to_string())?
            );
            if contents != self.last_saved {
                ctl460_rust::live_config::write_atomic(
                    &self.settings.join("settings.toml"),
                    &contents,
                )?;
                self.config = config;
                self.last_saved = contents;
                text(
                    w,
                    STATUS,
                    if self.child.is_some() {
                        "Settings saved automatically; applying between strokes."
                    } else {
                        "Settings saved automatically. Driver stopped."
                    },
                );
            }
            if self.settings_error {
                self.settings_error = false;
                text(w, STATUS, "Valid settings saved automatically.");
            }
            Ok(())
        }
        fn curve_fields(&self, w: HWND) {
            // Post a refresh after all edits: EN_CHANGE notifications must not borrow State recursively.
            text(w, GAMMA, &self.config.gamma.to_string());
            text(w, FLOOR, &self.config.floor.to_string());
            text(w, CEILING, &self.config.ceiling.to_string());
            text(w, GAIN, &self.config.gain.to_string());
            visuals::set(control(w, CURVE), &self.config);
            message(
                w,
                SENSITIVITY,
                TBM_SETPOS,
                1,
                ((self.config.gamma / 0.2).ln() / 20.0_f64.ln() * 100.0).round() as isize,
            );
        }
        fn preview_curve(&mut self, w: HWND) {
            let mut c = self.config.clone();
            if let (Ok(gamma), Ok(gain), Ok(floor), Ok(ceiling)) = (
                read(w, GAMMA).parse(),
                read(w, GAIN).parse(),
                read(w, FLOOR).parse(),
                read(w, CEILING).parse(),
            ) {
                c.gamma = gamma;
                c.gain = gain;
                c.floor = floor;
                c.ceiling = ceiling;
                if c.validate().is_ok() {
                    if Shape::from_config(&c) != Shape::from_config(&self.config) {
                        self.config = c.clone();
                        self.pressure_draft = Shape::from_config(&c);
                        message(w, 230, CB_SETCURSEL, 1, 0);
                        self.pressure_controls(w);
                    }
                    visuals::set(control(w, CURVE), &c);
                }
            }
        }
        fn start(&mut self, w: HWND) -> Result<(), String> {
            if self.child.is_some() {
                return Err("Stop the running driver before starting another.".into());
            }
            if self.count == 0 {
                return Err("Connect your CTL-460 and click Refresh.".into());
            }
            self.save(w)?;
            let trace_path = self.recording.then(|| {
                self.settings.join(format!(
                    "pen-trace-{}.csv",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos()
                ))
            });
            let name = format!("Local\\CTL460RustStop-{}", std::process::id());
            // SAFETY: Creates an owned manual-reset event, used only for worker cancellation.
            self.event = unsafe { CreateEventW(null(), 1, 0, wide(&name).as_ptr()) };
            if self.event.is_null() {
                return Err("Cannot create driver stop event".into());
            }
            self.hid_session = self.config.backend == "hid";
            if self.hid_session {
                let status = self.settings.join("hid-session-status.txt");
                let result = (|| -> Result<Child, String> {
                    fs::write(&status, "starting\nConnecting to Virtual HID service...")
                        .map_err(|e| e.to_string())?;
                    let log = fs::File::create(self.settings.join("driver.log"))
                        .map_err(|e| e.to_string())?;
                    let script = self.root.join("scripts").join("hid_session.ps1");
                    if !script.is_file() {
                        return Err(
                            "Virtual HID setup files are missing. Reinstall OpenCTL 460.".into(),
                        );
                    }
                    let system = PathBuf::from(
                        std::env::var_os("SystemRoot").ok_or("Windows directory unavailable")?,
                    );
                    let mut command =
                        Command::new(system.join("System32/WindowsPowerShell/v1.0/powershell.exe"));
                    command
                        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
                        .arg(script)
                        .arg("-ConfigPath")
                        .arg(self.settings.join("settings.toml"))
                        .arg("-DevicePath")
                        .arg(
                            self.paths
                                .get(choice(w, DEVICE))
                                .ok_or("Refresh the tablet list")?,
                        )
                        .arg("-StopEvent")
                        .arg(&name)
                        .arg("-StatusPath")
                        .arg(status)
                        .arg("-GuiProcessId")
                        .arg(std::process::id().to_string())
                        .stdin(Stdio::null())
                        .stdout(log.try_clone().map_err(|e| e.to_string())?)
                        .stderr(log)
                        .creation_flags(CREATE_NO_WINDOW);
                    if let Some(path) = &trace_path {
                        command.arg("-TracePath").arg(path);
                    }
                    command.spawn().map_err(|e| e.to_string())
                })();
                match result {
                    Ok(child) => self.child = Some(child),
                    Err(e) => {
                        unsafe {
                            CloseHandle(self.event);
                        }
                        self.event = null_mut();
                        return Err(e);
                    }
                }
                text(
                    w,
                    STATUS,
                    "Connecting to the installed Virtual HID service...",
                );
                unsafe {
                    EnableWindow(control(w, 141), 0);
                }
                return Ok(());
            }
            let log =
                fs::File::create(self.settings.join("driver.log")).map_err(|e| e.to_string())?;
            let mut command = Command::new(self.root.join("ctl460-rust.exe"));
            command
                .arg("run")
                .arg("--device-path")
                .arg(
                    self.paths
                        .get(choice(w, DEVICE))
                        .ok_or("Refresh the device list before starting")?,
                )
                .arg("--init")
                .arg("--config")
                .arg(self.settings.join("settings.toml"))
                .arg("--stop-event")
                .arg(name)
                .stdin(Stdio::null())
                .stdout(log.try_clone().map_err(|e| e.to_string())?)
                .stderr(log)
                .creation_flags(CREATE_NO_WINDOW);
            if let Some(path) = &trace_path {
                command.arg("--trace").arg(path);
            }
            match command.spawn() {
                Ok(child) => self.child = Some(child),
                Err(e) => {
                    unsafe {
                        CloseHandle(self.event);
                    }
                    self.event = null_mut();
                    return Err(e.to_string());
                }
            }
            text(
                w,
                STATUS,
                "Driver started. Draw in a Windows Ink application. Stop before changing settings.",
            );
            unsafe {
                EnableWindow(control(w, 141), 0);
            }
            Ok(())
        }
        fn stop(&self, w: HWND) {
            if !self.event.is_null() {
                unsafe {
                    SetEvent(self.event);
                }
                text(w, STATUS, "Stopping and releasing pen/buttons...");
            }
        }
        fn poll(&mut self, w: HWND) {
            // One attempt on opening/reopening, never a retry loop after Stop, failure or UAC cancellation.
            if std::mem::take(&mut self.start_pending) && !self.closing && self.child.is_none() {
                if let Err(e) = self.refresh(w).and_then(|()| self.start(w)) {
                    text(w, STATUS, &format!("Automatic start: {e}"));
                }
            }
            if self.hid_session && self.child.is_some() {
                if let Some(status) = self.hid_status() {
                    text(w, STATUS, &status);
                }
            }
            if let Some(path) = self.preview.take() {
                message(
                    w,
                    PAGE,
                    TCM_SETCURSEL,
                    if self.preview_page == 5 {
                        3
                    } else {
                        self.preview_page
                    },
                    0,
                );
                self.show_page(w);
                if let Err(e) = visuals::snapshot(w, &path) {
                    error(w, &e);
                }
                unsafe {
                    PostMessageW(w, tray::EXIT, 0, 0);
                }
            }
            if let Some(child) = self.child.as_mut() {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        self.child = None;
                        if !self.event.is_null() {
                            unsafe {
                                CloseHandle(self.event);
                            }
                            self.event = null_mut();
                        }
                        let log = fs::read_to_string(self.settings.join("driver.log"))
                            .unwrap_or_default();
                        let detail = log.lines().rev().find(|line| line.starts_with("Error:"));
                        let fallback = format!("Driver stopped ({status}). Open log for details.");
                        let hid_status = if self.hid_session {
                            self.hid_status()
                        } else {
                            None
                        };
                        text(
                            w,
                            STATUS,
                            hid_status.as_deref().or(detail).unwrap_or(&fallback),
                        );
                        unsafe {
                            EnableWindow(control(w, 141), i32::from(self.count > 0));
                        }
                    }
                    Ok(None) => (),
                    Err(e) => {
                        text(w, STATUS, &format!("Cannot query worker: {e}"));
                    }
                }
            }
            if self.closing && self.child.is_none() {
                unsafe {
                    DestroyWindow(w);
                }
            }
        }
        fn hid_status(&self) -> Option<String> {
            fs::read_to_string(self.settings.join("hid-session-status.txt"))
                .ok()
                .and_then(|s| {
                    s.split_once('\n').and_then(|(state, message)| {
                        if self.child.is_none()
                            && ![
                                "stopped",
                                "failed",
                                "cancelled",
                                "restart_required",
                                "secure_boot",
                            ]
                            .contains(&state)
                        {
                            None
                        } else {
                            Some(message.trim().to_owned())
                        }
                    })
                })
        }
    }
    unsafe extern "system" fn procedure(w: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        // SAFETY: Window creation stores a State pointer whose Box outlives the message loop.
        // Only the GUI thread accesses it. Worker state is external and checked asynchronously.
        unsafe {
            // Paint/focus notifications can arrive synchronously while controls are being
            // updated. Handle them without borrowing the application State a second time.
            if msg == WM_ERASEBKGND || msg == WM_PRINTCLIENT {
                let dc = wp as HDC;
                let mut r = RECT::default();
                GetClientRect(w, &mut r);
                FillRect(dc, &r, GetStockObject(WHITE_BRUSH) as HBRUSH);
                r.right = 204 * GetDpiForWindow(w) as i32 / 96;
                let b = CreateSolidBrush(0xf3f1ef);
                FillRect(dc, &r, b);
                DeleteObject(b as _);
                return 1;
            }
            if msg == WM_CTLCOLORSTATIC || msg == WM_CTLCOLORBTN {
                let dc = wp as HDC;
                let mut r = RECT::default();
                GetWindowRect(lp as HWND, &mut r);
                let mut p = POINT {
                    x: r.left,
                    y: r.top,
                };
                ScreenToClient(w, &mut p);
                SetTextColor(dc, 0x443c35);
                SetBkMode(dc, TRANSPARENT as i32);
                SetDCBrushColor(
                    dc,
                    if p.x < 204 * GetDpiForWindow(w) as i32 / 96 {
                        0xf3f1ef
                    } else {
                        0xffffff
                    },
                );
                return GetStockObject(DC_BRUSH) as isize;
            }
            if msg == WM_COMMAND {
                let id = (wp & 0xffff) as i32;
                let notification = (wp >> 16) & 0xffff;
                if id == 233 && notification == BN_CLICKED as usize {
                    if let Some(name) = prompt::name(w) {
                        let state = GetWindowLongPtrW(w, GWLP_USERDATA) as *mut State;
                        if !state.is_null() {
                            let result = (&mut *state).save_pressure(w, &name);
                            if let Err(e) = result {
                                error(w, &e);
                            }
                        }
                    }
                    return 0;
                }
                if notification == BN_CLICKED as usize && [SHORTCUT1, SHORTCUT2].contains(&id) {
                    if IsWindowEnabled(control(w, id)) == 0 {
                        return 0;
                    }
                    // The modal loop dispatches owner timers; borrow State only after it closes.
                    let state = GetWindowLongPtrW(w, GWLP_USERDATA) as *mut State;
                    if state.is_null() {
                        return 0;
                    }
                    let index = usize::from(id == SHORTCUT2);
                    let initial = (&(*state).button_shortcuts)[index].clone();
                    if let Some(chord) = shortcut_prompt::capture(w, &initial) {
                        if IsWindow(w) == 0 {
                            return 0;
                        }
                        let state = GetWindowLongPtrW(w, GWLP_USERDATA) as *mut State;
                        if state.is_null() {
                            return 0;
                        }
                        message(
                            w,
                            if id == SHORTCUT1 { B1 } else { B2 },
                            CB_SETCURSEL,
                            usize::MAX,
                            0,
                        );
                        text(w, if id == SHORTCUT1 { B1 } else { B2 }, &chord);
                        (&mut (*state).button_shortcuts)[index] = chord;
                        let state = GetWindowLongPtrW(w, GWLP_USERDATA) as *mut State;
                        if !state.is_null() {
                            if let Err(e) = (&mut *state).save(w) {
                                error(w, &e);
                            }
                        }
                    }
                    return 0;
                }
                if notification == EN_CHANGE as usize && [GAMMA, GAIN, FLOOR, CEILING].contains(&id)
                {
                    PostMessageW(w, PREVIEW_CURVE, 0, 0);
                    return 0;
                }
                if !([
                    101,
                    140,
                    141,
                    142,
                    143,
                    212,
                    LEFT,
                    RIGHT,
                    216,
                    217,
                    232,
                    234,
                    236,
                    237,
                    244,
                    START_WINDOWS,
                ]
                .contains(&id)
                    || [PROFILE, 230, B1, B2].contains(&id)
                        && notification == CBN_SELCHANGE as usize)
                {
                    return 0;
                }
            }
            if msg == WM_NOTIFY {
                let h = &*(lp as *const NMHDR);
                if h.idFrom != PAGE as usize || h.code != TCN_SELCHANGE {
                    return 0;
                }
            }
            if msg == WM_DESTROY {
                tray::remove(w);
                KillTimer(w, 1);
                KillTimer(w, 2);
                PostQuitMessage(0);
                return 0;
            }
            if msg == WM_NCCREATE {
                let create = &*(lp as *const CREATESTRUCTW);
                SetWindowLongPtrW(w, GWLP_USERDATA, create.lpCreateParams as isize);
            }
            let state = GetWindowLongPtrW(w, GWLP_USERDATA) as *mut State;
            if state.is_null() {
                return DefWindowProcW(w, msg, wp, lp);
            }
            // These operations can run nested message loops; do not hold a State borrow.
            if msg == tray::CALLBACK {
                tray::callback(w, wp, lp);
                return 0;
            }
            if msg == tray::OPEN {
                (*state).minimize_requested = false;
                tray::open(w);
                (*state).start_pending = (*state).auto_start;
                return 0;
            }
            if msg == WM_SIZE && wp == SIZE_RESTORED as usize && IsWindowVisible(w) != 0 {
                // A taskbar restore cancels a pending close-to-tray retry.
                (*state).minimize_requested = false;
            }
            if (msg == tray::taskbar_message() && msg != 0) || (msg == WM_TIMER && wp == 2) {
                if (*state).preview.is_none() {
                    if tray::ensure(w) {
                        KillTimer(w, 2);
                        if (*state).minimize_requested {
                            ShowWindow(w, SW_HIDE);
                        }
                    } else {
                        if (*state).minimize_requested {
                            ShowWindow(w, SW_MINIMIZE);
                        }
                        SetTimer(w, 2, 1500, None);
                    }
                }
                return 0;
            }
            if msg == WM_CLOSE {
                if let Err(e) = (*state).save(w) {
                    // Keep the last valid saved settings and retain the edit in the hidden window.
                    text(w, STATUS, &format!("Settings not applied: {e}"));
                }
                (*state).minimize_requested = true;
                if tray::ensure(w) {
                    KillTimer(w, 2);
                    ShowWindow(w, SW_HIDE);
                } else {
                    // Remain reachable while Explorer is unavailable; retry without stopping input.
                    SetTimer(w, 2, 1500, None);
                    ShowWindow(w, SW_MINIMIZE);
                }
                return 0;
            }
            if ![
                WM_CREATE,
                WM_DPICHANGED,
                WM_COMMAND,
                WM_NOTIFY,
                WM_HSCROLL,
                WM_TIMER,
                tray::EXIT,
                visuals::CURVE_CHANGED,
                PREVIEW_CURVE,
            ]
            .contains(&msg)
            {
                return DefWindowProcW(w, msg, wp, lp);
            }
            let s = &mut *state;
            match msg {
                WM_DPICHANGED => {
                    let r = &*(lp as *const RECT);
                    SetWindowPos(
                        w,
                        null_mut(),
                        r.left,
                        r.top,
                        r.right - r.left,
                        r.bottom - r.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                    s.scale_layout(w);
                    0
                }
                PREVIEW_CURVE => {
                    s.preview_curve(w);
                    0
                }
                WM_CREATE => {
                    if let Err(e) = s.controls(w) {
                        error(w, &e);
                        return -1;
                    }
                    if s.preview.is_none() && !tray::ensure(w) {
                        SetTimer(w, 2, 1500, None);
                    }
                    0
                }
                WM_COMMAND => {
                    let id = (wp & 0xffff) as i32;
                    let notification = (wp >> 16) & 0xffff;
                    let result = match id {
                        START_WINDOWS => {
                            let result = startup::set(
                                &s.root,
                                &s.startup_directory,
                                checked(w, START_WINDOWS),
                            );
                            check(
                                w,
                                START_WINDOWS,
                                s.startup_directory.join("OpenCTL 460.lnk").is_file(),
                            );
                            result
                        }
                        230 if notification == CBN_SELCHANGE as usize => {
                            let shape = match choice(w, 230) {
                                0 => Shape::from_config(&Config::default()),
                                1 => s.pressure_draft.clone(),
                                n => s.pressure_library.presets[n - 2].shape.clone(),
                            };
                            shape.apply(&mut s.config);
                            s.curve_fields(w);
                            s.pressure_controls(w);
                            Ok(())
                        }
                        232 => {
                            // A new preset starts with a full-range straight diagonal.
                            ctl460_rust::pressure_profiles::reset_linear(&mut s.config);
                            s.pressure_draft = Shape::from_config(&s.config);
                            message(w, 230, CB_SETCURSEL, 1, 0);
                            s.curve_fields(w);
                            s.pressure_controls(w);
                            Ok(())
                        }
                        234 => {
                            let index = choice(w, 230);
                            if index >= 2 {
                                let mut library = s.pressure_library.clone();
                                let name = library.presets[index - 2].name.clone();
                                library
                                    .remove(&name)
                                    .and_then(|()| {
                                        library.save(&s.settings.join("pressure-profiles.toml"))
                                    })
                                    .map(|()| {
                                        s.pressure_library = library;
                                        Shape::from_config(&Config::default()).apply(&mut s.config);
                                        s.curve_fields(w);
                                        s.pressure_list(w);
                                    })
                            } else {
                                Ok(())
                            }
                        }
                        236 => {
                            visuals::add_node(control(w, CURVE));
                            Ok(())
                        }
                        237 => {
                            visuals::remove_node(control(w, CURVE));
                            Ok(())
                        }
                        LEFT | RIGHT | 216 | 217 => {
                            check(w, LEFT, id == LEFT || id == 216);
                            check(w, RIGHT, id == RIGHT || id == 217);
                            Ok(())
                        }
                        212 => {
                            ctl460_rust::pressure_profiles::reset_linear(&mut s.config);
                            s.pressure_draft = Shape::from_config(&s.config);
                            message(w, 230, CB_SETCURSEL, 1, 0);
                            s.curve_fields(w);
                            s.pressure_controls(w);
                            Ok(())
                        }
                        GAMMA | GAIN | FLOOR | CEILING if notification == EN_CHANGE as usize => {
                            s.preview_curve(w);
                            Ok(())
                        }
                        244 => {
                            // Reset only side-button assignments; keep pressure and mapping edits.
                            let defaults = Config::default();
                            for (id, action) in [(B1, defaults.button1), (B2, defaults.button2)] {
                                let selected = BUTTON_ACTIONS
                                    .iter()
                                    .position(|a| *a == action)
                                    .unwrap_or(0);
                                message(w, id, CB_SETCURSEL, selected, 0);
                            }
                            s.button_controls(w);
                            s.save(w)
                        }
                        B1 | B2 if notification == CBN_SELCHANGE as usize => {
                            s.button_controls(w);
                            Ok(())
                        }
                        WRITING_FILTER if notification == CBN_SELCHANGE as usize => {
                            if choice(w, WRITING_FILTER) == 4 {
                                s.config.pen_control.get_or_insert(30.0);
                            } else {
                                s.config.pen_control = None;
                            }
                            s.writing_hint(w);
                            s.save(w)
                        }
                        101 => s.refresh(w),
                        PROFILE if notification == CBN_SELCHANGE as usize => {
                            if let Some((_, content)) = PRESETS.get(choice(w, PROFILE)) {
                                match Config::parse(content) {
                                    Ok(config) => {
                                        // Read current controls too: unsaved mapping changes must survive.
                                        s.config.double_click_distance =
                                            message(w, DOUBLE_DISTANCE, TBM_GETPOS, 0, 0) as u32;
                                        s.config.left_handed = checked(w, LEFT);
                                        s.config.preserve_aspect = checked(w, ASPECT);
                                        if let Ok(value) = s.button_value(w, B1, SHORTCUT1) {
                                            s.config.button1 = value;
                                        }
                                        if let Ok(value) = s.button_value(w, B2, SHORTCUT2) {
                                            s.config.button2 = value;
                                        }
                                        s.config.backend = if choice(w, BACKEND) == 1 {
                                            "hid"
                                        } else {
                                            "ink"
                                        }
                                        .into();
                                        s.config = s.config.drawing_profile(config);
                                        s.fill(w);
                                        Ok(())
                                    }
                                    Err(e) => Err(e),
                                }
                            } else {
                                Ok(())
                            }
                        }
                        141 => s.start(w),
                        142 => {
                            s.stop(w);
                            Ok(())
                        }
                        143 => Command::new("notepad.exe")
                            .arg(s.settings.join("driver.log"))
                            .spawn()
                            .map(|_| ())
                            .map_err(|e| e.to_string()),
                        _ => Ok(()),
                    };
                    if let Err(e) = result {
                        error(w, &e);
                    }
                    0
                }
                WM_NOTIFY => {
                    let header = &*(lp as *const NMHDR);
                    if header.idFrom == PAGE as usize && header.code == TCN_SELCHANGE {
                        s.show_page(w);
                    }
                    0
                }
                visuals::CURVE_CHANGED => {
                    let c = visuals::get(control(w, CURVE));
                    s.config.pressure_nodes = c.pressure_nodes.clone();
                    s.config.gamma = c.gamma;
                    s.config.floor = c.floor;
                    s.config.ceiling = c.ceiling;
                    s.config.gain = c.gain;
                    s.pressure_draft = Shape::from_config(&s.config);
                    // Change preset controls only once when the drag starts editing a copy.
                    if choice(w, 230) != 1 {
                        message(w, 230, CB_SETCURSEL, 1, 0);
                        s.pressure_controls(w);
                    }
                    s.curve_fields(w);
                    0
                }
                WM_HSCROLL => {
                    if lp as HWND == control(w, SENSITIVITY) {
                        let old_gamma = s.config.gamma;
                        s.config.gamma = 0.2
                            * 20.0_f64
                                .powf(message(w, SENSITIVITY, TBM_GETPOS, 0, 0) as f64 / 100.0);
                        for n in &mut s.config.pressure_nodes {
                            n.y = n.y.powf(s.config.gamma / old_gamma);
                        }
                        s.pressure_draft = Shape::from_config(&s.config);
                        message(w, 230, CB_SETCURSEL, 1, 0);
                        s.pressure_controls(w);
                        s.curve_fields(w);
                    }
                    s.line_labels(w);
                    0
                }
                WM_TIMER => {
                    if !s.closing {
                        if let Err(e) = s.save(w) {
                            // A partially typed or invalid value never replaces valid settings.
                            s.settings_error = true;
                            text(w, STATUS, &format!("Settings not applied: {e}"));
                            return 0;
                        }
                    }
                    s.poll(w);
                    if s.child.is_some() {
                        if let Ok(status) =
                            fs::read_to_string(s.settings.join("live-settings-status.txt"))
                        {
                            if let Some((revision, detail)) = status.split_once('\n') {
                                if revision
                                    == format!(
                                        "{:016x}",
                                        ctl460_rust::live_config::fingerprint(&s.last_saved)
                                    )
                                {
                                    text(w, STATUS, detail);
                                }
                            }
                        }
                    }
                    0
                }
                tray::EXIT => {
                    // Persist the last valid edit even when Exit precedes the next timer tick.
                    let _ = s.save(w);
                    if s.child.is_some() {
                        s.closing = true;
                        s.stop(w);
                    } else {
                        DestroyWindow(w);
                    }
                    0
                }
                WM_DESTROY => {
                    KillTimer(w, 1);
                    PostQuitMessage(0);
                    0
                }
                _ => DefWindowProcW(w, msg, wp, lp),
            }
        }
    }
    pub fn run() -> Result<(), String> {
        // Explicit awareness before creating any HWND; child geometry follows WM_DPICHANGED.
        unsafe {
            SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
        let root = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .parent()
            .ok_or("Executable directory unavailable")?
            .to_owned();
        let default_settings =
            PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("LocalAppData is unavailable")?)
                .join("CTL460Studio");
        // Explicit portable directory lets debug tests avoid changing the user's saved settings.
        let mut args: Vec<_> = std::env::args_os().skip(1).collect();
        let requested_start =
            args.len() == 3 && args[0] == "--settings-dir" && args[2] == "--start";
        if requested_start {
            args.pop();
        }
        // Recording is explicitly requested for an isolated comparison session.
        let recording = args.len() == 3 && args[0] == "--settings-dir" && args[2] == "--record";
        if recording {
            args.pop();
        }
        let startup_launch = args.len() == 1 && args[0] == "--startup";
        let normal_launch = args.is_empty() || startup_launch;
        // Reopen an existing tray window instead of launching a second competing feeder.
        // Explicit debug/preview directories remain independent and never auto-start hardware.
        if normal_launch {
            let existing = unsafe { FindWindowW(wide("CTL460RustControlPanel").as_ptr(), null()) };
            if !existing.is_null() {
                if !startup_launch {
                    unsafe {
                        ShowWindow(existing, SW_RESTORE);
                        SetForegroundWindow(existing);
                        PostMessageW(existing, tray::OPEN, 0, 0);
                    }
                }
                return Ok(());
            }
        }
        let settings = if normal_launch {
            default_settings
        } else if (args.len() == 2
            || (args.len() == 4 || args.len() == 6 && args[4] == "--preview-page")
                && args[2] == "--preview")
            && args[0] == "--settings-dir"
        {
            PathBuf::from(&args[1])
        } else {
            return Err(
                "Usage: ctl460-gui [--startup | --settings-dir DIRECTORY [--preview BMP]]".into(),
            );
        };
        fs::create_dir_all(&settings).map_err(|e| e.to_string())?;
        let mut config = match fs::read_to_string(settings.join("settings.toml")) {
            Ok(s) => Config::parse(&s)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::simple_default(),
            Err(e) => return Err(e.to_string()),
        };
        config.restore_line_controls();
        let pressure_library = Library::load(&settings.join("pressure-profiles.toml"))?;
        let pressure_draft = Shape::from_config(&config);
        let startup_directory = if normal_launch {
            startup::directory()?
        } else {
            settings.join("startup")
        };
        let mut state = Box::new(State {
            pressure_library,
            pressure_draft,
            root,
            settings,
            recording,
            startup_directory,
            config,
            button_shortcuts: Default::default(),
            last_saved: String::new(),
            settings_error: false,
            child: None,
            hid_session: false,
            event: null_mut(),
            closing: false,
            minimize_requested: false,
            auto_start: normal_launch || requested_start,
            start_pending: normal_launch || requested_start,
            count: 0,
            paths: Vec::new(),
            pages: std::array::from_fn(|_| Vec::new()),
            font: null_mut(),
            bold_font: null_mut(),
            title_font: null_mut(),
            layout: Vec::new(),
            preview_page: if args.len() == 6 {
                args[5]
                    .to_string_lossy()
                    .parse::<usize>()
                    .ok()
                    .filter(|n| *n < 6)
                    .ok_or("Preview page must be 0..5 (5 = advanced smoothing)")?
            } else {
                0
            },
            preview: if args.len() >= 4 {
                Some(PathBuf::from(&args[3]))
            } else {
                None
            },
        });
        let class = wide("CTL460RustControlPanel");
        // SAFETY: Standard Win32 registration and message loop. State and class strings live until exit.
        unsafe {
            let common = INITCOMMONCONTROLSEX {
                dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
                dwICC: ICC_BAR_CLASSES | ICC_TAB_CLASSES | ICC_PROGRESS_CLASS,
            };
            if InitCommonControlsEx(&common) == 0 {
                return Err("Cannot initialize smoothing sliders".into());
            }
            visuals::register()?;
            let instance = GetModuleHandleW(null());
            let wc = WNDCLASSW {
                lpfnWndProc: Some(procedure),
                hInstance: instance,
                lpszClassName: class.as_ptr(),
                hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                hbrBackground: GetStockObject(WHITE_BRUSH) as HBRUSH,
                ..Default::default()
            };
            if RegisterClassW(&wc) == 0 {
                return Err("Window registration failed".into());
            }
            // Center in the active monitor's usable area, excluding its taskbar.
            let mut cursor = POINT::default();
            GetCursorPos(&mut cursor);
            let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTOPRIMARY);
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            GetMonitorInfoW(monitor, &mut info);
            let area = info.rcWork;
            let mut dpi = 96;
            let mut dpi_y = 96;
            GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut dpi_y);
            let width = 1040 * dpi as i32 / 96;
            let height = 936 * dpi as i32 / 96;
            let x = area.left + ((area.right - area.left - width) / 2).max(0);
            let y = area.top + ((area.bottom - area.top - height) / 2).max(0);
            let w = CreateWindowExW(
                WS_EX_CONTROLPARENT,
                class.as_ptr(),
                wide(if recording {
                    "OpenCTL 460 - handwriting recording"
                } else {
                    "OpenCTL 460"
                })
                .as_ptr(),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
                x,
                y,
                width,
                height,
                null_mut(),
                null_mut(),
                instance,
                state.as_mut() as *mut State as *const c_void,
            );
            if w.is_null() {
                return Err("Window creation failed".into());
            }
            if startup_launch {
                state.minimize_requested = true;
                if tray::ensure(w) {
                    ShowWindow(w, SW_HIDE);
                } else {
                    ShowWindow(w, SW_SHOWMINNOACTIVE);
                    SetTimer(w, 2, 1500, None);
                }
            } else {
                ShowWindow(w, SW_SHOW);
            }
            UpdateWindow(w);
            let mut msg = MSG::default();
            loop {
                let result = GetMessageW(&mut msg, null_mut(), 0, 0);
                if result == -1 {
                    return Err("Window message loop failed".into());
                }
                if result == 0 {
                    break;
                }
                if IsDialogMessageW(w, &msg) == 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
        Ok(())
    }
    pub fn show_error(e: &str) {
        error(null_mut(), e);
    }
}
#[cfg(windows)]
fn main() {
    if let Err(e) = app::run() {
        app::show_error(&e);
    }
}
