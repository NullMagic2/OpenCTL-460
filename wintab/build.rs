//! Assigns the standard WinTab export ordinals and undecorated names on both Windows ABIs.
use std::{env, fs, path::PathBuf};
fn main() {
    let x86 = env::var("CARGO_CFG_TARGET_ARCH").unwrap() == "x86";
    let entries = [
        ("WTInfoA", 20, 12),
        ("WTOpenA", 21, 12),
        ("WTClose", 22, 4),
        ("WTPacketsGet", 23, 12),
        ("WTPacket", 24, 12),
        ("WTEnable", 40, 8),
        ("WTOverlap", 41, 8),
        ("WTConfig", 60, 8),
        ("WTGetA", 61, 8),
        ("WTSetA", 62, 8),
        ("WTExtGet", 63, 12),
        ("WTExtSet", 64, 12),
        ("WTSave", 65, 8),
        ("WTRestore", 66, 12),
        ("WTPacketsPeek", 80, 12),
        ("WTDataGet", 81, 24),
        ("WTDataPeek", 82, 24),
        ("WTQueueSizeGet", 84, 4),
        ("WTQueueSizeSet", 85, 8),
        ("WTMgrOpen", 100, 8),
        ("WTMgrClose", 101, 4),
        ("WTMgrContextEnum", 120, 12),
        ("WTMgrContextOwner", 121, 8),
        ("WTMgrDefContext", 122, 8),
        ("WTMgrDeviceConfig", 140, 12),
        ("WTMgrExt", 180, 12),
        ("WTMgrCsrEnable", 181, 12),
        ("WTMgrCsrButtonMap", 182, 16),
        ("WTMgrCsrPressureBtnMarks", 183, 16),
        ("WTMgrCsrPressureResponse", 184, 16),
        ("WTMgrCsrExt", 185, 16),
        ("WTMgrCsrPressureBtnMarksEx", 201, 16),
        ("WTMgrConfigReplaceExA", 202, 16),
        ("WTMgrConfigReplaceExW", 1202, 16),
        ("WTMgrPacketHookExA", 203, 16),
        ("WTMgrPacketHookExW", 1203, 16),
        ("WTMgrPacketUnhook", 204, 4),
        ("WTMgrPacketHookNext", 205, 16),
        ("WTMgrDefContextEx", 206, 12),
        ("WTQueuePacketsEx", 200, 12),
        ("WTInfoW", 1020, 12),
        ("WTOpenW", 1021, 12),
        ("WTGetW", 1061, 8),
        ("WTSetW", 1062, 8),
    ];
    let mut def=String::from("; Standard WinTab exports generated from the open ABI contract.\nLIBRARY Wintab32\nEXPORTS\n");
    for (name, ordinal, size) in entries {
        def += &if x86 {
            format!(" {name}=_{name}@{size} @{ordinal}\n")
        } else {
            format!(" {name} @{ordinal}\n")
        };
    }
    def += if x86 {
        " WacomCleanup=_WacomCleanup@0\n"
    } else {
        " WacomCleanup\n"
    };
    let path = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("wintab.def");
    fs::write(&path, def).unwrap();
    println!("cargo:rustc-cdylib-link-arg=/DEF:{}", path.display());
    println!("cargo:rerun-if-changed=build.rs");
}
