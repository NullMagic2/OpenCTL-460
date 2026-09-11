//! Window-local shortcut recorder. Captured keys are consumed, never injected or executed.
//! The owner may dispatch timers during this modal loop; no owner State borrow is retained.
use std::ptr::{null, null_mut};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        HiDpi::*,
        Input::KeyboardAndMouse::{EnableWindow, GetFocus, SetFocus},
        WindowsAndMessaging::*,
    },
};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

struct Capture {
    held: [bool; 256],
    pending: Option<u16>,
    chord: Option<String>,
    recording: bool,
    done: bool,
    result: Option<String>,
}

fn modifier(key: u16) -> bool {
    matches!(key, 0x10..=0x12 | 0x5b..=0x5c | 0xa0..=0xa5)
}

unsafe extern "system" fn proc(w: HWND, m: u32, a: WPARAM, b: LPARAM) -> LRESULT {
    unsafe {
        if m == WM_NCCREATE {
            SetWindowLongPtrW(
                w,
                GWLP_USERDATA,
                (*(b as *const CREATESTRUCTW)).lpCreateParams as isize,
            );
        }
        let p = GetWindowLongPtrW(w, GWLP_USERDATA) as *mut Capture;
        if !p.is_null() {
            if m == WM_CLOSE || m == WM_COMMAND && matches!(a & 0xffff, 1 | 2) {
                if m == WM_COMMAND && a & 0xffff == 1 {
                    if (*p).pending.is_some() || (*p).chord.is_none() {
                        return 0;
                    }
                    (*p).result = (*p).chord.clone();
                }
                (*p).done = true;
                DestroyWindow(w);
                return 0;
            }
            if m == WM_COMMAND && a & 0xffff == 3 {
                (*p).held = [false; 256];
                (*p).pending = None;
                (*p).chord = None;
                (*p).recording = true;
                SetWindowTextW(
                    GetDlgItem(w, 10),
                    wide("Press a key or combination...").as_ptr(),
                );
                EnableWindow(GetDlgItem(w, 1), 0);
                SetFocus(w);
                return 0;
            }
            if m == WM_ACTIVATE && a & 0xffff == WA_INACTIVE as usize {
                // A release may arrive in another app. Never retain stale modifiers on return.
                (*p).held = [false; 256];
                if (*p).recording {
                    (*p).pending = None;
                    (*p).chord = None;
                    EnableWindow(GetDlgItem(w, 1), 0);
                    SetWindowTextW(
                        GetDlgItem(w, 10),
                        wide("Press a key or combination...").as_ptr(),
                    );
                }
            }
            if (*p).recording && matches!(m, WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP) {
                if a > 255 {
                    return 0;
                }
                let key = a as u16;
                let down = matches!(m, WM_KEYDOWN | WM_SYSKEYDOWN);
                (*p).held[a] = down;
                if down && !modifier(key) && (*p).pending.is_none() && b & (1 << 30) == 0 {
                    let held = &(*p).held;
                    let modifiers = [
                        held[0x11] || held[0xa2] || held[0xa3],
                        held[0x10] || held[0xa0] || held[0xa1],
                        held[0x12] || held[0xa4] || held[0xa5],
                        held[0x5b] || held[0x5c],
                    ];
                    match ctl460_rust::button_actions::captured_shortcut(key, modifiers) {
                        Ok(chord) => {
                            SetWindowTextW(GetDlgItem(w, 10), wide(&chord).as_ptr());
                            (*p).chord = Some(chord);
                            (*p).pending = Some(key);
                            EnableWindow(GetDlgItem(w, 1), 0);
                        }
                        Err(error) => {
                            SetWindowTextW(GetDlgItem(w, 10), wide(&error).as_ptr());
                        }
                    }
                } else if !down && (*p).pending == Some(key) {
                    // Consume the captured key's release before focusing a button (especially Space).
                    (*p).recording = false;
                    (*p).pending = None;
                    EnableWindow(GetDlgItem(w, 1), 1);
                    SetFocus(GetDlgItem(w, 1));
                }
                return 0;
            }
            if m == WM_SYSCHAR {
                return 0;
            }
        }
        DefWindowProcW(w, m, a, b)
    }
}

