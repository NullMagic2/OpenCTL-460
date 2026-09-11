//! Identifies the pen top-level collection; the CTL-460 mouse collection cannot initialize pen mode.
pub fn is_pen_collection(interface: i32, usage_page: u16, usage: u16) -> bool {
    interface == 0 && usage_page == 0x0d && usage == 1
}

/// Never silently choose between multiple physical pens, or fall back to a mouse collection.
pub fn automatic_index(collections: &[(i32, u16, u16)]) -> Result<usize, String> {
    let candidates: Vec<_> = collections
        .iter()
        .enumerate()
        .filter(|(_, c)| is_pen_collection(c.0, c.1, c.2))
        .map(|(i, _)| i)
        .collect();
    match candidates.as_slice() {
        [index] => Ok(*index),
        [] => Err("CTL-460 pen interface not found. Reconnect the tablet and refresh. The mouse and auxiliary interfaces cannot initialize pen mode.".into()),
        _ => Err("Multiple CTL-460 pens found. Select the pen you want to use.".into()),
    }
}
