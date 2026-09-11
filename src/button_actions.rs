//! Validates button assignments and tracks injected input ownership independently of Windows.
//! The testable state machine never executes commands; shortcuts are one keyboard chord.

pub const CUSTOM: &str = "Shortcut: ";

/// Parse a named key chord into Windows virtual keys, modifiers first.
pub fn shortcut(text: &str) -> Result<Vec<u16>, String> {
    let error = || "Use a shortcut such as Ctrl+Z, Ctrl+Shift+S, Space or F5".to_string();
    if text.len() > 80 {
        return Err(error());
    }
    let mut modifiers = Vec::new();
    let mut key = None;
    for part in text.split('+') {
        let name = part.trim().to_ascii_uppercase();
        let modifier = match name.as_str() {
            "CTRL" | "CONTROL" => Some(0x11),
            "SHIFT" => Some(0x10),
            "ALT" => Some(0x12),
            "WIN" | "WINDOWS" => Some(0x5b),
            _ => None,
        };
        if let Some(vk) = modifier {
            if modifiers.contains(&vk) {
                return Err(error());
            }
            modifiers.push(vk);
            continue;
        }
        let vk = match name.as_str() {
            "SPACE" => 0x20,
            "ENTER" | "RETURN" => 0x0d,
            "TAB" => 9,
            "ESC" | "ESCAPE" => 0x1b,
            "BACKSPACE" => 8,
            "DELETE" | "DEL" => 0x2e,
            "INSERT" | "INS" => 0x2d,
            "HOME" => 0x24,
            "END" => 0x23,
            "PAGEUP" | "PGUP" => 0x21,
            "PAGEDOWN" | "PGDN" => 0x22,
            "LEFT" => 0x25,
            "UP" => 0x26,
            "RIGHT" => 0x27,
            "DOWN" => 0x28,
            "SEMICOLON" => 0xba,
            "PLUS" => 0xbb,
            "COMMA" => 0xbc,
            "MINUS" => 0xbd,
            "PERIOD" => 0xbe,
            "SLASH" => 0xbf,
            "BACKTICK" => 0xc0,
            "LEFTBRACKET" => 0xdb,
            "BACKSLASH" => 0xdc,
            "RIGHTBRACKET" => 0xdd,
            "QUOTE" => 0xde,
            "MULTIPLY" => 0x6a,
            "ADD" => 0x6b,
            "SUBTRACT" => 0x6d,
            "DECIMAL" => 0x6e,
            "DIVIDE" => 0x6f,
            _ if name
                .strip_prefix("NUMPAD")
                .is_some_and(|n| n.len() == 1 && n.as_bytes()[0].is_ascii_digit()) =>
            {
                0x60 + u16::from(name.as_bytes()[6] - b'0')
            }
            _ if name.len() == 1 && name.as_bytes()[0].is_ascii_alphanumeric() => {
                name.as_bytes()[0] as u16
            }
            _ => name
                .strip_prefix('F')
                .and_then(|n| n.parse::<u16>().ok())
                .filter(|n| (1..=24).contains(n))
                .map(|n| 0x6f + n)
                .ok_or_else(error)?,
        };
        if key.replace(vk).is_some() {
            return Err(error());
        }
    }
    modifiers.push(key.ok_or_else(error)?);
    Ok(modifiers)
}

/// Display and execute the same chord for built-in and custom shortcut actions.
pub fn action_shortcut(action: &str) -> Option<&str> {
    match action {
        "Undo (Ctrl+Z)" => Some("Ctrl+Z"),
        "Redo (Ctrl+Y)" => Some("Ctrl+Y"),
        "Eraser (E)" => Some("E"),
        "Brush (B)" => Some("B"),
        _ => action.strip_prefix(CUSTOM),
    }
}

