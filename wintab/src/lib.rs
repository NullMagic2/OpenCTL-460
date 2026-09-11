//! Independent WinTab provider: each process reads the shared pen stream and owns its contexts.
//! Implements the WinTab application API and shared manager/context services.
//! Hardware-specific extensions and legacy hooks are capability-limited; see docs/WINTAB-CONFORMANCE.md.
#![allow(non_snake_case)]
use std::{
    collections::{HashMap, VecDeque},
    ffi::c_void,
    mem::size_of,
    ptr,
    sync::{Mutex, MutexGuard, OnceLock},
    time::Duration,
};
use windows_sys::Win32::{Foundation::*, System::LibraryLoader::*, UI::WindowsAndMessaging::*};
#[path = "../../shared/ipc.rs"]
pub mod ipc;
use ipc::{Mapping, PenData};

const MAX_CONTEXTS: usize = 64;
// Internal routing metadata; never exposed in a WinTab packet.
const HARDWARE_PROXIMITY: i32 = 1 << 30;
const OUT_OF_BOUNDS: i32 = 1 << 29;
const OPTIONS: u32 = 0xc00d; // System, packet/cursor messages, inside/outside margins.
const RELATIVE: u32 = 0x0fc4; // Time, buttons, XYZ and the two pressures.
const SAVE_SIZE: usize = 244;
pub const SUPPORTED: u32 = 0x3fff; // Context through orientation; rotation/tangent are reported zero.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LogContextA {
    pub name: [u8; 40],
    pub words: [u32; 33],
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LogContextW {
    pub name: [u16; 40],
    pub words: [u32; 33],
}
const _: () = assert!(size_of::<LogContextA>() == 172 && size_of::<LogContextW>() == 212);
#[derive(Clone)]
struct Packet {
    serial: u32,
    pen: PenData,
    buttons: u32,
    changed: u32,
    previous: Option<PenData>,
    status: u32,
}
struct Context {
    revision: u32,
    cursor_mask: [u8; 16],
    name: String,
    words: [u32; 33],
    window: usize,
    enabled: bool,
    queue: VecDeque<Packet>,
    capacity: usize,
    previous: Option<PenData>,
    overflow: bool,
    serial: u32,
}
#[derive(Default)]
struct State {
    contexts: HashMap<usize, Context>,
    order: Vec<usize>,
    capture: Option<usize>,
}
static STATE: OnceLock<Mutex<State>> = OnceLock::new();
fn state() -> MutexGuard<'static, State> {
    STATE
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}
fn report_rate() -> u32 {
    static RATE: OnceLock<Mutex<(std::time::Instant, u32)>> = OnceLock::new();
    let mut cache = RATE
        .get_or_init(|| Mutex::new((std::time::Instant::now() - Duration::from_secs(2), 250)))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if cache.0.elapsed() >= Duration::from_secs(1) {
        cache.1 = Mapping::open_reader()
            .ok()
            .filter(|m| m.active())
            .map(|m| m.metadata().2)
            .filter(|hz| (60..=500).contains(hz))
            .unwrap_or(250);
        cache.0 = std::time::Instant::now();
    }
    cache.1
}
fn public_words(mut w: [u32; 33]) -> [u32; 33] {
    w[5] = report_rate();
    w
}
fn default_words() -> [u32; 33] {
    let mut w = [0u32; 33];
    w[0] = 4;
    w[1] = 0;
    w[3] = 0x7ff0;
    w[5] = report_rate();
    w[6] = 0x17ff;
    w[8] = SUPPORTED;
    w[9] = 3;
    w[10] = 3;
    w[14] = 14720;
    w[15] = 9200;
    w[16] = 1;
    w[20] = 14720;
    w[21] = 9200;
    w[22] = 1;
    w[23] = 65536;
    w[24] = 65536;
    w[25] = 65536;
    w[26] = 0;
    w[29] = unsafe { GetSystemMetrics(SM_CXSCREEN) }.max(1) as u32;
    w[30] = unsafe { GetSystemMetrics(SM_CYSCREEN) }.max(1) as u32;
    w[31] = 65536;
    w[32] = 65536;
    w
}
fn valid_words(w: &[u32; 33]) -> bool {
    matches!(w[4], 0 | u32::MAX)
        && w[0] & !OPTIONS == 0
        && w[6] & !SUPPORTED == 0
        && w[2] & !0x1f == 0
        && w[14] != 0
        && w[15] != 0
        && w[20] != 0
        && w[21] != 0
        && w[3] <= 0xfff0
}
fn validated(mut w: [u32; 33], enabled: bool) -> [u32; 33] {
    w[1] = if enabled { 4 } else { 1 };
    // Return the feeder rate instead of claiming a requested rate was honored.
    w[5] = report_rate();
    w[7] &= RELATIVE & w[6];
    w[8] &= SUPPORTED & w[6] & !0x7f;
    for (axis, max) in [14720i64, 9200].into_iter().enumerate() {
        let origin = i64::from(w[11 + axis] as i32).clamp(0, max - 1);
        let extent = i64::from(w[14 + axis] as i32);
        w[11 + axis] = origin as u32;
        w[14 + axis] = (extent.abs().min(max - origin) * extent.signum()) as u32;
    }
    w
}
fn apply_locks(old: &[u32; 33], mut new: [u32; 33]) -> [u32; 33] {
    let locks = old[2];
    if locks & 1 != 0 {
        new[14..17].copy_from_slice(&old[14..17]);
    } else if locks & 2 != 0 {
        let axis = (0..2).find(|&i| new[14 + i] != old[14 + i]).unwrap_or(0);
        let base = f64::from(old[14 + axis] as i32).abs().max(1.0);
        let mut ratio = f64::from(new[14 + axis] as i32).abs() / base;
        for (i, max) in [14720.0, 9200.0].into_iter().enumerate() {
            ratio = ratio.min(
                (max - f64::from(new[11 + i] as i32).clamp(0.0, max - 1.0))
                    / f64::from(old[14 + i] as i32).abs().max(1.0),
            );
        }
        for i in 0..2 {
            new[14 + i] = (f64::from(old[14 + i] as i32) * ratio).round() as i32 as u32;
        }
    }
    if locks & 4 != 0 {
        new[23..26].copy_from_slice(&old[23..26]);
    }
    if locks & 8 != 0 {
        new[0] = (new[0] & !0xc000) | (old[0] & 0xc000);
    }
    if locks & 16 != 0 && old[0] & 1 != 0 {
        new[26..33].copy_from_slice(&old[26..33]);
    }
    // Input origins remain movable, but cannot push a locked size off the tablet.
    if locks & 3 != 0 {
        for (i, max) in [14720i64, 9200].into_iter().enumerate() {
            let size = i64::from(new[14 + i] as i32).abs().min(max);
            new[11 + i] = i64::from(new[11 + i] as i32).clamp(0, max - size) as u32;
        }
    }
    new[2] = old[2];
    new
}
fn ansi(s: &str) -> Vec<u8> {
    use windows_sys::Win32::Globalization::{WideCharToMultiByte, CP_ACP};
    let wide: Vec<u16> = s.encode_utf16().collect();
    if wide.is_empty() {
        return Vec::new();
    }
    unsafe {
        let n = WideCharToMultiByte(
            CP_ACP,
            0,
            wide.as_ptr(),
            wide.len() as i32,
            ptr::null_mut(),
            0,
            ptr::null(),
            ptr::null_mut(),
        );
        let mut bytes = vec![0; n.max(0) as usize];
        WideCharToMultiByte(
            CP_ACP,
            0,
            wide.as_ptr(),
            wide.len() as i32,
            bytes.as_mut_ptr(),
            n,
            ptr::null(),
            ptr::null_mut(),
        );
        bytes
    }
}
fn from_ansi(bytes: &[u8]) -> String {
    use windows_sys::Win32::Globalization::{MultiByteToWideChar, CP_ACP};
    if bytes.is_empty() {
        return String::new();
    }
    unsafe {
        let n = MultiByteToWideChar(
            CP_ACP,
            0,
            bytes.as_ptr(),
            bytes.len() as i32,
            ptr::null_mut(),
            0,
        );
        let mut wide = vec![0; n.max(0) as usize];
        MultiByteToWideChar(
            CP_ACP,
            0,
            bytes.as_ptr(),
            bytes.len() as i32,
            wide.as_mut_ptr(),
            n,
        );
        String::from_utf16_lossy(&wide)
    }
}
fn name_a(s: &str) -> [u8; 40] {
    let mut a = [0; 40];
    let mut used = 0;
    for ch in s.chars() {
        let encoded = ansi(&ch.to_string());
        if used + encoded.len() > 39 {
            break;
        }
        a[used..used + encoded.len()].copy_from_slice(&encoded);
        used += encoded.len();
    }
    a
}
fn name_w(s: &str) -> [u16; 40] {
    let mut name = [0; 40];
    let mut used = 0;
    for ch in s.chars() {
        let mut pair = [0; 2];
        let units = ch.encode_utf16(&mut pair);
        if used + units.len() > 39 {
            break;
        }
        name[used..used + units.len()].copy_from_slice(units);
        used += units.len();
    }
    name
}
fn bytes<T: Copy>(v: &T) -> Vec<u8> {
    unsafe { std::slice::from_raw_parts((v as *const T).cast(), size_of::<T>()).to_vec() }
}
fn uint(n: u32) -> Vec<u8> {
    n.to_ne_bytes().to_vec()
}
fn string(s: &str, wide: bool) -> Vec<u8> {
    if wide {
        s.encode_utf16()
            .chain(Some(0))
            .flat_map(u16::to_ne_bytes)
            .collect()
    } else {
        ansi(s).into_iter().chain(Some(0)).collect()
    }
}
fn axis(min: i32, max: i32, units: u32, res: u32) -> Vec<u8> {
    [min as u32, max as u32, units, res]
        .into_iter()
        .flat_map(u32::to_ne_bytes)
        .collect()
}
unsafe fn copy_info(out: *mut c_void, data: Vec<u8>) -> u32 {
    if !out.is_null() {
        unsafe {
            ptr::copy_nonoverlapping(data.as_ptr(), out.cast(), data.len());
        }
    }
    data.len() as u32
}

