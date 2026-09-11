//! Bounded read-only comparison of the feeder stream and Windows cursor near display edges.
//! Does not open the physical tablet, change settings or inject any input.
use ctl460_rust::ipc::Mapping;
use std::{
    io::{BufWriter, Write},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::POINT,
    UI::{HiDpi::*, WindowsAndMessaging::*},
};
fn main() -> std::io::Result<()> {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let file = std::env::args().nth(1).expect("Pass an output CSV path");
    let mut out = BufWriter::new(std::fs::File::create(file)?);
    writeln!(
        out,
        "ms,sequence,owner,backend,x,y,pressure,flags,cursor_ok,cursor_x,cursor_y"
    )?;
    let start = Instant::now();
    let mut stream = None;
    let mut last = 0;
    let mut count = 0;
    let mut range = 0;
    let mut bounds = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
    println!("Read-only edge capture ready; waiting up to 90 seconds.");
    while start.elapsed() < Duration::from_secs(90) {
        if stream.is_none() {
            stream = Mapping::open_reader().ok();
        }
        if let Some(m) = &stream {
            if m.active() {
                let seq = m.latest();
                if seq != last {
                    last = seq;
                    if let Some(p) = m.read(seq) {
                        let mut cursor = POINT::default();
                        let ok = unsafe { GetCursorPos(&mut cursor) };
                        let (owner, backend, _, _) = m.metadata();
                        writeln!(
                            out,
                            "{},{seq},{owner},{backend},{},{},{},{},{ok},{},{}",
                            start.elapsed().as_millis(),
                            p.x,
                            p.y,
                            p.pressure,
                            p.flags,
                            cursor.x,
                            cursor.y
                        )?;
                        count += 1;
                        if p.in_range() {
                            range += 1;
                            bounds[0] = bounds[0].min(p.x);
                            bounds[1] = bounds[1].min(p.y);
                            bounds[2] = bounds[2].max(p.x);
                            bounds[3] = bounds[3].max(p.y);
                        }
                        if count % 250 == 0 {
                            out.flush()?;
                        }
                    }
                }
            } else {
                stream = None;
                last = 0;
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    out.flush()?;
    println!("Samples={count}; in-range={range}; normalized bounds={bounds:?}");
    Ok(())
}
