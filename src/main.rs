//! Command-line entry point: safe device listing/capture, offline demo, and explicit Ink mode.
//! Nothing installs itself, changes system drivers, or injects input unless `run` is requested.

use ctl460_rust::{config::Config, engine::Engine, protocol::Sample};
use std::{env, fs, path::PathBuf, process::ExitCode};
#[cfg(windows)]
mod buttons;
#[cfg(windows)]
mod output;
#[cfg(windows)]
mod transport;
#[cfg(windows)]
mod windows;

#[derive(Debug)]
pub struct Options {
    pub command: String,
    pub device: Option<usize>,
    pub config: Option<PathBuf>,
    pub legacy: bool,
    pub initialize: bool,
    pub stop_event: Option<String>,
    pub device_path: Option<String>,
    pub trace: Option<PathBuf>,
}

fn options() -> Result<Options, String> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".into());
    if ![
        "help",
        "--help",
        "-h",
        "list",
        "capture",
        "run",
        "demo",
        "status",
        "check",
        "version",
        "--version",
        "-V",
    ]
    .contains(&command.as_str())
    {
        return Err(format!("Unknown command: {command}; use --help"));
    }
    let mut result = Options {
        command,
        device: None,
        config: None,
        legacy: false,
        initialize: false,
        stop_event: None,
        device_path: None,
        trace: None,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--device" => {
                result.device = Some(
                    args.next()
                        .ok_or("Missing --device index")?
                        .parse()
                        .map_err(|_| "Invalid device index")?,
                )
            }
            "--config" => {
                result.config = Some(args.next().ok_or("Missing --config filename")?.into())
            }
            "--trace" => result.trace = Some(args.next().ok_or("Missing trace filename")?.into()),
            "--wacom11" => result.legacy = true,
            "--init" => result.initialize = true,
            "--device-path" => result.device_path = Some(args.next().ok_or("Missing HID path")?),
            "--stop-event" => result.stop_event = Some(args.next().ok_or("Missing event name")?),
            _ => return Err(format!("Unknown option: {arg}")),
        }
    }
    if result.trace.is_some() && result.command != "run" {
        return Err("--trace is only supported with run".into());
    }
    Ok(result)
}

fn run() -> Result<(), String> {
    let opts = options()?;
    // Report this executable without reading settings, opening hardware or starting input.
    if ["version", "--version", "-V"].contains(&opts.command.as_str()) {
        println!("OpenCTL 460 {}", ctl460_rust::VERSION);
        return Ok(());
    }
    if ["help", "--help", "-h"].contains(&opts.command.as_str()) {
        println!(
            "CTL-460 Rust experimental user-mode driver\n\
Usage: ctl460-rust <list|capture|run|demo|status|version> [options]\n\
  --version, -V     Print the driver executable version and exit\n\
  list              List matching tablet HID collections; no device writes\n\
  check             Auto-select and initialize the pen; no input injection\n\
  capture           Print raw and decoded reports; no input injection\n\
  run               Read tablet and inject pressure into Windows Ink\n\
  demo              Print synthetic pressure ramp; no tablet or injection\n\
  status            Show capabilities and live feeder status (read-only)\n\
  --device N        Select the collection index from list (required if ambiguous)\n\
  --config FILE     Load a commented TOML profile; otherwise built-in defaults\n\
  --trace FILE      Record original reports during run (max 5 minutes of reports); never overwrites\n\
  --init            Send feature report [02,02] to enable tablet mode\n\
  --wacom11         Explicitly decode the legacy 11-byte Wacom wrapper\n\
Ctrl+C stops capture/run. Default output maps to the primary display.\n\
Virtual HID requires the separately built, signed and installed kernel package."
        );
        return Ok(());
    }
    #[cfg(windows)]
    if opts.command == "status" {
        println!("{}", ctl460_rust::status::summary());
        return Ok(());
    }
    let config = if let Some(path) = &opts.config {
        Config::parse(&fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?)?
    } else {
        Config::default()
    };
    if opts.command == "demo" {
        let mut engine = Engine::new(config)?;
        println!("# Synthetic demo, not captured hardware data. Virtual4097 is diagnostic only.");
        println!("time_ms,raw_pressure,virtual4097,windows_ink1024");
        for time in (0..=1000).step_by(2) {
            let raw = ((time.min(1000 - time) as f64 / 500.0) * 1023.0).round() as u16;
            if time % 8 == 0 {
                engine.push(
                    Sample {
                        pressure: raw,
                        tip: raw > 0,
                        position_valid: true,
                        in_range: true,
                        ..Default::default()
                    },
                    time as f64,
                )?;
            }
            let frame = engine.tick(time as f64);
            println!(
                "{time},{raw},{},{}",
                frame.virtual_pressure(),
                frame.ink_pressure()
            );
        }
        return Ok(());
    }
    #[cfg(windows)]
    {
        transport::execute(&opts, config)
    }
    #[cfg(not(windows))]
    {
        Err("Device commands require Windows; demo and tests are portable".into())
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {error}");
            ExitCode::FAILURE
        }
    }
}
