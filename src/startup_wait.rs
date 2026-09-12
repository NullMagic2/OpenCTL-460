//! A bounded login readiness window. Never retries a launched or failed feeder.
use std::time::{Duration, Instant};
pub struct StartupWait {
    deadline: Option<Instant>,
    next: Instant,
}
impl StartupWait {
    pub fn new(now: Instant, login: bool) -> Self {
        Self {
            deadline: login.then_some(now + Duration::from_secs(60)),
            next: now,
        }
    }
    pub fn due(&self, now: Instant) -> bool {
        now >= self.next
    }
    /// Only an absent tablet may defer launch; other errors remain visible.
    pub fn defer_missing_device(&mut self, now: Instant) -> bool {
        if self.deadline.is_some_and(|end| now < end) {
            self.next = now + Duration::from_secs(2);
            true
        } else {
            self.cancel();
            false
        }
    }
    pub fn cancel(&mut self) {
        self.deadline = None;
    }
}
