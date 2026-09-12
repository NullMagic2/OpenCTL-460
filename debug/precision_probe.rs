//! Bounded physical report capture with the normal service handoff and no pen injection.
//! Records native buttons and replays the production precision transform after smoothing.
use ctl460_rust::{
    broker::Client,
    config::Config,
    engine::Engine,
    precision::{output_bounds, PrecisionHold},
    protocol::{self, ReportFormat},
};
use std::{
    fs::File,
    io::Write,
    time::{Duration, Instant},
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let config = Config::parse(&std::fs::read_to_string(&args[1])?)?;
    let mut csv = File::create(&args[2])?;
    // Same lease as the feeder: pauses the original services, restored on drop.
    let _lease = Client::connect()?;
    let api = hidapi::HidApi::new()?;
    let info = api
        .device_list()
        .find(|d| {
            d.vendor_id() == protocol::VENDOR_ID
                && d.product_id() == protocol::PRODUCT_ID
                && d.usage_page() == 0x0d
                && d.usage() == 1
        })
        .ok_or("No pen collection")?;
    let device = info.open_device(&api)?;
    ctl460_rust::tablet_mode::initialize(|| {
        device
            .send_feature_report(&[2, 2])
            .map_err(|e| e.to_string())?;
        let mut reply = [0; 17];
        reply[0] = 2;
        let len = device
            .get_feature_report(&mut reply)
            .map_err(|e| e.to_string())?;
        Ok(reply[..len].to_vec())
    })?;
    let bounds = unsafe {
        output_bounds(
            GetSystemMetrics(SM_CXSCREEN),
            GetSystemMetrics(SM_CYSCREEN),
            config.preserve_aspect,
        )
    };
    let mut engine = Engine::new(config.clone())?;
    let mut precision = PrecisionHold::default();
    let mut previous = [false; 2];
    let mut count = 0;
    writeln!(csv,"ms,flags,raw_x,raw_y,pressure,button1,button2,contact,enabled,active,filtered_x,filtered_y,output_x,output_y")?;
    println!("READY: service handoff acquired; recording for 120 seconds; no pen injection.");
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(120) {
        let mut bytes = [0; 128];
        let n = device.read_timeout(&mut bytes, 8)?;
        if n == 0 {
            continue;
        }
        let mut sample =
            protocol::decode(&bytes[..n], ReportFormat::Native9).map_err(|e| format!("{e:?}"))?;
        let raw = sample;
        let states = [
            sample.barrel && sample.in_range,
            sample.second_button && sample.in_range,
        ];
        sample.eraser |= config.button_eraser(states);
        sample.barrel = states
            .into_iter()
            .zip([&config.button1, &config.button2])
            .any(|(pressed, action)| pressed && action == "Right click")
            && !sample.eraser;
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        engine.push(sample, ms)?;
        precision.buttons(&config, states);
        let frame = engine.tick(ms);
        let output = precision.apply(frame, bounds);
        if states != previous {
            println!(
                "{ms:.0}ms flags={:02x} buttons={states:?} contact={} precision={}",
                bytes[1],
                frame.contact,
                precision.enabled()
            );
        }
        previous = states;
        writeln!(
            csv,
            "{ms:.3},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            bytes[1],
            raw.x,
            raw.y,
            raw.pressure,
            u8::from(states[0]),
            u8::from(states[1]),
            u8::from(frame.contact),
            u8::from(precision.enabled()),
            u8::from(precision.active()),
            frame.x,
            frame.y,
            output.x,
            output.y
        )?;
        count += 1;
    }
    println!("DONE: {count} reports recorded; restoring service handoff.");
    Ok(())
}
