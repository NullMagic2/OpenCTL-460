//! Native GDI pressure editor and pen illustration. All drawing is local, with no input injection.
//! The editor owns its state until WM_NCDESTROY and posts changes to avoid reentrant parent borrows.
use ctl460_rust::{
    config::Config,
    pressure_editor::{drag, Handle},
};
use std::ptr::{null, null_mut};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        HiDpi::GetDpiForWindow,
        Input::KeyboardAndMouse::{GetFocus, ReleaseCapture, SetCapture, SetFocus},
        WindowsAndMessaging::*,
    },
};
pub const CURVE_CHANGED: u32 = WM_APP + 10;
/// Renders only our own client area into a BMP for repeatable visual QA, even on a hidden desktop.
pub fn snapshot(w: HWND, path: &std::path::Path) -> Result<(), String> {
    unsafe {
        let mut r = RECT::default();
        GetClientRect(w, &mut r);
        let dc = GetDC(w);
        let memory = CreateCompatibleDC(dc);
        let bitmap = CreateCompatibleBitmap(dc, r.right, r.bottom);
        if memory.is_null() || bitmap.is_null() {
            if !memory.is_null() {
                DeleteDC(memory);
            }
            if !bitmap.is_null() {
                DeleteObject(bitmap as _);
            }
            ReleaseDC(w, dc);
            return Err("Cannot allocate preview bitmap".into());
        }
        let old = SelectObject(memory, bitmap as _);
        SendMessageW(
            w,
            WM_PRINT,
            memory as usize,
            (PRF_CLIENT | PRF_CHILDREN | PRF_ERASEBKGND) as isize,
        );
        GdiFlush();
        SelectObject(memory, old);
        let size = (r.right * r.bottom * 4) as usize;
        let mut pixels = vec![0u8; size];
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: 40,
                biWidth: r.right,
                biHeight: r.bottom,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..Default::default()
            },
            ..Default::default()
        };
        let rows = GetDIBits(
            memory,
            bitmap,
            0,
            r.bottom as u32,
            pixels.as_mut_ptr() as _,
            &mut info,
            DIB_RGB_COLORS,
        );
        DeleteObject(bitmap as _);
        DeleteDC(memory);
        ReleaseDC(w, dc);
        if rows == 0 {
            return Err("Cannot read preview bitmap".into());
        }
        let mut bytes = Vec::with_capacity(54 + size);
        bytes.extend_from_slice(b"BM");
        bytes.extend_from_slice(&((54 + size) as u32).to_le_bytes());
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&54u32.to_le_bytes());
        bytes.extend_from_slice(&40u32.to_le_bytes());
        bytes.extend_from_slice(&r.right.to_le_bytes());
        bytes.extend_from_slice(&r.bottom.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&32u16.to_le_bytes());
        bytes.extend_from_slice(&[0; 24]);
        bytes.extend_from_slice(&pixels);
        std::fs::write(path, bytes).map_err(|e| e.to_string())
    }
}
const SET_CONFIG: u32 = WM_APP + 11;
const GET_CONFIG: u32 = WM_APP + 12;
const ADD_NODE: u32 = WM_APP + 13;
const REMOVE_NODE: u32 = WM_APP + 14;
pub fn add_node(w: HWND) {
    unsafe {
        SendMessageW(w, ADD_NODE, 0, 0);
    }
}
pub fn remove_node(w: HWND) {
    unsafe {
        SendMessageW(w, REMOVE_NODE, 0, 0);
    }
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub fn set(w: HWND, c: &Config) {
    unsafe {
        SendMessageW(w, SET_CONFIG, 0, c as *const Config as isize);
    }
}
pub fn get(w: HWND) -> Config {
    let mut c = Config::default();
    unsafe {
        SendMessageW(w, GET_CONFIG, 0, &mut c as *mut Config as isize);
    }
    c
}
pub fn register() -> Result<(), String> {
    unsafe {
        let classes: [(&str, WNDPROC); 5] = [
            ("CTL460PressureCurve", Some(curve_proc)),
            ("CTL460PenPicture", Some(pen_proc)),
            ("CTL460DistanceRamp", Some(decoration_proc)),
            ("CTL460SunkenPanel", Some(decoration_proc)),
            ("CTL460Separator", Some(decoration_proc)),
        ];
        for (name, proc) in classes {
            let name = wide(name);
            let class = WNDCLASSW {
                lpfnWndProc: proc,
                hInstance: GetModuleHandleW(null()),
                lpszClassName: name.as_ptr(),
                hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                ..Default::default()
            };
            if RegisterClassW(&class) == 0 {
                return Err("Cannot create pressure editor".into());
            }
        }
    }
    Ok(())
}
struct Editor {
    config: Config,
    dragging: Option<Handle>,
    response_x: f64,
    selected: Option<usize>,
}
fn dpi_px(w: HWND, value: i32) -> i32 {
    (value * unsafe { GetDpiForWindow(w) }.max(96) as i32 + 48) / 96
}
fn plot(w: HWND) -> RECT {
    let mut r = RECT::default();
    unsafe {
        GetClientRect(w, &mut r);
    }
    RECT {
        left: dpi_px(w, 48),
        top: dpi_px(w, 34),
        right: r.right - dpi_px(w, 20),
        bottom: r.bottom - dpi_px(w, 42),
    }
}
fn point(r: RECT, x: f64, y: f64) -> (i32, i32) {
    (
        r.left + (x * f64::from(r.right - r.left)).round() as i32,
        r.bottom - (y * f64::from(r.bottom - r.top)).round() as i32,
    )
}
fn gesture(w: HWND, e: &mut Editor, lp: LPARAM, begin: bool) {
    let r = plot(w);
    let px = (lp as u16 as i16) as i32;
    let py = ((lp >> 16) as u16 as i16) as i32;
    let x = (f64::from(px - r.left) / f64::from(r.right - r.left)).clamp(0.0, 1.0);
    let y = (f64::from(r.bottom - py) / f64::from(r.bottom - r.top)).clamp(0.0, 1.0);
    if begin {
        // Dots straddle the graph border: their outer halves must also receive clicks.
        let inside = px >= r.left && px <= r.right && py >= r.top && py <= r.bottom;
        let near = |raw: u16| {
            let p = point(r, f64::from(raw) / 1023.0, e.config.curve(raw));
            (px - p.0).abs() <= dpi_px(w, 18) && (py - p.1).abs() <= dpi_px(w, 18)
        };
        if e.config.pressure_nodes.is_empty() {
            e.dragging = Some(if near(e.config.floor) {
                Handle::Zero
            } else if near(e.config.ceiling) {
                Handle::Full
            } else {
                if !inside {
                    return;
                }
                Handle::Response
            });
        } else {
            e.selected = e
                .config
                .pressure_nodes
                .iter()
                .enumerate()
                .find_map(|(i, n)| {
                    let raw = f64::from(e.config.floor)
                        + n.x * f64::from(e.config.ceiling - e.config.floor);
                    let p = point(r, raw / 1023.0, (n.y * e.config.gain).clamp(0.0, 1.0));
                    ((px - p.0).abs() <= dpi_px(w, 18) && (py - p.1).abs() <= dpi_px(w, 18))
                        .then_some(i)
                });
            if e.selected.is_none() {
                if !inside {
                    return;
                }
                let unit = (x * 1023.0 - f64::from(e.config.floor))
                    / f64::from(e.config.ceiling - e.config.floor);
                e.selected = ctl460_rust::pressure_profiles::add(&mut e.config, unit);
            }
            e.dragging = e.selected.map(Handle::Node);
        }
    }
    if let Some(handle) = e.dragging {
        drag(&mut e.config, handle, x, y);
        if matches!(handle, Handle::Response) {
            e.response_x = x;
        }
        unsafe {
            InvalidateRect(w, null(), 0);
            PostMessageW(GetParent(w), CURVE_CHANGED, 0, 0);
        }
    }
}
fn fill(dc: HDC, r: RECT, color: u32) {
    unsafe {
        let b = CreateSolidBrush(color);
        FillRect(dc, &r, b);
        DeleteObject(b as _);
    }
}
fn line(dc: HDC, a: (i32, i32), b: (i32, i32)) {
    unsafe {
        MoveToEx(dc, a.0, a.1, null_mut());
        LineTo(dc, b.0, b.1);
    }
}
fn caption(dc: HDC, text: &str, r: RECT) {
    unsafe {
        let mut r = r;
        DrawTextW(
            dc,
            wide(text).as_ptr(),
            -1,
            &mut r,
            DT_SINGLELINE | DT_VCENTER | if text == "Pen pressure" { DT_CENTER } else { 0 },
        );
    }
}
// Draw the graph at four times its display resolution and area-filter it down.
// Only the plot is supersampled, keeping labels crisp and pen response unchanged.
fn smooth_plot(w: HWND, dc: HDC, r: RECT, e: &Editor) {
    unsafe {
        const SCALE: i32 = 4;
        let pad = dpi_px(w, 9);
        let width = r.right - r.left + 2 * pad;
        let height = r.bottom - r.top + 2 * pad;
        let memory = CreateCompatibleDC(dc);
        let bitmap = CreateCompatibleBitmap(dc, width * SCALE, height * SCALE);
        if memory.is_null() || bitmap.is_null() {
            if !memory.is_null() {
                DeleteDC(memory);
            }
            if !bitmap.is_null() {
                DeleteObject(bitmap as _);
            }
            draw_plot(dc, r, e, dpi_px(w, 1));
            return;
        }
        let old = SelectObject(memory, bitmap as _);
        fill(
            memory,
            RECT {
                left: 0,
                top: 0,
                right: width * SCALE,
                bottom: height * SCALE,
            },
            0xffffff,
        );
        draw_plot(
            memory,
            RECT {
                left: pad * SCALE,
                top: pad * SCALE,
                right: (width - pad) * SCALE,
                bottom: (height - pad) * SCALE,
            },
            e,
            dpi_px(w, SCALE),
        );
        let saved = SaveDC(dc);
        SetStretchBltMode(dc, HALFTONE);
        SetBrushOrgEx(dc, 0, 0, null_mut());
        StretchBlt(
            dc,
            r.left - pad,
            r.top - pad,
            width,
            height,
            memory,
            0,
            0,
            width * SCALE,
            height * SCALE,
            SRCCOPY,
        );
        RestoreDC(dc, saved);
        SelectObject(memory, old);
        DeleteObject(bitmap as _);
        DeleteDC(memory);
    }
}
fn draw_plot(dc: HDC, r: RECT, e: &Editor, scale: i32) {
    unsafe {
        let grid = CreatePen(PS_SOLID, scale, 0xeee9e5);
        let old = SelectObject(dc, grid as _);
        for i in 0..=4 {
            let t = f64::from(i) / 4.0;
            line(dc, point(r, t, 0.0), point(r, t, 1.0));
            line(dc, point(r, 0.0, t), point(r, 1.0, t));
        }
        line(dc, point(r, 0.0, 0.0), point(r, 1.0, 1.0));
        SelectObject(dc, old);
        DeleteObject(grid as _);
        let pen = CreatePen(PS_SOLID, (5 * scale + 1) / 2, 0xc47d22);
        let old = SelectObject(dc, pen as _);
        // Two samples per supersampled pixel preserve curvature without integer sensor steps.
        let steps = ((r.right - r.left) * 2).max(256);
        for i in 0..=steps {
            let x = f64::from(i) / f64::from(steps);
            let p = point(r, x, e.config.curve_at(x * 1023.0));
            if i == 0 {
                MoveToEx(dc, p.0, p.1, null_mut());
            } else {
                LineTo(dc, p.0, p.1);
            }
        }
        let brush = CreateSolidBrush(0xc47d22);
        let oldbrush = SelectObject(dc, brush as _);
        let response = (e.response_x * 1023.0).round() as u16;
        let dots: Vec<(f64, f64)> = if e.config.pressure_nodes.is_empty() {
            [
                e.config.floor,
                response.clamp(e.config.floor, e.config.ceiling),
                e.config.ceiling,
            ]
            .into_iter()
            .map(|raw| (f64::from(raw) / 1023.0, e.config.curve(raw)))
            .collect()
        } else {
            e.config
                .pressure_nodes
                .iter()
                .map(|n| {
                    (
                        (f64::from(e.config.floor)
                            + n.x * f64::from(e.config.ceiling - e.config.floor))
                            / 1023.0,
                        (n.y * e.config.gain).clamp(0.0, 1.0),
                    )
                })
                .collect()
        };
        for (i, (x, y)) in dots.into_iter().enumerate() {
            let p = point(r, x, y);
            let radius = if e.selected == Some(i) {
                8 * scale
            } else {
                6 * scale
            };
            Ellipse(dc, p.0 - radius, p.1 - radius, p.0 + radius, p.1 + radius);
        }
        SelectObject(dc, oldbrush);
        DeleteObject(brush as _);
        SelectObject(dc, old);
        DeleteObject(pen as _);
    }
}
fn paint(w: HWND, dc: HDC, e: &Editor) {
    // Compose the entire control off screen, including its white background and
    // labels. Publishing one complete frame prevents a blank flash while dragging.
    unsafe {
        let mut client = RECT::default();
        GetClientRect(w, &mut client);
        if client.right <= 0 || client.bottom <= 0 {
            return;
        }
        let memory = CreateCompatibleDC(dc);
        let bitmap = CreateCompatibleBitmap(dc, client.right, client.bottom);
        if memory.is_null() || bitmap.is_null() {
            if !memory.is_null() {
                DeleteDC(memory);
            }
            if !bitmap.is_null() {
                DeleteObject(bitmap as _);
            }
            paint_frame(w, dc, e);
            return;
        }
        let old = SelectObject(memory, bitmap as _);
        paint_frame(w, memory, e);
        BitBlt(dc, 0, 0, client.right, client.bottom, memory, 0, 0, SRCCOPY);
        SelectObject(memory, old);
        DeleteObject(bitmap as _);
        DeleteDC(memory);
    }
}
fn paint_frame(w: HWND, dc: HDC, e: &Editor) {
    unsafe {
        let saved = SaveDC(dc);
        let mut client = RECT::default();
        GetClientRect(w, &mut client);
        fill(dc, client, 0xffffff);
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, 0x60564f);
        SelectObject(
            dc,
            SendMessageW(GetDlgItem(GetParent(w), 100), WM_GETFONT, 0, 0) as HGDIOBJ,
        );
        let r = plot(w);
        smooth_plot(w, dc, r, e);
        caption(
            dc,
            "Output",
            RECT {
                left: dpi_px(w, 8),
                top: dpi_px(w, 4),
                right: dpi_px(w, 100),
                bottom: dpi_px(w, 28),
            },
        );
        caption(
            dc,
            "Light",
            RECT {
                left: r.left,
                top: r.bottom + dpi_px(w, 10),
                right: r.left + dpi_px(w, 60),
                bottom: r.bottom + dpi_px(w, 34),
            },
        );
        caption(
            dc,
            "Pen pressure",
            RECT {
                left: r.left,
                top: r.bottom + dpi_px(w, 10),
                right: r.right,
                bottom: r.bottom + dpi_px(w, 34),
            },
        );
        caption(
            dc,
            "Firm",
            RECT {
                left: r.right - dpi_px(w, 30),
                top: r.bottom + dpi_px(w, 10),
                right: r.right + dpi_px(w, 20),
                bottom: r.bottom + dpi_px(w, 34),
            },
        );
        if GetFocus() == w {
            DrawFocusRect(dc, &client);
        }
        RestoreDC(dc, saved);
    }
}
unsafe extern "system" fn curve_proc(w: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_CREATE {
            let e = Box::new(Editor {
                config: Config::default(),
                dragging: None,
                response_x: 0.5,
                selected: None,
            });
            SetWindowLongPtrW(w, GWLP_USERDATA, Box::into_raw(e) as isize);
            return 0;
        }
        let ptr = GetWindowLongPtrW(w, GWLP_USERDATA) as *mut Editor;
        if ptr.is_null() {
            return DefWindowProcW(w, msg, wp, lp);
        }
        if msg == WM_NCDESTROY {
            SetWindowLongPtrW(w, GWLP_USERDATA, 0);
            drop(Box::from_raw(ptr));
            return DefWindowProcW(w, msg, wp, lp);
        }
        if msg == WM_SETFOCUS || msg == WM_KILLFOCUS {
            InvalidateRect(w, null(), 0);
            return 0;
        }
        if msg == WM_LBUTTONDOWN {
            SetFocus(w);
            SetCapture(w);
        }
        let e = &mut *ptr;
        match msg {
            ADD_NODE => {
                // Preserve the displayed response handle when converting a gamma curve.
                let response = (e.response_x * 1023.0 - f64::from(e.config.floor))
                    / f64::from(e.config.ceiling - e.config.floor);
                ctl460_rust::pressure_profiles::seed_at(&mut e.config, response);
                let gap = e
                    .config
                    .pressure_nodes
                    .windows(2)
                    .max_by(|a, b| (a[1].x - a[0].x).total_cmp(&(b[1].x - b[0].x)))
                    .unwrap();
                let x = (gap[0].x + gap[1].x) / 2.0;
                e.selected = ctl460_rust::pressure_profiles::add(&mut e.config, x);
                InvalidateRect(w, null(), 0);
                PostMessageW(GetParent(w), CURVE_CHANGED, 0, 0);
                0
            }
            REMOVE_NODE => {
                if let Some(i) = e.selected {
                    if ctl460_rust::pressure_profiles::remove(&mut e.config, i) {
                        e.selected = None;
                        InvalidateRect(w, null(), 0);
                        PostMessageW(GetParent(w), CURVE_CHANGED, 0, 0);
                    }
                }
                0
            }
            SET_CONFIG => {
                let config = &*(lp as *const Config);
                let changed = ctl460_rust::pressure_profiles::Shape::from_config(&e.config)
                    != ctl460_rust::pressure_profiles::Shape::from_config(config);
                e.config = config.clone();
                // Parent field synchronization echoes the same curve during a drag.
                if changed {
                    InvalidateRect(w, null(), 0);
                }
                0
            }
            GET_CONFIG => {
                *(lp as *mut Config) = e.config.clone();
                0
            }
            WM_LBUTTONDOWN => {
                gesture(w, e, lp, true);
                0
            }
            WM_MOUSEMOVE => {
                gesture(w, e, lp, false);
                0
            }
            WM_LBUTTONUP => {
                gesture(w, e, lp, false);
                e.dragging = None;
                ReleaseCapture();
                0
            }
            WM_CAPTURECHANGED => {
                e.dragging = None;
                0
            }
            WM_GETDLGCODE => DLGC_WANTARROWS as isize,
            WM_KEYDOWN if wp == 0x2e => {
                SendMessageW(w, REMOVE_NODE, 0, 0);
                0
            }
            WM_KEYDOWN if !e.config.pressure_nodes.is_empty() && (0x25..=0x28).contains(&wp) => {
                if let Some(i) = e.selected {
                    if let Some(n) = e.config.pressure_nodes.get(i).copied() {
                        let x = (f64::from(e.config.floor)
                            + n.x * f64::from(e.config.ceiling - e.config.floor))
                            / 1023.0;
                        let gain = e.config.gain;
                        drag(
                            &mut e.config,
                            Handle::Node(i),
                            x + if wp == 0x25 {
                                -0.005
                            } else if wp == 0x27 {
                                0.005
                            } else {
                                0.0
                            },
                            n.y * gain
                                + if wp == 0x26 {
                                    0.01
                                } else if wp == 0x28 {
                                    -0.01
                                } else {
                                    0.0
                                },
                        );
                        InvalidateRect(w, null(), 0);
                        PostMessageW(GetParent(w), CURVE_CHANGED, 0, 0);
                    }
                }
                0
            }
            WM_KEYDOWN if (0x25..=0x28).contains(&wp) => {
                e.config.gamma = (e.config.gamma
                    + if wp == 0x25 || wp == 0x26 {
                        -0.05
                    } else {
                        0.05
                    })
                .clamp(0.2, 4.0);
                InvalidateRect(w, null(), 0);
                PostMessageW(GetParent(w), CURVE_CHANGED, 0, 0);
                0
            }
            WM_SETFOCUS | WM_KILLFOCUS => {
                InvalidateRect(w, null(), 0);
                0
            }
            WM_PAINT => {
                let mut p = PAINTSTRUCT::default();
                let dc = BeginPaint(w, &mut p);
                paint(w, dc, e);
                EndPaint(w, &p);
                0
            }
            WM_PRINTCLIENT => {
                paint(w, wp as HDC, e);
                0
            }
            WM_ERASEBKGND => 1,
            _ => DefWindowProcW(w, msg, wp, lp),
        }
    }
}
struct PenImage {
    token: usize,
    image: *mut windows_sys::Win32::Graphics::GdiPlus::GpBitmap,
}
impl Drop for PenImage {
    fn drop(&mut self) {
        unsafe {
            use windows_sys::Win32::Graphics::GdiPlus::*;
            if !self.image.is_null() {
                GdipDisposeImage(self.image.cast());
            }
            if self.token != 0 {
                GdiplusShutdown(self.token);
            }
        }
    }
}
unsafe extern "system" fn pen_proc(w: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        use windows_sys::Win32::Graphics::GdiPlus::*;
        if msg == WM_CREATE {
            let mut pen = Box::new(PenImage {
                token: 0,
                image: null_mut(),
            });
            let input = GdiplusStartupInput {
                GdiplusVersion: 1,
                SuppressExternalCodecs: 1,
                ..Default::default()
            };
            if GdiplusStartup(&mut pen.token, &input, null_mut()) == 0 {
                if let std::result::Result::Ok(exe) = std::env::current_exe() {
                    if let Some(root) = exe.parent() {
                        let tablet = GetDlgCtrlID(w) == 216 || GetDlgCtrlID(w) == 217;
                        let path = wide(
                            &root
                                .join(if tablet {
                                    "assets/bamboo-tablet.png"
                                } else {
                                    "assets/bamboo-pen.png"
                                })
                                .to_string_lossy(),
                        );
                        GdipCreateBitmapFromFile(path.as_ptr(), &mut pen.image);
                        if GetDlgCtrlID(w) == 216 && !pen.image.is_null() {
                            GdipImageRotateFlip(pen.image.cast(), Rotate180FlipNone);
                        }
                    }
                }
            }
            SetWindowLongPtrW(w, GWLP_USERDATA, Box::into_raw(pen) as isize);
            return 0;
        }
        if msg == WM_LBUTTONUP && (GetDlgCtrlID(w) == 216 || GetDlgCtrlID(w) == 217) {
            PostMessageW(
                GetParent(w),
                WM_COMMAND,
                GetDlgCtrlID(w) as usize,
                w as isize,
            );
            return 0;
        }
        let ptr = GetWindowLongPtrW(w, GWLP_USERDATA) as *mut PenImage;
        if msg == WM_NCDESTROY && !ptr.is_null() {
            SetWindowLongPtrW(w, GWLP_USERDATA, 0);
            drop(Box::from_raw(ptr));
            return DefWindowProcW(w, msg, wp, lp);
        }
        if msg != WM_PAINT && msg != WM_PRINTCLIENT {
            return DefWindowProcW(w, msg, wp, lp);
        }
        let mut p = PAINTSTRUCT::default();
        let dc = if msg == WM_PAINT {
            BeginPaint(w, &mut p)
        } else {
            wp as HDC
        };
        let mut r = RECT::default();
        GetClientRect(w, &mut r);
        fill(dc, r, 0xffffff);
        if !ptr.is_null() && !(*ptr).image.is_null() {
            let mut graphics = null_mut();
            if GdipCreateFromHDC(dc, &mut graphics) == 0 {
                GdipSetInterpolationMode(graphics, InterpolationModeHighQualityBicubic);
                // Render the supplied PNG at native monitor resolution, preserving aspect ratio.
                // Tablet variants use a real 180-degree rotation, matching the input mapping.
                let tablet = GetDlgCtrlID(w) == 216 || GetDlgCtrlID(w) == 217;
                let mut image_width = 0;
                let mut image_height = 0;
                GdipGetImageWidth((*ptr).image.cast(), &mut image_width);
                GdipGetImageHeight((*ptr).image.cast(), &mut image_height);
                let (sx, sy, sw, sh) = if tablet {
                    (0, 0, image_width as i32, image_height as i32)
                } else {
                    // Crop side margins only at presentation time; the source file is untouched.
                    (
                        image_width as i32 / 3,
                        0,
                        image_width as i32 / 3,
                        image_height as i32,
                    )
                };
                let factor =
                    (r.right as f64 / sw.max(1) as f64).min(r.bottom as f64 / sh.max(1) as f64);
                let width = (sw as f64 * factor).round() as i32;
                let height = (sh as f64 * factor).round() as i32;
                GdipDrawImageRectRectI(
                    graphics,
                    (*ptr).image.cast(),
                    (r.right - width) / 2,
                    (r.bottom - height) / 2,
                    width,
                    height,
                    sx,
                    sy,
                    sw,
                    sh,
                    UnitPixel,
                    null(),
                    0,
                    null_mut(),
                );
                GdipDeleteGraphics(graphics);
            }
        }
        if msg == WM_PAINT {
            EndPaint(w, &p);
        }
        0
    }
}

