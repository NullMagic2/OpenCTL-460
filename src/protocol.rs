//! Decodes CTL-460 USB pen reports using documented upstream protocol facts.
//! See docs/SOURCES.md. No Linux or OpenTabletDriver implementation is bundled.

pub const VENDOR_ID: u16 = 0x056a;
pub const PRODUCT_ID: u16 = 0x00d4;
pub const MAX_X: u16 = 14720;
pub const MAX_Y: u16 = 9200;
pub const MAX_PRESSURE: u16 = 1023;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sample {
    pub x: u16,
    pub y: u16,
    pub pressure: u16,
    pub in_range: bool,
    pub position_valid: bool,
    pub tip: bool,
    pub barrel: bool,
    pub second_button: bool,
    pub eraser: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ReportFormat {
    Native9,
    Wacom11,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DecodeError {
    Length,
    ReportId,
    Bounds,
}

pub fn decode(bytes: &[u8], format: ReportFormat) -> Result<Sample, DecodeError> {
    // Legacy Wacom software wraps the native report with a prefix and suffix.
    // This format must be selected explicitly; never guess based on payload.
    let b = match (format, bytes.len()) {
        (ReportFormat::Native9, 9) => bytes,
        (ReportFormat::Wacom11, 11) => &bytes[1..10],
        _ => return Err(DecodeError::Length),
    };
    if b[0] != 2 {
        return Err(DecodeError::ReportId);
    }
    let in_range = b[1] & 0x80 != 0;
    let position_valid = in_range && b[1] & 0x40 != 0;
    let ready = in_range && b[1] & 0x20 != 0;
    let x = u16::from_le_bytes([b[2], b[3]]);
    let y = u16::from_le_bytes([b[4], b[5]]);
    let pressure = u16::from_le_bytes([b[6], b[7]]);
    // An out-of-range packet may contain stale bytes; it must still release.
    if (position_valid && (x > MAX_X || y > MAX_Y)) || (ready && pressure > MAX_PRESSURE) {
        return Err(DecodeError::Bounds);
    }
    Ok(Sample {
        x: if position_valid { x } else { 0 },
        y: if position_valid { y } else { 0 },
        pressure: if ready { pressure } else { 0 },
        in_range,
        position_valid,
        tip: ready && b[1] & 1 != 0,
        barrel: ready && b[1] & 2 != 0,
        second_button: ready && b[1] & 4 != 0,
        eraser: position_valid && b[1] & 8 != 0,
    })
}
