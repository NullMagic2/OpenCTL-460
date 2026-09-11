//! Verifies the CTL-460 pen-mode handshake before starting acquisition.
//! Like Linux's wacom_set_device_mode, repeat SET/GET if the tablet reports another mode.

pub fn initialize(mut exchange: impl FnMut() -> Result<Vec<u8>, String>) -> Result<(), String> {
    let mut last = String::new();
    for _ in 0..3 {
        match exchange() {
            Ok(reply) if reply.len() >= 2 && reply[0] == 2 && reply[1] == 2 => return Ok(()),
            Ok(reply) => last = format!("unexpected mode response {reply:02X?}"),
            Err(error) => last = error,
        }
    }
    Err(format!("Tablet did not confirm pen mode after 3 attempts: {last}. Reconnect the tablet and try again."))
}
