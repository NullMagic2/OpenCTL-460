//! Serializes processed pen frames using the same validated HID contract as the kernel.
pub use crate::wire::{PRESSURE_MAX, REPORT_DESCRIPTOR};
use crate::{
    engine::Frame,
    wire::{MAX_X, MAX_Y, REPORT_LEN},
};
pub const PRESSURE_LEVELS: u16 = PRESSURE_MAX + 1;
pub fn encode(frame: Frame) -> [u8; REPORT_LEN] {
    let contact = frame.in_range && frame.contact;
    let flags = u8::from(contact)
        | (u8::from(frame.in_range && frame.barrel) << 1)
        | (u8::from(frame.in_range && frame.eraser) << 2)
        | (u8::from(contact && frame.eraser) << 3)
        | (u8::from(frame.in_range) << 4);
    let x = frame.x.min(MAX_X).to_le_bytes();
    let y = frame.y.min(MAX_Y).to_le_bytes();
    let p = if contact { frame.virtual_pressure() } else { 0 }.to_le_bytes();
    [
        1, flags, x[0], x[1], y[0], y[1], p[0], p[1],
        // Reserved tilt bytes stay zero; preserve the installed HID contract.
        0, 0,
    ]
}
