//! Owns Windows synthetic-pen resources and translates tested lifecycle events.
//! Uses official windows-sys ABI definitions; every unsafe call documents its boundary.

use ctl460_rust::{
    engine::Frame,
    lifecycle::{self, Event, Kind},
    stroke::map_to_screen,
};
use std::{io, thread, time::Duration};
use windows_sys::Win32::{
    Foundation::{ERROR_NOT_READY, POINT},
    UI::{
        Controls::{
            CreateSyntheticPointerDevice, DestroySyntheticPointerDevice, HSYNTHETICPOINTERDEVICE,
            POINTER_FEEDBACK_NONE, POINTER_TYPE_INFO, POINTER_TYPE_INFO_0,
        },
        HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2},
        Input::Pointer::*,
        WindowsAndMessaging::*,
    },
};

pub struct Ink {
    handle: HSYNTHETICPOINTERDEVICE,
    previous: Frame,
    pointer_id: u32,
    width: i32,
    height: i32,
    preserve_aspect: bool,
    position: Option<(i32, i32)>,
}

impl Ink {
    pub fn new(preserve_aspect: bool) -> Result<Self, String> {
        // SAFETY: Predefined DPI context, no pointers borrowed. Run before screen queries.
        unsafe {
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
        // SAFETY: PT_PEN requires exactly one pointer. The returned handle is owned by Ink.
        let handle = unsafe { CreateSyntheticPointerDevice(PT_PEN, 1, POINTER_FEEDBACK_NONE) };
        if handle.is_null() {
            return Err(format!(
                "Cannot create Windows Ink pen: {}",
                io::Error::last_os_error()
            ));
        }
        // SAFETY: Read-only metrics, primary display physical pixels under per-monitor DPI.
        let (width, height) =
            unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };
        if width < 2 || height < 2 {
            // SAFETY: Close the successfully created, still-owned handle on initialization failure.
            unsafe {
                DestroySyntheticPointerDevice(handle);
            }
            return Err("Primary display dimensions unavailable".into());
        }
        Ok(Self {
            handle,
            previous: Frame::default(),
            pointer_id: 0,
            width,
            height,
            preserve_aspect,
            position: None,
        })
    }

    pub fn set_aspect(&mut self, preserve: bool) {
        self.preserve_aspect = preserve;
    }

    fn send(&mut self, event: Event) -> Result<(), String> {
        let frame = event.frame;
        if event.new_pointer {
            self.pointer_id = self.pointer_id.wrapping_add(1).max(1);
        }
        let mut flags = POINTER_FLAG_PRIMARY | POINTER_FLAG_CONFIDENCE;
        flags |= match event.kind {
            Kind::Down => POINTER_FLAG_DOWN,
            Kind::Update | Kind::Exit => POINTER_FLAG_UPDATE,
            Kind::Up => POINTER_FLAG_UP,
        };
        if event.new_pointer {
            flags |= POINTER_FLAG_NEW;
        }
        if frame.in_range {
            flags |= POINTER_FLAG_INRANGE;
        }
        if frame.contact {
            flags |= POINTER_FLAG_INCONTACT | POINTER_FLAG_FIRSTBUTTON;
        }
        if frame.barrel {
            flags |= POINTER_FLAG_SECONDBUTTON;
        }
        let (x, y) = map_to_screen(
            frame.x,
            frame.y,
            self.width,
            self.height,
            self.preserve_aspect,
        );
        let (x, y) = self.position.unwrap_or((x, y));
        let point = POINT { x, y };
        let mut pen_flags = 0;
        if frame.barrel {
            pen_flags |= PEN_FLAG_BARREL;
        }
        if frame.eraser {
            pen_flags |= PEN_FLAG_INVERTED | PEN_FLAG_ERASER;
        }
        let pen = POINTER_PEN_INFO {
            pointerInfo: POINTER_INFO {
                pointerType: PT_PEN,
                pointerId: self.pointer_id,
                pointerFlags: flags,
                ptPixelLocation: point,
                ptPixelLocationRaw: point,
                ..Default::default()
            },
            penFlags: pen_flags,
            penMask: PEN_MASK_PRESSURE
                | if frame.virtual_tilt {
                    PEN_MASK_TILT_X | PEN_MASK_TILT_Y
                } else {
                    0
                },
            tiltX: frame.tilt_x.clamp(-60, 60),
            tiltY: frame.tilt_y.clamp(-60, 60),
            pressure: if frame.contact {
                frame.ink_pressure()
            } else {
                0
            },
            ..Default::default()
        };
        let info = POINTER_TYPE_INFO {
            r#type: PT_PEN,
            Anonymous: POINTER_TYPE_INFO_0 { penInfo: pen },
        };
        // Short consecutive lifecycle events may share Windows' timestamp quantum.
        // Retry only NOT_READY, using the identical event, and bound the retry count.
        for attempt in 0..3 {
            // SAFETY: Handle is live, pointer references one initialized PT_PEN union for this call.
            if unsafe { InjectSyntheticPointerInput(self.handle, &info, 1) } != 0 {
                self.previous = frame;
                return Ok(());
            }
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_NOT_READY as i32) && attempt < 2 {
                thread::sleep(Duration::from_millis(1));
            } else {
                return Err(format!("Windows pen injection failed: {error}"));
            }
        }
        unreachable!()
    }

    pub fn submit_at(&mut self, frame: Frame, position: Option<(i32, i32)>) -> Result<(), String> {
        self.position = position;
        self.submit(frame)
    }
    pub fn submit(&mut self, frame: Frame) -> Result<(), String> {
        for event in lifecycle::transition(self.previous, frame) {
            self.send(event)?;
        }
        Ok(())
    }
}

impl Drop for Ink {
    fn drop(&mut self) {
        // Normal exit/error paths attempt an explicit lift before destroying the device.
        if let Err(error) = self.submit(Frame::default()) {
            eprintln!("Release: {error}");
        }
        // SAFETY: Exactly one destruction, after the last use of this owned handle.
        unsafe {
            DestroySyntheticPointerDevice(self.handle);
        }
    }
}
