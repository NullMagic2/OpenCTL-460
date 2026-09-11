<!-- Describes process boundaries, timing, protocol contracts, and shared-stream behavior. -->
# Architecture

```text
Physical CTL-460 -> HID reader thread -> bounded event queue -> Rust pressure/stroke engine
                                                            |             |
                                                       Ink or VHF    atomic shared ring
                                                                          |
                                                             WinTab DLLs + status viewers
```

`src/protocol.rs` decodes native 9-byte pen reports; the explicit `--wacom11` mode strips the
old driver's wrapper. The optional `--init` request sends feature report `[02,02]`. Capture
mode reads reports without injecting input. GUI Start requests initialization explicitly.

The HID reader owns the physical handle and timestamps completed reads. The output thread
processes every accepted event in order, preserving contact transitions, while timer ticks
add pressure interpolation. A bounded queue prevents an unbounded latency backlog. Overflow
or a stale queue terminates input and releases owned pen/buttons. There are no catch-up bursts
after scheduler stalls. Handwriting inference uses bounded summaries on completed strokes;
there is no heavyweight recognition task on the pen path.

The selected Windows backend receives each processed frame. Synthetic Ink uses the Windows
pointer injection API. VHF submits validated 10-byte reports to the kernel control device.
The kernel is limited to transport, report validation and release handling; floating-point
pressure and handwriting calculations stay in user mode. Its watchdog checks every 25 ms
and releases a stalled source after 100–125 ms. Close and cleanup paths also release the pen.

`shared/ipc.rs` provides a per-session ring of 256 atomic records, with a fixed 32/64-bit layout,
sequence checks, writer identity and heartbeat. Every WinTab process owns its own read cursor
and bounded context queues. Slow readers do not block the physical reader or other apps.
An overrun resets proximity before accepting the newest frame; delayed history is not replayed.
There is one intended feeder per Windows session. GUI status and CLI status are read-only readers.

The user-mode pressure processing is shared by both output APIs. WinTab reports a pressure axis
of 0–4097. HID advertises that logical range too, but Windows APIs may normalize pen pressure
to their own range. The status latency measures host read completion through Windows submission;
it excludes USB polling, display, rendering, WinTab polling and application scheduling.

WinTab X/Y now describe the same normalized primary-screen position as the HID output. The shared
screen mapping applies aspect preservation once before publication, while WinTab contexts retain
their own requested output extents. This prevents the Windows cursor and app pen coordinates from
using different aspect mappings. Simulated tilt changes only orientation, never the tip's X/Y.

Per-application input selection is handled by the application. The driver does not inject a
second drawing event into a program just because both APIs are available. WinTab delivery to
window-owned contexts follows the foreground process; a null-window context is pollable.

The control panel starts a hidden, non-elevated feeder and uses a named event for graceful
shutdown. Settings and WinTab publication stay in the interactive session. Virtual HID
reports go through the installed OpenCTL460Broker service; only the service opens the
administrator/SYSTEM-only kernel control device. A non-elevated supervisor watches the
stop event and GUI process handle. Disconnect releases the pen before the broker restores
the Wacom services it paused. See [service boundaries and installation](SERVICE.md).

Measured force passes through the editable pressure curve, adaptive pressure smoothing
and bounded temporal interpolation. Velocity and coordinate smoothing never replace the
pressure signal. Stroke starts use the measured force, and release clears output immediately.
Drawing and handwriting share the evaluator; handwriting caps its timing filters to reduce
lag. Obsolete speed_pressure configuration keys are ignored on loading older settings.

References: [Microsoft VHF](https://learn.microsoft.com/en-us/windows-hardware/drivers/hid/virtual-hid-framework--vhf-),
[Windows pen pressure](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-pointer_pen_info),
[Wacom WinTab reference](https://developer-docs.wacom.com/docs/icbt/windows/wintab/wintab-reference/).
