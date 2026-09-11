//! System-context coordinate policy consumed by the feeder, never by a second
//! competing cursor-injection loop inside a drawing application's DLL.
use super::*;

fn requests_system_pen(records: &[session::Record], foreground: u32) -> bool {
    foreground != 0
        && records.iter().any(|r| {
            r.id != 0
                && r.pid == foreground
                && r.enabled != 0
                && r.words[0] & 1 != 0
                && r.words[6] & 0x400 != 0
        })
}

/// A foreground client explicitly requesting a pressure-enabled system context
/// needs classic system mouse events alongside its WinTab pressure packets.
/// Context discovery alone does not select this path.
pub fn foreground_requests_system_pen() -> bool {
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(GetForegroundWindow(), &mut pid);
    }
    session::with(|d| {
        d.prune();
        requests_system_pen(&d.contexts[2..], pid)
    })
    .unwrap_or(false)
}

/// Keep one Windows input path for an entire contact, including its release.
#[derive(Default)]
pub struct InputRoute {
    mouse: bool,
    contact: bool,
}
impl InputRoute {
    pub fn update(&mut self, requested: bool, contact: bool) -> bool {
        if !self.contact {
            self.mouse = requested;
        }
        self.contact = contact;
        self.mouse
    }
}

/// Move the classic mouse through the same screen mapping as the Windows pen.
pub fn move_system_mouse(position: (i32, i32), desktop: [i32; 4]) -> Result<(), String> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    let normalize = |value: i32, origin: i32, extent: i32| {
        // Target the centre of the screen pixel, including negative monitor origins.
        (((i64::from(value) - i64::from(origin)) * 65536 + 32768) / i64::from(extent.max(1)))
            .clamp(0, 65535) as i32
    };
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: normalize(position.0, desktop[0], desktop[2]),
                dy: normalize(position.1, desktop[1], desktop[3]),
                dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                ..Default::default()
            },
        },
    };
    if unsafe { SendInput(1, &input, size_of::<INPUT>() as i32) } == 1 {
        Ok(())
    } else {
        Err("Windows rejected the WinTab system cursor event".into())
    }
}
#[derive(Default)]
pub struct SystemCursor {
    context: u32,
    revision: u32,
    previous: Option<(i32, i32)>,
    position: (f64, f64),
    captured: u32,
}
impl SystemCursor {
    /// `desktop` is [left, top, width, height] in physical pixels.
    /// `pointer` initializes relative tracking without making the cursor jump.
    pub fn map(
        &mut self,
        x: i32,
        y: i32,
        contact: bool,
        desktop: [i32; 4],
        pointer: (i32, i32),
    ) -> Option<(i32, i32)> {
        let p = PenData {
            x,
            y,
            flags: 8 | i32::from(contact),
            ..Default::default()
        };
        let record = session::with(|d| {
            d.contexts[2..]
                .iter()
                .filter(|r| r.id != 0 && r.enabled != 0 && r.words[0] & 1 != 0)
                .filter(|r| r.id == self.captured || contains(&r.words, p))
                .max_by_key(|r| (r.id == self.captured, r.order))
                .copied()
        })
        .flatten();
        let Some(record) = record else {
            self.context = 0;
            self.previous = None;
            self.captured = 0;
            return None;
        };
        self.captured = if contact { record.id } else { 0 };
        let w = effective_words(record.words);
        let raw = (x, y);
        let changed = self.context != record.id || self.revision != record.revision;
        if changed {
            self.previous = None;
            self.context = record.id;
            self.revision = record.revision;
            self.position = (f64::from(pointer.0), f64::from(pointer.1));
        }
        let result = self.coordinates(&w, raw, desktop);
        self.previous = Some(raw);
        Some(result)
    }
    fn coordinates(&mut self, w: &[u32; 33], raw: (i32, i32), desktop: [i32; 4]) -> (i32, i32) {
        if w[26] != 0 {
            if let Some(previous) = self.previous {
                self.position.0 +=
                    (f64::from(raw.0) - f64::from(previous.0)) * f64::from(w[31]) / 65536.0;
                self.position.1 +=
                    (f64::from(raw.1) - f64::from(previous.1)) * f64::from(w[32]) / 65536.0;
            }
        } else {
            self.position = (
                f64::from(coordinate(raw.0, w[11], w[14], w[27], w[29]) as i32),
                f64::from(coordinate(
                    9200 - raw.1,
                    w[12],
                    w[15],
                    w[28],
                    (w[30] as i32).saturating_neg() as u32,
                ) as i32),
            );
        }
        let right = i64::from(desktop[0]) + i64::from(desktop[2].max(1)) - 1;
        let bottom = i64::from(desktop[1]) + i64::from(desktop[3].max(1)) - 1;
        self.position.0 = self.position.0.clamp(f64::from(desktop[0]), right as f64);
        self.position.1 = self.position.1.clamp(f64::from(desktop[1]), bottom as f64);
        (
            self.position.0.round() as i32,
            self.position.1.round() as i32,
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_a_foreground_enabled_pressure_system_context_selects_mouse_input() {
        let mut records: [session::Record; 2] = unsafe { std::mem::zeroed() };
        records[0].id = 3;
        records[0].pid = 101;
        records[0].enabled = 1;
        records[0].words = default_words();
        assert!(!requests_system_pen(&records, 101)); // Digitizing context.
        records[0].words[0] |= 1;
        assert!(requests_system_pen(&records, 101));
        assert!(!requests_system_pen(&records, 202)); // Background client.
        assert!(!requests_system_pen(&records, 0));
        records[0].enabled = 0;
        assert!(!requests_system_pen(&records, 101));
        records[0].enabled = 1;
        records[0].words[6] &= !0x400;
        assert!(!requests_system_pen(&records, 101)); // Cursor-only utility.
    }
    #[test]
    fn changing_foreground_input_api_keeps_the_same_path_through_release() {
        let mut route = InputRoute::default();
        assert!(route.update(true, false));
        assert!(route.update(true, true));
        assert!(route.update(false, true));
        assert!(route.update(false, false));
        assert!(!route.update(false, false));
        assert!(!route.update(false, true));
        assert!(!route.update(true, true));
        assert!(!route.update(true, false));
        assert!(route.update(true, false));
    }
    #[test]
    fn system_context_maps_full_edges_and_negative_monitor_origin() {
        let mut w = default_words();
        w[27] = (-1920i32) as u32;
        w[29] = 1920;
        w[30] = 1080;
        let mut tracker = SystemCursor::default();
        assert_eq!(
            tracker.coordinates(&w, (0, 0), [-1920, 0, 1920, 1080]),
            (-1920, 0)
        );
        assert_eq!(
            tracker.coordinates(&w, (14720, 9200), [-1920, 0, 1920, 1080]),
            (-1, 1079)
        );
        assert_eq!(
            tracker.coordinates(&w, (7360, 4600), [-1920, 0, 1920, 1080]),
            (-960, 540)
        );
    }
    #[test]
    fn relative_system_mode_preserves_fractional_motion_and_sensitivity() {
        let mut w = default_words();
        w[26] = 1;
        w[31] = 32768;
        w[32] = 131072;
        let mut tracker = SystemCursor {
            previous: Some((100, 100)),
            position: (50.0, 60.0),
            ..Default::default()
        };
        assert_eq!(
            tracker.coordinates(&w, (103, 104), [0, 0, 1920, 1080]),
            (52, 68)
        );
        assert_eq!(tracker.position, (51.5, 68.0));
    }
}

// System-button preference codes are interpreted once in the feeder. The DLL
// never injects competing mouse events from every application reading WinTab.
#[derive(Default, Clone)]
struct ButtonPlan {
    previous: u32,
    held: u32,
    drag: u32,
    mapping: [u8; 32],
}
fn mouse_flag(button: usize, down: bool) -> u32 {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
    match (button, down) {
        (0, true) => MOUSEEVENTF_LEFTDOWN,
        (0, false) => MOUSEEVENTF_LEFTUP,
        (1, true) => MOUSEEVENTF_RIGHTDOWN,
        (1, false) => MOUSEEVENTF_RIGHTUP,
        (2, true) => MOUSEEVENTF_MIDDLEDOWN,
        _ => MOUSEEVENTF_MIDDLEUP,
    }
}
impl ButtonPlan {
    fn update(&mut self, pressed: u32, mapping: [u8; 32], active: bool) -> Vec<u32> {
        let mut events = Vec::new();
        if !active || self.mapping != mapping {
            for button in 0..3 {
                if self.held & (1 << button) != 0 {
                    events.push(mouse_flag(button, false));
                }
            }
            self.held = 0;
            self.drag = 0;
            self.previous = 0;
            self.mapping = mapping;
        }
        if !active {
            return events;
        }
        let down = pressed & !self.previous;
        let mut held = self.drag;
        for (i, &code) in mapping.iter().enumerate() {
            if !(1..=9).contains(&code) {
                continue;
            }
            let button = usize::from((code - 1) / 3);
            let bit = 1 << button;
            match (code - 1) % 3 {
                0 => {
                    if pressed & (1 << i) != 0 {
                        held |= bit;
                    }
                }
                1 => {
                    if down & (1 << i) != 0 && self.held & bit == 0 {
                        for _ in 0..2 {
                            events.push(mouse_flag(button, true));
                            events.push(mouse_flag(button, false));
                        }
                    }
                }
                _ => {
                    if down & (1 << i) != 0 {
                        self.drag ^= bit;
                    }
                }
            }
        }
        held |= self.drag;
        // Remove drag bits that have just been toggled off, unless a click holds them.
        for button in 0..3 {
            let bit = 1 << button;
            let click_held = mapping
                .iter()
                .enumerate()
                .any(|(i, &code)| code == button as u8 * 3 + 1 && pressed & (1 << i) != 0);
            if !click_held && self.drag & bit == 0 {
                held &= !bit;
            }
            if (held ^ self.held) & bit != 0 {
                events.push(mouse_flag(button, held & bit != 0));
            }
        }
        self.held = held;
        self.previous = pressed;
        events
    }
}
#[derive(Default)]
pub struct SystemButtons {
    plan: ButtonPlan,
    owned: u32,
    pending: Vec<u32>,
}
impl SystemButtons {
    /// Returns true when native pen button promotion must be suppressed because
    /// a manager explicitly chose different system mouse functions.
    pub fn prepare(&mut self, physical: u32, eraser: bool, in_range: bool) -> bool {
        self.prepare_with_route(physical, eraser, in_range, false)
    }
    pub fn prepare_with_route(
        &mut self,
        physical: u32,
        eraser: bool,
        in_range: bool,
        mouse: bool,
    ) -> bool {
        let preference = session::with(|d| {
            let c = &d.cursors[usize::from(eraser)];
            (c.button_map, c.system_map)
        });
        let Some((logical, system)) = preference else {
            return false;
        };
        let custom = mouse
            || logical != std::array::from_fn(|i| i as u8)
            || system
                != std::array::from_fn(|i| {
                    if i == 0 {
                        1
                    } else if i == 1 {
                        4
                    } else {
                        0
                    }
                });
        let pressed = (0..2)
            .filter(|&i| physical & (1 << i) != 0)
            .fold(0, |bits, i| bits | (1 << logical[i]));
        self.pending = self.plan.update(pressed, system, custom && in_range);
        custom
    }
    /// Dispatch only after the pen backend has submitted the corresponding position.
    pub fn flush(&mut self) -> Result<(), String> {
        let events = std::mem::take(&mut self.pending);
        self.send(&events)
    }
    fn send(&mut self, events: &[u32]) -> Result<(), String> {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
        for &flag in events {
            let (button, down) = (0..3)
                .flat_map(|b| [(b, true), (b, false)])
                .find(|&(b, d)| mouse_flag(b, d) == flag)
                .unwrap();
            let bit = 1 << button;
            if !down && self.owned & bit == 0 {
                continue;
            }
            if down && self.owned & bit == 0 {
                let key = [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON][button];
                if unsafe { GetAsyncKeyState(i32::from(key)) } < 0 {
                    continue;
                }
            }
            let input = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dwFlags: flag,
                        ..Default::default()
                    },
                },
            };
            if unsafe { SendInput(1, &input, size_of::<INPUT>() as i32) } != 1 {
                return Err("Windows rejected the WinTab system-button event".into());
            }
            if down {
                self.owned |= bit;
            } else {
                self.owned &= !bit;
            }
        }
        Ok(())
    }
}
impl Drop for SystemButtons {
    fn drop(&mut self) {
        let events: Vec<_> = (0..3)
            .filter(|&b| self.owned & (1 << b) != 0)
            .map(|b| mouse_flag(b, false))
            .collect();
        let _ = self.send(&events);
    }
}
#[cfg(test)]
mod button_tests {
    use super::*;
    #[test]
    fn system_click_double_click_drag_and_cleanup() {
        let mut map = [0; 32];
        map[0] = 1;
        map[1] = 5;
        map[2] = 9;
        let mut p = ButtonPlan::default();
        assert_eq!(p.update(1, map, true), vec![mouse_flag(0, true)]);
        assert!(p.update(1, map, true).is_empty());
        assert_eq!(p.update(0, map, true), vec![mouse_flag(0, false)]);
        assert_eq!(
            p.update(2, map, true),
            vec![
                mouse_flag(1, true),
                mouse_flag(1, false),
                mouse_flag(1, true),
                mouse_flag(1, false)
            ]
        );
        assert!(p.update(2, map, true).is_empty());
        p.update(0, map, true);
        assert_eq!(p.update(4, map, true), vec![mouse_flag(2, true)]);
        assert!(p.update(0, map, true).is_empty());
        assert_eq!(p.update(4, map, true), vec![mouse_flag(2, false)]);
        p.update(0, map, true);
        p.update(4, map, true);
        assert_eq!(p.update(0, map, false), vec![mouse_flag(2, false)]);
    }
    #[test]
    fn classic_wintab_default_tip_emits_down_up_and_releases_on_route_change() {
        let mut map = [0; 32];
        map[0] = 1;
        map[1] = 4;
        let mut plan = ButtonPlan::default();
        assert!(plan.update(0, map, true).is_empty());
        assert_eq!(plan.update(1, map, true), vec![mouse_flag(0, true)]);
        assert!(plan.update(1, map, true).is_empty());
        assert_eq!(plan.update(0, map, true), vec![mouse_flag(0, false)]);
        assert_eq!(
            plan.update(3, map, true),
            vec![mouse_flag(0, true), mouse_flag(1, true)]
        );
        assert_eq!(
            plan.update(3, map, false),
            vec![mouse_flag(0, false), mouse_flag(1, false)]
        );
        assert!(plan.update(0, map, false).is_empty());
    }
}