fn info(category: u32, index: u32, wide: bool) -> Vec<u8> {
    let mut words = default_words();
    // System-context clients (including Photoshop) inherit output in screen
    // units. Returning tablet units here puts their packets outside the canvas.
    // Digitizing contexts retain the native tablet coordinate output.
    if matches!(category, 4 | 500) {
        words[0] |= 1;
        words[17] = words[27];
        words[18] = words[28];
        words[20] = words[29];
        words[21] = words[30];
    }
    let default = if matches!(category, 3 | 4 | 400 | 500) {
        session::defaults(matches!(category, 4 | 500))
    } else {
        None
    };
    if let Some(r) = default {
        words = public_words(r.words);
    }
    let context_name = default
        .map(|r| {
            String::from_utf16_lossy(&r.name[..r.name.iter().position(|&c| c == 0).unwrap_or(40)])
        })
        .unwrap_or_else(|| "OpenCTL 460".into());
    let cursor = if matches!(category, 200 | 201) {
        session::with(|d| d.cursors[(category - 200) as usize])
    } else {
        None
    };
    match category {
        300 => match index {
            1 => string("Cursor Mask", wide),
            2 => uint(3),
            7 | 8 => vec![255; 16],
            _ => vec![],
        },
        301 => match index {
            1 => string("Out of Bounds Tracking", wide),
            2 => uint(0),
            6 => uint(session::with(|d| d.out_of_bounds).unwrap_or(0)),
            _ => vec![],
        },
        1 => match index {
            1 => string("OpenCTL 460 WinTab", wide),
            2 => 0x0104u16.to_ne_bytes().to_vec(),
            3 => 0x0002u16.to_ne_bytes().to_vec(),
            4 => uint(1),
            5 => uint(2), // Pen and inverted eraser, matching DVC_NCSRTYPES.
            6 => uint(MAX_CONTEXTS as u32), // Capacity, not the currently open count.
            7 => uint(OPTIONS),
            8 => uint(SAVE_SIZE as u32),
            9 => uint(2),
            10 => uint(session::MANAGERS as u32),
            _ => vec![],
        },
        2 => session::with(|d| {
            d.prune();
            let contexts: Vec<_> = d.contexts[2..].iter().filter(|c| c.id != 0).collect();
            uint(match index {
                1 => contexts.len() as u32,
                2 => contexts.iter().filter(|c| c.words[0] & 1 != 0).count() as u32,
                3 => contexts
                    .iter()
                    .filter(|c| c.enabled != 0)
                    .map(|_| report_rate())
                    .max()
                    .unwrap_or(0),
                4 => contexts.iter().fold(0, |v, c| v | c.words[6]),
                5 => d.managers.iter().filter(|m| m.id != 0).count() as u32,
                6 => 1,
                7 => contexts.iter().fold(0, |v, c| v | c.words[9] | c.words[10]),
                8 => 3,
                _ => return vec![],
            })
        })
        .unwrap_or_default(),
        3 | 4 | 400 | 500 => match index {
            0 => {
                if wide {
                    bytes(&LogContextW {
                        name: name_w(&context_name),
                        words,
                    })
                } else {
                    bytes(&LogContextA {
                        name: name_a(&context_name),
                        words,
                    })
                }
            }
            1 => {
                if wide {
                    bytes(&name_w(&context_name))
                } else {
                    bytes(&name_a(&context_name))
                }
            }
            2..=34 => uint(words[(index - 2) as usize]),
            _ => vec![],
        },
        100 => match index {
            1 => string("OpenCTL 460 virtual pen", wide),
            2 => uint(4),
            3 => uint(2),
            4 => uint(0),
            5 => uint(report_rate()),
            6 | 8 => uint(SUPPORTED),
            7 => uint(0),
            9 | 10 => uint(16),
            11 => uint(0),
            12 => axis(0, 14720, 2, 1000 << 16),
            13 => axis(0, 9200, 2, 1000 << 16),
            14 => axis(0, 0, 0, 0),
            15 => axis(0, 4097, 0, 0),
            16 => axis(0, 0, 0, 0),
            17 => [
                axis(0, 3600, 3, 3600 << 16),
                axis(-900, 900, 3, 3600 << 16),
                axis(0, 0, 0, 0),
            ]
            .concat(),
            18 => vec![0; 48],
            19 => string("CTL460Studio", wide),
            _ => vec![],
        },
        200 | 201 => match index {
            1 => string(if category == 201 { "Eraser" } else { "Pen" }, wide),
            2 => uint(cursor.map_or(1, |c| c.enabled)),
            3 => uint(SUPPORTED),
            4 => vec![2],
            5 => vec![2],
            6 => string("Tip\0Barrel\0", wide),
            7 => cursor.map_or_else(|| (0..32).collect(), |c| c.button_map.to_vec()),
            8 => {
                let mut map = vec![0; 32];
                map[0] = 1;
                map[1] = 4;
                cursor.map_or(map, |c| c.system_map.to_vec())
            }
            9 => vec![0],
            // Every positive output pressure represents contact. Advertising
            // full-scale as the press mark makes clients wait for maximum force.
            10 => cursor
                .map_or([0, 1], |c| c.marks)
                .into_iter()
                .flat_map(u32::to_ne_bytes)
                .collect(),
            11 => cursor.map_or_else(
                || {
                    (0..256)
                        .map(|i| ((i * 4097 + 127) / 255) as u32)
                        .flat_map(u32::to_ne_bytes)
                        .collect()
                },
                |c| c.response.into_iter().flat_map(u32::to_ne_bytes).collect(),
            ),
            12 => vec![255],
            13 => vec![0; 8],
            14 => uint(0),
            15 => uint(if category == 201 { 2 } else { 1 }),
            16 => uint(0),
            17 => uint(SUPPORTED),
            18 => uint(2),
            19 => uint(if category == 201 { 4 } else { 0 }),
            20 => uint(if category == 201 { 0x080a } else { 0x0802 }),
            _ => vec![],
        },
        _ => vec![],
    }
}
fn category_info(category: u32, wide: bool) -> Vec<u8> {
    if matches!(category, 3 | 4 | 400 | 500) {
        return info(category, 0, wide);
    }
    let last = match category {
        1 => 10,
        2 => 8,
        100 => 19,
        200 | 201 => 20,
        300 => 8,
        301 => 6,
        _ => return vec![],
    };
    (1..=last).flat_map(|i| info(category, i, wide)).collect()
}
fn tablet_present() -> bool {
    // Pure/unit and isolated DLL tests model a tablet without physical USB I/O.
    if cfg!(test) || ipc::NAME.starts_with("Local\\CTL460Conformance-") {
        return true;
    }
    static PRESENCE: OnceLock<Mutex<(std::time::Instant, bool)>> = OnceLock::new();
    let mut cache = PRESENCE
        .get_or_init(|| Mutex::new((std::time::Instant::now() - Duration::from_secs(2), false)))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if cache.0.elapsed() >= Duration::from_secs(1) {
        cache.1 = hidapi::HidApi::new().is_ok_and(|api| {
            api.device_list()
                .any(|d| d.vendor_id() == 0x056a && d.product_id() == 0x00d4)
        });
        cache.0 = std::time::Instant::now();
    }
    cache.1
}
unsafe fn query_info(c: u32, i: u32, p: *mut c_void, wide: bool) -> u32 {
    if !tablet_present() {
        return 0;
    }
    if c == 0 {
        // Category zero is always a size query; it must NEVER write to p.
        return [1, 2, 3, 4, 100, 200, 201, 300, 301, 400, 500]
            .into_iter()
            .map(|c| category_info(c, wide).len())
            .max()
            .unwrap_or(0) as u32;
    }
    let data = if i == 0 {
        category_info(c, wide)
    } else {
        info(c, i, wide)
    };
    unsafe { copy_info(p, data) }
}
/// # Safety
/// When non-null, p must have the size returned by the matching null-buffer query.
#[no_mangle]
pub unsafe extern "system" fn WTInfoA(c: u32, i: u32, p: *mut c_void) -> u32 {
    unsafe { query_info(c, i, p, false) }
}
/// # Safety
/// When non-null, p must have the size returned by the matching null-buffer query.
#[no_mangle]
pub unsafe extern "system" fn WTInfoW(c: u32, i: u32, p: *mut c_void) -> u32 {
    unsafe { query_info(c, i, p, true) }
}

