//! Read-only CTL-460 descriptor/report diagnostic; never creates synthetic input.
//! Records up to 20 reports per collection during a bounded 45-second capture.
use hidapi::HidApi;
use std::{
    thread,
    time::{Duration, Instant},
};
fn main() {
    let mode_only = std::env::args().any(|arg| arg == "--mode-only");
    let api = HidApi::new().expect("HID enumeration failed");
    let mut readers = Vec::new();
    for (index, info) in api
        .device_list()
        .filter(|d| d.vendor_id() == 0x056a && d.product_id() == 0x00d4)
        .enumerate()
    {
        println!(
            "Collection {index}: interface {} usage {:04X}:{:04X}",
            info.interface_number(),
            info.usage_page(),
            info.usage()
        );
        let device = match info.open_device(&api) {
            Ok(d) => d,
            Err(e) => {
                println!("Open: {e}");
                continue;
            }
        };
        if info.usage_page() == 0x0d {
            let mut mode = [0u8; 17];
            mode[0] = 2;
            match device.get_feature_report(&mut mode) {
                Ok(n) => println!("Tablet mode {index}: {:02X?}", &mode[..n]),
                Err(e) => println!("Tablet mode {index}: {e}"),
            }
        }
        if mode_only {
            continue;
        }
        let mut descriptor = [0u8; 4096];
        match device.get_report_descriptor(&mut descriptor) {
            Ok(n) => println!("Descriptor {index}: {:02X?}", &descriptor[..n]),
            Err(e) => println!("Descriptor {index}: {e}"),
        }
        readers.push(thread::spawn(move || {
            let start = Instant::now();
            let mut count = 0;
            while start.elapsed() < Duration::from_secs(45) {
                let mut bytes = [0u8; 128];
                match device.read_timeout(&mut bytes, 20) {
                    Ok(0) => {}
                    Ok(n) => {
                        count += 1;
                        if count <= 20 {
                            println!("Report {index}: {:02X?}", &bytes[..n]);
                        }
                    }
                    Err(e) => {
                        println!("Read {index}: {e}");
                        break;
                    }
                }
            }
            println!("Collection {index}: {count} reports");
        }));
    }
    for reader in readers {
        reader.join().expect("Diagnostic reader failed");
    }
}
