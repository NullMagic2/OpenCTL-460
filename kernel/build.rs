//! Configures the WDK linker for a KMDF binary and links the VHF kernel library.
fn main() -> Result<(), wdk_build::ConfigError> {
    wdk_build::configure_wdk_binary_build()?;
    println!("cargo:rustc-link-lib=static=vhfkm");
    Ok(())
}
