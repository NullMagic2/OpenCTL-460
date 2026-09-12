//! Native file pickers. Invoke without borrowing the owner State: dialogs pump messages.
use std::{path::PathBuf, ptr::null_mut};
use windows_sys::Win32::{
    Foundation::HWND,
    UI::{
        Controls::Dialogs::*,
        WindowsAndMessaging::{MessageBoxW, IDYES, MB_DEFBUTTON2, MB_ICONQUESTION, MB_YESNO},
    },
};
pub fn choose(owner: HWND, save: bool) -> Result<Option<PathBuf>, String> {
    let mut file = [0u16; 32768];
    if save {
        for (out, ch) in file
            .iter_mut()
            .zip("pressure.calibration_profile".encode_utf16())
        {
            *out = ch;
        }
    }
    let filter: Vec<u16> =
        "OpenCTL calibration profile (*.calibration_profile)\0*.calibration_profile\0\0"
            .encode_utf16()
            .collect();

    let mut picker = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner,
        lpstrFilter: filter.as_ptr(),
        lpstrFile: file.as_mut_ptr(),
        nMaxFile: file.len() as u32,
        // Append the full extension after selection; see OPENFILENAMEW lpstrDefExt docs.
        lpstrDefExt: std::ptr::null(),
        Flags: OFN_NOCHANGEDIR
            | OFN_PATHMUSTEXIST
            | if save {
                OFN_OVERWRITEPROMPT
            } else {
                OFN_FILEMUSTEXIST
            },
        pvReserved: null_mut(),
        ..Default::default()
    };
    let ok = unsafe {
        if save {
            GetSaveFileNameW(&mut picker)
        } else {
            GetOpenFileNameW(&mut picker)
        }
    };
    if ok == 0 {
        let code = unsafe { CommDlgExtendedError() };
        return if code == 0 {
            Ok(None)
        } else {
            Err(format!("Cannot open file picker ({code})."))
        };
    }
    let end = file.iter().position(|c| *c == 0).unwrap_or(file.len());
    let path = PathBuf::from(String::from_utf16_lossy(&file[..end]));
    let chosen = if save {
        ctl460_rust::calibration::export_path(&path)?
    } else {
        ctl460_rust::calibration::validate_profile_path(&path)?;
        path.clone()
    };
    // The picker only confirmed the path it returned. Confirm the actual target if we appended a suffix.
    if save && chosen != path && chosen.exists() {
        let question: Vec<u16> = format!(
            "Replace the existing calibration profile?\n{}",
            chosen.display()
        )
        .encode_utf16()
        .chain(Some(0))
        .collect();
        let title: Vec<u16> = "Export calibration profile"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        if unsafe {
            MessageBoxW(
                owner,
                question.as_ptr(),
                title.as_ptr(),
                MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2,
            )
        } != IDYES
        {
            return Ok(None);
        }
    }
    Ok(Some(chosen))
}