fn notification_allowed(options: u32, offset: u32) -> bool {
    match offset {
        0 => options & 4 != 0,
        7 => options & 8 != 0,
        _ => true,
    }
}
fn notify(c: &Context, id: usize, offset: u32, wp: usize, lp: isize) {
    if offset == 5 {
        session::with(|d| {
            for m in d.managers.iter().filter(|m| m.id != 0 && m.window != 0) {
                unsafe {
                    PostMessageW(m.window as HWND, m.base + offset, wp, lp);
                }
            }
        });
    }
    if c.window != 0 && notification_allowed(c.words[0], offset) {
        unsafe {
            PostMessageW(
                c.window as HWND,
                c.words[3] + offset,
                wp,
                if lp == isize::MIN { id as isize } else { lp },
            );
        }
    }
}
#[cfg(test)]
fn overlap_status(s: &mut State) {
    let mut higher: Vec<[u32; 33]> = Vec::new();
    for id in &s.order {
        let Some(c) = s.contexts.get_mut(id) else {
            continue;
        };
        let status = if !c.enabled {
            1
        } else if higher.iter().any(|w| rectangles_overlap(w, &c.words)) {
            2
        } else {
            4
        };
        if status != c.words[1] {
            c.words[1] = status;
            notify(c, *id, 4, *id, status as isize);
        }
        if c.enabled {
            higher.push(c.words);
        }
    }
}
fn interval(w: &[u32; 33], axis: usize) -> (i64, i64) {
    let origin = i64::from(w[11 + axis] as i32);
    (origin, origin + i64::from(w[14 + axis] as i32).abs())
}
fn rectangles_overlap(a: &[u32; 33], b: &[u32; 33]) -> bool {
    (0..2).all(|axis| {
        let (al, ar) = interval(a, axis);
        let (bl, br) = interval(b, axis);
        al <= br && bl <= ar
    })
}
fn contains(w: &[u32; 33], pen: PenData) -> bool {
    let coords = [i64::from(pen.x), 9200 - i64::from(pen.y)];
    (0..2).all(|axis| {
        let (l, r) = interval(w, axis);
        coords[axis] >= l && coords[axis] <= r
    })
}
fn buttons(p: PenData) -> u32 {
    let physical = (p.flags as u32) & 3;
    session::with(|d| {
        let map = &d.cursors[usize::from(p.flags & 4 != 0)].button_map;
        (0..2)
            .filter(|&i| physical & (1 << i) != 0)
            .fold(0, |bits, i| bits | (1 << map[i]))
    })
    .unwrap_or(physical)
}
fn changes(a: PenData, b: PenData) -> u32 {
    let mut m = 0x10;
    if a.time != b.time {
        m |= 4;
    }
    if (a.flags ^ b.flags) & 4 != 0 {
        m |= 0x22;
    }
    if a.x != b.x {
        m |= 0x80;
    }
    if a.y != b.y {
        m |= 0x100;
    }
    if a.pressure != b.pressure {
        m |= 0x400;
    }
    if buttons(a) != buttons(b) {
        m |= 0x40;
    }
    if (a.tilt_x, a.tilt_y) != (b.tilt_x, b.tilt_y) {
        m |= 0x1000;
    }
    if (a.flags ^ b.flags) & 12 != 0 {
        m |= 2;
    }
    m
}
fn effective_words(mut w: [u32; 33]) -> [u32; 33] {
    if w[0] & 0xc000 == 0xc000 {
        for axis in 0..2 {
            let extent = i64::from(w[14 + axis] as i32);
            let margin = 16.min((extent.abs() - 1).max(0) / 2);
            w[11 + axis] = (i64::from(w[11 + axis] as i32) + margin) as u32;
            w[14 + axis] = ((extent.abs() - 2 * margin) * extent.signum()) as u32;
        }
    }
    w
}
fn context_sample(w: &[u32; 33], mut p: PenData, grabbed: bool) -> Option<(PenData, u32)> {
    if !p.in_range() {
        return Some((p, 1));
    }
    let inside = contains(w, p);
    if !inside && (grabbed || p.flags & OUT_OF_BOUNDS != 0) {
        let (xl, xr) = interval(w, 0);
        let (yl, yr) = interval(w, 1);
        p.x = i64::from(p.x).clamp(xl, xr) as i32;
        p.y = (9200 - (9200 - i64::from(p.y)).clamp(yl, yr)) as i32;
        return Some((p, if p.flags & OUT_OF_BOUNDS != 0 { 4 } else { 9 }));
    }
    let margin = if w[0] & 0x8000 != 0 && w[0] & 0x4000 == 0 {
        16
    } else {
        0
    };
    let coords = [i64::from(p.x), 9200 - i64::from(p.y)];
    if !(0..2).all(|a| {
        let (l, r) = interval(w, a);
        coords[a] >= l - margin && coords[a] <= r + margin
    }) {
        return None;
    }
    let effective = effective_words(*w);
    let mut status = 0;
    let clamped: Vec<i64> = (0..2)
        .map(|a| {
            let (l, r) = interval(&effective, a);
            let v = coords[a].clamp(l, r);
            if v != coords[a] {
                status |= 4;
            }
            v
        })
        .collect();
    p.x = clamped[0] as i32;
    p.y = (9200 - clamped[1]) as i32;
    Some((p, status))
}
fn cursor_selected(mask: &[u8; 16], p: PenData) -> bool {
    mask[0] & (1 << u32::from(p.flags & 4 != 0)) != 0
}
fn selected(c: &Context, p: PenData) -> bool {
    if !cursor_selected(&c.cursor_mask, p) {
        return false;
    }
    let old = c.previous.unwrap_or_default();
    let down = buttons(p) & !buttons(old);
    let up = buttons(old) & !buttons(p);
    let changed = if c.previous.is_none() {
        SUPPORTED
    } else {
        changes(old, p)
    };
    changed & c.words[8] & c.words[6] & !0x7f != 0
        || down & c.words[9] != 0
        || up & c.words[10] != 0
}
fn proximity_flags(pen: PenData) -> isize {
    isize::from(pen.in_range()) | (isize::from(pen.flags & HARDWARE_PROXIMITY != 0) << 16)
}
fn post_sample(c: &mut Context, id: usize, pen: PenData, status: u32) {
    let old = c.previous.unwrap_or_default();
    let changed = if c.previous.is_none() {
        SUPPORTED
    } else {
        changes(old, pen)
    };
    if old.in_range() != pen.in_range() {
        notify(c, id, 5, id, proximity_flags(pen));
    }
    let delta = buttons(old) ^ buttons(pen);
    let events: Vec<u32> = if c.words[7] & 0x40 != 0 {
        let mut events = Vec::new();
        for bit in 0..32 {
            if delta & (1 << bit) != 0 {
                let down = buttons(pen) & (1 << bit) != 0;
                let mask = if down { c.words[9] } else { c.words[10] };
                if mask & (1 << bit) != 0 {
                    events.push(bit | if down { 2 << 16 } else { 1 << 16 });
                }
            }
        }
        if events.is_empty() {
            events.push(0);
        }
        events
    } else {
        vec![buttons(pen)]
    };
    for (event, b) in events.into_iter().enumerate() {
        c.serial = c.serial.wrapping_add(1);
        if c.capacity == 0 || c.queue.len() >= c.capacity {
            c.overflow = true;
            if let Some(last) = c.queue.back_mut() {
                last.status |= 2;
            }
            continue;
        }
        let error = std::mem::take(&mut c.overflow);
        let previous = if event == 0 { c.previous } else { Some(pen) };
        c.queue.push_back(Packet {
            serial: c.serial,
            pen,
            buttons: b,
            changed,
            previous,
            status: status | if error { 2 } else { 0 },
        });
        if (old.flags ^ pen.flags) & 4 != 0 || c.previous.is_none() {
            notify(c, id, 7, c.serial as usize, isize::MIN);
        }
        notify(c, id, 0, c.serial as usize, isize::MIN);
        c.previous = Some(pen);
    }
}
fn ingest_state(s: &mut State, pen: PenData, foreground: bool) {
    // Select one owner in overlap order; retain contact ownership until release.
    let captured = s
        .capture
        .filter(|id| s.contexts.get(id).is_some_and(|c| c.enabled));
    let chosen = if foreground && pen.in_range() {
        captured.or_else(|| {
            s.order.iter().copied().find(|id| {
                s.contexts.get(id).is_some_and(|c| {
                    c.enabled
                        && pen.in_range()
                        && context_sample(&c.words, pen, false).is_some_and(|(p, _)| selected(c, p))
                })
            })
        })
    } else {
        None
    };
    for (&id, c) in &mut s.contexts {
        // A sample that changes no requested field is not a proximity exit.
        // Keep stationary hover/contact alive until leaving, focus loss or
        // another eligible context actually claims the event.
        let leaving = !foreground
            || !pen.in_range()
            || !cursor_selected(&c.cursor_mask, pen)
            || (chosen.is_some() && Some(id) != chosen)
            || context_sample(&c.words, pen, captured == Some(id)).is_none();
        if leaving && c.previous.is_some_and(|p| p.in_range()) {
            let old = c.previous.unwrap();
            post_sample(
                c,
                id,
                PenData {
                    flags: (old.flags & 4) | (pen.flags & HARDWARE_PROXIMITY),
                    pressure: 0,
                    ..old
                },
                1,
            );
            c.previous = None;
        }
    }
    if let Some(id) = chosen {
        if let Some(c) = s.contexts.get_mut(&id) {
            if let Some((p, status)) = context_sample(&c.words, pen, captured == Some(id)) {
                if selected(c, p) {
                    post_sample(c, id, p, status);
                }
            }
        }
        let grab_buttons = s.contexts.get(&id).map_or(0, |c| c.words[9] & c.words[10]);
        s.capture = if buttons(pen) & grab_buttons != 0 && pen.in_range() {
            Some(id)
        } else {
            None
        };
    } else {
        s.capture = None;
    }
}
fn ingest(pen: PenData) {
    let mut s = state();
    session::sync(&mut s);
    ingest_state(&mut s, pen, false);
}
fn ingest_routed(pen: PenData, owner: u32, sequence: u64) {
    let Some((pen, winner)) = session::route(pen, owner, sequence) else {
        ingest(PenData::default());
        return;
    };
    let mut s = state();
    session::sync(&mut s);
    let owns_event = s.contexts.contains_key(&winner);
    if owns_event {
        s.order.retain(|&id| id != winner);
        s.order.insert(0, winner);
    }
    ingest_state(&mut s, pen, owns_event);
}
fn start_reader() -> bool {
    static START: OnceLock<bool> = OnceLock::new();
    *START.get_or_init(|| {
        // Pin our module before starting a thread: applications may otherwise unload a live DLL.
        let mut module = ptr::null_mut();
        if unsafe {
            GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
                start_reader as *const () as *const u16,
                &mut module,
            )
        } == 0
        {
            return false;
        }
        std::thread::Builder::new()
            .name("CTL460 WinTab reader".into())
            .spawn(|| {
                let mut mapping: Option<Mapping> = None;
                let mut number = 0;
                let mut owner = 0;
                let mut active = false;
                let mut present = tablet_present();
                loop {
                    let connected = tablet_present();
                    if connected != present {
                        present = connected;
                        session::with(|d| d.information_changed(0, 1, 4));
                    }
                    if state().contexts.is_empty() {
                        number = 0;
                        owner = 0;
                        active = false;
                        std::thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                    if mapping.is_none() {
                        mapping = Mapping::open_reader().ok();
                        if mapping.is_none() {
                            std::thread::sleep(Duration::from_millis(250));
                            continue;
                        }
                    }
                    if let Some(m) = &mapping {
                        if m.active() {
                            let current_owner = m.metadata().0;
                            let last = m.latest();
                            if owner != current_owner || last < number {
                                if active {
                                    ingest(PenData::default());
                                }
                                owner = current_owner;
                                number = last.saturating_sub(1);
                            }
                            if last.saturating_sub(number) > ipc::CAPACITY as u64 {
                                ingest(PenData::default());
                                number = last.saturating_sub(1);
                            }
                            while number < last {
                                number += 1;
                                if let Some(p) = m.read(number) {
                                    ingest_routed(p, current_owner, number);
                                } else {
                                    ingest(PenData::default());
                                    number = last;
                                }
                            }
                            active = true;
                        } else {
                            if active {
                                ingest(PenData::default());
                            }
                            active = false;
                            owner = 0;
                            number = 0;
                            mapping = None;
                        }
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
            })
            .is_ok()
    })
}
fn open(window: HWND, name: String, words: [u32; 33], enabled: bool) -> usize {
    if !valid_words(&words)
        || (!window.is_null() && unsafe { IsWindow(window) } == 0)
        || !start_reader()
    {
        return 0;
    }
    let mut s = state();
    if s.contexts.len() >= MAX_CONTEXTS {
        return 0;
    }
    let Some(id) = session::add(&name, words, window, enabled) else {
        return 0;
    };
    let c = Context {
        revision: 0,
        cursor_mask: [255; 16],
        name,
        words: validated(words, enabled),
        window: window as usize,
        enabled,
        queue: VecDeque::new(),
        capacity: 128,
        previous: None,
        overflow: false,
        serial: 0,
    };
    s.contexts.insert(id, c);
    session::sync(&mut s);
    id
}
/// # Safety
/// p must point to a readable and writable LOGCONTEXTA for this call.
#[no_mangle]
pub unsafe extern "system" fn WTOpenA(w: HWND, p: *mut LogContextA, e: i32) -> usize {
    if p.is_null() {
        return 0;
    }
    let input = unsafe { ptr::read_unaligned(p) };
    let id = open(
        w,
        from_ansi(&input.name[..input.name.iter().position(|&v| v == 0).unwrap_or(40)]),
        input.words,
        e != 0,
    );
    if id != 0 {
        unsafe {
            WTGetA(id, p);
        }
    }
    id
}
/// # Safety
/// p must point to a readable and writable LOGCONTEXTW for this call.
#[no_mangle]
pub unsafe extern "system" fn WTOpenW(w: HWND, p: *mut LogContextW, e: i32) -> usize {
    if p.is_null() {
        return 0;
    }
    let input = unsafe { ptr::read_unaligned(p) };
    let id = open(
        w,
        String::from_utf16_lossy(
            &input.name[..input.name.iter().position(|&v| v == 0).unwrap_or(40)],
        ),
        input.words,
        e != 0,
    );
    if id != 0 {
        unsafe {
            WTGetW(id, p);
        }
    }
    id
}
#[no_mangle]
pub extern "system" fn WTClose(id: usize) -> i32 {
    if !session::close(id) {
        return 0;
    }
    let mut s = state();
    session::sync(&mut s);
    if s.capture == Some(id) {
        s.capture = None;
    }
    1
}
#[no_mangle]
pub extern "system" fn WTEnable(id: usize, on: i32) -> i32 {
    if !session::enable(id, on != 0) {
        return 0;
    }
    let mut s = state();
    session::sync(&mut s);
    if on == 0 && s.capture == Some(id) {
        s.capture = None;
    }
    1
}
#[no_mangle]
pub extern "system" fn WTOverlap(id: usize, top: i32) -> i32 {
    if !session::overlap(id, top != 0) {
        return 0;
    }
    session::sync(&mut state());
    1
}
/// # Safety
/// A non-null p must point to writable storage for one LOGCONTEXTA.
#[no_mangle]
pub unsafe extern "system" fn WTGetA(id: usize, p: *mut LogContextA) -> i32 {
    if p.is_null() {
        return 0;
    }
    let Some(c) = session::get(id) else {
        return 0;
    };
    unsafe {
        ptr::write_unaligned(
            p,
            LogContextA {
                name: name_a(&String::from_utf16_lossy(
                    &c.name[..c.name.iter().position(|&x| x == 0).unwrap_or(40)],
                )),
                words: public_words(c.words),
            },
        );
    }
    1
}
/// # Safety
/// A non-null p must point to writable storage for one LOGCONTEXTW.
#[no_mangle]
pub unsafe extern "system" fn WTGetW(id: usize, p: *mut LogContextW) -> i32 {
    if p.is_null() {
        return 0;
    }
    let Some(c) = session::get(id) else {
        return 0;
    };
    unsafe {
        ptr::write_unaligned(
            p,
            LogContextW {
                name: c.name,
                words: public_words(c.words),
            },
        );
    }
    1
}
fn set(id: usize, name: String, words: [u32; 33]) -> i32 {
    if id <= 2 || (0x7fff0001..=0x7fff0004).contains(&id) {
        return 0;
    }
    if !valid_words(&words) || !session::set(id, &name, words) {
        return 0;
    }
    session::sync(&mut state());
    1
}
/// # Safety
/// A non-null p must point to one initialized LOGCONTEXTA.
#[no_mangle]
pub unsafe extern "system" fn WTSetA(id: usize, p: *const LogContextA) -> i32 {
    if p.is_null() {
        return 0;
    }
    let p = unsafe { ptr::read_unaligned(p) };
    set(
        id,
        from_ansi(&p.name[..p.name.iter().position(|&v| v == 0).unwrap_or(40)]),
        p.words,
    )
}
/// # Safety
/// A non-null p must point to one initialized LOGCONTEXTW.
#[no_mangle]
pub unsafe extern "system" fn WTSetW(id: usize, p: *const LogContextW) -> i32 {
    if p.is_null() {
        return 0;
    }
    let p = unsafe { ptr::read_unaligned(p) };
    set(
        id,
        String::from_utf16_lossy(&p.name[..p.name.iter().position(|&v| v == 0).unwrap_or(40)]),
        p.words,
    )
}

fn coordinate(raw: i32, org: u32, extent: u32, out: u32, scale: u32) -> u32 {
    let input = f64::from(extent as i32);
    if input == 0.0 {
        return out;
    }
    let output = f64::from(scale as i32);
    let distance = f64::from(raw) - f64::from(org as i32);
    let fraction = if input.signum() == output.signum() {
        distance / input.abs()
    } else {
        1.0 - distance / input.abs()
    };
    let value = f64::from(out as i32) + fraction * output.abs();
    value
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32 as u32
}
fn relative_coordinate(raw: i32, old: i32, sensitivity: u32, extent: u32, out: u32) -> u32 {
    let sign = if (extent as i32).signum() == (out as i32).signum() {
        1.0
    } else {
        -1.0
    };
    ((f64::from(raw) - f64::from(old)) * f64::from(sensitivity) / 65536.0 * sign)
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32 as u32
}
fn pack(id: usize, c: &Context, p: &Packet) -> Vec<u8> {
    let mut out = Vec::new();
    let mask = c.words[6];
    let pen = p.pen;
    let previous = p.previous.unwrap_or(pen);
    let words = effective_words(c.words);
    let tx = f64::from(pen.tilt_x).to_radians().tan();
    let ty = -f64::from(pen.tilt_y).to_radians().tan();
    let az = if pen.tilt_x == 0 && pen.tilt_y == 0 {
        0
    } else {
        (tx.atan2(ty).to_degrees().rem_euclid(360.0) * 10.0).round() as u32
    };
    let alt = ((90.0 - tx.hypot(ty).atan().to_degrees()) * 10.0).round() as i32
        * if pen.flags & 4 != 0 { -1 } else { 1 };
    for bit in 0..=13 {
        if mask & (1 << bit) == 0 {
            continue;
        }
        if bit == 0 {
            out.extend_from_slice(&id.to_ne_bytes());
            continue;
        }
        let n = match bit {
            1 => p.status | if pen.flags & 4 != 0 { 16 } else { 0 },
            2 => {
                if c.words[7] & 4 != 0 {
                    pen.time.wrapping_sub(previous.time)
                } else {
                    pen.time
                }
            }
            3 => p.changed & mask,
            4 => p.serial,
            5 => u32::from(pen.flags & 4 != 0),
            6 => p.buttons,
            7..=9 => {
                let axis = bit - 7;
                let raw = [pen.x, 9200 - pen.y, 0][axis];
                let old = [previous.x, 9200 - previous.y, 0][axis];
                if c.words[7] & (1 << bit) != 0 {
                    relative_coordinate(
                        raw,
                        old,
                        c.words[23 + axis],
                        words[14 + axis],
                        words[20 + axis],
                    )
                } else {
                    coordinate(
                        raw,
                        words[11 + axis],
                        words[14 + axis],
                        words[17 + axis],
                        words[20 + axis],
                    )
                }
            }
            10 => {
                if c.words[7] & 0x400 != 0 {
                    (pen.pressure - previous.pressure) as u32
                } else {
                    pen.pressure.clamp(0, 4097) as u32
                }
            }
            11 => 0,
            12 => az,
            _ => 0,
        };
        out.extend_from_slice(&n.to_ne_bytes());
        if bit == 13 {
            out.extend_from_slice(&[0; 8]);
        }
        if bit == 12 {
            out.extend_from_slice(&alt.to_ne_bytes());
            out.extend_from_slice(&0u32.to_ne_bytes());
        }
    }
    let align = if mask & 1 != 0 { size_of::<usize>() } else { 4 };
    out.resize(out.len().div_ceil(align) * align, 0);
    out
}
unsafe fn packets(id: usize, max: i32, out: *mut c_void, remove: bool) -> i32 {
    if max <= 0 {
        return 0;
    }
    let mut s = state();
    session::sync(&mut s);
    let Some(c) = s.contexts.get_mut(&id) else {
        return 0;
    };
    let count = (max as usize).min(c.queue.len());
    let mut offset = 0;
    for p in c.queue.iter().take(count) {
        let b = pack(id, c, p);
        if !out.is_null() {
            unsafe {
                ptr::copy_nonoverlapping(b.as_ptr(), out.cast::<u8>().add(offset), b.len());
            }
        }
        offset += b.len();
    }
    if remove {
        c.queue.drain(..count);
        if c.queue.is_empty() {
            c.overflow = false;
        }
    }
    count as i32
}
/// # Safety
/// A non-null p must hold n packets using the context-selected native packet layout.
#[no_mangle]
pub unsafe extern "system" fn WTPacketsGet(id: usize, n: i32, p: *mut c_void) -> i32 {
    unsafe { packets(id, n, p, true) }
}
/// # Safety
/// A non-null p must hold n packets using the context-selected native packet layout.
#[no_mangle]
pub unsafe extern "system" fn WTPacketsPeek(id: usize, n: i32, p: *mut c_void) -> i32 {
    unsafe { packets(id, n, p, false) }
}
/// # Safety
/// A non-null out must hold one packet using the context-selected native packet layout.
#[no_mangle]
pub unsafe extern "system" fn WTPacket(id: usize, serial: u32, out: *mut c_void) -> i32 {
    let mut s = state();
    session::sync(&mut s);
    let Some(c) = s.contexts.get_mut(&id) else {
        return 0;
    };
    let Some(pos) = c.queue.iter().position(|p| p.serial == serial) else {
        return 0;
    };
    let b = pack(id, c, &c.queue[pos]);
    if !out.is_null() {
        unsafe {
            ptr::copy_nonoverlapping(b.as_ptr(), out.cast(), b.len());
        }
    }
    c.queue.drain(..=pos);
    1
}
#[no_mangle]
pub extern "system" fn WTQueueSizeGet(id: usize) -> i32 {
    session::get(id).map_or(0, |c| c.queue_capacity as i32)
}
#[no_mangle]
pub extern "system" fn WTQueueSizeSet(id: usize, n: i32) -> i32 {
    if !session::queue_capacity(id, 0) {
        return 0;
    }
    let mut s = state();
    session::sync(&mut s);
    if let Some(c) = s.contexts.get_mut(&id) {
        c.queue = VecDeque::new();
        c.overflow = false;
    }
    if !(1..=4096).contains(&n) {
        return 0;
    }
    if let Some(c) = s.contexts.get_mut(&id) {
        if c.queue.try_reserve_exact(n as usize).is_err() {
            return 0;
        }
    }
    if !session::queue_capacity(id, n as usize) {
        return 0;
    }
    session::sync(&mut s);
    1
}
/// # Safety
/// Non-null old and new must each point to writable u32 storage.
#[no_mangle]
pub unsafe extern "system" fn WTQueuePacketsEx(id: usize, old: *mut u32, new: *mut u32) -> i32 {
    let s = state();
    let Some(c) = s.contexts.get(&id) else {
        return 0;
    };
    let (Some(a), Some(b)) = (c.queue.front(), c.queue.back()) else {
        return 0;
    };
    unsafe {
        if !old.is_null() {
            ptr::write_unaligned(old, a.serial);
        }
        if !new.is_null() {
            ptr::write_unaligned(new, b.serial);
        }
    }
    1
}
#[no_mangle]
pub extern "system" fn WTConfig(id: usize, owner: HWND) -> i32 {
    let Some(record) = session::get(id) else {
        return 0;
    };
    if id <= 2 || (0x7fff0001..=0x7fff0004).contains(&id) {
        return 0;
    }
    let initial = (
        String::from_utf16_lossy(
            &record.name[..record.name.iter().position(|&c| c == 0).unwrap_or(40)],
        ),
        record.words,
    );
    match context_dialog::edit(owner, initial) {
        Some((name, mut words)) => {
            // WTConfig may not edit attributes locked by the owning client.
            let Some(c) = session::get(id) else {
                return 0;
            };
            words = apply_locks(&c.words, words);
            set(id, name, words)
        }
        None => 0,
    }
}
/// # Safety
/// data must hold 16 writable bytes for WTX_CSRMASK.
#[no_mangle]
pub unsafe extern "system" fn WTExtGet(id: usize, tag: u32, data: *mut c_void) -> i32 {
    if tag != 3 || data.is_null() {
        return 0;
    }
    let Some(c) = session::get(id) else {
        return 0;
    };
    unsafe {
        ptr::copy_nonoverlapping(c.cursor_mask.as_ptr(), data.cast(), 16);
    }
    1
}
/// # Safety
/// data must hold 16 readable bytes for WTX_CSRMASK.
#[no_mangle]
pub unsafe extern "system" fn WTExtSet(id: usize, tag: u32, data: *const c_void) -> i32 {
    if tag != 3 || data.is_null() {
        return 0;
    }
    let mask = unsafe { ptr::read_unaligned(data.cast::<[u8; 16]>()) };
    let changed = session::cursor_mask(id, mask);
    if changed {
        session::sync(&mut state());
    }
    i32::from(changed)
}

// Advanced range reads preserve independent context queues and handle wrapping serial numbers.
unsafe fn data_range(
    id: usize,
    begin: u32,
    end: u32,
    max: i32,
    out: *mut c_void,
    copied: *mut i32,
    remove: bool,
) -> i32 {
    if !copied.is_null() {
        unsafe {
            ptr::write_unaligned(copied, 0);
        }
    }
    if max < 0 {
        return 0;
    }
    let mut s = state();
    session::sync(&mut s);
    let Some(c) = s.contexts.get_mut(&id) else {
        return 0;
    };
    let selected: Vec<usize> = c
        .queue
        .iter()
        .enumerate()
        .filter(|(_, p)| p.serial.wrapping_sub(begin) <= end.wrapping_sub(begin))
        .map(|(i, _)| i)
        .collect();
    let count = selected.len().min(max as usize);
    let mut offset = 0;
    for &index in selected.iter().take(count) {
        let b = pack(id, c, &c.queue[index]);
        if !out.is_null() {
            unsafe {
                ptr::copy_nonoverlapping(b.as_ptr(), out.cast::<u8>().add(offset), b.len());
            }
        }
        offset += b.len();
    }
    if remove {
        for &index in selected.iter().rev() {
            c.queue.remove(index);
        }
    }
    if !copied.is_null() {
        unsafe {
            ptr::write_unaligned(copied, count as i32);
        }
    }
    selected.len() as i32
}
/// # Safety
/// A non-null p must hold n context packets; a non-null c must point to writable i32 storage.
#[no_mangle]
pub unsafe extern "system" fn WTDataGet(
    id: usize,
    b: u32,
    e: u32,
    n: i32,
    p: *mut c_void,
    c: *mut i32,
) -> i32 {
    unsafe { data_range(id, b, e, n, p, c, true) }
}
/// # Safety
/// A non-null p must hold n context packets; a non-null c must point to writable i32 storage.
#[no_mangle]
pub unsafe extern "system" fn WTDataPeek(
    id: usize,
    b: u32,
    e: u32,
    n: i32,
    p: *mut c_void,
    c: *mut i32,
) -> i32 {
    unsafe { data_range(id, b, e, n, p, c, false) }
}
fn checksum(data: &[u8]) -> u32 {
    data.iter().fold(2166136261u32, |h, b| {
        (h ^ u32::from(*b)).wrapping_mul(16777619)
    })
}
fn save_context(c: &Context) -> Vec<u8> {
    let mut data = b"OCTLWT03".to_vec();
    data.extend(name_w(&c.name).into_iter().flat_map(u16::to_le_bytes));
    data.extend(c.words.into_iter().flat_map(u32::to_le_bytes));
    data.extend((c.capacity as u32).to_le_bytes());
    data.extend(c.cursor_mask);
    data.extend(checksum(&data).to_le_bytes());
    debug_assert_eq!(data.len(), SAVE_SIZE);
    data
}
type SavedContext = (String, [u32; 33], usize, [u8; 16]);
fn restore_context(data: &[u8]) -> Option<SavedContext> {
    if !((data.len() == 228 && &data[..8] == b"OCTLWT02")
        || (data.len() == SAVE_SIZE && &data[..8] == b"OCTLWT03"))
    {
        return None;
    }
    let end = data.len() - 4;
    let checksum_expected = u32::from_le_bytes(data[end..].try_into().ok()?);
    if checksum(&data[..end]) != checksum_expected {
        return None;
    }
    let name: Vec<u16> = data[8..88]
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
        .take_while(|&n| n != 0)
        .collect();
    let mut words = [0; 33];
    for (word, b) in words.iter_mut().zip(data[88..220].chunks_exact(4)) {
        *word = u32::from_le_bytes(b.try_into().unwrap());
    }
    let capacity = u32::from_le_bytes(data[220..224].try_into().ok()?) as usize;
    if !valid_words(&words) || capacity > 4096 {
        return None;
    }
    let mask = if data.len() == SAVE_SIZE {
        data[224..240].try_into().ok()?
    } else {
        [255; 16]
    };
    Some((String::from_utf16_lossy(&name), words, capacity, mask))
}
/// # Safety
/// out must hold IFC_CTXSAVESIZE writable bytes.
#[no_mangle]
pub unsafe extern "system" fn WTSave(id: usize, out: *mut c_void) -> i32 {
    if out.is_null() {
        return 0;
    }
    let mut s = state();
    session::sync(&mut s);
    let Some(c) = s.contexts.get(&id) else {
        return 0;
    };
    let data = save_context(c);
    unsafe {
        ptr::copy_nonoverlapping(data.as_ptr(), out.cast(), data.len());
    }
    1
}
/// # Safety
/// data must point to IFC_CTXSAVESIZE readable bytes returned by WTSave.
#[no_mangle]
pub unsafe extern "system" fn WTRestore(window: HWND, data: *const c_void, enabled: i32) -> usize {
    if data.is_null() {
        return 0;
    }
    let header = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), 8) };
    let length = match header {
        b"OCTLWT02" => 228,
        b"OCTLWT03" => SAVE_SIZE,
        _ => return 0,
    };
    let input = unsafe { std::slice::from_raw_parts(data.cast(), length) };
    let Some((name, words, capacity, mask)) = restore_context(input) else {
        return 0;
    };
    let id = open(window, name, words, false);
    if id != 0 {
        session::cursor_mask(id, mask);
        session::queue_capacity(id, capacity);
        session::sync(&mut state());
        if enabled != 0 {
            WTEnable(id, 1);
        }
    }
    id
}
// Manager services are session-wide. Unsupported hardware extensions and legacy
// hook APIs return explicit failure; no success is fabricated for those calls.
mod context_dialog;
mod optional;
mod session;
pub mod system_output;
#[no_mangle]
pub extern "system" fn WacomCleanup() -> i32 {
    let ids: Vec<_> = state().contexts.keys().copied().collect();
    for id in ids {
        WTClose(id);
    }
    session::with(|d| {
        for m in &mut d.managers {
            if m.pid == std::process::id() {
                m.id = 0;
            }
        }
    });
    1
}

#[cfg(test)]
#[path = "../../debug/wintab_unit_tests.rs"]
mod mapping_tests;
