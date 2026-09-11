//! External manager entry points. Metadata and ergonomic preferences are session-wide.
use super::*;
fn unsupported() -> i32 {
    unsafe {
        SetLastError(ERROR_NOT_SUPPORTED);
    }
    0
}
fn manager(id: usize) -> bool {
    session::with(|d| d.manager(id)).unwrap_or(false)
}
#[no_mangle]
pub extern "system" fn WTMgrOpen(window: HWND, base: u32) -> usize {
    if base > 0xfff0 || (!window.is_null() && unsafe { IsWindow(window) } == 0) {
        return 0;
    }
    session::with(|d| {
        d.prune();
        let Some(i) = d.managers.iter().position(|m| m.id == 0) else {
            return 0;
        };
        let id = d.next_id();
        d.managers[i] = session::Manager {
            id,
            pid: std::process::id(),
            window: window as u64,
            base: if base == 0 { 0x7ff0 } else { base },
            reserved: 0,
        };
        id as usize
    })
    .unwrap_or(0)
}
#[no_mangle]
pub extern "system" fn WTMgrClose(id: usize) -> i32 {
    session::with(|d| {
        if !d.manager(id) {
            return 0;
        }
        d.managers
            .iter_mut()
            .find(|m| m.id as usize == id)
            .unwrap()
            .id = 0;
        1
    })
    .unwrap_or(0)
}
#[no_mangle]
pub unsafe extern "system" fn WTMgrContextEnum(
    id: usize,
    callback: Option<unsafe extern "system" fn(usize, isize) -> i32>,
    argument: isize,
) -> i32 {
    let Some(callback) = callback else {
        return 0;
    };
    let Some(ids) = session::with(|d| {
        if !d.manager(id) {
            return None;
        }
        d.prune();
        let mut records: Vec<_> = d.contexts[2..]
            .iter()
            .filter(|r| r.id != 0)
            .copied()
            .collect();
        records.sort_by_key(|r| -r.order);
        Some(records.into_iter().map(|r| r.id).collect::<Vec<_>>())
    })
    .flatten() else {
        return 0;
    };
    // Reentrant callbacks may query, close, or open contexts.
    for context in ids {
        if unsafe { callback(context as usize, argument) } == 0 {
            return 0;
        }
    }
    1
}
#[no_mangle]
pub extern "system" fn WTMgrContextOwner(id: usize, context: usize) -> HWND {
    if !manager(id) {
        return ptr::null_mut();
    }
    session::get(context).map_or(ptr::null_mut(), |r| r.window as HWND)
}
#[no_mangle]
pub extern "system" fn WTMgrDefContext(id: usize, system: i32) -> usize {
    if !manager(id) {
        return 0;
    }
    0x7fff0001 + usize::from(system != 0)
}
#[no_mangle]
pub extern "system" fn WTMgrDefContextEx(id: usize, device: u32, system: i32) -> usize {
    if !manager(id) || !matches!(device, 0 | u32::MAX) {
        return 0;
    }
    if device == u32::MAX {
        WTMgrDefContext(id, system)
    } else {
        0x7fff0003 + usize::from(system != 0)
    }
}
#[no_mangle]
pub extern "system" fn WTMgrDeviceConfig(id: usize, device: u32, owner: HWND) -> u32 {
    if !manager(id) || !matches!(device, 0 | u32::MAX) {
        return 0;
    }
    let Some(initial) = session::defaults(false) else {
        return 0;
    };
    let name = String::from_utf16_lossy(
        &initial.name[..initial.name.iter().position(|&c| c == 0).unwrap_or(40)],
    );
    match context_dialog::edit(owner, (name, initial.words)) {
        Some((name, words)) => {
            if session::set(1, &name, words) {
                2
            } else {
                0
            }
        }
        None => 1,
    }
}
#[no_mangle]
pub extern "system" fn WTMgrCsrEnable(id: usize, cursor: u32, enabled: i32) -> i32 {
    session::with(|d| {
        if !d.manager(id) || cursor > 1 {
            return 0;
        }
        d.cursors[cursor as usize].enabled = u32::from(enabled != 0);
        d.information_changed(id, 200 + cursor, 2);
        1
    })
    .unwrap_or(0)
}
unsafe fn array<T: Copy, const N: usize>(p: *const T, default: [T; N]) -> Option<[T; N]> {
    if p.is_null() {
        None
    } else if p as usize == usize::MAX {
        Some(default)
    } else {
        Some(unsafe { ptr::read_unaligned(p.cast::<[T; N]>()) })
    }
}
#[no_mangle]
pub unsafe extern "system" fn WTMgrCsrButtonMap(
    id: usize,
    cursor: u32,
    logical: *mut u8,
    system: *mut u8,
) -> i32 {
    if !manager(id) || cursor > 1 {
        return 0;
    }
    let logical = unsafe { array(logical, std::array::from_fn(|i| i as u8)) };
    let system = unsafe {
        array(
            system,
            std::array::from_fn(|i| {
                if i == 0 {
                    1
                } else if i == 1 {
                    4
                } else {
                    0
                }
            }),
        )
    };
    if logical.is_some_and(|a| a.iter().any(|&v| v > 31)) {
        return 0;
    }
    if system.is_some_and(|a| a.iter().any(|&code| code > 9)) {
        return unsupported();
    }
    session::with(|d| {
        if !d.manager(id) {
            return 0;
        }
        let c = &mut d.cursors[cursor as usize];
        if let Some(a) = logical {
            c.button_map = a;
        }
        if let Some(a) = system {
            c.system_map = a;
        }
        d.information_changed(id, 200 + cursor, 7);
        1
    })
    .unwrap_or(0)
}
#[no_mangle]
pub unsafe extern "system" fn WTMgrCsrPressureBtnMarksEx(
    id: usize,
    cursor: u32,
    normal: *mut u32,
    tangent: *mut u32,
) -> i32 {
    if !manager(id) || cursor > 1 {
        return 0;
    }
    if !tangent.is_null() && tangent as usize != usize::MAX {
        return unsupported();
    }
    let marks = unsafe { array(normal, [0, 1]) };
    if marks.is_some_and(|m| m.iter().any(|&n| n > 4097)) {
        return 0;
    }
    session::with(|d| {
        if !d.manager(id) {
            return 0;
        }
        if let Some(marks) = marks {
            d.cursors[cursor as usize].marks = marks;
        }
        d.information_changed(id, 200 + cursor, 10);
        1
    })
    .unwrap_or(0)
}
#[no_mangle]
pub unsafe extern "system" fn WTMgrCsrPressureBtnMarks(
    id: usize,
    cursor: u32,
    normal: u32,
    tangent: u32,
) -> i32 {
    let mut marks = [normal & 0xffff, normal >> 16];
    let p = if normal == u32::MAX {
        usize::MAX as *mut u32
    } else {
        marks.as_mut_ptr()
    };
    let t = if tangent == u32::MAX {
        usize::MAX as *mut u32
    } else {
        ptr::null_mut()
    };
    unsafe { WTMgrCsrPressureBtnMarksEx(id, cursor, p, t) }
}
#[no_mangle]
pub unsafe extern "system" fn WTMgrCsrPressureResponse(
    id: usize,
    cursor: u32,
    normal: *mut u32,
    tangent: *mut u32,
) -> i32 {
    if !manager(id) || cursor > 1 {
        return 0;
    }
    if !tangent.is_null() && tangent as usize != usize::MAX {
        return unsupported();
    }
    let response = unsafe {
        array(
            normal,
            std::array::from_fn::<_, 256, _>(|i| ((i * 4097 + 127) / 255) as u32),
        )
    };
    if response.is_some_and(|a| a.iter().any(|&v| v > 4097)) {
        return 0;
    }
    session::with(|d| {
        if !d.manager(id) {
            return 0;
        }
        if let Some(a) = response {
            d.cursors[cursor as usize].response = a;
            d.cursors[cursor as usize].response_custom = u32::from(normal as usize != usize::MAX);
        }
        d.information_changed(id, 200 + cursor, 11);
        1
    })
    .unwrap_or(0)
}
#[no_mangle]
pub unsafe extern "system" fn WTMgrExt(id: usize, tag: u32, data: *mut c_void) -> i32 {
    if !manager(id) || tag != 0 || data.is_null() {
        return 0;
    }
    let enabled = unsafe { ptr::read_unaligned(data.cast::<i32>()) } != 0;
    session::with(|d| {
        if !d.manager(id) {
            return 0;
        }
        d.out_of_bounds = u32::from(enabled);
        d.information_changed(id, 301, 6);
        1
    })
    .unwrap_or(0)
}
#[no_mangle]
pub extern "system" fn WTMgrCsrExt(_: usize, _: u32, _: u32, _: *mut c_void) -> i32 {
    unsupported()
}
#[no_mangle]
pub extern "system" fn WTMgrConfigReplaceExA(_: usize, _: i32, _: *const u8, _: *const u8) -> i32 {
    unsupported()
}
#[no_mangle]
pub extern "system" fn WTMgrConfigReplaceExW(_: usize, _: i32, _: *const u16, _: *const u8) -> i32 {
    unsupported()
}
#[no_mangle]
pub extern "system" fn WTMgrPacketHookExA(_: usize, _: i32, _: *const u8, _: *const u8) -> usize {
    unsupported();
    0
}
#[no_mangle]
pub extern "system" fn WTMgrPacketHookExW(_: usize, _: i32, _: *const u16, _: *const u8) -> usize {
    unsupported();
    0
}
#[no_mangle]
pub extern "system" fn WTMgrPacketUnhook(_: usize) -> i32 {
    unsupported()
}
#[no_mangle]
pub extern "system" fn WTMgrPacketHookNext(_: usize, _: i32, _: usize, _: isize) -> isize {
    unsupported();
    0
}
