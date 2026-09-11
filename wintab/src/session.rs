//! Session-wide WinTab metadata. Fixed-width layout is shared by x86 and x64.
//! All access is serialized with an OS mutex; callbacks run only after unlocking.
use super::*;
use windows_sys::Win32::System::{Memory::*, Threading::*};
const MAGIC: u32 = 0x57543206;
pub const CONTEXTS: usize = 66;
pub const MANAGERS: usize = 16;
#[repr(C, align(8))]
#[derive(Clone, Copy)]
pub struct Record {
    pub window: u64,
    pub order: i64,
    pub id: u32,
    pub pid: u32,
    pub revision: u32,
    pub enabled: u32,
    pub name: [u16; 40],
    pub words: [u32; 33],
    pub queue_capacity: u32,
    pub cursor_mask: [u8; 16],
}
#[repr(C, align(8))]
#[derive(Clone, Copy)]
pub struct Manager {
    pub window: u64,
    pub id: u32,
    pub pid: u32,
    pub base: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Cursor {
    pub enabled: u32,
    pub button_map: [u8; 32],
    pub system_map: [u8; 32],
    pub marks: [u32; 2],
    pub response: [u32; 256],
    pub response_custom: u32,
}
#[repr(C, align(8))]
#[derive(Clone, Copy)]
pub struct Routed {
    sequence: u64,
    owner: u32,
    winner: u32,
    sample: [i32; 8],
}
#[repr(C, align(8))]
pub struct Data {
    magic: u32,
    next: u32,
    order: i64,
    pub revision: u32,
    reserved: u32,
    pub contexts: [Record; CONTEXTS],
    pub managers: [Manager; MANAGERS],
    pub cursors: [Cursor; 2],
    route_sequence: u64,
    route_owner: u32,
    capture: u32,
    previous_winner: u32,
    pressure_held: [u32; 2],
    previous: [i32; 8],
    routes: [Routed; 256],
    last_prune: u64,
    hardware_in_range: u32,
    pub out_of_bounds: u32,
    obt_context: u32,
}
struct Session {
    mapping: HANDLE,
    mutex: HANDLE,
    view: MEMORY_MAPPED_VIEW_ADDRESS,
}
// The handles and view live for the process; access to mapped data is mutex-protected.
unsafe impl Send for Session {}
impl Drop for Session {
    fn drop(&mut self) {
        unsafe {
            UnmapViewOfFile(self.view);
            CloseHandle(self.mapping);
            CloseHandle(self.mutex);
        }
    }
}
struct Unlock(HANDLE);
impl Drop for Unlock {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.0);
        }
    }
}
impl Session {
    fn new() -> Option<Self> {
        unsafe {
            let name: Vec<u16> = format!(
                "{}-WinTab-session-v6{}",
                ipc::NAME,
                if cfg!(test) {
                    format!("-unit-{}", std::process::id())
                } else {
                    String::new()
                }
            )
            .encode_utf16()
            .chain(Some(0))
            .collect();
            let mutex_name: Vec<u16> = format!(
                "{}-WinTab-lock-v6{}",
                ipc::NAME,
                if cfg!(test) {
                    format!("-unit-{}", std::process::id())
                } else {
                    String::new()
                }
            )
            .encode_utf16()
            .chain(Some(0))
            .collect();
            let mutex = CreateMutexW(ptr::null(), 0, mutex_name.as_ptr());
            if mutex.is_null() {
                return None;
            }
            let mapping = CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                ptr::null(),
                PAGE_READWRITE,
                0,
                size_of::<Data>() as u32,
                name.as_ptr(),
            );
            if mapping.is_null() {
                CloseHandle(mutex);
                return None;
            }
            let view = MapViewOfFile(
                mapping,
                FILE_MAP_READ | FILE_MAP_WRITE,
                0,
                0,
                size_of::<Data>(),
            );
            if view.Value.is_null() {
                CloseHandle(mapping);
                CloseHandle(mutex);
                return None;
            }
            Some(Self {
                mapping,
                mutex,
                view,
            })
        }
    }
}
pub fn with<R>(f: impl FnOnce(&mut Data) -> R) -> Option<R> {
    static SESSION: OnceLock<Mutex<Option<Session>>> = OnceLock::new();
    let guard = SESSION
        .get_or_init(|| Mutex::new(Session::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let session = guard.as_ref()?;
    let wait = unsafe { WaitForSingleObject(session.mutex, 1000) };
    if wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED {
        return None;
    }
    let _unlock = Unlock(session.mutex);
    let data = unsafe { &mut *session.view.Value.cast::<Data>() };
    if data.magic != MAGIC {
        // A fresh page mapping is zero-filled. Initialization is under the mutex.
        data.next = 3;
        data.revision = 1;
        data.order = 0;
        for (i, c) in data.contexts[..2].iter_mut().enumerate() {
            c.id = i as u32 + 1;
            c.cursor_mask = [255; 16];
            c.name = name_w("OpenCTL 460");
            c.words = default_words();
            if i == 1 {
                c.words[0] |= 1;
                c.words[17] = c.words[27];
                c.words[18] = c.words[28];
                c.words[20] = c.words[29];
                c.words[21] = c.words[30];
            }
        }
        for c in &mut data.cursors {
            c.enabled = 1;
            c.button_map = std::array::from_fn(|i| i as u8);
            c.system_map[0] = 1;
            c.system_map[1] = 4;
            c.marks = [0, 1];
            c.response = std::array::from_fn(|i| ((i * 4097 + 127) / 255) as u32);
        }
        data.magic = MAGIC;
    }
    let now = unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() };
    if now.saturating_sub(data.last_prune) >= 1000 {
        data.last_prune = now;
        data.prune();
    }
    Some(f(data))
}
impl Data {
    pub fn next_id(&mut self) -> u32 {
        loop {
            let id = self.next.max(3);
            self.next = id.wrapping_add(1) & 0x7fff_ffff;
            if !self.contexts.iter().any(|r| r.id == id)
                && !self.managers.iter().any(|r| r.id == id)
            {
                return id;
            }
        }
    }
    pub fn notify(&self, r: &Record, offset: u32, wp: usize, lp: isize) {
        if r.window != 0 && notification_allowed(r.words[0], offset) {
            unsafe {
                PostMessageW(r.window as HWND, r.words[3] + offset, wp, lp);
            }
        }
        if matches!(offset, 1..=4 | 6) {
            for m in self.managers.iter().filter(|m| m.id != 0 && m.window != 0) {
                unsafe {
                    PostMessageW(m.window as HWND, m.base + offset, wp, lp);
                }
            }
        }
    }
    pub fn information_changed(&mut self, manager: usize, category: u32, index: u32) {
        self.revision = self.revision.wrapping_add(1);
        let lp = ((category & 0xffff) | (index << 16)) as isize;
        for r in self.contexts.iter().filter(|r| r.id > 2 && r.window != 0) {
            unsafe {
                PostMessageW(r.window as HWND, r.words[3] + 6, manager, lp);
            }
        }
        for m in self.managers.iter().filter(|m| m.id != 0 && m.window != 0) {
            unsafe {
                PostMessageW(m.window as HWND, m.base + 6, manager, lp);
            }
        }
    }

    /// Match the foreground application's context group, preserving its internal overlap order.
    /// A background WinTab client must not starve the application the user is drawing in.
    pub fn foreground(&mut self, pid: u32) -> bool {
        if pid == 0 {
            return false;
        }
        let top = self.contexts[2..]
            .iter()
            .filter(|r| r.id != 0 && r.enabled != 0)
            .max_by_key(|r| r.order);
        if top.is_some_and(|r| r.pid == pid) {
            return false;
        }
        let mut group: Vec<_> = (2..CONTEXTS)
            .filter(|&i| {
                let r = &self.contexts[i];
                r.id != 0 && r.pid == pid && r.enabled != 0
            })
            .collect();
        if group.is_empty() {
            return false;
        }
        group.sort_by_key(|&i| self.contexts[i].order);
        for i in group {
            self.order += 1;
            self.contexts[i].order = self.order;
        }
        self.capture = 0;
        self.previous_winner = 0;
        self.status();
        true
    }
    pub fn status(&mut self) {
        let mut order: Vec<usize> = (2..CONTEXTS)
            .filter(|&i| self.contexts[i].id != 0)
            .collect();
        order.sort_by_key(|&i| -self.contexts[i].order);
        let mut higher = Vec::new();
        for i in order {
            let c = &mut self.contexts[i];
            let status = if c.enabled == 0 {
                1
            } else if higher.iter().any(|w| rectangles_overlap(w, &c.words)) {
                2
            } else {
                4
            };
            let changed = c.words[1] != status;
            c.words[1] = status;
            if c.enabled != 0 {
                higher.push(c.words);
            }
            let c = *c;
            if changed {
                self.notify(&c, 4, c.id as usize, status as isize);
            }
        }
    }
    pub fn prune(&mut self) {
        let mut pids: Vec<u32> = self
            .contexts
            .iter()
            .map(|r| r.pid)
            .chain(self.managers.iter().map(|r| r.pid))
            .filter(|&p| p != 0 && p != std::process::id())
            .collect();
        pids.sort_unstable();
        pids.dedup();
        for pid in pids {
            unsafe {
                let h = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
                // An inaccessible process is not assumed dead. Invalid PID or a
                // signaled process handle is conclusive; neither requires elevation.
                let dead = if h.is_null() {
                    GetLastError() == ERROR_INVALID_PARAMETER
                } else {
                    let dead = WaitForSingleObject(h, 0) == WAIT_OBJECT_0;
                    CloseHandle(h);
                    dead
                };
                if dead {
                    for i in 2..CONTEXTS {
                        if self.contexts[i].pid == pid && self.contexts[i].id != 0 {
                            let r = self.contexts[i];
                            self.contexts[i].id = 0;
                            self.notify(&r, 2, r.id as usize, r.words[1] as isize);
                        }
                    }
                    for m in &mut self.managers {
                        if m.pid == pid {
                            m.id = 0;
                        }
                    }
                }
            }
        }
        self.status();
    }
    pub fn can_edit(&self, record: &Record) -> bool {
        record.pid == std::process::id()
            || self
                .managers
                .iter()
                .any(|m| m.id != 0 && m.pid == std::process::id())
    }
    pub fn manager(&self, id: usize) -> bool {
        self.managers
            .iter()
            .any(|m| m.id as usize == id && m.id != 0 && m.pid == std::process::id())
    }
}
pub fn get(id: usize) -> Option<Record> {
    if (0x7fff0001..=0x7fff0004).contains(&id) {
        let mut r = get(1 + (id - 0x7fff0001) % 2)?;
        r.id = id as u32;
        r.words[4] = if id <= 0x7fff0002 { u32::MAX } else { 0 };
        return Some(r);
    }
    with(|d| {
        d.contexts
            .iter()
            .find(|r| r.id != 0 && r.id as usize == id)
            .copied()
    })
    .flatten()
}
pub fn defaults(system: bool) -> Option<Record> {
    get(if system { 2 } else { 1 })
}
pub fn add(name: &str, words: [u32; 33], window: HWND, enabled: bool) -> Option<usize> {
    with(|d| {
        d.prune();
        let i = (2..CONTEXTS).find(|&i| d.contexts[i].id == 0)?;
        let id = d.next_id();
        d.order += 1;
        let r = Record {
            id,
            pid: std::process::id(),
            window: window as u64,
            order: d.order,
            revision: 1,
            enabled: u32::from(enabled),
            name: name_w(name),
            words: validated(words, enabled),
            queue_capacity: 128,
            cursor_mask: [255; 16],
        };
        d.contexts[i] = r;
        d.notify(&r, 1, id as usize, r.words[1] as isize);
        d.status();
        Some(id as usize)
    })
    .flatten()
}
pub fn close(id: usize) -> bool {
    with(|d| {
        let Some(i) = (2..CONTEXTS).find(|&i| d.contexts[i].id as usize == id) else {
            return false;
        };
        let r = d.contexts[i];
        if r.id == 0 || !d.can_edit(&r) {
            return false;
        }
        d.contexts[i].id = 0;
        d.notify(&r, 2, id, r.words[1] as isize);
        d.status();
        true
    })
    .unwrap_or(false)
}
pub fn enable(id: usize, enabled: bool) -> bool {
    with(|d| {
        if !d
            .contexts
            .iter()
            .any(|r| r.id != 0 && r.id as usize == id && d.can_edit(r))
        {
            return false;
        }
        let Some(r) = d.contexts[2..]
            .iter_mut()
            .find(|r| r.id != 0 && r.id as usize == id)
        else {
            return false;
        };
        if r.enabled != u32::from(enabled) {
            r.enabled = u32::from(enabled);
            r.revision = r.revision.wrapping_add(1);
        }
        d.status();
        true
    })
    .unwrap_or(false)
}
pub fn overlap(id: usize, top: bool) -> bool {
    with(|d| {
        let Some(i) =
            (2..CONTEXTS).find(|&i| d.contexts[i].id != 0 && d.contexts[i].id as usize == id)
        else {
            return false;
        };
        if !d.can_edit(&d.contexts[i]) {
            return false;
        }
        if top {
            d.order += 1;
            d.contexts[i].order = d.order;
        } else {
            d.contexts[i].order = d.contexts[2..].iter().map(|r| r.order).min().unwrap_or(0) - 1;
        }
        d.status();
        true
    })
    .unwrap_or(false)
}
pub fn set(id: usize, name: &str, mut words: [u32; 33]) -> bool {
    with(|d| {
        let Some(i) = d
            .contexts
            .iter()
            .position(|r| r.id != 0 && r.id as usize == id)
        else {
            return false;
        };
        if i >= 2 && !d.can_edit(&d.contexts[i]) {
            return false;
        }
        let r = &mut d.contexts[i];
        if r.pid != 0 && r.pid != std::process::id() {
            words = apply_locks(&r.words, words);
        }
        r.name = name_w(name);
        r.words = validated(words, r.enabled != 0);
        r.revision = r.revision.wrapping_add(1);
        let r = *r;
        d.notify(&r, 3, id, r.words[1] as isize);
        d.status();
        if i < 2 {
            d.information_changed(0, if i == 0 { 3 } else { 4 }, 0);
        }
        true
    })
    .unwrap_or(false)
}
pub fn sync(s: &mut State) {
    let records = with(|d| d.contexts.to_vec()).unwrap_or_default();
    if records.is_empty() {
        return;
    }
    s.contexts.retain(|id, c| {
        let Some(r) = records.iter().find(|r| r.id as usize == *id && r.id != 0) else {
            return false;
        };
        let name =
            String::from_utf16_lossy(&r.name[..r.name.iter().position(|&c| c == 0).unwrap_or(40)]);
        // Status alone does not invalidate queued data.
        let mut old = c.words;
        old[1] = r.words[1];
        if old != r.words
            || c.enabled != (r.enabled != 0)
            || c.name != name
            || c.capacity != r.queue_capacity as usize
            || c.cursor_mask != r.cursor_mask
        {
            c.queue.clear();
            c.previous = None;
            c.overflow = false;
        }
        // A remote queue resize must discard packets even if the size is unchanged.
        if c.revision != r.revision {
            c.queue.clear();
            c.overflow = false;
        }
        c.revision = r.revision;
        c.cursor_mask = r.cursor_mask;
        c.name = name;
        c.words = r.words;
        c.enabled = r.enabled != 0;
        c.capacity = r.queue_capacity as usize;
        true
    });
    s.order = records
        .iter()
        .filter(|r| s.contexts.contains_key(&(r.id as usize)))
        .map(|r| r.id as usize)
        .collect();
    s.order
        .sort_by_key(|id| -records.iter().find(|r| r.id as usize == *id).unwrap().order);
}

fn decode(v: [i32; 8]) -> PenData {
    PenData {
        time: v[0] as u32,
        x: v[1],
        y: v[2],
        pressure: v[3],
        flags: v[4],
        tilt_x: v[5],
        tilt_y: v[6],
        raw: v[7],
    }
}
fn logical_buttons(p: PenData, cursors: &[Cursor; 2]) -> u32 {
    let c = &cursors[usize::from(p.flags & 4 != 0)];
    (0..2)
        .filter(|&i| p.flags & (1 << i) != 0)
        .fold(0, |bits, i| bits | (1 << c.button_map[i]))
}
fn apply_pressure(mut p: PenData, c: &Cursor, held: &mut u32) -> PenData {
    if c.enabled == 0 || !p.in_range() {
        *held = 0;
        return PenData {
            flags: 0,
            pressure: 0,
            ..p
        };
    }
    let pressure = p.pressure.clamp(0, 4097) as u32;
    *held = u32::from(
        c.marks[1] > c.marks[0]
            && if *held != 0 {
                pressure > c.marks[0]
            } else {
                pressure >= c.marks[1]
            },
    );
    p.flags = (p.flags & !1) | *held as i32;
    if c.response_custom != 0 {
        let position = pressure * 255;
        let index = (position / 4097) as usize;
        let f = position % 4097;
        p.pressure = ((u64::from(c.response[index]) * u64::from(4097 - f)
            + u64::from(c.response[(index + 1).min(255)]) * u64::from(f)
            + 2048)
            / 4097) as i32;
    }
    p
}
/// First reader arbitrates each sequence; all processes replay the same decision.
/// A bounded cache keeps a slower x86/x64 client from changing global capture state
/// by reprocessing an older event after another client has reached tip-up.
pub fn route(p: PenData, owner: u32, sequence: u64) -> Option<(PenData, usize)> {
    with(|d| {
        let slot = sequence as usize % 256;
        if d.routes[slot].sequence == sequence && d.routes[slot].owner == owner {
            let r = d.routes[slot];
            return Some((decode(r.sample), r.winner as usize));
        }
        if d.route_owner != owner {
            d.route_owner = owner;
            d.route_sequence = 0;
            d.capture = 0;
            d.previous = [0; 8];
            d.previous_winner = 0;
            d.pressure_held = [0; 2];
            d.hardware_in_range = 0;
            d.obt_context = 0;
            for r in &mut d.routes {
                r.sequence = 0;
                r.owner = 0;
            }
        }
        if sequence <= d.route_sequence {
            return None;
        }
        let mut foreground_pid = 0;
        unsafe {
            GetWindowThreadProcessId(GetForegroundWindow(), &mut foreground_pid);
        }
        let focus_changed = d.foreground(foreground_pid);
        let index = usize::from(p.flags & 4 != 0);
        let hardware_changed = d.hardware_in_range != u32::from(p.in_range());
        d.hardware_in_range = u32::from(p.in_range());
        let mut p = apply_pressure(p, &d.cursors[index], &mut d.pressure_held[index]);
        if hardware_changed {
            p.flags |= HARDWARE_PROXIMITY;
        }
        let previous = decode(d.previous);
        let old_buttons = logical_buttons(previous, &d.cursors);
        let buttons = logical_buttons(p, &d.cursors);
        let down = buttons & !old_buttons;
        let up = old_buttons & !buttons;
        let mut motion = 0;
        if p.x != previous.x {
            motion |= 0x80;
        }
        if p.y != previous.y {
            motion |= 0x100;
        }
        if p.pressure != previous.pressure {
            motion |= 0x400;
        }
        if (p.tilt_x, p.tilt_y) != (previous.tilt_x, previous.tilt_y) {
            motion |= 0x1000;
        }
        if !previous.in_range() || focus_changed {
            motion = SUPPORTED & !0x7f;
        }
        let captured = d.contexts[2..]
            .iter()
            .find(|r| {
                r.id != 0
                    && r.id == d.capture
                    && r.enabled != 0
                    && cursor_selected(&r.cursor_mask, p)
            })
            .copied();
        let winner = if !p.in_range() {
            None
        } else {
            captured
                .or_else(|| {
                    d.contexts[2..]
                        .iter()
                        .filter(|r| {
                            r.id != 0
                                && r.enabled != 0
                                && cursor_selected(&r.cursor_mask, p)
                                && context_sample(&r.words, p, false).is_some()
                                && (motion & r.words[8] & r.words[6] != 0
                                    || down & r.words[9] != 0
                                    || up & r.words[10] != 0)
                        })
                        .max_by_key(|r| r.order)
                        .copied()
                })
                .or_else(|| {
                    // A stationary sample keeps proximity, without fabricating a move.
                    d.contexts[2..]
                        .iter()
                        .find(|r| {
                            r.id != 0
                                && r.id == d.previous_winner
                                && r.enabled != 0
                                && cursor_selected(&r.cursor_mask, p)
                                && context_sample(&r.words, p, false).is_some()
                        })
                        .copied()
                })
        };
        let winner = winner.or_else(|| {
            if d.out_of_bounds == 0 || !p.in_range() {
                return None;
            }
            let eligible = |r: &&Record| {
                r.id != 0
                    && r.enabled != 0
                    && cursor_selected(&r.cursor_mask, p)
                    && r.words[8] & 0x380 != 0
            };
            // OBT only receives events that lie outside every eligible context.
            if d.contexts[2..]
                .iter()
                .filter(eligible)
                .any(|r| context_sample(&r.words, p, false).is_some())
            {
                return None;
            }
            let r = d.contexts[2..]
                .iter()
                .filter(eligible)
                .find(|r| r.id == d.obt_context)
                .or_else(|| {
                    d.contexts[2..]
                        .iter()
                        .filter(eligible)
                        .max_by_key(|r| r.order)
                })
                .copied();
            if r.is_some() {
                p.flags |= OUT_OF_BOUNDS;
            }
            r
        });
        if let Some(r) = winner.filter(|r| r.words[8] & 0x380 != 0) {
            d.obt_context = r.id;
        }
        let winner = winner.map_or(0, |r| r.id);
        d.capture = if let Some(r) = d.contexts[2..].iter().find(|r| r.id != 0 && r.id == winner) {
            if buttons & r.words[9] & r.words[10] != 0 {
                winner
            } else {
                0
            }
        } else {
            0
        };
        d.previous = p.values();
        d.previous_winner = winner;
        d.route_sequence = sequence;
        d.routes[slot] = Routed {
            sequence,
            owner,
            winner,
            sample: p.values(),
        };
        Some((p, winner as usize))
    })
    .flatten()
}

pub fn queue_capacity(id: usize, capacity: usize) -> bool {
    with(|d| {
        let Some(i) = (2..CONTEXTS).find(|&i| {
            d.contexts[i].id != 0 && d.contexts[i].id as usize == id && d.can_edit(&d.contexts[i])
        }) else {
            return false;
        };
        d.contexts[i].queue_capacity = capacity as u32;
        d.contexts[i].revision = d.contexts[i].revision.wrapping_add(1);
        true
    })
    .unwrap_or(false)
}

pub fn cursor_mask(id: usize, mask: [u8; 16]) -> bool {
    with(|d| {
        let Some(i) = (2..CONTEXTS).find(|&i| {
            d.contexts[i].id as usize == id && d.contexts[i].id != 0 && d.can_edit(&d.contexts[i])
        }) else {
            return false;
        };
        d.contexts[i].cursor_mask = mask;
        d.contexts[i].revision = d.contexts[i].revision.wrapping_add(1);
        let r = d.contexts[i];
        d.notify(&r, 3, id, r.words[1] as isize);
        true
    })
    .unwrap_or(false)
}

#[cfg(test)]
mod foreground_tests {
    use super::*;
    #[test]
    fn foreground_app_reclaims_contexts_from_a_background_drawing_app() {
        let mut d: Box<Data> = unsafe { Box::new(std::mem::zeroed()) };
        for (index, pid, order) in [(2, 100, 1), (3, 100, 2), (4, 200, 3)] {
            d.contexts[index] = Record {
                id: index as u32 + 1,
                pid,
                order,
                enabled: 1,
                words: default_words(),
                ..unsafe { std::mem::zeroed() }
            };
        }
        d.order = 3;
        d.capture = 5;
        assert!(d.foreground(100));
        assert!(d.contexts[3].order > d.contexts[2].order);
        assert!(d.contexts[2].order > d.contexts[4].order);
        assert_eq!(d.contexts[3].words[1], 4);
        assert_eq!(d.contexts[4].words[1], 2);
        assert_eq!(d.capture, 0);
        assert!(!d.foreground(100));
        assert!(!d.foreground(0));
        assert!(!d.foreground(300));
        assert!(d.foreground(200));
        assert_eq!(d.contexts[4].words[1], 4);
    }
}
