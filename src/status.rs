//! Read-only, live capabilities and feeder status shared by the CLI and native control panel.
//! A viewer never opens the physical pen for writing, injects input, or starts a driver.

use crate::ipc::Mapping;

/// Report the running feeder's mode separately from assistance on the current stroke.
pub fn handwriting_summary(flags: i32) -> String {
    let activity = if flags & 32 != 0 { "active" } else { "idle" };
    match (flags >> 16) & 3 {
        1 => "Handwriting: Off".into(),
        2 => format!("Handwriting: On (manual); stroke assistance {activity}"),
        3 => format!(
            "Handwriting: Auto; stroke assistance {activity}; evidence {}/100 (heuristic)",
            (flags >> 8) & 127
        ),
        _ => format!("Handwriting mode unavailable (older feeder); stroke assistance {activity}"),
    }
}

pub fn summary() -> String {
    let caps = "CTL-460: 1,024 measured pressure values; no measured tilt; two pen side buttons.\r\nSoftware: 4,098 pressure values (0-4097); synthetic Ink API uses 0-1024.";
    let Ok(stream) = Mapping::open_reader() else {
        return format!("{caps}\r\nFeeder stopped / unavailable in this Windows session.");
    };
    if !stream.active() {
        return format!("{caps}\r\nFeeder stopped or unresponsive; live values unavailable.");
    }
    let (pid, backend, hz, latency) = stream.metadata();
    let backend = match backend {
        1 => "Virtual HID",
        2 => "Synthetic Windows Ink",
        _ => "Starting",
    };
    let Some(pen) = stream.read(stream.latest()) else {
        return format!("{caps}\r\n{backend}: waiting for tablet input. Move the pen; if it stays unresponsive, reconnect the tablet and restart the driver. Feeder PID {pid}.");
    };
    let state = if pen.flags & 1 != 0 {
        "touching"
    } else if pen.in_range() {
        "hovering"
    } else {
        "out of range"
    };
    let writing = handwriting_summary(pen.flags);
    format!("{caps}\r\n{backend}, {hz} Hz target; pen {state}; pressure {}/1023 -> {}/4097.\r\n{writing}. Last read-to-submit: {latency} us. Feeder PID {pid}.",pen.raw,pen.pressure)
}
