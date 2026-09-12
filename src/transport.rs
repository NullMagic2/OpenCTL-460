//! A dedicated HID reader feeds the output thread through a bounded FIFO.
//! Timestamped reports preserve edge order; overflow ends the session instead of dropping lifts.
//! No logging or pressure computation runs on the acquisition thread.

use crate::{output::Output, Options};
use ctl460_rust::{
    config::Config,
    engine::Engine,
    protocol::{self, ReportFormat, PRODUCT_ID, VENDOR_ID},
};
use hidapi::{HidApi, HidDevice};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, TrySendError},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use windows_sys::Win32::Media::{timeBeginPeriod, timeEndPeriod};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0},
    System::Threading::{OpenEventW, WaitForSingleObject, SYNCHRONIZATION_SYNCHRONIZE},
};

struct StopEvent(HANDLE);
impl StopEvent {
    fn open(name: Option<&str>) -> Result<Self, String> {
        if let Some(name) = name {
            let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
            // SAFETY: NUL-terminated name, read/synchronize access, handle closed in Drop.
            let handle = unsafe { OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, 0, wide.as_ptr()) };
            if handle.is_null() {
                return Err("Cannot open GUI stop event".into());
            }
            Ok(Self(handle))
        } else {
            Ok(Self(std::ptr::null_mut()))
        }
    }
    fn signaled(&self) -> bool {
        // SAFETY: Zero-time wait on a live handle, no blocking.
        !self.0.is_null() && unsafe { WaitForSingleObject(self.0, 0) } == WAIT_OBJECT_0
    }
}
impl Drop for StopEvent {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

struct Report {
    bytes: [u8; 128],
    len: usize,
    received: Instant,
}

struct Reader {
    running: Arc<AtomicBool>,
    overflow: Arc<AtomicBool>,
    thread: Option<JoinHandle<Result<(), String>>>,
}

impl Reader {
    fn start(
        device: HidDevice,
        running: Arc<AtomicBool>,
    ) -> Result<(Self, Receiver<Report>), String> {
        let (tx, rx) = mpsc::sync_channel(64);
        let active = Arc::clone(&running);
        let overflow = Arc::new(AtomicBool::new(false));
        let full = Arc::clone(&overflow);
        let thread = thread::Builder::new()
            .name("ctl460-hid".into())
            .spawn(move || {
                while active.load(Ordering::Relaxed) {
                    let mut bytes = [0u8; 128];
                    let len = device
                        .read_timeout(&mut bytes, 8)
                        .map_err(|e| format!("Tablet read failed/disconnected: {e}"))?;
                    if len == 0 {
                        continue;
                    }
                    let report = Report {
                        bytes,
                        len,
                        received: Instant::now(),
                    };
                    match tx.try_send(report) {
                        Ok(()) => (),
                        Err(TrySendError::Full(_)) => {
                            full.store(true, Ordering::Release);
                            return Err(
                                "Input queue overflow; stopping to preserve pen-state correctness"
                                    .into(),
                            );
                        }
                        Err(TrySendError::Disconnected(_)) => break,
                    }
                }
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        Ok((
            Self {
                running,
                overflow,
                thread: Some(thread),
            },
            rx,
        ))
    }

    fn finish(&mut self) -> Result<(), String> {
        self.running.store(false, Ordering::Relaxed);
        match self.thread.take() {
            Some(handle) => handle
                .join()
                .map_err(|_| "HID reader thread panicked".to_string())?,
            None => Ok(()),
        }
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

struct TimerResolution(bool);
impl TimerResolution {
    fn new() -> Self {
        // SAFETY: Requests a supported 1ms timer period; balanced in Drop, with no pointers.
        let enabled = unsafe { timeBeginPeriod(1) } == 0;
        if !enabled {
            eprintln!("1ms timer resolution unavailable; scheduler jitter may be higher.");
        }
        Self(enabled)
    }
}
impl Drop for TimerResolution {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: Paired with exactly one successful timeBeginPeriod(1).
            unsafe {
                timeEndPeriod(1);
            }
        }
    }
}

pub fn execute(opts: &Options, mut config: Config) -> Result<(), String> {
    let api = HidApi::new().map_err(|e| e.to_string())?;
    let devices: Vec<_> = api
        .device_list()
        .filter(|d| d.vendor_id() == VENDOR_ID && d.product_id() == PRODUCT_ID)
        .collect();
    if opts.command == "list" {
        for (index, device) in devices.iter().enumerate() {
            println!(
                "{index}: {} interface={} usage={:04X}:{:04X}\n   {}",
                device.product_string().unwrap_or("CTL-460"),
                device.interface_number(),
                device.usage_page(),
                device.usage(),
                device.path().to_string_lossy()
            );
        }
        if devices.is_empty() {
            println!("No CTL-460 (056A:00D4) HID collections found.");
        }
        return Ok(());
    }
    let explicit_index = if let Some(path) = &opts.device_path {
        Some(
            devices
                .iter()
                .position(|d| d.path().to_string_lossy() == path.as_str())
                .ok_or("Selected tablet collection disappeared; refresh the device list")?,
        )
    } else {
        opts.device
    };
    let index = if let Some(index) = explicit_index {
        index
    } else {
        ctl460_rust::device_selection::automatic_index(
            &devices
                .iter()
                .map(|d| (d.interface_number(), d.usage_page(), d.usage()))
                .collect::<Vec<_>>(),
        )?
    };
    let info = devices
        .get(index)
        .ok_or("Device index out of range; run list again")?;
    if (opts.initialize || opts.command == "run" || opts.command == "check")
        && !ctl460_rust::device_selection::is_pen_collection(
            info.interface_number(),
            info.usage_page(),
            info.usage(),
        )
    {
        return Err("Selected interface is not the CTL-460 pen. Select usage 000D:0001, or omit --device for automatic selection. The mouse interface rejects HidD_SetFeature with 'Incorrect function'.".into());
    }
    // The service acquires VHF and pauses competing Wacom services before physical initialization.
    let output = if opts.command == "run" {
        Some(Output::new(&config)?)
    } else {
        None
    };
    let device = info.open_device(&api).map_err(|e| format!("Cannot open selected HID collection: {e}. A competing tablet driver or exclusive Windows collection may own it."))?;
    if opts.initialize || opts.command == "check" || opts.command == "run" {
        ctl460_rust::tablet_mode::initialize(|| {
            device
                .send_feature_report(&[0x02, 0x02])
                .map_err(|e| e.to_string())?;
            // Match the collection's maximum feature size; report 2 itself returns two bytes.
            let mut reply = [0u8; 17];
            reply[0] = 2;
            let len = device
                .get_feature_report(&mut reply)
                .map_err(|e| e.to_string())?;
            Ok(reply[..len].to_vec())
        })?;
        eprintln!("Tablet confirmed pen mode (02 02); native input reports are 9 bytes.");
    }
    if opts.command == "check" {
        println!("CTL-460 pen interface 000D:0001 selected; pen mode confirmed by read-back. Report delivery has not been tested. No pen input was injected.");
        return Ok(());
    }
    let running = Arc::new(AtomicBool::new(true));
    let stop = Arc::clone(&running);
    if opts.stop_event.is_none() {
        ctrlc::set_handler(move || stop.store(false, Ordering::Relaxed))
            .map_err(|e| e.to_string())?;
    }
    let _timer = TimerResolution::new();
    config.legacy_reports |= opts.legacy;
    let mut period = Duration::from_secs_f64(1.0 / f64::from(config.output_hz));
    let mut stale = Duration::from_millis(u64::from(config.stale_ms));
    let mut output_config = config.clone();
    let mut buttons = crate::buttons::Buttons::new(config.button1.clone(), config.button2.clone());
    let stop_event = StopEvent::open(opts.stop_event.as_deref())?;
    let mut engine = Engine::new(config)?;
    let mut format = if output_config.legacy_reports {
        ReportFormat::Wacom11
    } else {
        ReportFormat::Native9
    };
    let start = Instant::now();
    let mut trace = if opts.command == "run" {
        opts.trace
            .as_ref()
            .map(|path| ctl460_rust::trace::Trace::start(path, &output_config))
            .transpose()?
    } else {
        None
    };
    let (mut reader, rx) = Reader::start(device, Arc::clone(&running))?;
    // Drop order matters: lift/destroy Ink before waiting for the reader on errors.
    let mut ink = output;
    // Parsing and file access never run on the HID acquisition/output path.
    let watcher = if opts.command == "run" {
        opts.config
            .clone()
            .map(ctl460_rust::live_config::Watcher::start)
            .transpose()?
    } else {
        None
    };
    let mut pending = None;
    let mut held_buttons = [false; 2];
    let mut next_output = start;
    let mut received_pen = false;
    let mut waiting_logged = false;
    let mut rejected = 0u64;
    let mut reports = 0u64;
    let mut total_us = 0u128;
    let mut maximum_us = 0u128;
    eprintln!(
        "{} on collection {index}; reader and output threads active. Ctrl+C to stop.",
        opts.command
    );
    while running.load(Ordering::Relaxed) {
        if stop_event.signaled() {
            break;
        }
        if reader.overflow.load(Ordering::Acquire) {
            return Err(
                "Input queue overflow; session stopped. Reduce system load and restart.".into(),
            );
        }
        if let Some(watcher) = &watcher {
            if let Some(update) = watcher.take() {
                pending = Some(update);
            }
            let frame = engine.tick(start.elapsed().as_secs_f64() * 1000.0);
            if !frame.in_range {
                held_buttons = [false; 2];
            }
            if !frame.contact && !held_buttons.iter().any(|held| *held) {
                if let Some(update) = pending.take() {
                    let mut next = update.config;
                    next.legacy_reports |= opts.legacy;
                    // Deliver any timeout/lift before changing backend or mapping.
                    if let Some(sink) = ink.as_mut() {
                        sink.submit(frame)?;
                    }
                    let result = ink.as_mut().map_or(Ok(()), |sink| sink.reconfigure(&next));
                    if result.is_ok() {
                        buttons.update([false; 2])?;
                        buttons = crate::buttons::Buttons::new(
                            next.button1.clone(),
                            next.button2.clone(),
                        );
                        engine.reconfigure(next.clone())?;
                        period = Duration::from_secs_f64(1.0 / f64::from(next.output_hz));
                        stale = Duration::from_millis(u64::from(next.stale_ms));
                        format = if next.legacy_reports {
                            ReportFormat::Wacom11
                        } else {
                            ReportFormat::Native9
                        };
                        output_config = next;
                        if let Some(trace) = &mut trace {
                            trace.reconfigure(&output_config);
                        }
                    }
                    if let Err(e) = &result {
                        eprintln!("Live settings: {e}");
                    }
                    watcher.report(update.revision, &result);
                }
            }
        }
        let wait = next_output.saturating_duration_since(Instant::now());
        match rx.recv_timeout(wait) {
            Ok(report) => {
                if report.received.elapsed() >= stale {
                    return Err("Queued input exceeded stale deadline; session stopped instead of replaying delayed ink".into());
                }
                let decoded = protocol::decode(&report.bytes[..report.len], format);
                if opts.command == "capture" {
                    println!(
                        "{:.3}ms {:02X?} {decoded:?}",
                        report.received.duration_since(start).as_secs_f64() * 1000.0,
                        &report.bytes[..report.len]
                    );
                }
                match decoded {
                    Ok(mut sample) => {
                        let raw_sample = sample;
                        if !received_pen {
                            eprintln!("Physical pen input received; report decoding active.");
                        }
                        received_pen = true;
                        if let Some(sink) = ink.as_mut() {
                            sink.telemetry(sample.pressure, report.received);
                        }
                        if ink.is_some() {
                            let states = [
                                sample.barrel && sample.in_range,
                                sample.second_button && sample.in_range,
                            ];
                            held_buttons = states;
                            sample.eraser |= output_config.button_eraser(states);
                            sample.barrel = states
                                .into_iter()
                                .zip([&output_config.button1, &output_config.button2])
                                .any(|(pressed, action)| pressed && action == "Right click");
                            // Erasing takes precedence over a simultaneous right-click assignment.
                            if sample.eraser {
                                sample.barrel = false;
                            }
                        }
                        engine.push(
                            sample,
                            report.received.duration_since(start).as_secs_f64() * 1000.0,
                        )?;
                        let processed = engine.tick(start.elapsed().as_secs_f64() * 1000.0);
                        // Submit each transition immediately; timer ticks add intermediate pressure.
                        if let Some(sink) = ink.as_mut() {
                            sink.set_precision(&output_config, held_buttons);
                            sink.submit(processed)?;
                            // Position the pen first so a side-button click uses this report location.
                            buttons.update(held_buttons)?;
                        }
                        if let Some(trace) = &mut trace {
                            trace.record(
                                raw_sample,
                                sample,
                                report.received.duration_since(start).as_secs_f64() * 1000.0,
                                start.elapsed().as_secs_f64() * 1000.0,
                                processed,
                            );
                        }
                        let us = report.received.elapsed().as_micros();
                        reports += 1;
                        total_us += us;
                        maximum_us = maximum_us.max(us);
                    }
                    Err(_) => {
                        rejected += 1;
                        if ink.is_some() && rejected == 1 {
                            eprintln!("Ignoring unexpected report. Inspect capture and verify collection/report format.");
                        }
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => (),
            Err(RecvTimeoutError::Disconnected) => {
                reader.finish()?;
                break;
            }
        }
        let now = Instant::now();
        if now >= next_output {
            if let Some(sink) = ink.as_mut() {
                if !received_pen {
                    // Stay discoverable while waiting, without generating fake HID/WinTab packets.
                    sink.waiting();
                } else {
                    let frame = engine.tick(start.elapsed().as_secs_f64() * 1000.0);
                    if !frame.in_range {
                        buttons.update([false, false])?;
                        held_buttons = [false; 2];
                        sink.set_precision(&output_config, [false; 2]);
                    }
                    sink.submit(frame)?;
                }
            }
            next_output = now + period; // No catch-up bursts after scheduling stalls.
        }
        // Diagnostic capture allows time to switch windows and begin a coordinated hardware test.
        let startup_seconds = if opts.command == "capture" { 60 } else { 15 };
        if !received_pen && start.elapsed() > Duration::from_secs(startup_seconds) {
            if rejected > 0 {
                return Err(format!("Received {rejected} reports, but none matched the selected format. For the native CTL-460, turn off Legacy 11-byte reports and restart."));
            }
            if opts.command == "capture" {
                return Err("No physical input during the 60-second capture. Reconnect the tablet and repeat the capture while moving the pen.".into());
            }
            if !waiting_logged {
                eprintln!("Waiting for physical tablet input; the feeder remains ready. If moving the pen produces nothing, reconnect the tablet and restart. Stop driver ends this wait.");
                waiting_logged = true;
            }
        }
    }
    drop(ink); // Explicit pen-up before joining the reader.
    reader.finish()?;
    if reports > 0 {
        eprintln!("Reports: {reports}; rejected: {rejected}; read-to-output processing: mean={}us max={maximum_us}us (excludes USB, display and app latency).", total_us / u128::from(reports));
    }
    Ok(())
}
