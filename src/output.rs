//! Sends one processed stream simultaneously to the selected Windows pen backend and WinTab IPC.
//! The VHF path is a real HID device; synthetic Ink remains a fallback for unsigned development.
use crate::windows::Ink;
use ctl460_rust::{
    config::Config,
    double_click::DoubleClick,
    engine::Frame,
    hid,
    ipc::{Mapping, PenData},
    stroke::map_to_virtual_screen,
};
use windows_sys::Win32::{
    System::{SystemInformation::GetTickCount64, WindowsProgramming::QueryInterruptTimePrecise},
    UI::{Input::KeyboardAndMouse::GetDoubleClickTime, WindowsAndMessaging::*},
};

// The unprivileged feeder sends fixed reports to the installed service; only the service opens VHF.
struct Hid {
    client: ctl460_rust::broker::Client,
    aspect: bool,
    width: i32,
    height: i32,
}
impl Hid {
    fn new(aspect: bool) -> Result<Self, String> {
        Ok(Self {
            client: ctl460_rust::broker::Client::connect()?,
            aspect,
            width: unsafe { GetSystemMetrics(SM_CXSCREEN) },
            height: unsafe { GetSystemMetrics(SM_CYSCREEN) },
        })
    }
    fn submit(&mut self, mut frame: Frame, position: Option<(i32, i32)>) -> Result<(), String> {
        (frame.x, frame.y) =
            map_to_virtual_screen(frame.x, frame.y, self.width, self.height, self.aspect);
        if let Some((x, y)) = position {
            // The HID logical range spans the virtual desktop. Preserve negative
            // monitor origins until the final normalized HID conversion.
            let (left, top, width, height) = unsafe {
                (
                    GetSystemMetrics(SM_XVIRTUALSCREEN),
                    GetSystemMetrics(SM_YVIRTUALSCREEN),
                    GetSystemMetrics(SM_CXVIRTUALSCREEN),
                    GetSystemMetrics(SM_CYVIRTUALSCREEN),
                )
            };
            frame.x = ((f64::from(x - left) / f64::from((width - 1).max(1))) * 14720.0)
                .round()
                .clamp(0.0, 14720.0) as u16;
            frame.y = ((f64::from(y - top) / f64::from((height - 1).max(1))) * 9200.0)
                .round()
                .clamp(0.0, 9200.0) as u16;
        }
        self.client.submit(&mut hid::encode(frame))
    }
}
impl Drop for Hid {
    fn drop(&mut self) {
        let _ = self.submit(Frame::default(), None);
    }
}
enum WindowsPen {
    Ink(Ink),
    Hid(Hid),
}
pub struct Output {
    pen: WindowsPen,
    // Keep the service handoff while switching to Ink, so Wacom does not reclaim input.
    hid_lease: Option<Hid>,
    stream: Mapping,
    backend: u32,
    hz: u32,
    handwriting_mode_flags: i32,
    raw: i32,
    latency: u32,
    received: Option<std::time::Instant>,
    shape_epoch: std::time::Instant,
    width: i32,
    height: i32,
    aspect: bool,
    taps: DoubleClick,
    distance: u32,
    previous: Frame,
    precision: ctl460_rust::precision::PrecisionHold,
    shape: ctl460_rust::shape_assist::ShapeAssist,
    shape_config: Config,
    marker: Option<ctl460_rust::start_marker::StartMarker>,
    marker_point: Option<(i32, i32)>,
    marker_foreground: isize,
    system_cursor: wintab32::system_output::SystemCursor,
    system_buttons: wintab32::system_output::SystemButtons,
    input_route: wintab32::system_output::InputRoute,
}
impl Output {
    pub fn new(c: &Config) -> Result<Self, String> {
        unsafe {
            use windows_sys::Win32::UI::HiDpi::*;
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
        let stream = Mapping::create_writer()?;
        let (pen, backend) = if c.backend == "hid" {
            (WindowsPen::Hid(Hid::new(c.preserve_aspect)?), 1)
        } else {
            (WindowsPen::Ink(Ink::new(c.preserve_aspect)?), 2)
        };
        Ok(Self {
            pen,
            hid_lease: None,
            stream,
            backend,
            hz: c.output_hz,
            handwriting_mode_flags: PenData::handwriting_mode_flags(&c.handwriting_mode),
            raw: 0,
            latency: 0,
            received: None,
            shape_epoch: std::time::Instant::now(),
            width: unsafe { GetSystemMetrics(SM_CXSCREEN) },
            height: unsafe { GetSystemMetrics(SM_CYSCREEN) },
            aspect: c.preserve_aspect,
            taps: DoubleClick::default(),
            distance: c.double_click_distance,
            previous: Frame::default(),
            precision: Default::default(),
            shape: Default::default(),
            shape_config: c.clone(),
            marker: if c.show_start_marker {
                Some(ctl460_rust::start_marker::StartMarker::new()?)
            } else {
                None
            },
            marker_point: None,
            marker_foreground: 0,
            system_cursor: Default::default(),
            system_buttons: Default::default(),
            input_route: Default::default(),
        })
    }
    /// Reuse the current device for tuning; switch output APIs only while lifted.
    /// Build a replacement first so a failed switch leaves the existing pen usable.
    pub fn reconfigure(&mut self, c: &Config) -> Result<(), String> {
        c.validate()?;
        if self.previous.contact {
            return Err("Lift the pen before changing output".into());
        }
        // Allocate an optional overlay before changing any live output state.
        let new_marker = if c.show_start_marker && self.marker.is_none() {
            Some(ctl460_rust::start_marker::StartMarker::new()?)
        } else {
            None
        };
        let backend = if c.backend == "hid" { 1 } else { 2 };
        if self.backend != backend {
            let replacement = if backend == 1 {
                WindowsPen::Hid(match self.hid_lease.take() {
                    Some(mut lease) => {
                        lease.aspect = c.preserve_aspect;
                        lease
                    }
                    None => Hid::new(c.preserve_aspect)?,
                })
            } else {
                WindowsPen::Ink(Ink::new(c.preserve_aspect)?)
            };
            self.send(Frame {
                in_range: false,
                contact: false,
                pressure: 0.0,
                barrel: false,
                ..self.previous
            })?;
            let previous = std::mem::replace(&mut self.pen, replacement);
            if let WindowsPen::Hid(lease) = previous {
                self.hid_lease = Some(lease);
            }
            self.backend = backend;
        }
        match &mut self.pen {
            WindowsPen::Ink(p) => p.set_aspect(c.preserve_aspect),
            WindowsPen::Hid(p) => p.aspect = c.preserve_aspect,
        }
        self.aspect = c.preserve_aspect;
        self.hz = c.output_hz;
        self.handwriting_mode_flags = PenData::handwriting_mode_flags(&c.handwriting_mode);
        self.distance = c.double_click_distance;
        self.taps = DoubleClick::default();
        self.precision.reconfigure(c);
        self.shape.reset();
        self.shape_config = c.clone();
        self.marker_point = None;
        if let Some(marker) = &self.marker {
            marker.show(None, 0);
        }
        if let Some(marker) = new_marker {
            self.marker = Some(marker);
        } else if !c.show_start_marker {
            self.marker = None;
        }
        Ok(())
    }

    pub fn telemetry(&mut self, raw: u16, received: std::time::Instant) {
        self.raw = i32::from(raw);
        self.received = Some(received);
    }
    pub fn waiting(&self) {
        self.stream.set_metadata(self.backend, self.hz, 0);
        self.stream.heartbeat();
    }
    pub fn set_precision(&mut self, config: &Config, states: [bool; 2]) {
        self.precision.buttons(config, states);
    }
    pub fn submit(&mut self, frame: Frame) -> Result<(), String> {
        let was_precision = self.precision.active();
        let frame = self.precision.apply(
            frame,
            ctl460_rust::precision::output_bounds(self.width, self.height, self.aspect),
        );
        let precision = was_precision || self.precision.active();
        if precision {
            self.taps = DoubleClick::default();
        }
        let screen = ctl460_rust::stroke::map_to_screen(
            frame.x,
            frame.y,
            self.width,
            self.height,
            self.aspect,
        );
        let frame = self.taps.apply(
            frame,
            screen,
            unsafe { GetTickCount64() },
            if precision { 0 } else { self.distance },
            unsafe { GetDoubleClickTime() },
        );
        let measured_ms = self.received.map_or(0.0, |t| {
            t.saturating_duration_since(self.shape_epoch).as_secs_f64() * 1000.0
        });
        let frame = self.shape.apply(frame, &self.shape_config, measured_ms);
        // All backends see the old tool lift and leave before the eraser/pen replaces it.
        // This also prevents a held eraser button from becoming an ordinary drawing stroke.
        if self.previous.in_range && frame.in_range && self.previous.eraser != frame.eraser {
            if self.previous.contact {
                self.send(Frame {
                    contact: false,
                    pressure: 0.0,
                    barrel: false,
                    ..self.previous
                })?;
            }
            self.send(Frame {
                in_range: false,
                contact: false,
                pressure: 0.0,
                barrel: false,
                ..self.previous
            })?;
        }
        // HID and WinTab need the same stationary lift as Ink. A raw hover sample
        // can jump away from the smoothed stroke endpoint and must follow the lift.
        if let Some(up) = ctl460_rust::lifecycle::lift_before_hover(self.previous, frame) {
            self.send(up)?;
        }
        self.send(frame)
    }
    fn send(&mut self, frame: Frame) -> Result<(), String> {
        let (mapped_x, mapped_y) =
            map_to_virtual_screen(frame.x, frame.y, self.width, self.height, self.aspect);
        let mut cursor = windows_sys::Win32::Foundation::POINT::default();
        let desktop = unsafe {
            GetCursorPos(&mut cursor);
            [
                GetSystemMetrics(SM_XVIRTUALSCREEN),
                GetSystemMetrics(SM_YVIRTUALSCREEN),
                GetSystemMetrics(SM_CXVIRTUALSCREEN),
                GetSystemMetrics(SM_CYVIRTUALSCREEN),
            ]
        };
        let position = self.system_cursor.map(
            i32::from(mapped_x),
            i32::from(mapped_y),
            frame.contact,
            desktop,
            (cursor.x, cursor.y),
        );
        let marker_eligible =
            frame.in_range && frame.contact && !frame.handwriting && !frame.eraser && !frame.barrel;
        if marker_eligible {
            if self.marker_point.is_none() {
                self.marker_point = Some(position.unwrap_or_else(|| {
                    if self.backend == 2 {
                        ctl460_rust::stroke::map_to_screen(
                            frame.x,
                            frame.y,
                            self.width,
                            self.height,
                            self.aspect,
                        )
                    } else {
                        (
                            desktop[0]
                                + (f64::from(mapped_x) / 14720.0
                                    * f64::from((desktop[2] - 1).max(1)))
                                .round() as i32,
                            desktop[1]
                                + (f64::from(mapped_y) / 9200.0
                                    * f64::from((desktop[3] - 1).max(1)))
                                .round() as i32,
                        )
                    }
                }));
                self.marker_foreground = unsafe { GetForegroundWindow() } as isize;
            }
        } else {
            self.marker_point = None;
        }
        if let Some(marker) = &self.marker {
            marker.show(self.marker_point, self.marker_foreground);
        }
        let mouse_route = self.input_route.update(
            wintab32::system_output::foreground_requests_system_pen(),
            frame.contact,
        );
        let custom_buttons = self.system_buttons.prepare_with_route(
            u32::from(frame.contact) | (u32::from(frame.barrel) << 1),
            frame.eraser,
            frame.in_range,
            mouse_route,
        );
        let windows_frame = if mouse_route {
            // End Windows pen proximity while the foreground app explicitly uses
            // WinTab. Its pressure remains in the independent shared stream below.
            Frame {
                in_range: false,
                contact: false,
                barrel: false,
                pressure: 0.0,
                ..frame
            }
        } else if custom_buttons {
            Frame {
                contact: false,
                barrel: false,
                ..frame
            }
        } else {
            frame
        };
        let flags = i32::from(frame.contact)
            | (i32::from(frame.barrel) << 1)
            | (i32::from(frame.eraser) << 2)
            | (i32::from(frame.in_range) << 3)
            | (i32::from(frame.handwriting) << 5)
            | (i32::from(frame.handwriting_score) << 8)
            | self.handwriting_mode_flags;
        let (x, y) = map_to_virtual_screen(frame.x, frame.y, self.width, self.height, self.aspect);
        // Preserve report cadence in WinTab's millisecond field. GetTickCount64 can
        // repeat for 10-16 ms, making client pressure filters see zero then large dt.
        let mut precise_time = 0u64;
        unsafe {
            QueryInterruptTimePrecise(&mut precise_time);
        }
        self.stream.publish(PenData {
            time: (precise_time / 10_000) as u32,
            x: i32::from(x),
            y: i32::from(y),
            pressure: i32::from(frame.virtual_pressure()),
            flags,
            tilt_x: 0,
            tilt_y: 0,
            raw: self.raw,
        });
        // Make tablet data available before dispatching the corresponding system
        // input, so consumers awakened by a mouse event can read its pressure.
        match &mut self.pen {
            WindowsPen::Ink(p) => p.submit_at(windows_frame, position)?,
            WindowsPen::Hid(p) => p.submit(windows_frame, position)?,
        }
        if mouse_route && frame.in_range {
            let position = position.unwrap_or_else(|| {
                (
                    desktop[0]
                        + (i64::from(mapped_x) * i64::from((desktop[2] - 1).max(0)) / 14720) as i32,
                    desktop[1]
                        + (i64::from(mapped_y) * i64::from((desktop[3] - 1).max(0)) / 9200) as i32,
                )
            });
            wintab32::system_output::move_system_mouse(position, desktop)?;
        }
        self.system_buttons.flush()?;
        if let Some(received) = self.received.take() {
            self.latency = received.elapsed().as_micros().min(u128::from(u32::MAX)) as u32;
        }
        self.stream
            .set_metadata(self.backend, self.hz, self.latency);
        self.previous = frame;
        Ok(())
    }
}
