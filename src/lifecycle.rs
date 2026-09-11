//! Plans pen transitions and shared release coordinates independently of the Windows API.
//! The caller commits each event only after successful injection, enabling safe retry.

use crate::engine::Frame;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Down,
    Update,
    Up,
    Exit,
}

#[derive(Clone, Copy, Debug)]
pub struct Event {
    pub kind: Kind,
    pub new_pointer: bool,
    pub frame: Frame,
}

/// Finish contact at its last submitted location before moving or leaving in hover.
/// Shared by HID/WinTab and synthetic Ink; never extends contact after physical lift.
pub fn lift_before_hover(previous: Frame, next: Frame) -> Option<Frame> {
    (previous.contact && (!next.contact || !next.in_range)).then_some(Frame {
        contact: false,
        pressure: 0.0,
        barrel: false,
        ..previous
    })
}

pub fn transition(previous: Frame, next: Frame) -> Vec<Event> {
    let mut events = Vec::with_capacity(4);
    let mut old = previous;
    // A tool identity change must finish the old pointer before starting another.
    if old.in_range && next.in_range && old.eraser != next.eraser {
        end(&mut events, &mut old);
    }
    if !next.in_range {
        end(&mut events, &mut old);
        return events;
    }
    if let Some(up) = lift_before_hover(old, next) {
        events.push(Event {
            kind: Kind::Up,
            new_pointer: false,
            frame: up,
        });
        old = up;
    }
    events.push(Event {
        kind: if next.contact && !old.contact {
            Kind::Down
        } else {
            Kind::Update
        },
        new_pointer: !old.in_range,
        frame: next,
    });
    events
}

fn end(events: &mut Vec<Event>, old: &mut Frame) {
    if old.contact {
        old.contact = false;
        old.pressure = 0.0;
        old.barrel = false;
        events.push(Event {
            kind: Kind::Up,
            new_pointer: false,
            frame: *old,
        });
    }
    if old.in_range {
        old.in_range = false;
        old.barrel = false;
        old.pressure = 0.0;
        events.push(Event {
            kind: Kind::Exit,
            new_pointer: false,
            frame: *old,
        });
    }
}
