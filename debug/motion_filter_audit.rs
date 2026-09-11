//! Reproducible synthetic RMS/geometry audit. Never opens a tablet or input sink.
use ctl460_rust::{config, line_smoothing::LineSmoothing};
#[path = "fixtures/line_filter_0_3_3.rs"]
mod baseline;
use std::{fs, path::PathBuf};
fn main() {
    let directory = PathBuf::from(std::env::args().nth(1).expect("Output directory required"));
    fs::create_dir_all(&directory).unwrap();
    let mut metrics = String::from(
        "wavelength_mm,raw_rms_mm,previous_rms_mm,current_rms_mm,previous_ratio,current_ratio\n",
    );
    let mut samples = String::from("wavelength_mm,x_mm,raw_y_mm,previous_y_mm,current_y_mm\n");
    for wavelength in [60.0, 120.0, 240.0, 600.0, 2000.0] {
        let c = config::Config {
            motion_filter_amount: 70.0,
            ..config::Config::default()
        };
        let mut before = baseline::LineSmoothing::default();
        before.reset();
        let mut after = LineSmoothing::default();
        let (mut raw_energy, mut old_energy, mut new_energy) = (0.0, 0.0, 0.0);
        for i in 0..1000 {
            let x = 2000 + i * 10;
            let y = (4000.0 + 15.0 * (f64::from(i * 10) * std::f64::consts::TAU / wavelength).sin())
                .round() as u16;
            let old = before.update(x, y, 8.0, &c);
            let new = after.update(x, y, 8.0, &c);
            let (raw, old, new) = (
                f64::from(y) - 4000.0,
                f64::from(old.1) - 4000.0,
                f64::from(new.1) - 4000.0,
            );
            if i >= 100 {
                raw_energy += raw * raw;
                old_energy += old * old;
                new_energy += new * new;
            }
            samples += &format!(
                "{},{},{},{},{}\n",
                wavelength / 100.0,
                f64::from(x) / 100.0,
                raw / 100.0,
                old / 100.0,
                new / 100.0
            );
        }
        let raw = (raw_energy / 900.0).sqrt() / 100.0;
        let old = (old_energy / 900.0).sqrt() / 100.0;
        let new = (new_energy / 900.0).sqrt() / 100.0;
        metrics += &format!(
            "{},{raw},{old},{new},{},{}\n",
            wavelength / 100.0,
            old / raw,
            new / raw
        );
    }
    fs::write(directory.join("motion-filter-metrics.csv"), &metrics).unwrap();
    fs::write(directory.join("motion-filter-samples.csv"), &samples).unwrap();
    print!("{metrics}");
}
