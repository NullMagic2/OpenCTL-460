//! Read-only sensor/output/cursor comparison; no initialization, service lease or input injection.
use ctl460_rust::{
    ipc::Mapping,
    protocol::{self, ReportFormat},
};
use std::{
    io::{BufWriter, Write},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{POINT, RECT},
    UI::{HiDpi::*, WindowsAndMessaging::*},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let path = std::env::args().nth(1).ok_or("CSV path required")?;
    let mut out = BufWriter::new(
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?,
    );
    let api = hidapi::HidApi::new()?;
    let device = api
        .device_list()
        .find(|d| {
            d.vendor_id() == protocol::VENDOR_ID
                && d.product_id() == protocol::PRODUCT_ID
                && d.usage_page() == 0x0d
                && d.usage() == 1
        })
        .and_then(|d| d.open_device(&api).ok());
    let stream = Mapping::open_reader()?;
    if !stream.active() {
        return Err("Driver is not running".into());
    }
    unsafe {
        writeln!(
            out,
            "# primary={},{}; desktop={},{},{},{}; raw_reader={}",
            GetSystemMetrics(SM_CXSCREEN),
            GetSystemMetrics(SM_CYSCREEN),
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
            device.is_some()
        )?;
    }
    writeln!(out,"ms,sequence,raw_x,raw_y,raw_flags,output_x,output_y,flags,cursor_x,cursor_y,clip_left,clip_top,clip_right,clip_bottom")?;
    println!(
        "ARMED: waiting for pen activity, then recording for 45 seconds; input stays running."
    );
    let start = Instant::now();
    let mut recording: Option<Instant> = None;
    let mut previous = 0;
    let mut count = 0;
    while recording.map_or_else(
        || start.elapsed() < Duration::from_secs(180),
        |t| t.elapsed() < Duration::from_secs(45),
    ) {
        let mut raw = (-1, -1, -1);
        if let Some(d) = &device {
            let mut b = [0; 128];
            if let Ok(n) = d.read_timeout(&mut b, 2) {
                if let Ok(s) = protocol::decode(&b[..n], ReportFormat::Native9) {
                    raw = (i32::from(s.x), i32::from(s.y), i32::from(b[1]));
                }
            }
        } else {
            std::thread::sleep(Duration::from_millis(2));
        }
        let seq = stream.latest();
        if seq == previous && raw.2 < 0 {
            continue;
        }
        previous = seq;
        if let Some(p) = stream.read(seq) {
            if recording.is_none() && (p.in_range() || raw.2 >= 128) {
                recording = Some(Instant::now());
                println!("RECORDING STARTED: pen activity detected.");
            }
            let mut cursor = POINT::default();
            let mut clip = RECT::default();
            unsafe {
                GetCursorPos(&mut cursor);
                GetClipCursor(&mut clip);
            }
            writeln!(
                out,
                "{:.3},{seq},{},{},{},{},{},{},{},{},{},{},{},{}",
                start.elapsed().as_secs_f64() * 1000.,
                raw.0,
                raw.1,
                raw.2,
                p.x,
                p.y,
                p.flags,
                cursor.x,
                cursor.y,
                clip.left,
                clip.top,
                clip.right,
                clip.bottom
            )?;
            count += 1;
            if count % 250 == 0 {
                out.flush()?;
            }
        }
    }
    out.flush()?;
    println!("DONE: {count} samples");
    Ok(())
}
