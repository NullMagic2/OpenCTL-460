<!-- Reproducible build requirements and commands for the app, ABI DLLs, VHF package and installer. -->
# Building on Windows

Tested with Rust 1.92 MSVC, Windows SDK/WDK 10.0.28000.0, KMDF 1.33, libclang 17.0.6,
and Inno Setup 6.7.3. Install Visual Studio C++ Build Tools and the Windows SDK first.

```bat
rem Install the 32-bit Rust standard library needed for 32-bit WinTab applications.
rustup target add i686-pc-windows-msvc
rem Build the app and both WinTab DLLs, and run hardware-free tests.
build_driver.bat
```

`build_driver.bat` checks formatting, runs the root tests, builds release executables and both
WinTab DLL architectures, tests the actual DLLs, collects dependency notices, and copies the
result to `dist`. It fails if required tools, tests or license notices are missing. It never
installs a driver. Cargo lockfiles pin resolved versions; subsequent builds use `--locked`.

## Kernel

Install the Windows Driver Kit with KMDF headers/libraries, InfVerif, Inf2Cat and SignTool.
`LIBCLANG_PATH` must identify a compatible libclang directory. libclang 22.1.1 produced incorrect
binding layouts with this dependency version; the build must fail those checks. Do not remove
layout assertions to force it through. libclang 17.0.6 built correctly here.

One way to obtain that library with Python is `py -m pip install --target tools\clang17
libclang==17.0.6`. Then run these commands from the project root:

```bat
rem Point bindgen at the compatible library, then build and sign local development artifacts.
set "LIBCLANG_PATH=%CD%\tools\clang17\clang\native"
build_kernel_driver.bat
test_sign_driver.bat
```

The kernel script runs inside `kernel/` so its kernel-specific Cargo configuration is applied.
It copies the native PE image to `dist\kernel\ctl460_vhf.sys`, validates the INF and creates a
catalog. The separate test-signing script signs the SYS first, regenerates the catalog with
that signed SYS, then signs the catalog. Trust/boot settings remain unchanged until you follow
[SIGNING.md](SIGNING.md). Re-running signing creates a new public certificate; install the
matching certificate for the exact package you use.

For restricted build hosts where SignTool cannot import a temporary private key,
`scripts\test_sign_kernel.ps1 -OsslSignCode C:\path\to\osslsigncode.exe` supports
osslsigncode 2.14 as an alternative Authenticode signer. Its key stays in the build
directory and is deleted after signing. Windows still requires the matching trusted
certificate and active Test Mode; this option changes no Windows security policy.

## Installer

```bat
rem Use an installed Inno Setup compiler; the path may differ on your machine.
set "ISCC=C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
build_installer.bat
```

Build the test-signed kernel package first if you want it included under `kernel-test` in the
installer. The normal installation always includes both WinTab DLLs. Kernel Test Mode setup
is explained in the included guide rather than silently changing machine security settings.

## Debug and cleanup

`debug\run_tests.bat` runs root tests. `build_driver.bat` also runs both WinTab test suites.
`debug\gui_smoke.ps1` launches only our settings window, saves test values in `debug\generated`,
and verifies persistence without starting tablet input. Hidden-window screenshots can be blank
on some Windows desktops and are not a visual-compatibility result.

`clean_temporary.bat -WhatIf` previews cleanup. Running it without `-WhatIf` removes root,
WinTab and kernel target directories and `debug\generated`, after validating project boundaries
and rejecting directory links. Source tests, profiles, distribution files and personal settings
are preserved. All authored test source lives in `debug/`, including native DLL and GUI tests.