pub fn validate(action: &str) -> Result<(), String> {
    if let Some(chord) = action.strip_prefix(CUSTOM) {
        shortcut(chord).map(|_| ())
    } else if crate::config::BUTTON_ACTIONS.contains(&action) {
        Ok(())
    } else {
        Err("Unsupported button action".into())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputCode {
    Key(u16),
    LeftMouse,
    MiddleMouse,
}

/// A sink permits testing transitions and failures without injecting real input.
pub trait InputSink {
    fn held(&self, code: InputCode) -> bool;
    fn send(&mut self, code: InputCode, up: bool) -> Result<(), String>;
}

pub struct Bindings {
    actions: [String; 2],
    previous: [bool; 2],
    owned: Vec<InputCode>,
}
impl Bindings {
    pub fn new(actions: [String; 2]) -> Self {
        Self {
            actions,
            previous: [false; 2],
            owned: Vec::new(),
        }
    }
    fn press(&mut self, input: &mut impl InputSink, code: InputCode) -> Result<bool, String> {
        if self.owned.contains(&code) || input.held(code) {
            return Ok(false);
        }
        input.send(code, false)?;
        self.owned.push(code);
        Ok(true)
    }
    fn release(&mut self, input: &mut impl InputSink, code: InputCode) -> Result<(), String> {
        input.send(code, true)?;
        self.owned.retain(|v| *v != code);
        Ok(())
    }
    fn tap(&mut self, input: &mut impl InputSink, keys: &[InputCode]) -> Result<(), String> {
        let mut pressed = Vec::new();
        let mut result = Ok(());
        for &key in keys {
            match self.press(input, key) {
                Ok(true) => pressed.push(key),
                Ok(false) => (),
                Err(error) => {
                    result = Err(error);
                    break;
                }
            }
        }
        // Always unwind a partial chord. Failed releases stay owned for shutdown retry.
        for key in pressed.into_iter().rev() {
            if let Err(error) = self.release(input, key) {
                result = Err(error);
            }
        }
        result
    }
    pub fn update(
        &mut self,
        input: &mut impl InputSink,
        states: [bool; 2],
    ) -> Result<bool, String> {
        let desired: Vec<InputCode> = self
            .actions
            .iter()
            .zip(states)
            .filter_map(|(a, held)| {
                if !held {
                    return None;
                }
                match a.as_str() {
                    "Left click" => Some(InputCode::LeftMouse),
                    "Middle click" => Some(InputCode::MiddleMouse),
                    "Pan (hold Space)" => Some(InputCode::Key(0x20)),
                    _ => None,
                }
            })
            .collect();
        for code in self.owned.clone().into_iter().rev() {
            if !desired.contains(&code) {
                self.release(input, code)?;
            }
        }
        for code in desired {
            self.press(input, code)?;
        }
        for (i, pressed) in states.into_iter().enumerate() {
            if !pressed || self.previous[i] {
                continue;
            }
            let action = &self.actions[i];
            let chord = action_shortcut(action);
            if let Some(chord) = chord {
                let keys: Vec<_> = shortcut(chord)?.into_iter().map(InputCode::Key).collect();
                self.tap(input, &keys)?;
            } else if action == "Double click" {
                let code = InputCode::LeftMouse;
                // Inspect external state once. Windows may not update async key state
                // between the two queued clicks, so rechecking could suppress click two.
                if !self.owned.contains(&code) && !input.held(code) {
                    for _ in 0..2 {
                        input.send(code, false)?;
                        self.owned.push(code);
                        self.release(input, code)?;
                    }
                }
            }
        }
        self.previous = states;
        Ok(states
            .into_iter()
            .zip(&self.actions)
            .any(|(p, a)| p && a == "Right click"))
    }
    /// Stop, proximity loss and failures release only input pressed by this binding.
    pub fn release_all(&mut self, input: &mut impl InputSink) -> Result<(), String> {
        let mut result = Ok(());
        for code in self.owned.clone().into_iter().rev() {
            if let Err(error) = self.release(input, code) {
                result = Err(error);
            }
        }
        self.previous = [false; 2];
        result
    }
}

/// Format a captured Windows virtual key using names accepted by the shortcut parser.
/// Modifier order is stable; names are virtual keys, matching injected shortcut behavior.
pub fn captured_shortcut(key: u16, modifiers: [bool; 4]) -> Result<String, String> {
    let name = match key {
        0x30..=0x39 | 0x41..=0x5a => char::from_u32(u32::from(key)).unwrap().to_string(),
        0x70..=0x87 => format!("F{}", key - 0x6f),
        0x60..=0x69 => format!("Numpad{}", key - 0x60),
        _ => match key {
            8 => "Backspace",
            9 => "Tab",
            13 => "Enter",
            27 => "Escape",
            32 => "Space",
            0x21 => "PageUp",
            0x22 => "PageDown",
            0x23 => "End",
            0x24 => "Home",
            0x25 => "Left",
            0x26 => "Up",
            0x27 => "Right",
            0x28 => "Down",
            0x2d => "Insert",
            0x2e => "Delete",
            0x6a => "Multiply",
            0x6b => "Add",
            0x6d => "Subtract",
            0x6e => "Decimal",
            0x6f => "Divide",
            0xba => "Semicolon",
            0xbb => "Plus",
            0xbc => "Comma",
            0xbd => "Minus",
            0xbe => "Period",
            0xbf => "Slash",
            0xc0 => "Backtick",
            0xdb => "LeftBracket",
            0xdc => "Backslash",
            0xdd => "RightBracket",
            0xde => "Quote",
            _ => return Err("This key is not supported. Try another key or combination.".into()),
        }
        .into(),
    };
    let mut parts: Vec<&str> = ["Ctrl", "Shift", "Alt", "Win"]
        .into_iter()
        .zip(modifiers)
        .filter_map(|(name, held)| held.then_some(name))
        .collect();
    parts.push(&name);
    let chord = parts.join("+");
    shortcut(&chord)?;
    Ok(chord)
}
