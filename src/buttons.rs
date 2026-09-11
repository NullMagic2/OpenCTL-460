//! Windows keyboard/mouse adapter for tested Button 1/2 bindings.
//! Releases only owned synthetic holds on stop, disconnect, timeout and errors.
use ctl460_rust::button_actions::{Bindings, InputCode, InputSink};
use std::{io, mem::size_of};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
struct WindowsInput;
impl InputSink for WindowsInput {
    fn held(&self, code: InputCode) -> bool {
        let vk = match code {
            InputCode::Key(vk) => vk,
            InputCode::LeftMouse => VK_LBUTTON,
            InputCode::MiddleMouse => VK_MBUTTON,
        };
        // Read only: preserve input held before our chord.
        unsafe { GetAsyncKeyState(i32::from(vk)) < 0 }
    }
    fn send(&mut self, code: InputCode, up: bool) -> Result<(), String> {
        let input = match code {
            InputCode::Key(vk) => INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: vk,
                        dwFlags: (if up { KEYEVENTF_KEYUP } else { 0 })
                            | if matches!(vk, 0x21..=0x28 | 0x2d..=0x2e | 0x5b | 0x6f) {
                                KEYEVENTF_EXTENDEDKEY
                            } else {
                                0
                            },
                        ..Default::default()
                    },
                },
            },
            InputCode::LeftMouse | InputCode::MiddleMouse => INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dwFlags: match (code, up) {
                            (InputCode::LeftMouse, false) => MOUSEEVENTF_LEFTDOWN,
                            (InputCode::LeftMouse, true) => MOUSEEVENTF_LEFTUP,
                            (_, false) => MOUSEEVENTF_MIDDLEDOWN,
                            (_, true) => MOUSEEVENTF_MIDDLEUP,
                        },
                        ..Default::default()
                    },
                },
            },
        };
        // One initialized INPUT; no pointer movement is injected.
        if unsafe { SendInput(1, &input, size_of::<INPUT>() as i32) } != 1 {
            return Err(format!(
                "Button input failed: {}",
                io::Error::last_os_error()
            ));
        }
        Ok(())
    }
}
pub struct Buttons {
    bindings: Bindings,
}
impl Buttons {
    pub fn new(first: String, second: String) -> Self {
        Self {
            bindings: Bindings::new([first, second]),
        }
    }
    pub fn update(&mut self, states: [bool; 2]) -> Result<bool, String> {
        self.bindings.update(&mut WindowsInput, states)
    }
}
impl Drop for Buttons {
    fn drop(&mut self) {
        if let Err(error) = self.bindings.release_all(&mut WindowsInput) {
            eprintln!("Button release: {error}");
        }
    }
}
