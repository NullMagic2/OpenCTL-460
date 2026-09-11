//! Bounded, pen-only double-tap assistance in screen pixels. Never changes global mouse settings.
//! First strokes pass immediately. A nearby second tap is anchored to the first click until
//! it lifts, exceeds the selected motion radius, or exceeds the Windows double-click interval.
use crate::engine::Frame;

#[derive(Clone, Copy)]
struct Tap {
    frame: Frame,
    screen: (i32, i32),
    down_ms: u64,
    moved: bool,
}
#[derive(Default)]
pub struct DoubleClick {
    down: Option<Tap>,
    candidate: Option<Tap>,
    anchor: Option<Frame>,
}
impl DoubleClick {
    pub fn apply(
        &mut self,
        mut frame: Frame,
        screen: (i32, i32),
        now: u64,
        distance: u32,
        interval: u32,
    ) -> Frame {
        if distance == 0 || !frame.in_range || frame.eraser || frame.barrel || frame.handwriting {
            *self = Self::default();
            return frame;
        }
        let separation =
            |p: (i32, i32), q: (i32, i32)| f64::from(p.0 - q.0).hypot(f64::from(p.1 - q.1));
        if frame.contact && self.down.is_none() {
            self.anchor = self
                .candidate
                .take()
                .filter(|tap| {
                    now.saturating_sub(tap.down_ms) <= u64::from(interval)
                        && separation(screen, tap.screen) <= f64::from(distance)
                })
                .map(|tap| tap.frame);
            self.down = Some(Tap {
                frame,
                screen,
                down_ms: now,
                moved: false,
            });
        }
        if let Some(tap) = self.down.as_mut() {
            let travel = separation(screen, tap.screen);
            tap.moved |= travel > 3.0;
            if travel > f64::from(distance) || now.saturating_sub(tap.down_ms) > u64::from(interval)
            {
                self.anchor = None;
            }
            if let Some(anchor) = self.anchor {
                frame.x = anchor.x;
                frame.y = anchor.y;
            }
            if !frame.contact {
                // Consume a completed pair: a third tap begins a new pair.
                self.candidate = if self.anchor.is_none()
                    && !tap.moved
                    && now.saturating_sub(tap.down_ms) <= u64::from(interval)
                {
                    Some(*tap)
                } else {
                    None
                };
                self.down = None;
                self.anchor = None;
            }
        }
        frame
    }
}
