//! Windows notification-area icon and menu. Hiding settings never stops pen input.
//! Menu actions are posted to the owner so its worker shutdown remains asynchronous.
use std::{mem::size_of, sync::OnceLock};
use windows_sys::Win32::{
    Foundation::*,
    UI::{Shell::*, WindowsAndMessaging::*},
};

pub const CALLBACK: u32 = WM_APP + 30;
pub const EXIT: u32 = WM_APP + 31;
pub const OPEN: u32 = WM_APP + 32;
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub fn taskbar_message() -> u32 {
    static MESSAGE: OnceLock<u32> = OnceLock::new();
    *MESSAGE.get_or_init(|| unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) })
}
fn data(w: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: w,
        uID: 1,
        ..Default::default()
    }
}
/// Refresh an existing icon or recreate it after Explorer restarts.
pub fn ensure(w: HWND) -> bool {
    unsafe {
        let mut icon = data(w);
        icon.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP;
        icon.uCallbackMessage = CALLBACK;
        icon.hIcon = SendMessageW(w, WM_GETICON, ICON_SMALL as usize, 0) as HICON;
        let tip = wide("OpenCTL 460 - Tablet settings");
        icon.szTip[..tip.len()].copy_from_slice(&tip);
        let present =
            Shell_NotifyIconW(NIM_MODIFY, &icon) != 0 || Shell_NotifyIconW(NIM_ADD, &icon) != 0;
        if present {
            icon.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            Shell_NotifyIconW(NIM_SETVERSION, &icon);
        }
        present
    }
}
pub fn remove(w: HWND) {
    unsafe {
        Shell_NotifyIconW(NIM_DELETE, &data(w));
    }
}
pub fn open(w: HWND) {
    unsafe {
        ShowWindow(w, SW_RESTORE);
        SetForegroundWindow(w);
    }
}
/// Called before borrowing GUI state: TrackPopupMenu runs a nested message loop.
pub fn callback(w: HWND, wp: WPARAM, lp: LPARAM) {
    unsafe {
        match (lp & 0xffff) as u32 {
            WM_LBUTTONUP | WM_LBUTTONDBLCLK | NIN_SELECT | 1025 => {
                // NIN_KEYSELECT
                PostMessageW(w, OPEN, 0, 0);
            }
            WM_CONTEXTMENU | WM_RBUTTONUP => {
                let menu = CreatePopupMenu();
                if menu.is_null() {
                    return;
                }
                AppendMenuW(menu, MF_STRING, 1, wide("Open settings").as_ptr());
                AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
                AppendMenuW(menu, MF_STRING, 2, wide("Exit").as_ptr());
                SetMenuDefaultItem(menu, 1, 0);
                let mut point = POINT {
                    x: (wp as u16 as i16) as i32,
                    y: ((wp >> 16) as u16 as i16) as i32,
                };
                if (lp >> 16) == 0 || point.x == -1 && point.y == -1 {
                    GetCursorPos(&mut point);
                }
                SetForegroundWindow(w);
                let action = TrackPopupMenu(
                    menu,
                    TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
                    point.x,
                    point.y,
                    0,
                    w,
                    std::ptr::null(),
                );
                DestroyMenu(menu);
                // A posted message lets the shell dismiss the menu before opening/exiting.
                PostMessageW(
                    w,
                    if action == 1 {
                        OPEN
                    } else if action == 2 {
                        EXIT
                    } else {
                        WM_NULL
                    },
                    0,
                    0,
                );
                if action == 0 {
                    Shell_NotifyIconW(NIM_SETFOCUS, &data(w));
                }
            }
            _ => {}
        }
    }
}
