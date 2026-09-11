<!-- Changes and verification limits for the experimental 0.1.16 update. -->
# OpenCTL 460 0.1.16

- Fixes WinTab negative output extents: an inverted axis stays inside the output
  rectangle. Graphite requests this mapping and previously received negative Y
  positions, which can put strokes above its canvas despite a moving HID cursor.
  The convention matches the screen-coordinate setup in
  [Wacom's TiltTest sample](https://github.com/Wacom-Developer/wacom-device-kit-windows/blob/master/Wintab%20TiltTest/SampleCode/TiltTest.cpp).
  Both 32-bit and 64-bit WinTab providers are updated.
- Stabilization integrates fractional report intervals, removing the one-report
  dead zone and abrupt changes caused by rounding its smoothing window.
- Small alternating pressure fluctuations no longer bias output toward thinner
  strokes. Sustained pressure release remains fast, with no invented tail.
- StreamLine's coordinate slider is labelled **Line shape**. Pressure's help
  clarifies that it changes thickness, not the line path. All artistic controls
  continue to work with manual and automatic handwriting.
- The settings footer now shows only **Pressure levels: 1024 physical, 4098 software**.
  Detailed runtime diagnostics remain available with `ctl460-rust status`.

Validation: 84 Rust integration tests passed, plus two pure WinTab mapping/packet
tests on each of x64 and x86. The WinTab regression reproduces Graphite's packet
mask, output extents, contact flag and pressure values at all screen corners and
the center. These tests never publish synthetic data into the running pen stream.
The shared live-status test and full shared-stream DLL tests were not run because
the user's feeder owns that stream. Clippy passed with warnings denied. Native GUI smoke tests passed; an isolated
rendered preview confirmed the labels and simplified footer. Actual tray hiding
requires Explorer; the test verified the no-Explorer fallback and tray callbacks.
See [the smoothing audit](../debug/SMOOTHING-AUDIT.md) for repeatable measurements.

Save your Graphite drawing and close Graphite before upgrading, then reopen it
so it loads the corrected WinTab DLL. No live Graphite drawing has been verified
with this build yet; compatibility and stroke feel still need that check.
