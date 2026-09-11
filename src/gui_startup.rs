//! Shortcut registration is per user and separate from tablet/drawing profiles.
use std::{os::windows::process::CommandExt, path::Path, process::Command};

pub fn directory() -> Result<std::path::PathBuf, String> {
    let mut path = [0u16; 260];
    // Ask the shell so redirected user Startup folders are respected.
    let result = unsafe {
        windows_sys::Win32::UI::Shell::SHGetFolderPathW(
            std::ptr::null_mut(),
            7,
            std::ptr::null_mut(),
            0,
            path.as_mut_ptr(),
        )
    };
    if result < 0 {
        return Err("Cannot find your Windows Startup folder".into());
    }
    let length = path.iter().position(|v| *v == 0).unwrap_or(path.len());
    Ok(std::path::PathBuf::from(String::from_utf16_lossy(
        &path[..length],
    )))
}

pub fn set(root: &Path, directory: &Path, enabled: bool) -> Result<(), String> {
    let script = root.join("scripts/startup_shortcut.ps1");
    let script = if script.is_file() {
        script
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/startup_shortcut.ps1")
    };
    let powershell =
        Path::new(&std::env::var_os("SystemRoot").ok_or("Windows directory unavailable")?)
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let result = Command::new(powershell)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(script)
        .arg("-Action")
        .arg(if enabled { "Enable" } else { "Disable" })
        .arg("-Executable")
        .arg(std::env::current_exe().map_err(|e| e.to_string())?)
        .arg("-StartupDirectory")
        .arg(directory)
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| e.to_string())?;
    if !result.status.success() {
        return Err(format!(
            "Cannot change Windows startup: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        ));
    }
    Ok(())
}
