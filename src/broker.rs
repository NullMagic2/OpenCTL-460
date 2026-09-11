//! Bounded local pipe protocol for the installed Virtual HID service.
//! Clients send only validated pen reports; no paths, commands, or handles cross this boundary.
use std::{
    io,
    ptr::{null, null_mut},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Storage::FileSystem::*,
    System::{Threading::*, IO::*},
};
pub const PIPE: &str = r"\\.\pipe\OpenCTL460Pen-v1";
pub const HELLO: [u8; 8] = *b"CTL460\x01\x00";
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub struct Handle(pub HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

/// Cancel and drain a pending operation before its stack buffer/event can be destroyed.
pub fn finish(
    handle: &Handle,
    operation: &mut OVERLAPPED,
    timeout: Duration,
    cancelled: impl Fn() -> bool,
) -> io::Result<usize> {
    let start = Instant::now();
    loop {
        if cancelled() || start.elapsed() >= timeout {
            unsafe {
                CancelIoEx(handle.0, operation);
                let mut count = 0;
                GetOverlappedResult(handle.0, operation, &mut count, 1);
            }
            return Err(io::Error::from_raw_os_error(ERROR_OPERATION_ABORTED as i32));
        }
        match unsafe { WaitForSingleObject(operation.hEvent, 50) } {
            WAIT_OBJECT_0 => {
                let mut count = 0;
                return if unsafe { GetOverlappedResult(handle.0, operation, &mut count, 0) } != 0 {
                    Ok(count as usize)
                } else {
                    Err(io::Error::last_os_error())
                };
            }
            WAIT_TIMEOUT => (),
            _ => {
                let error = io::Error::last_os_error();
                unsafe {
                    CancelIoEx(handle.0, operation);
                    let mut count = 0;
                    GetOverlappedResult(handle.0, operation, &mut count, 1);
                }
                return Err(error);
            }
        }
    }
}
pub fn transfer(
    handle: &Handle,
    bytes: &mut [u8],
    write: bool,
    timeout: Duration,
    cancelled: impl Fn() -> bool,
) -> io::Result<usize> {
    let event = Handle(unsafe { CreateEventW(null(), 1, 0, null()) });
    if event.0.is_null() {
        return Err(io::Error::last_os_error());
    }
    let mut operation = OVERLAPPED {
        hEvent: event.0,
        ..Default::default()
    };
    let mut count = 0;
    let ok = unsafe {
        if write {
            WriteFile(
                handle.0,
                bytes.as_ptr(),
                bytes.len() as u32,
                &mut count,
                &mut operation,
            )
        } else {
            ReadFile(
                handle.0,
                bytes.as_mut_ptr(),
                bytes.len() as u32,
                &mut count,
                &mut operation,
            )
        }
    };
    if ok != 0 {
        return Ok(count as usize);
    }
    if unsafe { GetLastError() } != ERROR_IO_PENDING {
        return Err(io::Error::last_os_error());
    }
    finish(handle, &mut operation, timeout, cancelled)
}

pub struct Client(Handle);
impl Client {
    pub fn connect() -> Result<Self, String> {
        Self::connect_named(PIPE)
    }
    /// An explicit local endpoint allows isolated protocol tests without the installed service.
    pub fn connect_named(name: &str) -> Result<Self, String> {
        if !name.starts_with(r"\\.\pipe\") {
            return Err("The pen service must be local".into());
        }
        // Identification-only prevents an impostor pipe server from impersonating this user.
        let handle = unsafe {
            CreateFileW(
                wide(name).as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                null(),
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(format!("Cannot connect to the OpenCTL 460 service: {}. Run the latest installer to install or repair the service.", io::Error::last_os_error()));
        }
        let client = Self(Handle(handle));
        let mut hello = HELLO;
        client.exchange(&mut hello, Duration::from_secs(30))?;
        Ok(client)
    }
    fn exchange(&self, bytes: &mut [u8], timeout: Duration) -> Result<(), String> {
        let n = transfer(&self.0, bytes, true, timeout, || false).map_err(|e| e.to_string())?;
        if n != bytes.len() {
            return Err("Incomplete service request".into());
        }
        let mut reply = [0u8; 4];
        let n =
            transfer(&self.0, &mut reply, false, timeout, || false).map_err(|e| e.to_string())?;
        if n != reply.len() {
            return Err("Incomplete service response".into());
        }
        let error = u32::from_le_bytes(reply);
        if error == 0 {
            Ok(())
        } else {
            Err(format!("Virtual HID service: {}. Stop other tablet feeders; if the driver is missing, run setup to repair it.", io::Error::from_raw_os_error(error as i32)))
        }
    }
    pub fn submit(&self, bytes: &mut [u8]) -> Result<(), String> {
        if !crate::wire::valid_report(bytes) {
            return Err("Invalid virtual pen report".into());
        }
        self.exchange(bytes, Duration::from_secs(5))
    }
}
