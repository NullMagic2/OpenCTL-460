//! Embeds the Windows common-controls manifest in executables for native themed widgets.
fn main() {
    println!("cargo:rerun-if-changed=app.manifest");
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let manifest = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
            .join("app.manifest");
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
            manifest.display()
        );
        // The SDK resource compiler embeds the supplied multi-size icon in the settings EXE.
        println!("cargo:rerun-if-changed=assets/app.ico");
        let root = manifest.parent().unwrap();
        let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let resource = out.join("app.rc");
        let compiled = out.join("app.res");
        let icon = root
            .join("assets/app.ico")
            .to_string_lossy()
            .replace('\\', "/");
        std::fs::write(
            &resource,
            format!("// Application icon resource.\n1 ICON \"{icon}\"\n"),
        )
        .unwrap();
        let kits = std::path::PathBuf::from(std::env::var_os("ProgramFiles(x86)").unwrap())
            .join("Windows Kits/10/bin");
        let mut compilers: Vec<_> = std::fs::read_dir(kits)
            .expect("Windows SDK is required")
            .filter_map(Result::ok)
            .map(|e| e.path().join("x64/rc.exe"))
            .filter(|p| p.is_file())
            .collect();
        compilers.sort();
        let rc = compilers
            .last()
            .expect("Install the Windows SDK resource compiler (rc.exe)");
        let result = std::process::Command::new(rc)
            .arg("/nologo")
            .arg("/fo")
            .arg(&compiled)
            .arg(&resource)
            .status()
            .expect("Cannot run rc.exe");
        assert!(result.success(), "Icon resource compilation failed");
        println!("cargo:rustc-link-arg-bin=ctl460-gui={}", compiled.display());
    }
}
