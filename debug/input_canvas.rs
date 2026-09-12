//! Controlled fullscreen Windows Ink receiver for physical-tablet testing, with pressure telemetry.
//! Signals the feeder to stop on Escape, focus loss or timeout; leaves the canvas up during release.
#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(windows)]
#[allow(dead_code)]
#[path = "../src/output.rs"]
mod output;
#[cfg(windows)]
#[path = "../src/windows.rs"]
mod windows;
#[cfg(not(windows))]
fn main() {}
#[cfg(windows)]
mod app {
    use std::{
        ffi::c_void,
        fs,
        path::PathBuf,
        ptr::{null, null_mut},
    };
    use windows_sys::Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::{LibraryLoader::GetModuleHandleW, Threading::*},
        UI::{HiDpi::SetProcessDpiAwarenessContext, Input::Pointer::*, WindowsAndMessaging::*},
    };
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    struct State {
        event: HANDLE,
        ready: PathBuf,
        report: PathBuf,
        points: u32,
        downs: u32,
        ups: u32,
        max: u32,
        last: Option<POINT>,
        lines: Vec<(POINT, POINT, u32)>,
        ending: bool,
        started: bool,
        samples: Vec<String>,
    }
    fn end(w: HWND, s: &mut State) {
        if !s.ending {
            unsafe {
                SetEvent(s.event);
                SetTimer(w, 3, 1200, None);
                SetWindowTextW(
                    w,
                    wide("CTL-460 test ending; restoring the original driver...").as_ptr(),
                );
            }
            s.ending = true;
        }
    }
    unsafe extern "system" fn procedure(w: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        unsafe {
            if msg == WM_NCCREATE {
                let cs = &*(lp as *const CREATESTRUCTW);
                SetWindowLongPtrW(w, GWLP_USERDATA, cs.lpCreateParams as isize);
            }
            let p = GetWindowLongPtrW(w, GWLP_USERDATA) as *mut State;
            if p.is_null() {
                return DefWindowProcW(w, msg, wp, lp);
            }
            let s = &mut *p;
            match msg {
                WM_CREATE => {
                    SetTimer(w, 1, 45000, None);
                    SetTimer(w, 2, 100, None);
                    0
                }
                WM_ACTIVATE => {
                    if wp & 0xffff == WA_INACTIVE as usize && s.started {
                        end(w, s);
                    }
                    0
                }
                WM_KEYDOWN if wp == 27 => {
                    end(w, s);
                    0
                }
                WM_CLOSE => {
                    end(w, s);
                    0
                }
                WM_TIMER => {
                    if wp == 3 {
                        DestroyWindow(w);
                    } else if wp == 1 || WaitForSingleObject(s.event, 0) == WAIT_OBJECT_0 {
                        end(w, s);
                    }
                    0
                }
                WM_POINTERDOWN | WM_POINTERUPDATE | WM_POINTERUP => {
                    let mut pen = POINTER_PEN_INFO::default();
                    if GetPointerPenInfo((wp & 0xffff) as u32, &mut pen) != 0 {
                        s.samples.push(format!(
                            "{},{},{},{},{},{}",
                            pen.pointerInfo.dwTime,
                            msg,
                            pen.pointerInfo.ptPixelLocation.x,
                            pen.pointerInfo.ptPixelLocation.y,
                            pen.pressure,
                            pen.pointerInfo.sourceDevice as usize
                        ));
                        s.points += 1;
                        s.max = s.max.max(pen.pressure);
                        let mut point = pen.pointerInfo.ptPixelLocation;
                        ScreenToClient(w, &mut point);
                        if msg == WM_POINTERDOWN {
                            s.downs += 1;
                            s.last = Some(point);
                        }
                        if pen.pointerInfo.pointerFlags & POINTER_FLAG_INCONTACT != 0 {
                            let old = s.last.unwrap_or(point);
                            s.lines.push((old, point, pen.pressure));
                            s.last = Some(point);
                        }
                        if msg == WM_POINTERUP {
                            s.ups += 1;
                            s.last = None;
                        }
                        // Draw just the new segment; avoid re-rendering the whole stroke history per report.
                        if let Some((a, b, pressure)) = s.lines.last() {
                            let dc = GetDC(w);
                            let brush =
                                CreatePen(PS_SOLID, 1 + (*pressure * 8 / 1024) as i32, 0xA05020);
                            let old = SelectObject(dc, brush as HGDIOBJ);
                            MoveToEx(dc, a.x, a.y, null_mut());
                            LineTo(dc, b.x, b.y);
                            if a.x == b.x && a.y == b.y {
                                SetPixel(dc, a.x, a.y, 0xA05020);
                            }
                            SelectObject(dc, old);
                            DeleteObject(brush as HGDIOBJ);
                            ReleaseDC(w, dc);
                        }
                        let header = RECT {
                            left: 0,
                            top: 0,
                            right: GetSystemMetrics(SM_CXSCREEN),
                            bottom: 80,
                        };
                        InvalidateRect(w, &header, 0);
                    }
                    0
                }
                WM_PAINT => {
                    let mut paint = PAINTSTRUCT::default();
                    let dc = BeginPaint(w, &mut paint);
                    FillRect(dc, &paint.rcPaint, GetStockObject(WHITE_BRUSH) as HBRUSH);
                    SetBkMode(dc, TRANSPARENT as i32);
                    SetTextColor(dc, 0x303030);
                    let title =
                        wide("CTL-460 live ink test - write below. Escape finishes the test.");
                    TextOutW(dc, 24, 20, title.as_ptr(), title.len() as i32 - 1);
                    let label=wide(&format!("Pen events: {}   Strokes: {}   Releases: {}   Maximum Ink pressure: {}/1024",s.points,s.downs,s.ups,s.max));
                    TextOutW(dc, 24, 48, label.as_ptr(), label.len() as i32 - 1);
                    for (a, b, pressure) in &s.lines {
                        if a.y.min(b.y) > paint.rcPaint.bottom + 10
                            || a.y.max(b.y) < paint.rcPaint.top - 10
                        {
                            continue;
                        }
                        let pen = CreatePen(PS_SOLID, 1 + (*pressure * 8 / 1024) as i32, 0xA05020);
                        let old = SelectObject(dc, pen as HGDIOBJ);
                        MoveToEx(dc, a.x, a.y, null_mut());
                        LineTo(dc, b.x, b.y);
                        if a.x == b.x && a.y == b.y {
                            SetPixel(dc, a.x, a.y, 0xA05020);
                        }
                        SelectObject(dc, old);
                        DeleteObject(pen as HGDIOBJ);
                    }
                    EndPaint(w, &paint);
                    0
                }
                WM_DESTROY => {
                    SetEvent(s.event);
                    KillTimer(w, 1);
                    KillTimer(w, 2);
                    KillTimer(w, 3);
                    PostQuitMessage(0);
                    0
                }
                _ => DefWindowProcW(w, msg, wp, lp),
            }
        }
    }
    pub fn run() -> Result<(), String> {
        let args: Vec<_> = std::env::args().skip(1).collect();
        if args.len() != 3 && args.len() != 4 {
            return Err("Expected stop-event, ready-file and report-file arguments".into());
        }
        unsafe {
            SetProcessDpiAwarenessContext(-4isize as *mut c_void);
            let automatic = args.get(3).is_some_and(|a| a == "--auto");
            let event = if automatic {
                CreateEventW(null(), 1, 0, wide(&args[0]).as_ptr())
            } else {
                OpenEventW(
                    EVENT_MODIFY_STATE | SYNCHRONIZATION_SYNCHRONIZE,
                    0,
                    wide(&args[0]).as_ptr(),
                )
            };
            if event.is_null() {
                return Err("Cannot open test stop event".into());
            }
            let mut s = Box::new(State {
                event,
                ready: args[1].clone().into(),
                report: args[2].clone().into(),
                points: 0,
                downs: 0,
                ups: 0,
                max: 0,
                last: None,
                lines: Vec::new(),
                ending: false,
                started: false,
                samples: Vec::new(),
            });
            let class = wide("CTL460InputTestCanvas");
            let instance = GetModuleHandleW(null());
            let wc = WNDCLASSW {
                lpfnWndProc: Some(procedure),
                hInstance: instance,
                lpszClassName: class.as_ptr(),
                hCursor: LoadCursorW(null_mut(), IDC_CROSS),
                hbrBackground: GetStockObject(WHITE_BRUSH) as HBRUSH,
                ..Default::default()
            };
            if RegisterClassW(&wc) == 0 {
                CloseHandle(event);
                return Err("Cannot register test canvas".into());
            }
            let w = CreateWindowExW(
                0,
                class.as_ptr(),
                wide("CTL-460 input test").as_ptr(),
                WS_POPUP | WS_VISIBLE,
                0,
                0,
                GetSystemMetrics(SM_CXSCREEN),
                GetSystemMetrics(SM_CYSCREEN),
                null_mut(),
                null_mut(),
                instance,
                s.as_mut() as *mut State as *const c_void,
            );
            if w.is_null() {
                CloseHandle(event);
                return Err("Cannot open test canvas".into());
            }
            ShowWindow(w, SW_SHOW);
            SetForegroundWindow(w);
            if GetForegroundWindow() != w {
                let foreground_thread = GetWindowThreadProcessId(GetForegroundWindow(), null_mut());
                let current_thread = GetCurrentThreadId();
                if foreground_thread != 0
                    && foreground_thread != current_thread
                    && AttachThreadInput(current_thread, foreground_thread, 1) != 0
                {
                    SetForegroundWindow(w);
                    AttachThreadInput(current_thread, foreground_thread, 0);
                }
            }
            UpdateWindow(w);
            if GetForegroundWindow() != w {
                DestroyWindow(w);
                CloseHandle(event);
                return Err("Canvas did not get focus; input test cancelled".into());
            }
            s.started = true;
            fs::write(&s.ready, "Canvas is foreground; input may start.\n")
                .map_err(|e| e.to_string())?;
            let worker = if automatic {
                let hwnd = w as usize;
                let stop = event as usize;
                let expected = s.report.with_extension("expected.csv");
                Some(std::thread::spawn(move || -> Result<(), String> {
                    use ctl460_rust::{config::Config, engine::Frame, stroke::map_to_screen};
                    use std::{io::Write, time::Duration};
                    let mut c = Config {
                        backend: "hid".into(),
                        button1: "Precision Hold".into(),
                        precision_gain: [25.0; 2],
                        double_click_distance: 0,
                        show_start_marker: false,
                        close_shapes: false,
                        ..Default::default()
                    };
                    let mut sink = crate::output::Output::new(&c)?;
                    let mut log = fs::File::create(expected).map_err(|e| e.to_string())?;
                    writeln!(log, "stroke,step,expected_x,expected_y").unwrap();
                    for stroke in 0..2 {
                        sink.submit(Frame {
                            x: 5000,
                            y: 4000,
                            in_range: true,
                            ..Default::default()
                        })?;
                        if stroke == 1 {
                            sink.set_precision(&c, [true, false]);
                            sink.set_precision(&c, [false, false]);
                        }
                        for step in 0..=80u16 {
                            if GetForegroundWindow() as usize != hwnd {
                                sink.submit(Frame::default())?;
                                return Err("Canvas lost focus".into());
                            }
                            let f = Frame {
                                x: 5000 + step * 40,
                                y: 4000 + step * 15,
                                pressure: if step >= 40 { 0.5 } else { 0.0 },
                                contact: step >= 40,
                                in_range: true,
                                ..Default::default()
                            };
                            let gain = if stroke == 1 { 0.25 } else { 1.0 };
                            let (x, y) = (
                                5000.0 + f64::from(step) * 40.0 * gain,
                                4000.0 + f64::from(step) * 15.0 * gain,
                            );
                            let p = map_to_screen(
                                x.round() as u16,
                                y.round() as u16,
                                GetSystemMetrics(SM_CXSCREEN),
                                GetSystemMetrics(SM_CYSCREEN),
                                c.preserve_aspect,
                            );
                            writeln!(log, "{stroke},{step},{},{}", p.0, p.1).unwrap();
                            sink.submit(f)?;
                            std::thread::sleep(Duration::from_millis(12));
                        }
                        sink.submit(Frame::default())?;
                        std::thread::sleep(Duration::from_millis(150));
                    }
                    // Also exercise the same live-settings path as the GUI.
                    c.streamline_amount = 30.0;
                    sink.reconfigure(&c)?;
                    SetEvent(stop as HANDLE);
                    Ok(())
                }))
            } else {
                None
            };
            let mut msg = MSG::default();
            loop {
                let n = GetMessageW(&mut msg, null_mut(), 0, 0);
                if n <= 0 {
                    break;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            fs::write(&s.report,format!("# Real CTL-460 Windows Ink receiver test; counts are delivered application events.\npen_events={}\nstrokes={}\nreleases={}\nmaximum_ink_pressure={}\n",s.points,s.downs,s.ups,s.max)).map_err(|e|e.to_string())?;
            fs::write(
                s.report.with_extension("received.csv"),
                format!(
                    "time,message,x,y,pressure,device\n{}\n",
                    s.samples.join("\n")
                ),
            )
            .map_err(|e| e.to_string())?;
            if let Some(worker) = worker {
                worker.join().map_err(|_| "Audit worker panicked")??;
            }
            CloseHandle(event);
            Ok(())
        }
    }
}
#[cfg(windows)]
fn main() {
    if let Err(e) = app::run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
