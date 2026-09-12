<!-- Records implemented API coverage and unverified behavior without claiming universal support. -->
# Compatibility and testing limits

Target: Windows 11 x64, physical Wacom CTL-460 USB VID 056A / PID 00D4. This is not an ARM64
kernel package. The installer targets native x64 Windows; native ARM64 driver
support is not implemented. Other Wacom models are not automatically selected.

Windows Ink and WinTab can be available together. Applications choose their input API; do not
enable duplicate capture paths inside one application. WinTab-only applications need to load
our architecture-matched `Wintab32.dll`; a DLL in the application's own directory may take
precedence over the installed system provider. Existing Wacom services can compete for physical
device access or create duplicate input. Diagnose that conflict before using two feeders.

## WinTab coverage (0.2.0)

The standard application API is implemented on both x86 and x64, including context editing,
configuration, save/restore, packet and range retrieval, relative modes, overlap arbitration,
button capture/masks, and lifecycle/proximity/cursor messages. Shared manager services cover
context enumeration/ownership/defaults, cross-process editing subject to locks, cursor
enablement, button maps and pressure thresholds/response. See
[WINTAB-CONFORMANCE.md](WINTAB-CONFORMANCE.md) for the exact scope and validation commands.

Optional vendor extensions, legacy packet hooks/configuration replacement, Pen Windows and
physical features absent from the CTL-460 are not implemented or advertised. Unsupported
extension calls return failure. This does not claim universal WinTab 1.x certification.

Isolated tests exercise the actual x86/x64 DLL exports and ordinals, native packet layouts,
pressure/contact, relative motion, queues, save/restore, window notifications, and a 32-bit
manager editing a 64-bit application's context. They use a unique IPC namespace and never
replace the installed DLLs or inject pen/mouse input into the user's desktop.

The Photoshop screen-context regression reproduces its sixteenfold subpixel mapping; it
is not a live Photoshop drawing test. Existing applications must be restarted after an
update to load the new provider. Live Photoshop pressure, custom system mouse mappings,
multi-monitor display association, hotplug and suspend/resume still need hardware testing.
The release keeps both Windows Ink and WinTab available; it does not change Graphite Studio
or Photoshop configuration. Synthetic Ink's normal screen mapping is still based on the
primary display; this release is not a claim that all Windows Ink behavior is complete.

## Kernel status

Virtual HID selection now runs prerequisite checks and a consented elevated setup/session flow.
Mocked tests cover setup decisions, service restoration on failure, and early cancellation; package
hash/signature matching is checked without importing trust. Actual privileged installation and
kernel loading still require hardware verification. The active Test Mode query uses Windows code
integrity information rather than parsing localized BCDEdit output.

The pencil preset now retains mapping choices, and WinTab uses the same normalized screen position
as the Windows output. Automatic tilt has been removed. Graphite Studio's reported stationary
offset still needs a real application retest.

The Rust KMDF/VHF component compiles, passes WDK INF/signability checks, and has a generated
free self-signed package. Loading it, Driver Verifier, sleep/wake, unplug/replug and Memory
Integrity compatibility have not been tested on hardware. The setup helper is compiled but
has not been exercised against the Windows Driver Store. Follow SIGNING.md and the debug
hardware checklist. The kernel dependency is Microsoft's experimental windows-drivers-rs.

## Installer status

The Inno Setup source installs WinTab as a required part of normal installation. Backups are
kept under the protected installation directory with original SHA-256 names. Uninstall only
restores while the current DLL hash matches our installed hash; a later replacement is kept.
Backup files and ownership records remain for recovery. If a DLL is locked during restoration,
uninstall identifies the backup for manual recovery after closing tablet applications.

Installer compilation is checked. The 0.2.0 build has not been installed during its isolated
validation. The native settings test uses an isolated debug directory and never starts pen
injection. Earlier runtime captures confirmed physical tablet pressure, but do not validate
the newly built provider in third-party applications.
