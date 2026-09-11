@echo off
rem Builds and tests the Rust user-mode driver; does not install system drivers.
setlocal
pushd "%~dp0"
set "CARGO_TARGET_DIR=%CD%\target"
where cargo >nul 2>nul
if errorlevel 1 goto no_rust
cargo fmt --all -- --check
if errorlevel 1 goto failed
cargo test --locked
if errorlevel 1 goto failed
cargo build --release --locked
if errorlevel 1 goto failed
set "CARGO_TARGET_DIR=%CD%\wintab\target"
cargo fmt --manifest-path wintab\Cargo.toml -- --check
if errorlevel 1 goto failed
cargo build --manifest-path wintab\Cargo.toml --release --locked
if errorlevel 1 goto failed
cargo build --manifest-path wintab\Cargo.toml --release --locked --target i686-pc-windows-msvc
if errorlevel 1 goto failed
rem Pure packet mapping regressions never open the live input stream.
cargo test --manifest-path wintab\Cargo.toml --locked --lib
if errorlevel 1 goto failed
cargo test --manifest-path wintab\Cargo.toml --locked --lib --target i686-pc-windows-msvc
if errorlevel 1 goto failed
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "debug\run_wintab_conformance.ps1"
if errorlevel 1 goto failed
set "CARGO_TARGET_DIR=%CD%\target"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "scripts\collect_licenses.ps1"
if errorlevel 1 goto failed
if not exist dist mkdir dist
copy /y "target\release\ctl460-rust.exe" "dist\ctl460-rust.exe" >nul
if errorlevel 1 goto failed
copy /y "target\release\ctl460-gui.exe" "dist\ctl460-gui.exe" >nul
if errorlevel 1 goto failed
copy /y "target\release\ctl460-setup.exe" "dist\ctl460-setup.exe" >nul
if errorlevel 1 goto failed
copy /y "target\release\ctl460-service.exe" "dist\ctl460-service.exe" >nul
if errorlevel 1 goto failed
if not exist dist\wintab\x64 mkdir dist\wintab\x64
if not exist dist\wintab\x86 mkdir dist\wintab\x86
copy /y "wintab\target\release\wintab32.dll" "dist\wintab\x64\Wintab32.dll" >nul
if errorlevel 1 goto failed
copy /y "wintab\target\i686-pc-windows-msvc\release\wintab32.dll" "dist\wintab\x86\Wintab32.dll" >nul
if errorlevel 1 goto failed
echo Built driver and control panel in dist. Open dist\ctl460-gui.exe to configure.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "scripts\package_hid_setup.ps1"
if errorlevel 1 goto failed
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "debug\hid_setup_tests.ps1"
if errorlevel 1 goto failed
popd
exit /b 0
:no_rust
echo Rust with the MSVC toolchain is required. See README.md.
:failed
echo Build failed. See the error above.
popd
exit /b 1
