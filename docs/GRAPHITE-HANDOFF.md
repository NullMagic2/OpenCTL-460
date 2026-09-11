<!-- Requested change description for a separate Graphite task; no Graphite files were edited here. -->
# Graphite WinTab precision and pen eraser handoff

The inspected source is `D:\Work\Programming\Rust\Graphite Studio\graphite-studio`.
Verify against the version you are editing before applying these changes.

## Preserve fractional coordinates

In `src/wintab.rs`, `Context::open` currently sets `lc.out_extent` to screen
width/height in pixels. WinTab packet XY are integers, so this asks the driver
to round away fractional-pixel motion. `Context::poll` then creates an integer
POINT and calls ScreenToClient. In contrast, `src/native_pen.rs` reconstructs
fractional Windows Ink positions from himetric device coordinates.

Request finer WinTab output coordinates, for example a checked factor of 32
units per screen pixel, while retaining the driver's input region and system
monitor mapping. Store the actual context transform returned by WTOpen. Convert
each packet to floating-point screen coordinates, then apply the screen-to-client
origin offset without converting the packet position itself to an integer POINT.
Keep the requested vertical-axis reversal and signed desktop origins. Handle
overflow and reject unusable mappings cleanly; do not change global tablet settings.

Tests should cover subpixel movements that formerly collapsed to identical points,
all corners and center, negative desktop origins, DPI scaling, monitor changes,
and consistent positions with the Windows Ink path within digitizer quantization.
Preserve packet order, pressure, orientation, focus resets and down/up/cancel events.
Do not add another smoothing stage merely to hide quantization.

## Preserve tablet eraser identity

The inspected `PenSample` has no eraser/tool field. Windows Ink pen flags are not
copied into it, and WinTab's inverted-tool status is not propagated to the brush.
Consequently an OpenCTL eraser assignment may still draw in this application even
though the driver correctly reports an eraser.

Add explicit pen/eraser identity to PenSample. Populate it from Windows Ink's
PEN_FLAG_ERASER / PEN_FLAG_INVERTED and WinTab's documented inverted-tool status
(TPS_INVERT, bit 0x10; query cursor capabilities if additional providers need it).
Use this identity as a temporary eraser override while the tool is active. Restore
the selected drawing tool when the button is released, rather than permanently
switching brushes or treating erasing as an E-key shortcut. Keep pressure available
to the eraser. Handle tool changes, focus loss, cancel and proximity loss without
connecting strokes or leaving erasing latched.

## OpenCTL changes delivered separately

OpenCTL 0.1.17 removes an unnecessary whole-pixel conversion before publishing its
shared HID/WinTab coordinates. It also timestamps WinTab packets from precise system
interrupt time, converted to the standard wrapping 32-bit millisecond field, instead
of the coarse GetTickCount64 clock. Pressure smoothing and line filters run before
the Windows/WinTab split. The driver does not apply a separate WinTab smoother.

Check packet timing when comparing backends: multiple lifecycle events may genuinely
share a millisecond. Do not invent a full frame interval for each queued packet or
discard valid same-time contact/pressure transitions. Graphite's own pressure filter
in `src/input.rs` uses packet time deltas, so compare equal timed force traces too.

No Graphite source, executable, settings or running process was changed by this task.

## Follow-up verification for the eraser report

The 0.1.18 check still finds no eraser/tool field in Graphite's PenSample.
OpenCTL packet tests verify TPS_INVERT, cursor 1, contact and pressure for held
eraser packets (and cursor 0 without inversion after release). Core tests cover
button assignment and HID tool transitions. A live poll received zero packets;
that capture is inconclusive and must not be treated as end-to-end verification.
After implementing the application change, test Button 2 held during hover,
contact and a mid-stroke transition, then release and verify normal drawing.