pub fn capture(owner: HWND, initial: &str) -> Option<String> {
    unsafe {
        let class = wide("OpenCTLShortcutCapture");
        let instance = GetModuleHandleW(null());
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: GetSysColorBrush(COLOR_WINDOW),
            ..Default::default()
        });
        let dpi = GetDpiForWindow(owner).max(96);
        let px = |v: i32| v * dpi as i32 / 96;
        let style = WS_CAPTION | WS_SYSMENU | WS_POPUP;
        let mut bounds = RECT {
            right: px(510),
            bottom: px(184),
            ..Default::default()
        };
        AdjustWindowRectExForDpi(&mut bounds, style, 0, WS_EX_DLGMODALFRAME, dpi);
        let width = bounds.right - bounds.left;
        let height = bounds.bottom - bounds.top;
        let mut owner_bounds = RECT::default();
        GetWindowRect(owner, &mut owner_bounds);
        let initial = ctl460_rust::button_actions::shortcut(initial)
            .is_ok()
            .then(|| initial.to_string());
        let initial_text = initial
            .as_ref()
            .map(|c| format!("Current: {c}\r\nPress a key or combination to replace it."))
            .unwrap_or_else(|| "Press a key or combination...".into());
        let mut state = Capture {
            held: [false; 256],
            pending: None,
            chord: initial,
            recording: true,
            done: false,
            result: None,
        };
        let w = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            class.as_ptr(),
            wide("Set keyboard shortcut").as_ptr(),
            style,
            owner_bounds.left + (owner_bounds.right - owner_bounds.left - width) / 2,
            owner_bounds.top + (owner_bounds.bottom - owner_bounds.top - height) / 2,
            width,
            height,
            owner,
            null_mut(),
            instance,
            &mut state as *mut Capture as _,
        );
        if w.is_null() {
            return None;
        }
        let font = SendMessageW(GetDlgItem(owner, 100), WM_GETFONT, 0, 0);
        for (id, kind, title, x, y, width, height, style) in [
            (
                0,
                "STATIC",
                "Press your shortcut, then choose Use shortcut.",
                16,
                16,
                478,
                26,
                0,
            ),
            (
                10,
                "STATIC",
                initial_text.as_str(),
                16,
                52,
                478,
                54,
                WS_BORDER,
            ),
            (
                1,
                "BUTTON",
                "Use shortcut",
                16,
                128,
                148,
                32,
                WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
            ),
            (3, "BUTTON", "Record again", 176, 128, 148, 32, WS_TABSTOP),
            (2, "BUTTON", "Cancel", 336, 128, 158, 32, WS_TABSTOP),
        ] {
            let c = CreateWindowExW(
                0,
                wide(kind).as_ptr(),
                wide(title).as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                px(x),
                px(y),
                px(width),
                px(height),
                w,
                id as usize as HMENU,
                instance,
                null(),
            );
            SendMessageW(c, WM_SETFONT, font as usize, 1);
        }
        EnableWindow(GetDlgItem(w, 1), i32::from(state.chord.is_some()));
        let previous_focus = GetFocus();
        EnableWindow(owner, 0);
        ShowWindow(w, SW_SHOW);
        SetFocus(w);
        let mut msg = MSG::default();
        // The window procedure updates state through GWLP_USERDATA.
        #[allow(clippy::while_immutable_condition)]
        while !state.done {
            let status = GetMessageW(&mut msg, null_mut(), 0, 0);
            if status <= 0 {
                if status == 0 {
                    PostQuitMessage(msg.wParam as i32);
                }
                break;
            }
            let ours = msg.hwnd == w || IsChild(w, msg.hwnd) != 0;
            if ours
                && state.recording
                && matches!(
                    msg.message,
                    WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP
                )
            {
                SendMessageW(w, msg.message, msg.wParam, msg.lParam);
            } else if ours && msg.message == WM_KEYDOWN && matches!(msg.wParam, 13 | 27) {
                SendMessageW(w, WM_COMMAND, if msg.wParam == 13 { 1 } else { 2 }, 0);
            } else if IsDialogMessageW(w, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        if IsWindow(w) != 0 {
            DestroyWindow(w);
        }
        EnableWindow(owner, 1);
        SetForegroundWindow(owner);
        if IsWindow(previous_focus) != 0 {
            SetFocus(previous_focus);
        }
        state.result
    }
}
