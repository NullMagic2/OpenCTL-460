# WinTab implementation and conformance - OpenCTL 460 0.2.1

## Scope

This is an independent Rust implementation for Windows 11 x64 and the physical CTL-460.
It provides native 64-bit and 32-bit `Wintab32.dll` builds. No proprietary Wacom DLL is
included. It follows the standard WinTab application interface and the manager services
listed below; it does not implement every optional historical WinTab extension or claim
certification against all WinTab 1.x implementations.

The primary references are the [Wacom WinTab reference](https://developer-docs.wacom.com/docs/icbt/windows/wintab/wintab-reference/)
and [Wacom WinTab 1.4 reference](https://www.wacomeng.com/windows/docs/Wintab_v140.htm).
The coverage below describes this project's code and tests, not a vendor endorsement.

## Implementation locations

- `wintab/src/lib.rs`: exported application API, information categories, contexts,
  packet construction/queues, event selection, and feeder reader.
- `wintab/src/session.rs`: versioned, fixed-width shared context/manager records,
  global overlap/capture arbitration, cursor preferences, process cleanup and event routing.
- `wintab/src/optional.rs`: manager API and explicit failures for unsupported capabilities.
- `wintab/src/context_dialog.rs`: native context configuration dialog.
- `wintab/src/system_output.rs`: feeder-owned system-cursor and mouse-button behavior.
- `wintab/build.rs`: export names, legacy ordinals and x86 calling-convention aliases.
- `shared/ipc.rs`: bounded, timestamped processed pen stream shared by both architectures.
- `src/output.rs`: publishes the same processed pressure/position to WinTab and Windows output.

## API coverage

| Area | Implemented behavior |
| --- | --- |
| Discovery | WTInfoA/W; interface/status, digitizing/system defaults, CTL-460 device, pen and eraser categories, size-only and aggregate queries; physical tablet presence |
| Context lifecycle | WTOpenA/W, WTClose, WTEnable, WTOverlap, WTGetA/W, WTSetA/W; enable state, global overlap order, input bounds/margins and manager locks |
| Context editing | WTConfig native dialog, WTSave/WTRestore with a versioned, checked, architecture-independent save block |
| Packet access | WTPacket, WTPacketsGet/Peek, WTDataGet/Peek, WTQueuePacketsEx, WTQueueSizeGet/Set; serial wrap, bounded queues and loss flags |
| Packet layout | Every standard field through rotation, native handle size/alignment, requested-field packing, change masks and immutable status snapshots |
| Coordinates | Signed input/output extents; absolute or relative coordinates, fixed-point sensitivities, screen-based system contexts and tablet-based digitizing contexts |
| Pressure/buttons | 0-4097 pressure; normal-pressure button thresholds; absolute/relative logical buttons, down/up masks and capture; distinct pen/eraser cursors |
| Notifications | Packet/cursor notification options; mandatory context lifecycle and proximity messages; manager broadcasts and information changes |
| Managers | WTMgrOpen/Close, ContextEnum, ContextOwner, DefContext/Ex, DeviceConfig, CsrEnable, CsrButtonMap, CsrPressureBtnMarks/Ex, CsrPressureResponse |
| Manager isolation | Cross-process/x86-x64 context discovery/editing with locks, read-only default handles, bounded manager/context capacity and stale-process cleanup |
| System output | Feeder applies system context coordinates/sensitivity and manager click/double-click/drag maps; normal pen promotion remains unchanged with default maps |
| Cleanup | WacomCleanup closes only the calling process's contexts/managers; it never shuts down other clients |

The provider supports 64 application contexts and 16 managers per Windows session. The
pressure response table contains exactly 256 UINT entries, interpolated across the declared
0-4097 range. The default response preserves measured pressure. Context packet rates report
the feeder's configured rate (60-500 Hz, default 250 Hz), rather than an unhonored requested rate.

The CTL-460 does not measure tilt, rotation, Z or tangent pressure. Their standard packet
fields remain valid to request: unsupported physical quantities return neutral values.
Orientation contains explicitly simulated tilt only when enabled in OpenCTL. Eraser input
uses the configured side-button action; the pen has no measured rear eraser sensor.

## Unsupported optional capabilities

The standard Cursor Mask (tag 3) and Out of Bounds Tracking (tag 0) extensions are
advertised. WTExtGet/Set persist a context's 128-bit cursor mask; WTMgrExt configures
out-of-bounds tracking. Context restore accepts the previous save format with an
all-cursors mask. Other extension requests fail. ExpressKeys, touch rings/strips,
airbrush wheels, multi-tablet aggregation and Pen Windows remain outside this device's scope.

Foreground clients opening an enabled, pressure-enabled system context select classic
Windows mouse movement/buttons alongside WinTab pressure; Windows pen proximity ends
for that route. Other clients retain the configured Windows pen backend. A stroke keeps
its route through release. This is not an application-name or executable-path special case.

Legacy WTMgrConfigReplaceExA/W, WTMgrPacketHookExA/W, WTMgrPacketUnhook and
WTMgrPacketHookNext exports return failure with ERROR_NOT_SUPPORTED. Their presence is
ABI compatibility, not a claim that hooks or third-party configuration DLL replacement
work. Applications requiring those historical manager features are not supported.

## Automated validation

Run from the project root:

```powershell
cargo test --locked --manifest-path wintab/Cargo.toml --lib
cargo test --locked --manifest-path wintab/Cargo.toml --lib --target i686-pc-windows-msvc
./debug/run_wintab_conformance.ps1
```

`debug/wintab_unit_tests.rs` and the system-output unit tests cover all 16,384 standard
packet masks and native strides, signed/relative mapping, Photoshop's system-context
request, Graphite-style polling, pressure and eraser data, queues/overflow/serial wrap,
stationary strokes, overlap/capture/margins, context locks, save corruption, proximity
flags, system cursor edges and system button transitions.

`debug/wintab_tests.rs` loads the actual release DLLs. It checks all standard exported
names/ordinals, ANSI/Unicode structures, live packet/range APIs and buffer guards,
relative pressure, manager response tables, cursor disabling, context save/restore,
queue failure semantics, real Windows messages and cross-architecture manager editing.
The test is ignored in ordinary `cargo test`: only the dedicated runner supplies a
unique compile-time IPC namespace. It neither writes the real tablet stream nor invokes
system mouse output. Never ship the isolated DLLs from `wintab/target/conformance`.

Normal distributable DLLs are built in `wintab/target/release` and
`wintab/target/i686-pc-windows-msvc/release`, then copied to `dist/wintab/x64` and
`dist/wintab/x86`. The installer installs both architectures automatically.

## Remaining hardware/application validation

Automated tests establish implementation behavior; they are not third-party application
certification. The new DLLs still need physical-pen testing in Photoshop 2021, Graphite
Studio and other consumers, including pressure taper, eraser changes, monitor edges,
relative system cursor mode, manager mouse maps, hotplug, sleep/wake and concurrent apps.
Virtual HID display association on multiple monitors needs hardware verification.
No Graphite or Photoshop source, preference file or private application directory was
modified for this release. Restart drawing applications after installation so they load
the new architecture-matched provider; an app-local DLL can override the system DLL.
