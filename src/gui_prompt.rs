//! Native modal name prompt for saving a new pressure preset. Owns no application state.
//! The disabled owner continues dispatching timers without retaining a mutable State borrow.
use std::ptr::{null, null_mut};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        HiDpi::*,
        Input::KeyboardAndMouse::{EnableWindow, SetFocus},
        WindowsAndMessaging::*,
    },
};
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
struct Prompt {
    result: Option<String>,
    done: bool,
}
unsafe extern "system" fn proc(w: HWND, m: u32, a: WPARAM, b: LPARAM) -> LRESULT {
    unsafe {
        if m == WM_NCCREATE {
            let c = &*(b as *const CREATESTRUCTW);
            SetWindowLongPtrW(w, GWLP_USERDATA, c.lpCreateParams as isize);
        }
        let p = GetWindowLongPtrW(w, GWLP_USERDATA) as *mut Prompt;
        if !p.is_null()
            && (m == WM_CLOSE || m == WM_COMMAND && (a & 0xffff == 1 || a & 0xffff == 2))
        {
            if m == WM_COMMAND && a & 0xffff == 1 {
                let mut text = [0u16; 256];
                let n = GetWindowTextW(GetDlgItem(w, 10), text.as_mut_ptr(), 256);
                (*p).result = Some(String::from_utf16_lossy(&text[..n.max(0) as usize]));
            }
            (*p).done = true;
            DestroyWindow(w);
            return 0;
        }
        DefWindowProcW(w, m, a, b)
    }
}
pub fn name(owner: HWND) -> Option<String> {
    unsafe {
        let class = wide("OpenCTLPressureName");
        let instance = GetModuleHandleW(null());
        let wc = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: GetSysColorBrush(COLOR_WINDOW),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let dpi = GetDpiForWindow(owner);
        let px = |v: i32| v * dpi as i32 / 96;
        let mut r = RECT::default();
        GetWindowRect(owner, &mut r);
        let mut state = Prompt {
            result: None,
            done: false,
        };
        let w = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            class.as_ptr(),
            wide("Save pressure profile").as_ptr(),
            WS_CAPTION | WS_SYSMENU | WS_POPUP,
            r.left + (r.right - r.left - px(410)) / 2,
            r.top + (r.bottom - r.top - px(190)) / 2,
            px(410),
            px(190),
            owner,
            null_mut(),
            instance,
            &mut state as *mut Prompt as _,
        );
        if w.is_null() {
            return None;
        }
        let font = SendMessageW(GetDlgItem(owner, 100), WM_GETFONT, 0, 0);
        for (id, kind, text, x, y, width, height, style) in [
            (
                0,
                "STATIC",
                "Name the new pressure profile:",
                16,
                16,
                370,
                24,
                0,
            ),
            (
                10,
                "EDIT",
                "",
                16,
                48,
                370,
                28,
                WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32,
            ),
            (
                1,
                "BUTTON",
                "Save",
                190,
                96,
                92,
                30,
                WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
            ),
            (2, "BUTTON", "Cancel", 294, 96, 92, 30, WS_TABSTOP),
        ] {
            let c = CreateWindowExW(
                0,
                wide(kind).as_ptr(),
                wide(text).as_ptr(),
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
        SendMessageW(GetDlgItem(w, 10), 0x00c5, 64, 0);
        EnableWindow(owner, 0);
        ShowWindow(w, SW_SHOW);
        SetFocus(GetDlgItem(w, 10));
        let mut msg = MSG::default();
        // DispatchMessage invokes proc, which updates state through GWLP_USERDATA.
        #[allow(clippy::while_immutable_condition)]
        while !state.done {
            let status = GetMessageW(&mut msg, null_mut(), 0, 0);
            if status <= 0 {
                if status == 0 {
                    PostQuitMessage(msg.wParam as i32);
                }
                break;
            }
            if msg.message == WM_KEYDOWN && (msg.wParam == 13 || msg.wParam == 27) {
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
        state.result
    }
}
