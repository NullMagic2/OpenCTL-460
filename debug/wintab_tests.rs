//! Public DLL ABI tests. Run only with debug/run_wintab_conformance.ps1.
//! The compile-time stream namespace is isolated from the real tablet feeder.
#![allow(non_snake_case)]
use std::{
    ffi::c_void,
    mem::{size_of, zeroed},
    ptr,
    thread::sleep,
    time::{Duration, Instant},
};
use windows_sys::Win32::{Foundation::*, System::LibraryLoader::*, UI::WindowsAndMessaging::*};
use wintab32::{
    ipc::{Mapping, PenData, NAME},
    LogContextA, LogContextW,
};
unsafe fn function<T: Copy>(module: HMODULE, name: &str) -> T {
    let bytes: Vec<_> = name.bytes().chain(Some(0)).collect();
    let pointer = unsafe { GetProcAddress(module, bytes.as_ptr()) }
        .unwrap_or_else(|| panic!("missing {name}"));
    assert_eq!(size_of::<T>(), size_of::<usize>());
    unsafe { std::mem::transmute_copy(&pointer) }
}
fn wait(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(4);
    while !ready() {
        assert!(
            Instant::now() < deadline,
            "DLL did not deliver the expected event"
        );
        sleep(Duration::from_millis(5));
    }
}
unsafe extern "system" fn enumerate(id: usize, data: isize) -> i32 {
    unsafe {
        (*(data as *mut Vec<usize>)).push(id);
    }
    1
}
unsafe extern "system" fn stop_enumeration(_: usize, _: isize) -> i32 {
    0
}
#[test]
#[ignore = "requires the isolated conformance runner; never writes to the live tablet stream"]
fn actual_dll_abi_contexts_managers_and_packets() {
    assert!(
        NAME.starts_with("Local\\CTL460Conformance-"),
        "refusing to write the live IPC stream"
    );
    unsafe {
        let key = if cfg!(target_arch = "x86") {
            "CTL460_TEST_DLL32"
        } else {
            "CTL460_TEST_DLL64"
        };
        let path: Vec<u16> = std::env::var(key)
            .expect("use the conformance runner")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let module = LoadLibraryW(path.as_ptr());
        assert!(!module.is_null(), "DLL load failed: {}", GetLastError());
        macro_rules! api {
            ($name:ident, $ty:ty) => {
                let $name: $ty = function(module, stringify!($name));
            };
        }
        api!(
            WTInfoA,
            unsafe extern "system" fn(u32, u32, *mut c_void) -> u32
        );
        api!(
            WTInfoW,
            unsafe extern "system" fn(u32, u32, *mut c_void) -> u32
        );
        api!(
            WTOpenA,
            unsafe extern "system" fn(HWND, *mut LogContextA, i32) -> usize
        );
        api!(
            WTOpenW,
            unsafe extern "system" fn(HWND, *mut LogContextW, i32) -> usize
        );
        api!(
            WTGetW,
            unsafe extern "system" fn(usize, *mut LogContextW) -> i32
        );
        api!(
            WTSetW,
            unsafe extern "system" fn(usize, *const LogContextW) -> i32
        );
        api!(WTClose, unsafe extern "system" fn(usize) -> i32);
        api!(WTEnable, unsafe extern "system" fn(usize, i32) -> i32);
        api!(WTOverlap, unsafe extern "system" fn(usize, i32) -> i32);
        api!(
            WTPacketsGet,
            unsafe extern "system" fn(usize, i32, *mut c_void) -> i32
        );
        api!(
            WTPacketsPeek,
            unsafe extern "system" fn(usize, i32, *mut c_void) -> i32
        );
        api!(
            WTPacket,
            unsafe extern "system" fn(usize, u32, *mut c_void) -> i32
        );
        api!(WTQueueSizeSet, unsafe extern "system" fn(usize, i32) -> i32);
        api!(WTQueueSizeGet, unsafe extern "system" fn(usize) -> i32);
        api!(
            WTQueuePacketsEx,
            unsafe extern "system" fn(usize, *mut u32, *mut u32) -> i32
        );
        api!(
            WTDataGet,
            unsafe extern "system" fn(usize, u32, u32, i32, *mut c_void, *mut i32) -> i32
        );
        api!(
            WTDataPeek,
            unsafe extern "system" fn(usize, u32, u32, i32, *mut c_void, *mut i32) -> i32
        );
        api!(WTSave, unsafe extern "system" fn(usize, *mut c_void) -> i32);
        api!(
            WTExtGet,
            unsafe extern "system" fn(usize, u32, *mut c_void) -> i32
        );
        api!(
            WTExtSet,
            unsafe extern "system" fn(usize, u32, *const c_void) -> i32
        );
        api!(
            WTMgrExt,
            unsafe extern "system" fn(usize, u32, *mut c_void) -> i32
        );
        api!(
            WTRestore,
            unsafe extern "system" fn(HWND, *const c_void, i32) -> usize
        );
        api!(WTMgrOpen, unsafe extern "system" fn(HWND, u32) -> usize);
        api!(WTMgrClose, unsafe extern "system" fn(usize) -> i32);
        api!(
            WTMgrContextEnum,
            unsafe extern "system" fn(
                usize,
                Option<unsafe extern "system" fn(usize, isize) -> i32>,
                isize,
            ) -> i32
        );
        api!(
            WTMgrDefContextEx,
            unsafe extern "system" fn(usize, u32, i32) -> usize
        );
        api!(
            WTMgrCsrEnable,
            unsafe extern "system" fn(usize, u32, i32) -> i32
        );
        api!(
            WTMgrCsrPressureResponse,
            unsafe extern "system" fn(usize, u32, *mut u32, *mut u32) -> i32
        );
        api!(WacomCleanup, unsafe extern "system" fn() -> i32);
        // Check every standard name AND ordinal, including 32-bit stdcall decoration.
        let declarations = include_str!("../wintab/build.rs");
        for line in declarations
            .lines()
            .filter(|l| l.trim().starts_with("(\"WT"))
        {
            let parts: Vec<_> = line
                .trim()
                .trim_matches(|c| c == '(' || c == ')' || c == ',')
                .split(',')
                .collect();
            let name = parts[0].trim().trim_matches('"');
            let ordinal: usize = parts[1].trim().parse().unwrap();
            let bytes: Vec<_> = name.bytes().chain(Some(0)).collect();
            assert_eq!(
                GetProcAddress(module, bytes.as_ptr()).unwrap() as usize,
                GetProcAddress(module, ordinal as *const u8).unwrap() as usize,
                "ordinal for {name}"
            );
        }
        let mut lc: LogContextA = zeroed();
        assert_eq!(WTInfoA(3, 0, (&mut lc as *mut LogContextA).cast()), 172);
        let mut wc: LogContextW = zeroed();
        assert_eq!(WTInfoW(3, 0, (&mut wc as *mut LogContextW).cast()), 212);
        let mut guard = [0xa5u8; 8192];
        assert!(WTInfoA(0, 0, guard.as_mut_ptr().cast()) >= 1024);
        assert!(guard.iter().all(|&v| v == 0xa5));
        let mut value = 0u32;
        WTInfoA(1, 10, (&mut value as *mut u32).cast());
        assert_eq!(value, 16);
        let mut axis = [0i32; 4];
        WTInfoA(100, 15, axis.as_mut_ptr().cast());
        assert_eq!(axis[1], 4097);
        let manager = WTMgrOpen(ptr::null_mut(), 0);
        assert_ne!(manager, 0);
        assert_eq!(WTInfoA(1, 9, (&mut value as *mut u32).cast()), 4);
        assert_eq!(value, 2);
        assert_eq!(WTInfoA(300, 2, (&mut value as *mut u32).cast()), 4);
        assert_eq!(value, 3);
        assert_eq!(WTInfoA(301, 2, (&mut value as *mut u32).cast()), 4);
        assert_eq!(value, 0);
        value = 1;
        assert_eq!(WTMgrExt(manager, 0, (&mut value as *mut u32).cast()), 1);
        value = 0;
        assert_eq!(WTInfoA(301, 6, (&mut value as *mut u32).cast()), 4);
        assert_eq!(value, 1);
        value = 0;
        assert_eq!(WTMgrExt(manager, 0, (&mut value as *mut u32).cast()), 1);
        assert_eq!(WTMgrExt(manager, 99, (&mut value as *mut u32).cast()), 0);
        let default = WTMgrDefContextEx(manager, u32::MAX, 0);
        assert_ne!(default, 0);
        assert_eq!(WTGetW(default, &mut wc), 1);
        assert_eq!(wc.words[4], u32::MAX);
        assert_eq!(
            WTSetW(default, &wc),
            0,
            "default manager context is read-only"
        );
        if let Ok(parent) = std::env::var("CTL460_CONFORMANCE_PARENT") {
            let parent: usize = parent.parse().unwrap();
            let mut ids = Vec::new();
            assert_eq!(
                WTMgrContextEnum(
                    manager,
                    Some(enumerate),
                    (&mut ids as *mut Vec<usize>) as isize
                ),
                1
            );
            assert!(ids.contains(&parent));
            assert_eq!(WTGetW(parent, &mut wc), 1);
            // A cross-architecture manager can change an unlocked field but not the locked area.
            wc.words[11] = 50;
            wc.words[17] = 123;
            assert_eq!(WTSetW(parent, &wc), 1);
            assert_eq!(WTGetW(parent, &mut wc), 1);
            assert_eq!(wc.words[11], 0);
            assert_eq!(wc.words[17], 123);
            let reader = Mapping::open_reader().unwrap();
            assert!(reader.active());
            assert_eq!(reader.read(reader.latest()).unwrap().pressure, 4097);
            assert_eq!(WTMgrClose(manager), 1);
            return;
        }
        // Lifecycle messages are mandatory even without CXO_MESSAGES.
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let window = CreateWindowExW(
            0,
            class.as_ptr(),
            class.as_ptr(),
            0,
            0,
            0,
            1,
            1,
            HWND_MESSAGE,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null(),
        );
        assert!(!window.is_null());
        lc.words[0] = 0;
        lc.words[3] = 0x6000;
        let notification = WTOpenA(window, &mut lc, 1);
        assert_ne!(notification, 0);
        assert_eq!(lc.words[1], 4);
        let mut message: MSG = zeroed();
        assert_ne!(
            PeekMessageW(&mut message, window, 0x6001, 0x6001, PM_REMOVE),
            0
        );
        assert_eq!((message.wParam, message.lParam), (notification, 4));
        assert_eq!(WTClose(notification), 1);
        assert_ne!(
            PeekMessageW(&mut message, window, 0x6002, 0x6002, PM_REMOVE),
            0
        );
        assert_eq!(message.lParam, 4);
        DestroyWindow(window);
        let writer = Mapping::create_writer().expect("isolated writer");
        writer.set_metadata(2, 250, 0);
        assert!(Mapping::create_writer().is_err());
        // Packet layout: serial, buttons, X, Y, pressure (five DWORDs on either ABI).
        lc.words[2] = 1;
        lc.words[6] = 0x5d0;
        lc.words[8] = 0x580;
        lc.words[7] = u32::MAX;
        let a = WTOpenA(ptr::null_mut(), &mut lc, 1);
        assert_ne!(a, 0);
        assert_eq!(lc.words[7], 0x5c0);
        assert_eq!(WTGetW(a, &mut wc), 1);
        wc.words[7] = 0;
        assert_eq!(WTSetW(a, &wc), 1);
        let mut packet = [0u32; 5];
        let sample = |time, x, pressure, flags| PenData {
            time,
            x,
            y: 2345,
            pressure,
            flags,
            raw: 1023,
            ..Default::default()
        };
        writer.publish(sample(1, 1234, 4097, 9));
        wait(|| WTPacketsPeek(a, 1, packet.as_mut_ptr().cast()) == 1);
        assert_eq!(packet, [1, 1, 1234, 6855, 4097]);
        if cfg!(target_arch = "x86_64") {
            let peer = std::env::var("CTL460_TEST_PEER32").unwrap();
            let result = std::process::Command::new(peer)
                .args(["--ignored", "--nocapture"])
                .env("CTL460_CONFORMANCE_PARENT", a.to_string())
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "32-bit manager failed: {} {}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(WTGetW(a, &mut wc), 1);
            assert_eq!(wc.words[17], 123);
            wc.words[17] = 0;
            assert_eq!(WTSetW(a, &wc), 1);
        }
        assert_eq!(WTQueueSizeSet(a, 32), 1);
        assert_eq!(WTQueueSizeGet(a), 32);
        for n in 2..=5 {
            writer.publish(sample(n, 1234 + n as i32, 1000 + n as i32, 9));
        }
        wait(|| WTPacketsPeek(a, 99, ptr::null_mut()) == 4);
        let (mut first, mut last) = (0, 0);
        assert_eq!(WTQueuePacketsEx(a, &mut first, &mut last), 1);
        assert_eq!(last.wrapping_sub(first), 3);
        let mut copied = -1;
        assert_eq!(
            WTDataPeek(a, first, last, 2, ptr::null_mut(), &mut copied),
            4
        );
        assert_eq!(copied, 2);
        assert_eq!(
            WTDataGet(a, first, last, 2, ptr::null_mut(), &mut copied),
            4
        );
        assert_eq!(copied, 2);
        assert_eq!(WTPacketsPeek(a, 99, ptr::null_mut()), 0);
        writer.publish(sample(6, 1240, 1024, 9));
        wait(|| WTPacketsPeek(a, 1, packet.as_mut_ptr().cast()) == 1);
        assert_eq!(WTEnable(a, 1), 1);
        assert_eq!(
            WTPacketsPeek(a, 1, ptr::null_mut()),
            1,
            "idempotent enable preserves queue"
        );
        assert_eq!(WTPacket(a, packet[0], ptr::null_mut()), 1);
        assert_eq!(WTPacketsPeek(a, 1, ptr::null_mut()), 0);
        // Relative packets start with zero; subsequent deltas preserve signed pressure.
        assert_eq!(WTGetW(a, &mut wc), 1);
        wc.words[7] = 0x580;
        assert_eq!(WTSetW(a, &wc), 1);
        writer.publish(sample(7, 1400, 1200, 9));
        wait(|| WTPacketsGet(a, 1, packet.as_mut_ptr().cast()) == 1);
        assert_eq!(&packet[2..], &[0, 0, 0]);
        writer.publish(sample(8, 1420, 1000, 9));
        wait(|| WTPacketsGet(a, 1, packet.as_mut_ptr().cast()) == 1);
        assert_eq!(&packet[2..], &[20, 0, (-200i32) as u32]);
        // Pressure response is exactly a 256-UINT API array, guarded by the Rust allocation.
        let mut response = [2048u32; 256];
        assert_eq!(
            WTMgrCsrPressureResponse(manager, 0, response.as_mut_ptr(), ptr::null_mut()),
            1
        );
        wc.words[7] = 0;
        assert_eq!(WTSetW(a, &wc), 1);
        writer.publish(sample(9, 1440, 500, 9));
        wait(|| WTPacketsGet(a, 1, packet.as_mut_ptr().cast()) == 1);
        assert_eq!(packet[4], 2048);
        assert_eq!(
            WTMgrCsrPressureResponse(manager, 0, usize::MAX as *mut u32, ptr::null_mut()),
            1
        );
        assert_eq!(WTMgrCsrEnable(manager, 0, 0), 1);
        writer.publish(sample(10, 1450, 600, 9));
        wait(|| WTPacketsGet(a, 1, packet.as_mut_ptr().cast()) == 1);
        assert_eq!(packet[1], 0);
        assert_eq!(WTMgrCsrEnable(manager, 0, 1), 1);
        // A second overlapping context takes ownership; disabling it restores the first.
        let b = WTOpenA(ptr::null_mut(), &mut lc, 1);
        assert_ne!(b, 0);
        WTGetW(b, &mut wc);
        wc.words[7] = 0;
        WTSetW(b, &wc);
        writer.publish(sample(11, 1500, 777, 9));
        wait(|| WTPacketsGet(b, 1, packet.as_mut_ptr().cast()) == 1);
        assert_eq!(packet[4], 777);
        assert_eq!(WTEnable(b, 0), 1);
        assert_eq!(WTOverlap(a, 1), 1);
        writer.publish(sample(12, 1510, 888, 9));
        wait(|| WTPacketsGet(a, 1, packet.as_mut_ptr().cast()) == 1);
        assert_eq!(packet[4], 888);
        // Save format is architecture-independent; restore creates a fresh context/queue.
        let size = WTInfoA(1, 8, (&mut value as *mut u32).cast());
        assert_eq!(size, 4);
        let mut save = vec![0u8; value as usize];
        let mask = [0x55u8; 16];
        assert_eq!(WTExtSet(a, 3, mask.as_ptr().cast()), 1);
        assert_eq!(WTSave(a, save.as_mut_ptr().cast()), 1);
        assert_eq!(WTClose(a), 1);
        let restored = WTRestore(ptr::null_mut(), save.as_ptr().cast(), 0);
        assert_ne!(restored, 0);
        let mut restored_mask = [0; 16];
        assert_eq!(WTExtGet(restored, 3, restored_mask.as_mut_ptr().cast()), 1);
        assert_eq!(restored_mask, mask);
        assert_eq!(WTQueueSizeGet(restored), 32);
        assert_eq!(WTPacketsPeek(restored, 1, ptr::null_mut()), 0);
        save[100] ^= 1;
        assert_eq!(WTRestore(ptr::null_mut(), save.as_ptr().cast(), 0), 0);
        assert_eq!(WTQueueSizeSet(restored, 0), 0);
        assert_eq!(WTQueueSizeGet(restored), 0);
        assert_eq!(WTQueueSizeSet(restored, 128), 1);
        // Unicode roundtrip uses 40 UTF-16 code units, never UTF-8 bytes.
        WTInfoW(3, 0, (&mut wc as *mut LogContextW).cast());
        wc.name.fill(0);
        let title: Vec<_> = "Caneta – pressão".encode_utf16().collect();
        wc.name[..title.len()].copy_from_slice(&title);
        let unicode = WTOpenW(ptr::null_mut(), &mut wc, 0);
        assert_ne!(unicode, 0);
        let expected = wc.name;
        WTGetW(unicode, &mut wc);
        assert_eq!(wc.name, expected);
        assert_eq!(WTMgrContextEnum(manager, Some(stop_enumeration), 0), 0);
        // Real window messages: proximity is mandatory for owners AND managers;
        // the high word distinguishes hardware exits from context-only transitions.
        let window = CreateWindowExW(
            0,
            class.as_ptr(),
            class.as_ptr(),
            0,
            0,
            0,
            1,
            1,
            HWND_MESSAGE,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null(),
        );
        assert!(!window.is_null());
        let observer = WTMgrOpen(window, 0x6100);
        assert_ne!(observer, 0);
        WTInfoA(3, 0, (&mut lc as *mut LogContextA).cast());
        lc.words[0] = 8; // Cursor notifications enabled, packet notifications disabled.
        lc.words[3] = 0x6200;
        lc.words[6] = 0x5d0;
        lc.words[8] = 0x580;
        let events = WTOpenA(window, &mut lc, 1);
        assert_ne!(events, 0);
        writer.publish(sample(20, 1500, 0, 0));
        writer.publish(sample(21, 1500, 1000, 9));
        wait(|| PeekMessageW(&mut message, window, 0x6205, 0x6205, PM_REMOVE) != 0);
        assert_eq!((message.wParam, message.lParam), (events, 0x10001));
        wait(|| PeekMessageW(&mut message, window, 0x6105, 0x6105, PM_REMOVE) != 0);
        assert_eq!((message.wParam, message.lParam), (events, 0x10001));
        wait(|| PeekMessageW(&mut message, window, 0x6207, 0x6207, PM_REMOVE) != 0);
        assert_eq!(message.lParam, events as isize);
        assert_eq!(
            PeekMessageW(&mut message, window, 0x6200, 0x6200, PM_REMOVE),
            0
        );
        let mut guarded_packet = [0xa5a5a5a5u32; 7];
        wait(|| WTPacketsGet(events, 1, guarded_packet.as_mut_ptr().add(1).cast()) == 1);
        assert_eq!(
            (guarded_packet[0], guarded_packet[6]),
            (0xa5a5a5a5, 0xa5a5a5a5)
        );
        writer.publish(sample(22, 1500, 0, 0));
        wait(|| PeekMessageW(&mut message, window, 0x6205, 0x6205, PM_REMOVE) != 0);
        assert_eq!(message.lParam, 0x10000);
        wait(|| PeekMessageW(&mut message, window, 0x6105, 0x6105, PM_REMOVE) != 0);
        assert_eq!(message.lParam, 0x10000);
        assert_eq!(WTClose(events), 1);
        assert_eq!(WTMgrClose(observer), 1);
        DestroyWindow(window);
        assert_eq!(WTMgrClose(manager), 1);
        assert_eq!(WTMgrClose(manager), 0);
        assert_eq!(WacomCleanup(), 1);
        assert_eq!(WTClose(b), 0);
        assert_eq!(WTClose(restored), 0);
        assert_eq!(WTClose(unicode), 0);
        drop(writer);
        FreeLibrary(module);
    }
}