/// Paints the native sunken option borders and the reference-style filled distance ramp.
unsafe extern "system" fn decoration_proc(w: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_NCHITTEST {
            return HTTRANSPARENT as isize;
        }
        // Panels supply only a border. Their parent paints the white background;
        // erasing the interior can cover sibling controls during WM_PRINT previews.
        let panel = GetDlgCtrlID(w) >= 300;
        if panel && msg == WM_ERASEBKGND {
            return 1;
        }
        if msg != WM_PAINT && msg != WM_PRINTCLIENT {
            return DefWindowProcW(w, msg, wp, lp);
        }
        let mut ps = PAINTSTRUCT::default();
        let dc = if msg == WM_PAINT {
            BeginPaint(w, &mut ps)
        } else {
            wp as HDC
        };
        let mut r = RECT::default();
        GetClientRect(w, &mut r);
        if !panel {
            fill(dc, r, 0xffffff);
        }
        if GetDlgCtrlID(w) == 220 {
            DrawEdge(dc, &mut r, EDGE_SUNKEN, BF_TOP);
        } else if GetDlgCtrlID(w) >= 300 {
            DrawEdge(dc, &mut r, EDGE_SUNKEN, BF_RECT);
        } else {
            let old_brush = SelectObject(dc, GetStockObject(BLACK_BRUSH));
            let old_pen = SelectObject(dc, GetStockObject(NULL_PEN));
            let points = [
                POINT {
                    x: 0,
                    y: r.bottom - 1,
                },
                POINT { x: r.right, y: 0 },
                POINT {
                    x: r.right,
                    y: r.bottom,
                },
            ];
            Polygon(dc, points.as_ptr(), 3);
            SelectObject(dc, old_pen);
            SelectObject(dc, old_brush);
        }
        if msg == WM_PAINT {
            EndPaint(w, &ps);
        }
        0
    }
}
