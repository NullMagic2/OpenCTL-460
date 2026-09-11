//! Validates live settings off the pen threads and exchanges only the newest update.
//! Atomic file replacement prevents partial settings; acknowledgements report actual application.
use crate::config::Config;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub fn fingerprint(contents: &str) -> u64 {
    contents.bytes().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

/// Replace in the same directory, preserving the previous valid file if writing fails.
pub fn write_atomic(path: &Path, contents: &str) -> Result<(), String> {
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    fs::write(&temporary, contents).map_err(|e| e.to_string())?;
    fs::rename(&temporary, path).map_err(|e| e.to_string())
}

#[derive(Clone)]
pub struct Update {
    pub config: Config,
    pub revision: u64,
}
#[derive(Default)]
struct Mailbox {
    latest: Option<Update>,
    status: Option<String>,
}

pub struct Watcher {
    mailbox: Arc<Mutex<Mailbox>>,
    running: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Watcher {
    pub fn start(path: PathBuf) -> Result<Self, String> {
        let mailbox = Arc::new(Mutex::new(Mailbox::default()));
        let shared = Arc::clone(&mailbox);
        let running = Arc::new(AtomicBool::new(true));
        let active = Arc::clone(&running);
        let worker = thread::Builder::new().name("ctl460-settings".into()).spawn(move || {
            let mut previous = String::new();
            let status_path = path.with_file_name("live-settings-status.txt");
            while active.load(Ordering::Relaxed) {
                // GUI writes are small. Limit size before parsing external edits as well.
                if fs::metadata(&path).is_ok_and(|m| m.len() <= 65536) {
                    if let Ok(contents) = fs::read_to_string(&path) {
                        if contents != previous {
                            let revision = fingerprint(&contents);
                            let parsed = Config::parse(&contents);
                            if let Ok(mut mail) = shared.lock() {
                                match parsed {
                                    Ok(config) => {
                                        mail.latest = Some(Update {config, revision});
                                        mail.status = Some(format!("{revision:016x}\nChanges saved; applying when the pen is lifted."));
                                    }
                                    Err(error) => {
                                        mail.latest = None;
                                        mail.status = Some(format!("{revision:016x}\nSettings not applied: {error}"));
                                    }
                                }
                            }
                            previous = contents;
                        }
                    }
                }
                let status = shared.lock().ok().and_then(|mut m| m.status.take());
                if let Some(status) = status { let _ = write_atomic(&status_path, &status); }
                thread::park_timeout(Duration::from_millis(100));
            }
        }).map_err(|e| e.to_string())?;
        Ok(Self {
            mailbox,
            running,
            worker: Some(worker),
        })
    }
    /// Never wait for disk IO or a settings lock on the output thread.
    pub fn take(&self) -> Option<Update> {
        self.mailbox
            .try_lock()
            .ok()
            .and_then(|mut m| m.latest.take())
    }
    pub fn report(&self, revision: u64, result: &Result<(), String>) {
        if let Ok(mut mail) = self.mailbox.lock() {
            let message = match result {
                Ok(()) => "Settings applied.".to_owned(),
                Err(e) => format!("Settings not applied; previous settings remain active: {e}"),
            };
            mail.status = Some(format!("{revision:016x}\n{message}"));
        }
    }
}
impl Drop for Watcher {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            let _ = worker.join();
        }
    }
}
