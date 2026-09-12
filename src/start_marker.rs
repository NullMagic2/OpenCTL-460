//! A tiny non-activating, click-through marker owned by the feeder.
//! Its UI thread never blocks pen submission. No hooks, cursor changes or input injection.
use std::{
    sync::{
        atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering},
        mpsc, Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{LibraryLoader::GetModuleHandleW, SystemInformation::GetTickCount64},
    UI::{HiDpi::*, WindowsAndMessaging::*},
};
struct Shared {
    visible: AtomicBool,
    position: AtomicU64,
    foreground: AtomicIsize,
    heartbeat: AtomicU64,
    stop: AtomicBool,
}
pub struct StartMarker {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}
impl StartMarker {
    pub fn new() -> Result<Self, String> {
        let shared = Arc::new(Shared {
            visible: AtomicBool::new(false),
            position: AtomicU64::new(0),
            foreground: AtomicIsize::new(0),
            heartbeat: AtomicU64::new(0),
            stop: AtomicBool::new(false),
        });
        let state = shared.clone();
        let (tx, rx) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("ctl460-start-marker".into())
            .spawn(move || unsafe {
                SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
                let class: Vec<u16> = "OpenCTLStrokeStartMarker\0".encode_utf16().collect();
                let instance = GetModuleHandleW(std::ptr::null());
                let wc = WNDCLASSW {
                    lpfnWndProc: Some(window_proc),
                    hInstance: instance,
                    lpszClassName: class.as_ptr(),
                    ..Default::default()
                };
                RegisterClassW(&wc);
                let w = CreateWindowExW(
                    WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                    class.as_ptr(),
                    class.as_ptr(),
                    WS_POPUP,
                    0,
                    0,
                    40,
                    40,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    instance,
                    std::ptr::null(),
                );
                if w.is_null() {
                    let _ = tx.send(Err("Cannot create stroke-start marker".to_string()));
                    return;
                }
                SetLayeredWindowAttributes(w, 0x00ff00ff, 255, LWA_COLORKEY);
                let _ = tx.send(Ok(()));
                let mut shown = false;
                let mut last = u64::MAX;
                while !state.stop.load(Ordering::Acquire) {
                    let mut msg = MSG::default();
                    while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                    let alive = GetTickCount64()
                        .saturating_sub(state.heartbeat.load(Ordering::Acquire))
                        < 250;
                    let show = alive
                        && state.visible.load(Ordering::Acquire)
                        && GetForegroundWindow() as isize
                            == state.foreground.load(Ordering::Acquire);
                    let point = state.position.load(Ordering::Acquire);
                    if show && (!shown || point != last) {
                        let x = (point as u32) as i32;
                        let y = ((point >> 32) as u32) as i32;
                        SetWindowPos(
                            w,
                            HWND_TOPMOST,
                            x.saturating_sub(20),
                            y.saturating_sub(20),
                            40,
                            40,
                            SWP_NOACTIVATE | SWP_SHOWWINDOW,
                        );
                        InvalidateRect(w, std::ptr::null(), 1);
                        last = point;
                    } else if !show && shown {
                        ShowWindow(w, SW_HIDE);
                    }
                    shown = show;
                    thread::sleep(Duration::from_millis(12));
                }
                DestroyWindow(w);
            })
            .map_err(|e| e.to_string())?;
        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(())) => Ok(Self {
                shared,
                thread: Some(thread),
            }),
            result => {
                shared.stop.store(true, Ordering::Release);
                let _ = thread.join();
                Err(match result {
                    Ok(Err(e)) => e,
                    _ => "Stroke-start marker did not initialize".into(),
                })
            }
        }
    }
    pub fn show(&self, point: Option<(i32, i32)>, foreground: isize) {
        if let Some((x, y)) = point {
            self.shared.position.store(
                u64::from(x as u32) | (u64::from(y as u32) << 32),
                Ordering::Release,
            );
            self.shared.foreground.store(foreground, Ordering::Release);
            self.shared
                .heartbeat
                .store(unsafe { GetTickCount64() }, Ordering::Release);
            self.shared.visible.store(true, Ordering::Release);
        } else {
            self.shared.visible.store(false, Ordering::Release);
        }
    }
}
impl Drop for StartMarker {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
unsafe extern "system" fn window_proc(w: HWND, m: u32, a: WPARAM, b: LPARAM) -> LRESULT {
    unsafe {
        match m {
            WM_NCHITTEST => HTTRANSPARENT as LRESULT,
            WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
            WM_ERASEBKGND => 1,
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let dc = BeginPaint(w, &mut ps);
                let bg = CreateSolidBrush(0x00ff00ff);
                let rect = RECT {
                    left: 0,
                    top: 0,
                    right: 40,
                    bottom: 40,
                };
                FillRect(dc, &rect, bg);
                DeleteObject(bg as _);
                let old_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
                for (width, color) in [(4, 0x00000000), (2, 0x00ffffff)] {
                    let pen = CreatePen(PS_SOLID, width, color);
                    let old = SelectObject(dc, pen as _);
                    Ellipse(dc, 12, 12, 28, 28);
                    MoveToEx(dc, 20, 7, std::ptr::null_mut());
                    LineTo(dc, 20, 14);
                    MoveToEx(dc, 20, 26, std::ptr::null_mut());
                    LineTo(dc, 20, 33);
                    MoveToEx(dc, 7, 20, std::ptr::null_mut());
                    LineTo(dc, 14, 20);
                    MoveToEx(dc, 26, 20, std::ptr::null_mut());
                    LineTo(dc, 33, 20);
                    SelectObject(dc, old);
                    DeleteObject(pen as _);
                }
                SelectObject(dc, old_brush);
                EndPaint(w, &ps);
                0
            }
            _ => DefWindowProcW(w, m, a, b),
        }
    }
}
