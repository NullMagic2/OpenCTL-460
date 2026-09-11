//! Versioned, bounded, atomic shared-memory fan-out for WinTab clients and status viewers.
//! One writer is enforced by a named mutex. Readers open read-only mappings in the same session.
//! Fixed-width atomics and explicit alignment give identical layouts in x86/x64 processes.

use std::{
    mem::size_of,
    ptr::null,
    sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering},
};
use windows_sys::Win32::{
    Foundation::*,
    System::{Memory::*, SystemInformation::GetTickCount64, Threading::*},
};
const MAGIC: u32 = 0x43544c32;
// Windows mutexes allow recursive acquisition on the same thread; reject duplicate local writers.
static LOCAL_WRITER: AtomicBool = AtomicBool::new(false);
pub const CAPACITY: usize = 256;
// Test builds can use an isolated compile-time namespace; release defaults remain fixed.
pub const NAME: &str = match option_env!("CTL460_TEST_STREAM_NAME") {
    Some(name) => name,
    None => "Local\\CTL460StudioStream-v1",
};
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct PenData {
    pub time: u32,
    pub x: i32,
    pub y: i32,
    pub pressure: i32,
    pub flags: i32,
    pub tilt_x: i32,
    pub tilt_y: i32,
    pub raw: i32,
}
impl PenData {
    /// Bits 16-17 carry configured mode independently of bit 5 (current stroke assistance).
    /// Zero means an older feeder did not provide the mode; existing WinTab fields are unchanged.
    pub fn handwriting_mode_flags(mode: &str) -> i32 {
        match mode {
            "off" => 1 << 16,
            "on" => 2 << 16,
            "auto" => 3 << 16,
            _ => 0,
        }
    }
    pub fn values(self) -> [i32; 8] {
        [
            self.time as i32,
            self.x,
            self.y,
            self.pressure,
            self.flags,
            self.tilt_x,
            self.tilt_y,
            self.raw,
        ]
    }
    fn from_values(v: [i32; 8]) -> Self {
        Self {
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
    pub fn in_range(self) -> bool {
        self.flags & 8 != 0
    }
}
#[repr(C, align(8))]
struct Slot {
    sequence: AtomicU64,
    values: [AtomicI32; 8],
}
#[repr(C, align(8))]
struct Stream {
    magic: AtomicU32,
    owner: AtomicU32,
    heartbeat: AtomicU64,
    latest: AtomicU64,
    backend: AtomicU32,
    hz: AtomicU32,
    latency_us: AtomicU32,
    reserved: AtomicU32,
    slots: [Slot; CAPACITY],
}
const _: () = assert!(size_of::<Slot>() == 40 && size_of::<Stream>() == 10280);

pub struct Mapping {
    handle: HANDLE,
    view: MEMORY_MAPPED_VIEW_ADDRESS,
    mutex: HANDLE,
    writer: bool,
}
// Intentionally !Send: Windows mutex ownership remains on the thread that created the writer.
impl Mapping {
    pub fn open_reader() -> Result<Self, String> {
        Self::open(false)
    }
    pub fn create_writer() -> Result<Self, String> {
        if LOCAL_WRITER
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err("A writer already exists in this process".into());
        }
        let result = Self::open(true);
        if result.is_err() {
            LOCAL_WRITER.store(false, Ordering::SeqCst);
        }
        result
    }
    fn open(writer: bool) -> Result<Self, String> {
        // SAFETY: Names are NUL-terminated; handles/views are owned and closed in Drop.
        unsafe {
            let mut mutex = std::ptr::null_mut();
            if writer {
                let writer_name = match option_env!("CTL460_TEST_STREAM_NAME") {
                    Some(name) => format!("{name}-Writer"),
                    None => "Local\\CTL460StudioWriter-v1".to_string(),
                };
                mutex = CreateMutexW(null(), 0, wide(&writer_name).as_ptr());
                if mutex.is_null() {
                    return Err("Cannot create stream ownership mutex".into());
                }
                let result = WaitForSingleObject(mutex, 0);
                if result != WAIT_OBJECT_0 && result != WAIT_ABANDONED {
                    CloseHandle(mutex);
                    return Err("Another CTL-460 driver is already running in this session".into());
                }
            }
            let handle = if writer {
                CreateFileMappingW(
                    INVALID_HANDLE_VALUE,
                    null(),
                    PAGE_READWRITE,
                    0,
                    size_of::<Stream>() as u32,
                    wide(NAME).as_ptr(),
                )
            } else {
                OpenFileMappingW(FILE_MAP_READ, 0, wide(NAME).as_ptr())
            };
            if handle.is_null() {
                if !mutex.is_null() {
                    ReleaseMutex(mutex);
                    CloseHandle(mutex);
                }
                return Err("Driver stream is not available".into());
            }
            let view = MapViewOfFile(
                handle,
                if writer {
                    FILE_MAP_WRITE | FILE_MAP_READ
                } else {
                    FILE_MAP_READ
                },
                0,
                0,
                size_of::<Stream>(),
            );
            if view.Value.is_null() {
                CloseHandle(handle);
                if !mutex.is_null() {
                    ReleaseMutex(mutex);
                    CloseHandle(mutex);
                }
                return Err("Cannot map driver stream".into());
            }
            let result = Self {
                handle,
                view,
                mutex,
                writer,
            };
            if writer {
                let s = result.stream();
                s.magic.store(0, Ordering::SeqCst);
                s.latest.store(0, Ordering::SeqCst);
                for slot in &s.slots {
                    slot.sequence.store(0, Ordering::SeqCst);
                    for v in &slot.values {
                        v.store(0, Ordering::SeqCst);
                    }
                }
                s.owner.store(std::process::id(), Ordering::SeqCst);
                s.heartbeat.store(GetTickCount64(), Ordering::SeqCst);
                s.magic.store(MAGIC, Ordering::SeqCst);
            }
            Ok(result)
        }
    }
    fn stream(&self) -> &Stream {
        // SAFETY: View is page-aligned, fixed-length, and alive. All-bit-zero is valid atomic storage.
        unsafe { &*self.view.Value.cast::<Stream>() }
    }
    pub fn active(&self) -> bool {
        let s = self.stream();
        s.magic.load(Ordering::SeqCst) == MAGIC
            && s.owner.load(Ordering::SeqCst) != 0
            && unsafe { GetTickCount64() }.saturating_sub(s.heartbeat.load(Ordering::SeqCst)) < 1000
    }
    pub fn latest(&self) -> u64 {
        self.stream().latest.load(Ordering::SeqCst)
    }
    /// Keep a waiting feeder visible without publishing an invented pen sample.
    pub fn heartbeat(&self) {
        if self.writer {
            self.stream()
                .heartbeat
                .store(unsafe { GetTickCount64() }, Ordering::SeqCst);
        }
    }
    pub fn publish(&self, data: PenData) {
        if !self.writer {
            return;
        }
        let s = self.stream();
        let number = s.latest.load(Ordering::SeqCst) + 1;
        let slot = &s.slots[number as usize % CAPACITY];
        slot.sequence.store(number * 2 + 1, Ordering::SeqCst);
        for (dest, value) in slot.values.iter().zip(data.values()) {
            dest.store(value, Ordering::SeqCst);
        }
        slot.sequence.store(number * 2, Ordering::SeqCst);
        s.latest.store(number, Ordering::SeqCst);
        s.heartbeat
            .store(unsafe { GetTickCount64() }, Ordering::SeqCst);
    }
    pub fn read(&self, number: u64) -> Option<PenData> {
        if number == 0 {
            return None;
        }
        let slot = &self.stream().slots[number as usize % CAPACITY];
        if slot.sequence.load(Ordering::SeqCst) != number * 2 {
            return None;
        }
        let values = std::array::from_fn(|i| slot.values[i].load(Ordering::SeqCst));
        if slot.sequence.load(Ordering::SeqCst) != number * 2 {
            return None;
        }
        Some(PenData::from_values(values))
    }
    pub fn metadata(&self) -> (u32, u32, u32, u32) {
        let s = self.stream();
        (
            s.owner.load(Ordering::SeqCst),
            s.backend.load(Ordering::SeqCst),
            s.hz.load(Ordering::SeqCst),
            s.latency_us.load(Ordering::SeqCst),
        )
    }
    pub fn set_metadata(&self, backend: u32, hz: u32, latency: u32) {
        if self.writer {
            let s = self.stream();
            s.backend.store(backend, Ordering::SeqCst);
            s.hz.store(hz, Ordering::SeqCst);
            s.latency_us.store(latency, Ordering::SeqCst);
        }
    }
}
impl Drop for Mapping {
    fn drop(&mut self) {
        if self.writer {
            self.publish(PenData::default());
            self.stream().owner.store(0, Ordering::SeqCst);
            LOCAL_WRITER.store(false, Ordering::SeqCst);
        }
        // SAFETY: Each view/handle is closed exactly once. Writer is retained on its owning thread.
        unsafe {
            UnmapViewOfFile(self.view);
            CloseHandle(self.handle);
            if !self.mutex.is_null() {
                ReleaseMutex(self.mutex);
                CloseHandle(self.mutex);
            }
        }
    }
}
