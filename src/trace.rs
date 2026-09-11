//! Opt-in, bounded raw-report recording. The pen loop only tries a bounded queue;
//! file writes run on a worker. No second tablet handle or WinTab ABI change.
use crate::{config::Config, engine::Frame, protocol::Sample};
use std::{
    fs::OpenOptions,
    io::{BufWriter, Write},
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{sync_channel, SyncSender},
        Arc,
    },
    thread::{self, JoinHandle},
};

enum Event {
    Report {
        sequence: u64,
        acquisition_ms: f64,
        submit_ms: f64,
        raw: Sample,
        effective: Sample,
        frame: Frame,
    },
    Config(Config),
}

pub struct Trace {
    sender: Option<SyncSender<Event>>,
    worker: Option<JoinHandle<std::io::Result<()>>>,
    sequence: u64,
    dropped: Arc<AtomicU64>,
    first_ms: Option<f64>,
}

impl Trace {
    pub fn start(path: &Path, config: &Config) -> Result<Self, String> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        let (sender, receiver) = sync_channel(4096);
        let initial = config.clone();
        let dropped = Arc::new(AtomicU64::new(0));
        let worker_dropped = Arc::clone(&dropped);
        let worker = thread::Builder::new().name("ctl460-trace".into()).spawn(move || {
            let mut out = BufWriter::new(file);
            writeln!(out, "# CTL460 trace v1; version={}; acquisition is host HID read completion; submit is driver submission return, not display time", crate::VERSION)?;
            fn profile(out: &mut impl Write, config: &Config) -> std::io::Result<()> {
                writeln!(out, "# config-begin")?;
                for line in toml::to_string(config).map_err(std::io::Error::other)?.lines() {
                    writeln!(out, "# {line}")?;
                }
                writeln!(out, "# config-end")
            }
            profile(&mut out, &initial)?;
            writeln!(out, "sequence,acquisition_ms,submit_ms,raw_x,raw_y,raw_pressure,in_range,position_valid,tip,raw_barrel,second_button,raw_eraser,effective_barrel,effective_eraser,filtered_x,filtered_y,pressure,contact,handwriting")?;
            while let Ok(event) = receiver.recv() {
                match event {
                    Event::Config(config) => profile(&mut out, &config)?,
                    Event::Report {sequence, acquisition_ms, submit_ms, raw, effective, frame} => {
                        writeln!(out, "{sequence},{acquisition_ms:.6},{submit_ms:.6},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.9},{},{}",
                            raw.x,raw.y,raw.pressure,u8::from(raw.in_range),u8::from(raw.position_valid),
                            u8::from(raw.tip),u8::from(raw.barrel),u8::from(raw.second_button),u8::from(raw.eraser),
                            u8::from(effective.barrel),u8::from(effective.eraser),frame.x,frame.y,frame.pressure,
                            u8::from(frame.contact),u8::from(frame.handwriting))?;
                        if sequence % 128 == 0 { out.flush()?; }
                    }
                }
            }
            writeln!(out, "# complete; dropped_reports={}", worker_dropped.load(Ordering::Relaxed))?;
            out.flush()
        }).map_err(|e| e.to_string())?;
        Ok(Self {
            sender: Some(sender),
            worker: Some(worker),
            sequence: 0,
            dropped,
            first_ms: None,
        })
    }
    pub fn reconfigure(&mut self, config: &Config) {
        if let Some(sender) = &self.sender {
            if sender.try_send(Event::Config(config.clone())).is_err() {
                // Never silently label subsequent reports with the wrong settings.
                self.sender = None;
                eprintln!("Trace ended: settings marker could not be recorded.");
            }
        }
    }
    pub fn record(
        &mut self,
        raw: Sample,
        effective: Sample,
        acquisition_ms: f64,
        submit_ms: f64,
        frame: Frame,
    ) {
        let first = *self.first_ms.get_or_insert(acquisition_ms);
        if acquisition_ms - first >= 300_000.0 || self.sequence >= 120_000 {
            if self.sender.take().is_some() {
                eprintln!("Trace completed its bounded capture; driver continues.");
            }
            return;
        }
        self.sequence += 1;
        if let Some(sender) = &self.sender {
            if sender
                .try_send(Event::Report {
                    sequence: self.sequence,
                    acquisition_ms,
                    submit_ms,
                    raw,
                    effective,
                    frame,
                })
                .is_err()
            {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}
impl Drop for Trace {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            match worker.join() {
                Ok(Ok(())) => eprintln!(
                    "Trace closed: {} queued-report drops (sequence gaps are visible).",
                    self.dropped.load(Ordering::Relaxed)
                ),
                Ok(Err(e)) => eprintln!("Trace write failed: {e}"),
                Err(_) => eprintln!("Trace writer stopped unexpectedly."),
            }
        }
    }
}
