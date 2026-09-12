//! Native square ink surface. Receives read-only feeder samples, never injects input.
use ctl460_rust::calibration_pad::Pad;
use std::ptr::{null, null_mut};
use windows_sys::Win32::{
    Foundation::*, Graphics::Gdi::*, System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::*,
};
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
#[derive(Default)]
struct Canvas {
    pad: Pad,
    font: HFONT,
    recording: bool,
}
pub fn register() -> Result<(), String> {
    let name = wide("CTL460CalibrationCanvas");
    let wc = WNDCLASSW {
        lpfnWndProc: Some(procedure),
        hInstance: unsafe { GetModuleHandleW(null()) },
        lpszClassName: name.as_ptr(),
        hCursor: unsafe { LoadCursorW(null_mut(), IDC_CROSS) },
        ..Default::default()
    };
    if unsafe { RegisterClassW(&wc) } == 0 {
        return Err("Cannot register calibration drawing pad".into());
    }
    Ok(())
}
pub fn feed(w: HWND, x: f64, y: f64, pressure: f64, contact: bool) {
    unsafe {
        let p = GetWindowLongPtrW(w, GWLP_USERDATA) as *mut Canvas;
        if !p.is_null() && (*p).recording && (*p).pad.feed(x, y, pressure, contact) {
            InvalidateRect(w, null(), 0);
        }
    }
}
pub fn recording(w: HWND, active: bool) {
    unsafe {
        let p = GetWindowLongPtrW(w, GWLP_USERDATA) as *mut Canvas;
        if !p.is_null() {
            (*p).recording = active;
            (*p).pad.lift();
            InvalidateRect(w, null(), 0);
        }
    }
}
pub fn clear(w: HWND) {
    unsafe {
        let p = GetWindowLongPtrW(w, GWLP_USERDATA) as *mut Canvas;
        if !p.is_null() {
            (*p).pad.clear();
            InvalidateRect(w, null(), 0);
        }
    }
}
pub fn lift(w: HWND) {
    unsafe {
        let p = GetWindowLongPtrW(w, GWLP_USERDATA) as *mut Canvas;
        if !p.is_null() {
            (*p).pad.lift();
        }
    }
}
unsafe fn paint(w: HWND, dc: HDC, s: &Canvas) {
    unsafe {
        let mut r = RECT::default();
        GetClientRect(w, &mut r);
        FillRect(dc, &r, GetStockObject(WHITE_BRUSH) as HBRUSH);
        FrameRect(dc, &r, GetStockObject(GRAY_BRUSH) as HBRUSH);
        if s.pad.segments.is_empty() {
            SetBkMode(dc, TRANSPARENT as i32);
            SetTextColor(dc, 0x808080);
            let old = SelectObject(
                dc,
                if s.font.is_null() {
                    GetStockObject(DEFAULT_GUI_FONT)
                } else {
                    s.font as _
                },
            );
            let text = wide(if s.recording {
                "Draw here"
            } else {
                "Start a stage to draw here"
            });
            DrawTextW(
                dc,
                text.as_ptr(),
                -1,
                &mut r,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
            SelectObject(dc, old);
        }
        let pens: Vec<_> = (0..16)
            .map(|i| {
                let shade = (150 - i * 8) as u32;
                CreatePen(
                    PS_SOLID,
                    ((1. + i as f64 / 15. * 8.) * r.right as f64 / 300.)
                        .round()
                        .max(1.) as i32,
                    shade * 0x010101,
                )
            })
            .collect();
        let old = SelectObject(dc, pens[0] as _);
        for seg in &s.pad.segments {
            let i = (seg.b.pressure * 15.).round() as usize;
            SelectObject(dc, pens[i] as _);
            let x = |n: f64| 1 + (n * (r.right - 3).max(1) as f64).round() as i32;
            let y = |n: f64| 1 + (n * (r.bottom - 3).max(1) as f64).round() as i32;
            let (ax, ay, bx, by) = (x(seg.a.x), y(seg.a.y), x(seg.b.x), y(seg.b.y));
            MoveToEx(dc, ax, ay, null_mut());
            LineTo(dc, if ax == bx && ay == by { bx + 1 } else { bx }, by);
        }
        SelectObject(dc, old);
        for pen in pens {
            DeleteObject(pen as _);
        }
    }
}
unsafe extern "system" fn procedure(w: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_NCCREATE {
            SetWindowLongPtrW(
                w,
                GWLP_USERDATA,
                Box::into_raw(Box::<Canvas>::default()) as isize,
            );
            return 1;
        }
        let p = GetWindowLongPtrW(w, GWLP_USERDATA) as *mut Canvas;
        if p.is_null() {
            return DefWindowProcW(w, msg, wp, lp);
        }
        match msg {
            WM_SETFONT => {
                (*p).font = wp as HFONT;
                0
            }
            WM_GETFONT => (*p).font as isize,
            WM_ERASEBKGND => 1,
            WM_PRINTCLIENT => {
                paint(w, wp as HDC, &*p);
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let dc = BeginPaint(w, &mut ps);
                let mut r = RECT::default();
                GetClientRect(w, &mut r);
                let mem = CreateCompatibleDC(dc);
                let bmp = CreateCompatibleBitmap(dc, r.right.max(1), r.bottom.max(1));
                if !mem.is_null() && !bmp.is_null() {
                    let old = SelectObject(mem, bmp as _);
                    paint(w, mem, &*p);
                    BitBlt(dc, 0, 0, r.right, r.bottom, mem, 0, 0, SRCCOPY);
                    SelectObject(mem, old);
                } else {
                    paint(w, dc, &*p);
                }
                if !bmp.is_null() {
                    DeleteObject(bmp as _);
                }
                if !mem.is_null() {
                    DeleteDC(mem);
                }
                EndPaint(w, &ps);
                0
            }
            WM_POINTERDOWN | WM_POINTERUPDATE | WM_POINTERUP => 0,
            WM_SHOWWINDOW if wp == 0 => {
                (*p).pad.lift();
                DefWindowProcW(w, msg, wp, lp)
            }
            WM_NCDESTROY => {
                SetWindowLongPtrW(w, GWLP_USERDATA, 0);
                drop(Box::from_raw(p));
                DefWindowProcW(w, msg, wp, lp)
            }
            _ => DefWindowProcW(w, msg, wp, lp),
        }
    }
}

#[cfg(test)]
pub fn sample_count(w: HWND) -> usize {
    unsafe {
        let p = GetWindowLongPtrW(w, GWLP_USERDATA) as *mut Canvas;
        if p.is_null() {
            0
        } else {
            (*p).pad.segments.len()
        }
    }
}
