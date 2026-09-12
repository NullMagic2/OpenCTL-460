use ctl460_rust::startup_wait::StartupWait;
use std::time::{Duration, Instant};
#[test]
fn login_waits_for_late_usb_but_expires_and_stop_cancels_it() {
    let now = Instant::now();
    let mut wait = StartupWait::new(now, true);
    assert!(wait.due(now));
    assert!(wait.defer_missing_device(now));
    assert!(!wait.due(now + Duration::from_millis(150)));
    assert!(wait.due(now + Duration::from_secs(2)));
    assert!(wait.defer_missing_device(now + Duration::from_secs(58)));
    assert!(!wait.defer_missing_device(now + Duration::from_secs(60)));
    let mut stopped = StartupWait::new(now, true);
    stopped.cancel();
    assert!(!stopped.defer_missing_device(now));
    let mut manual = StartupWait::new(now, false);
    assert!(!manual.defer_missing_device(now));
}
