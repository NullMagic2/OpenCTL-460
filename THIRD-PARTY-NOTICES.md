<!-- Identifies dependency license handling and separates proprietary research inputs from distribution. -->
# Third-party notices

Project source is MIT licensed; see LICENSE.txt. Dependency notices are collected from pinned
Cargo metadata into `third-party-licenses/` for Windows dependencies, including kernel build
dependencies. The Microsoft windows-drivers-rs crates use the MIT option of their dual license;
their upstream notice is preserved in `licenses/windows-drivers-rs-MIT.txt` because those
published crate archives omit the workspace-level license file.

The implementation uses Rust, hidapi, windows-sys/windows-rs, serde, toml, ctrlc and the
Microsoft Windows Driver Kit Rust bindings, with their transitive dependencies. Their exact
versions are recorded in the Cargo lockfiles. Windows SDK/WDK and Microsoft system libraries
remain subject to their respective SDK/platform licenses; this source archive does not bundle
the SDK, WDK, Visual Studio, libclang toolchain or Inno Setup compiler.

Installers are produced with Inno Setup. Its generated setup engine carries its own product
notices. No proprietary Wacom binaries, certificates or source code are redistributed.
Wacom/OpenTabletDriver/Linux sources were consulted for protocol and public API facts;
the pressure engine, report decoder and WinTab implementation here are independently authored.

See docs/LEGACY-ANALYSIS.md for the scope of static research, and docs/COMPATIBILITY.md for
implementation limits. Brand names identify compatibility targets and do not imply endorsement.
