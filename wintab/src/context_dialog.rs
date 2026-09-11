//! In-process modal editor for WTConfig. Edits only the requesting context.
use super::*;
use windows_sys::Win32::{Graphics::Gdi::*, UI::Input::KeyboardAndMouse::EnableWindow};
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
struct Dialog {
    name: String,
    words: [u32; 33],
    done: bool,
    accepted: bool,
}
const FIELDS: [(&str, usize); 8] = [
    ("Input X", 11),
    ("Input Y", 12),
    ("Input width", 14),
    ("Input height", 15),
    ("Output X", 17),
    ("Output Y", 18),
    ("Output width", 20),
    ("Output height", 21),
];
unsafe fn text(w: HWND, id: i32) -> String {
    let mut chars = [0u16; 256];
    let n = unsafe { GetWindowTextW(GetDlgItem(w, id), chars.as_mut_ptr(), chars.len() as i32) };
    String::from_utf16_lossy(&chars[..n.max(0) as usize])
}
unsafe fn child(w: HWND, class: &str, title: &str, id: i32, r: [i32; 4], style: u32) {
    unsafe {
        let h = CreateWindowExW(
            0,
            wide(class).as_ptr(),
            wide(title).as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            r[0],
            r[1],
            r[2],
            r[3],
            w,
            id as usize as HMENU,
            GetModuleHandleW(ptr::null()),
            ptr::null(),
        );
        SendMessageW(h, WM_SETFONT, GetStockObject(DEFAULT_GUI_FONT) as usize, 0);
    }
}
unsafe extern "system" fn procedure(w: HWND, msg: u32, wp: usize, lp: isize) -> isize {
    unsafe {
        if msg == WM_NCCREATE {
            let create = &*(lp as *const CREATESTRUCTW);
            SetWindowLongPtrW(w, GWLP_USERDATA, create.lpCreateParams as _);
        }
        let p = GetWindowLongPtrW(w, GWLP_USERDATA) as *mut Dialog;
        if p.is_null() {
            return DefWindowProcW(w, msg, wp, lp);
        }
        let d = &mut *p;
        match msg {
            WM_CREATE => {
                child(w, "STATIC", "Context name", 0, [16, 18, 120, 22], 0);
                child(
                    w,
                    "EDIT",
                    &d.name,
                    100,
                    [142, 14, 260, 26],
                    WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32,
                );
                for (i, (label, index)) in FIELDS.iter().enumerate() {
                    let y = 58 + i as i32 * 32;
                    child(w, "STATIC", label, 0, [16, y + 3, 120, 22], 0);
                    child(
                        w,
                        "EDIT",
                        &(d.words[*index] as i32).to_string(),
                        110 + i as i32,
                        [142, y, 260, 26],
                        WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32,
                    );
                }
                child(
                    w,
                    "BUTTON",
                    "OK",
                    1,
                    [220, 328, 86, 28],
                    WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
                );
                child(w, "BUTTON", "Cancel", 2, [316, 328, 86, 28], WS_TABSTOP);
                0
            }
            WM_COMMAND if wp & 0xffff == 1 => {
                let mut words = d.words;
                for (i, (_, index)) in FIELDS.iter().enumerate() {
                    match text(w, 110 + i as i32).parse::<i32>() {
                        Ok(v) => words[*index] = v as u32,
                        Err(_) => {
                            MessageBoxW(
                                w,
                                wide("Enter a whole number in each coordinate field.").as_ptr(),
                                wide("WinTab context").as_ptr(),
                                MB_OK | MB_ICONWARNING,
                            );
                            return 0;
                        }
                    }
                }
                if !valid_words(&words) {
                    MessageBoxW(
                        w,
                        wide("Input and output widths and heights must be nonzero.").as_ptr(),
                        wide("WinTab context").as_ptr(),
                        MB_OK | MB_ICONWARNING,
                    );
                    return 0;
                }
                d.name = text(w, 100);
                d.words = words;
                d.accepted = true;
                d.done = true;
                DestroyWindow(w);
                0
            }
            WM_COMMAND if wp & 0xffff == 2 => {
                d.done = true;
                DestroyWindow(w);
                0
            }
            WM_CLOSE => {
                d.done = true;
                DestroyWindow(w);
                0
            }
            WM_DESTROY => {
                d.done = true;
                0
            }
            _ => DefWindowProcW(w, msg, wp, lp),
        }
    }
}
pub fn edit(owner: HWND, initial: (String, [u32; 33])) -> Option<(String, [u32; 33])> {
    unsafe {
        if !owner.is_null() {
            let mut pid = 0;
            GetWindowThreadProcessId(owner, &mut pid);
            if pid != std::process::id() {
                return None;
            }
        }
        let module = GetModuleHandleW(ptr::null());
        let class = wide("OpenCTLWinTabContext");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: module,
            lpszClassName: class.as_ptr(),
            hbrBackground: (COLOR_BTNFACE + 1) as HBRUSH,
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let mut d = Dialog {
            name: initial.0,
            words: initial.1,
            done: false,
            accepted: false,
        };
        let w = CreateWindowExW(
            WS_EX_DLGMODALFRAME,
            class.as_ptr(),
            wide("OpenCTL WinTab context").as_ptr(),
            WS_CAPTION | WS_SYSMENU | WS_POPUP,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            440,
            404,
            owner,
            ptr::null_mut(),
            module,
            (&mut d as *mut Dialog).cast(),
        );
        if w.is_null() {
            return None;
        }
        let owner_enabled = !owner.is_null()
            && windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled(owner) != 0;
        if owner_enabled {
            EnableWindow(owner, 0);
        }
        ShowWindow(w, SW_SHOW);
        SetForegroundWindow(w);
        let mut message = MSG::default();
        // DispatchMessage invokes procedure, which sets d.done through userdata.
        #[allow(clippy::while_immutable_condition)]
        while !d.done {
            let result = GetMessageW(&mut message, ptr::null_mut(), 0, 0);
            if result <= 0 {
                if result == 0 {
                    PostQuitMessage(message.wParam as i32);
                }
                break;
            }
            if IsDialogMessageW(w, &message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        if IsWindow(w) != 0 {
            DestroyWindow(w);
        }
        if owner_enabled {
            EnableWindow(owner, 1);
            SetForegroundWindow(owner);
        }
        if d.accepted {
            Some((d.name, d.words))
        } else {
            None
        }
    }
}
