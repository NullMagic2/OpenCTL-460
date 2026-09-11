//! LocalSystem broker for one active-console pen stream, installed once by setup.
//! It accepts fixed pen packets only and restores Wacom services when the client disconnects.
#[cfg(not(windows))]
fn main() {
    eprintln!("Windows only");
}
#[cfg(windows)]
mod app {
    use ctl460_rust::{
        broker::{self, wide, Handle},
        wire,
    };
    use std::{
        io,
        mem::size_of,
        ptr::{null, null_mut},
        sync::atomic::{AtomicBool, AtomicPtr, Ordering},
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::*,
        Security::{Authorization::*, SECURITY_ATTRIBUTES},
        Storage::FileSystem::*,
        System::{Pipes::*, RemoteDesktop::*, Services::*, Threading::*, IO::*},
    };
    const NAME: &str = "OpenCTL460Broker";
    static STOP: AtomicBool = AtomicBool::new(false);
    static STATUS: AtomicPtr<core::ffi::c_void> = AtomicPtr::new(null_mut());
    fn stopped() -> bool {
        STOP.load(Ordering::Acquire)
    }
    fn error() -> u32 {
        unsafe { GetLastError() }
    }
    fn log_error(message: &str) {
        use windows_sys::Win32::System::EventLog::*;
        // Windows Application event log avoids writable user paths in a privileged process.
        unsafe {
            let source = RegisterEventSourceW(null(), wide(NAME).as_ptr());
            if !source.is_null() {
                let text = wide(message);
                let strings = [text.as_ptr()];
                ReportEventW(
                    source,
                    EVENTLOG_ERROR_TYPE,
                    0,
                    1,
                    null_mut(),
                    1,
                    0,
                    strings.as_ptr(),
                    null(),
                );
                DeregisterEventSource(source);
            }
        }
    }
    fn status(state: u32, code: u32) {
        let value = SERVICE_STATUS {
            dwServiceType: SERVICE_WIN32_OWN_PROCESS,
            dwCurrentState: state,
            dwControlsAccepted: if state == SERVICE_RUNNING {
                SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN
            } else {
                0
            },
            dwWin32ExitCode: code,
            dwWaitHint: if state == SERVICE_STOP_PENDING {
                30000
            } else {
                0
            },
            ..Default::default()
        };
        unsafe {
            SetServiceStatus(STATUS.load(Ordering::Acquire), &value);
        }
    }
    unsafe extern "system" fn control(
        code: u32,
        _: u32,
        _: *mut core::ffi::c_void,
        _: *mut core::ffi::c_void,
    ) -> u32 {
        if code == SERVICE_CONTROL_STOP || code == SERVICE_CONTROL_SHUTDOWN {
            STOP.store(true, Ordering::Release);
            status(SERVICE_STOP_PENDING, 0);
        }
        NO_ERROR
    }
    struct Sc(SC_HANDLE);
    impl Drop for Sc {
        fn drop(&mut self) {
            unsafe {
                CloseServiceHandle(self.0);
            }
        }
    }
    struct Handoff(Vec<Sc>);
    impl Handoff {
        fn pause() -> Result<Self, u32> {
            let mut result = Self(Vec::new());
            let manager = Sc(unsafe { OpenSCManagerW(null(), null(), SC_MANAGER_CONNECT) });
            if manager.0.is_null() {
                return Err(error());
            }
            for name in ["TouchServicePen", "TabletServicePen"] {
                let service = Sc(unsafe {
                    OpenServiceW(
                        manager.0,
                        wide(name).as_ptr(),
                        SERVICE_QUERY_STATUS | SERVICE_STOP | SERVICE_START,
                    )
                });
                if service.0.is_null() {
                    if error() == ERROR_SERVICE_DOES_NOT_EXIST {
                        continue;
                    }
                    return Err(error());
                }
                let mut value = SERVICE_STATUS::default();
                if unsafe { QueryServiceStatus(service.0, &mut value) } == 0 {
                    return Err(error());
                }
                if value.dwCurrentState != SERVICE_RUNNING {
                    continue;
                }
                // Retain responsibility before waiting: the stop may succeed just before a timeout.
                result.0.push(service);
                let service = result.0.last().unwrap();
                if unsafe { ControlService(service.0, SERVICE_CONTROL_STOP, &mut value) } == 0 {
                    return Err(error());
                }
                let deadline = Instant::now() + Duration::from_secs(10);
                loop {
                    if unsafe { QueryServiceStatus(service.0, &mut value) } == 0 {
                        return Err(error());
                    }
                    if value.dwCurrentState == SERVICE_STOPPED {
                        break;
                    }
                    if stopped() || Instant::now() > deadline {
                        return Err(ERROR_TIMEOUT);
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
            Ok(result)
        }
    }
    impl Drop for Handoff {
        fn drop(&mut self) {
            for service in self.0.iter().rev() {
                // A handle remains bound to this exact service; clients cannot choose service names.
                let deadline = Instant::now() + Duration::from_secs(10);
                let mut value = SERVICE_STATUS::default();
                loop {
                    if unsafe { QueryServiceStatus(service.0, &mut value) } == 0 {
                        log_error("Could not query a Wacom service during restoration.");
                        break;
                    }
                    if value.dwCurrentState == SERVICE_RUNNING
                        || value.dwCurrentState == SERVICE_START_PENDING
                    {
                        break;
                    }
                    if value.dwCurrentState == SERVICE_STOPPED {
                        if unsafe { StartServiceW(service.0, 0, null()) } == 0 {
                            log_error(&format!("Could not restore a previously running Wacom service: Windows error {}", error()));
                        }
                        break;
                    }
                    if Instant::now() >= deadline {
                        log_error("A Wacom service did not finish stopping in time to restore it; restart it in Windows Services.");
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }
    }
    fn reply(pipe: &Handle, code: u32) -> io::Result<()> {
        let mut bytes = code.to_le_bytes();
        broker::transfer(pipe, &mut bytes, true, Duration::from_secs(5), stopped).map(|_| ())
    }
    fn authorized(pipe: &Handle) -> bool {
        let mut session = 0;
        unsafe {
            GetNamedPipeClientSessionId(pipe.0, &mut session) != 0
                && session != 0
                && session == WTSGetActiveConsoleSessionId()
        }
    }
    fn submit(device: &Handle, report: &[u8]) -> u32 {
        if !wire::valid_report(report) {
            return ERROR_INVALID_DATA;
        }
        let mut count = 0;
        if unsafe {
            DeviceIoControl(
                device.0,
                wire::IOCTL_SUBMIT,
                report.as_ptr().cast(),
                report.len() as u32,
                null_mut(),
                0,
                &mut count,
                null_mut(),
            )
        } == 0
        {
            error()
        } else {
            0
        }
    }
    fn session(pipe: &Handle) -> io::Result<()> {
        let mut hello = [0u8; 8];
        let n = broker::transfer(pipe, &mut hello, false, Duration::from_secs(5), stopped)?;
        if n != hello.len() || hello != broker::HELLO || !authorized(pipe) {
            return reply(pipe, ERROR_ACCESS_DENIED);
        }
        let device = Handle(unsafe {
            CreateFileW(
                wide(r"\\.\CTL460VirtualPen").as_ptr(),
                GENERIC_WRITE,
                0,
                null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                null_mut(),
            )
        });
        if device.0 == INVALID_HANDLE_VALUE {
            return reply(pipe, error());
        }
        let handoff = match Handoff::pause() {
            Ok(h) => h,
            Err(code) => return reply(pipe, code),
        };
        let mut last = [0u8; wire::REPORT_LEN];
        last[0] = 1;
        let result = (|| {
            reply(pipe, 0)?;
            loop {
                let mut bytes = [0u8; wire::REPORT_LEN];
                let n = broker::transfer(pipe, &mut bytes, false, Duration::MAX, || {
                    stopped() || !authorized(pipe)
                })?;
                if n != bytes.len() {
                    reply(pipe, ERROR_INVALID_DATA)?;
                    break;
                }
                let code = if authorized(pipe) {
                    submit(&device, &bytes)
                } else {
                    ERROR_ACCESS_DENIED
                };
                reply(pipe, code)?;
                if code != 0 {
                    break;
                }
                last = bytes;
            }
            Ok(())
        })();
        // Lift and close VHF before the original Wacom services resume.
        let mut lift = last;
        lift[1] = 0;
        lift[6..].fill(0);
        let _ = submit(&device, &lift);
        drop(device);
        drop(handoff);
        result
    }
    fn run() -> io::Result<()> {
        // Interactive users may send pen data; remote and anonymous clients cannot connect.
        let mut descriptor = null_mut();
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide("D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)").as_ptr(),
                1,
                &mut descriptor,
                null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let security = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let raw = unsafe {
            CreateNamedPipeW(
                wide(broker::PIPE).as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                64,
                64,
                0,
                &security,
            )
        };
        unsafe {
            LocalFree(descriptor);
        }
        if raw == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let pipe = Handle(raw);
        status(SERVICE_RUNNING, 0);
        while !stopped() {
            let event = Handle(unsafe { CreateEventW(null(), 1, 0, null()) });
            if event.0.is_null() {
                return Err(io::Error::last_os_error());
            }
            let mut operation = OVERLAPPED {
                hEvent: event.0,
                ..Default::default()
            };
            let connected = unsafe { ConnectNamedPipe(pipe.0, &mut operation) };
            if connected == 0 {
                match error() {
                    ERROR_PIPE_CONNECTED => (),
                    ERROR_IO_PENDING => {
                        broker::finish(&pipe, &mut operation, Duration::MAX, stopped)?;
                    }
                    _ => return Err(io::Error::last_os_error()),
                }
            }
            let _ = session(&pipe);
            unsafe {
                DisconnectNamedPipe(pipe.0);
            }
        }
        Ok(())
    }
    unsafe extern "system" fn service_main(_: u32, _: *mut *mut u16) {
        let handle =
            unsafe { RegisterServiceCtrlHandlerExW(wide(NAME).as_ptr(), Some(control), null()) };
        if handle.is_null() {
            return;
        }
        STATUS.store(handle, Ordering::Release);
        status(SERVICE_START_PENDING, 0);
        let result = run();
        status(
            SERVICE_STOPPED,
            if stopped() {
                0
            } else {
                result.err().and_then(|e| e.raw_os_error()).unwrap_or(0) as u32
            },
        );
    }
    pub fn main() {
        let mut name = wide(NAME);
        let table = [
            SERVICE_TABLE_ENTRYW {
                lpServiceName: name.as_mut_ptr(),
                lpServiceProc: Some(service_main),
            },
            SERVICE_TABLE_ENTRYW {
                lpServiceName: null_mut(),
                lpServiceProc: None,
            },
        ];
        if unsafe { StartServiceCtrlDispatcherW(table.as_ptr()) } == 0 {
            eprintln!(
                "Start from Windows Services: {}",
                io::Error::last_os_error()
            );
            std::process::exit(1);
        }
    }
}
#[cfg(windows)]
fn main() {
    app::main();
}
