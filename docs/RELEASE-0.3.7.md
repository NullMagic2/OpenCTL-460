# OpenCTL 460 0.3.7

This release starts from the original 0.3.5 source, reuses the 0.3.6 shortcut UI
and radial geometry helpers, and replaces the Precision Hold transform and lifecycle.

## Using Precision Hold

In Buttons, assign Precision Hold to either pen button. The Movement reduction
slider ranges from 0% (normal movement) to 90% (one tenth of normal local travel).
50% halves local movement. Click once to enable, again to disable. Move the pen
near the intended drawing area before enabling.

The selected reduction is exact within a circular region around activation.
Its radius is half the distance to the closest mapped edge, capped at 15 mm on
the tablet. Beyond it, reduction gradually fades so all mapped screen edges and
corners remain reachable. Large shapes extending outside this region can be
warped by the transition; this is a precision tool, not a shape recognizer.
Constant reduced gain across the entire tablet would necessarily shrink screen
coverage. The transition is the deliberate compromise.

Both axes use the same radial factor. Mapping uses the visible aspect-cropped
tablet rectangle, not hidden sensor margins. At 0% reduction the input is
unchanged. There is no integrated displacement, repeated-report drift or extra
temporal filtering. Finite-resolution rounding remains.

The anchor is the current mapped pen position, not an asynchronous OS cursor
position. Toggling while touching waits until pen lift. The last ink point is
released before returning to absolute hover, without a line across the canvas.
Leaving proximity clears the anchor but retains the toggle; reentry anchors at
the new absolute pen position. This may reposition hover, as absolute mapping does.
Applying settings or restarting resets Precision Hold. Existing precision_gain
settings remain compatible; the UI displays 100 minus that stored percentage.
Double-click assistance is bypassed while precision is active.

## Automatic tilt removed

Removed the tilt generator, checkbox, angle field and simulated-pencil label.
Old virtual_tilt and tilt_max_degrees keys are accepted, ignored and omitted on
save. Even obsolete in-memory tilt flags cannot produce tilt in HID/Ink output
or the WinTab publication path. HID and IPC packet layouts remain compatible with
the existing kernel and WinTab components.

## Validation

- Complete root Rust test suite passed after the running feeder was closed.
- 14 Precision Hold tests cover all 91 reduction settings, edge/corner anchors,
  full-axis monotonic sweeps, arbitrary target reachability, aspect cropping,
  local circle geometry/closure, zero-reduction identity, repeated reports,
  both button assignments, contact transitions, reentry and pressure/tilt invariants.
- Earlier exclusive-session status test failed because the user's feeder was
  running; it passed in the complete rerun after the feeder was closed.
- Release build and installer/package validation are recorded in the delivery notes.

No physical drawing session or third-party application trial was performed.
The shared mapping is tested; this does not add multi-monitor support beyond
the existing 0.3.5 backend behavior. Windows Ink normally maps the primary display.

The installer retains the existing signed development kernel package and its
original signed version. Application and WinTab version is 0.3.7.
Native UI checks also passed for both sliders, all 91 displayed reductions, persisted gain conversion, restart/reload, removed tilt controls, and measured label fit. Release builds (application, x86/x64 WinTab) and Inno Setup compilation passed.
