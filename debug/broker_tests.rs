//! Isolated pipe-protocol tests; no real VHF device, service installation or pen injection.
#![cfg(windows)]
use ctl460_rust::broker::{self, Client, Handle};
use std::{ptr::null, time::Duration};
use windows_sys::Win32::{
    Foundation::*,
    Storage::FileSystem::*,
    System::{Pipes::*, Threading::*, IO::*},
};

#[test]
fn broker_exchanges_fixed_reports_and_drains_cancelled_reads() {
    let name = format!(r"\\.\pipe\OpenCTL460Test-{}", std::process::id());
    let server_name = name.clone();
    let (ready, wait) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let pipe = Handle(unsafe {
            CreateNamedPipeW(
                broker::wide(&server_name).as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                64,
                64,
                0,
                null(),
            )
        });
        assert_ne!(pipe.0, INVALID_HANDLE_VALUE);
        let event = Handle(unsafe { CreateEventW(null(), 1, 0, null()) });
        let mut operation = OVERLAPPED {
            hEvent: event.0,
            ..Default::default()
        };
        let connected = unsafe { ConnectNamedPipe(pipe.0, &mut operation) };
        let connect_error = unsafe { GetLastError() };
        ready.send(()).unwrap();
        if connected == 0 && connect_error != ERROR_PIPE_CONNECTED {
            assert_eq!(connect_error, ERROR_IO_PENDING);
            broker::finish(&pipe, &mut operation, Duration::from_secs(5), || false).unwrap();
        }
        let mut hello = [0u8; 8];
        assert_eq!(
            broker::transfer(&pipe, &mut hello, false, Duration::from_secs(5), || false).unwrap(),
            8
        );
        assert_eq!(hello, broker::HELLO);
        broker::transfer(&pipe, &mut [0u8; 4], true, Duration::from_secs(5), || false).unwrap();
        let mut report = [0u8; 10];
        assert_eq!(
            broker::transfer(&pipe, &mut report, false, Duration::from_secs(5), || false).unwrap(),
            10
        );
        assert!(ctl460_rust::wire::valid_report(&report));
        broker::transfer(&pipe, &mut [0u8; 4], true, Duration::from_secs(5), || false).unwrap();
        let error = broker::transfer(&pipe, &mut report, false, Duration::from_millis(20), || {
            false
        })
        .unwrap_err();
        assert_eq!(error.raw_os_error(), Some(ERROR_OPERATION_ABORTED as i32));
        unsafe {
            DisconnectNamedPipe(pipe.0);
        }
    });
    wait.recv_timeout(Duration::from_secs(5)).unwrap();
    let client = Client::connect_named(&name).unwrap();
    assert!(client.submit(&mut [0u8; 10]).is_err());
    let mut hover = [0u8; 10];
    hover[0] = 1;
    client.submit(&mut hover).unwrap();
    server.join().unwrap();
    assert!(client.submit(&mut hover).is_err());
    assert!(Client::connect_named(r"\\remote\pipe\fake").is_err());
}
