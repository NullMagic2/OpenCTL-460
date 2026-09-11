//! Elevated installation helper for our root virtual device only; never rebinds physical Wacom USB.
//! Windows validates the driver package during installation. No signing or boot-policy bypass exists.
#[cfg(not(windows))]
fn main() {
    eprintln!("Windows only");
}
#[cfg(windows)]
mod app {
    use std::{
        ffi::c_void,
        io,
        mem::size_of,
        path::Path,
        ptr::{null, null_mut},
    };
    use windows_sys::Win32::{
        Devices::{DeviceAndDriverInstallation::*, Properties::*},
        Foundation::*,
    };
    const HARDWARE: &str = "ROOT\\CTL460VirtualPen";
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    fn check(ok: i32) -> Result<(), String> {
        if ok == 0 {
            Err(io::Error::last_os_error().to_string())
        } else {
            Ok(())
        }
    }
    struct Set(HDEVINFO);
    impl Drop for Set {
        fn drop(&mut self) {
            unsafe {
                SetupDiDestroyDeviceInfoList(self.0);
            }
        }
    }
    fn existing() -> Result<Option<(Set, SP_DEVINFO_DATA)>, String> {
        unsafe {
            let handle =
                SetupDiGetClassDevsW(null(), wide("ROOT").as_ptr(), null_mut(), DIGCF_ALLCLASSES);
            if handle == INVALID_HANDLE_VALUE as isize {
                return Err(io::Error::last_os_error().to_string());
            }
            let set = Set(handle);
            let mut index = 0;
            loop {
                let mut data = SP_DEVINFO_DATA {
                    cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
                    ..Default::default()
                };
                if SetupDiEnumDeviceInfo(set.0, index, &mut data) == 0 {
                    if GetLastError() == ERROR_NO_MORE_ITEMS {
                        return Ok(None);
                    }
                    return Err(io::Error::last_os_error().to_string());
                }
                index += 1;
                let mut buffer = [0u16; 4096];
                let mut required = 0;
                let mut kind = 0;
                if SetupDiGetDeviceRegistryPropertyW(
                    set.0,
                    &data,
                    SPDRP_HARDWAREID,
                    &mut kind,
                    buffer.as_mut_ptr().cast(),
                    (buffer.len() * 2) as u32,
                    &mut required,
                ) != 0
                {
                    let text = String::from_utf16_lossy(
                        &buffer[..(required as usize / 2).min(buffer.len())],
                    );
                    if text.split('\0').any(|id| id.eq_ignore_ascii_case(HARDWARE)) {
                        return Ok(Some((set, data)));
                    }
                }
            }
        }
    }
    unsafe fn remove(set: &Set, data: &mut SP_DEVINFO_DATA) -> Result<(), String> {
        let mut params = SP_REMOVEDEVICE_PARAMS {
            ClassInstallHeader: SP_CLASSINSTALL_HEADER {
                cbSize: size_of::<SP_CLASSINSTALL_HEADER>() as u32,
                InstallFunction: DIF_REMOVE,
            },
            Scope: DI_REMOVEDEVICE_GLOBAL,
            HwProfile: 0,
        };
        unsafe {
            check(SetupDiSetClassInstallParamsW(
                set.0,
                data,
                (&mut params as *mut SP_REMOVEDEVICE_PARAMS).cast(),
                size_of::<SP_REMOVEDEVICE_PARAMS>() as u32,
            ))?;
            check(SetupDiCallClassInstaller(DIF_REMOVE, set.0, data))
        }
    }
    pub fn run() -> Result<(), String> {
        let args: Vec<_> = std::env::args().skip(1).collect();
        // Read-only diagnostics work without elevation and report the actual PnP failure.
        if args.len() == 1 && args[0] == "status" {
            if let Some((_set, data)) = existing()? {
                let mut flags = 0;
                let mut problem = 0;
                let mut status = 0u32;
                let mut kind = 0;
                let mut length = size_of::<u32>() as u32;
                unsafe {
                    let result = CM_Get_DevNode_Status(&mut flags, &mut problem, data.DevInst, 0);
                    if result != CR_SUCCESS {
                        return Err(format!(
                            "Cannot query virtual device status: ConfigMgr {result}"
                        ));
                    }
                    let result = CM_Get_DevNode_PropertyW(
                        data.DevInst,
                        &DEVPKEY_Device_ProblemStatus,
                        &mut kind,
                        (&mut status as *mut u32).cast(),
                        &mut length,
                        0,
                    );
                    println!(
                        "Virtual HID: Device Manager code {problem}; started: {}.",
                        flags & DN_STARTED != 0
                    );
                    if result == CR_SUCCESS && kind == DEVPROP_TYPE_NTSTATUS && length == 4 {
                        println!("Problem status: 0x{status:08X}.");
                        if status == 0xC0200209 {
                            println!("Invalid KMDF object attributes. Install OpenCTL 460 version 0.1.7 or newer; changing Secure Boot will not fix this driver initialization error.");
                        }
                    }
                }
            } else {
                println!("Virtual HID device is not installed.");
            }
            return Ok(());
        }
        if args.len() == 1 && args[0] == "remove" {
            if let Some((set, mut data)) = existing()? {
                unsafe {
                    remove(&set, &mut data)?;
                }
            }
            println!("Virtual device removed; physical Wacom driver unchanged.");
            return Ok(());
        }
        if args.len() != 2 || args[0] != "install" {
            return Err(
                "Usage: ctl460-setup status | install SIGNED_INF | remove (install/remove require administrator)".into(),
            );
        }
        let path = Path::new(&args[1])
            .canonicalize()
            .map_err(|e| e.to_string())?;
        // SetupAPI expects an ordinary absolute DOS path, not Rust's verbatim path prefix.
        let name = path
            .to_string_lossy()
            .trim_start_matches("\\\\?\\")
            .to_string();
        let inf = wide(&name);
        let mut created = false;
        unsafe {
            let (set, mut data) = if let Some(pair) = existing()? {
                pair
            } else {
                let mut guid = std::mem::zeroed();
                let mut class = [0u16; 256];
                let mut required = 0;
                check(SetupDiGetINFClassW(
                    inf.as_ptr(),
                    &mut guid,
                    class.as_mut_ptr(),
                    class.len() as u32,
                    &mut required,
                ))?;
                let handle = SetupDiCreateDeviceInfoList(&guid, null_mut());
                if handle == INVALID_HANDLE_VALUE as isize {
                    return Err(io::Error::last_os_error().to_string());
                }
                let set = Set(handle);
                let mut data = SP_DEVINFO_DATA {
                    cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
                    ..Default::default()
                };
                check(SetupDiCreateDeviceInfoW(
                    set.0,
                    class.as_ptr(),
                    &guid,
                    wide("CTL-460 Studio Virtual Pen").as_ptr(),
                    null_mut(),
                    DICD_GENERATE_ID,
                    &mut data,
                ))?;
                let mut id = wide(HARDWARE);
                id.push(0);
                check(SetupDiSetDeviceRegistryPropertyW(
                    set.0,
                    &mut data,
                    SPDRP_HARDWAREID,
                    id.as_ptr().cast(),
                    (id.len() * 2) as u32,
                ))?;
                check(SetupDiCallClassInstaller(DIF_REGISTERDEVICE, set.0, &data))?;
                created = true;
                (set, data)
            };
            let mut reboot = 0;
            if UpdateDriverForPlugAndPlayDevicesW(
                null_mut(),
                wide(HARDWARE).as_ptr(),
                inf.as_ptr(),
                0,
                &mut reboot,
            ) == 0
            {
                let error = io::Error::last_os_error();
                if created {
                    let _ = remove(&set, &mut data);
                }
                return Err(format!("Driver installation failed: {error}. The development package needs its matching trusted certificate and active Windows Test Mode."));
            }
            println!("Virtual HID installed. Reboot required: {}", reboot != 0);
            if reboot != 0 {
                std::process::exit(3010);
            }
        }
        Ok(())
    }
    // Keep Windows raw pointer types explicit at the ABI boundary.
    const _: usize = size_of::<*mut c_void>();
}
#[cfg(windows)]
fn main() {
    if let Err(e) = app::run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
