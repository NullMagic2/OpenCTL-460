//! Exports measured attenuation, broad-curve response and release-build CPU cost.
use ctl460_rust::{config::Config, line_smoothing::LineSmoothing};
use std::{fs, io::Write, path::PathBuf, time::Instant};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = PathBuf::from(std::env::args().nth(1).ok_or("Provide output directory")?);
    fs::create_dir_all(&out)?;
    let mut file = fs::File::create(out.join("frequency-response.csv"))?;
    writeln!(file, "amount,expression,wavelength_mm,rms_ratio")?;
    for amount in [0.0, 25.0, 50.0, 75.0, 100.0] {
        for expression in [0.0, 100.0] {
            for wavelength in [60.0, 120.0, 240.0, 600.0, 2000.0] {
                let c = Config {
                    motion_filter_amount: amount,
                    motion_filter_expression: expression,
                    endpoint_settling: false,
                    ..Default::default()
                };
                let mut filter = LineSmoothing::default();
                let (mut input, mut output) = (0.0, 0.0);
                for i in 0..1200 {
                    let y = (4500.0
                        + 15.0 * (i as f64 * 10.0 * std::f64::consts::TAU / wavelength).sin())
                    .round() as u16;
                    let p = filter.update(1500 + i * 10, y, 8.0, &c);
                    if i > 400 {
                        input += (f64::from(y) - 4500.0).powi(2);
                        output += (f64::from(p.1) - 4500.0).powi(2);
                    }
                }
                writeln!(
                    file,
                    "{amount},{expression},{},{:.8}",
                    wavelength / 100.0,
                    (output / input).sqrt()
                )?;
            }
        }
    }
    let c = Config {
        motion_filter_amount: 100.0,
        streamline_amount: 100.0,
        stabilization_amount: 100.0,
        ..Default::default()
    };
    let mut f = LineSmoothing::default();
    let start = Instant::now();
    let mut checksum = 0u64;
    for i in 0..20000 {
        let p = f.update(2000 + (i % 1000) * 10, 4500 + (i % 2) * 20, 8.0, &c);
        checksum += u64::from(p.0);
    }
    let elapsed = start.elapsed().as_secs_f64();
    let report=format!("20,000 reports; all three controls at 100%; {:.3} us/report average; checksum {checksum}\n",elapsed*1e6/20000.0);
    print!("{report}");
    fs::write(out.join("performance.txt"), report)?;
    Ok(())
}
