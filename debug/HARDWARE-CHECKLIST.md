<!-- Manual test plan for behavior that synthetic tests and package validation cannot establish. -->
# CTL-460 hardware and application checks

Record the Windows version, hardware collection path, profile, display scale, application and
input API for each result. On 2026-09-09 native 9-byte acquisition and Windows Ink delivery to
our test canvas passed on the connected CTL-460 (18 strokes, 18 releases). The original Wacom
services were temporarily paused and restored. The remaining checks below are still pending;
new artistic smoothing requires hands-on evaluation. Full traces remain in debug/generated.

- Capture native reports before enabling output. Confirm ready/range/tip flags, pressure 0–1023,
  coordinates, both pen side buttons, and whether native or legacy wrapping is actually required.
- Compare Raw, Natural and Handwriting using small loops, sharp corners, dots, crossbars, light
  strokes, fast strokes and slowly increasing pressure. Compare at equal brush settings and zoom.
- Test manual writing and Auto with multiple writing styles and drawings. Record false positives
  and false negatives; do not interpret the evidence score as recognition accuracy.
- Verify pressure customization endpoints, left-handed rotation, aspect preservation and buttons.
  Ensure held Space and any owned key state are released on Stop, disconnect and worker failure.
- Open one Windows Ink application and one WinTab application. Switch focus repeatedly; confirm
  each uses its selected API with pressure and no duplicate strokes. Include 32-bit and 64-bit apps.
- Check a stalled/closed feeder, process termination, USB unplug/replug, sleep/wake and display
  changes. Verify no stuck contact, stale stroke replay, unintended mouse clicks or stretched text.
- Load the test-signed VHF package using SIGNING.md. Check Device Manager, kernel event logs,
  HVCI/Memory Integrity behavior and Driver Verifier. WDK package checks alone do not cover these.
- Exercise context opening/closing, relative buttons, packet queue overflow, orientation and app
  configuration dialogs in target WinTab applications. Record APIs missing from the basic provider.
- Check installer upgrade, existing third-party WinTab backup, normal uninstall, later-driver
  replacement, locked DLL handling and recovery from interrupted setup. Confirm only owned files
  are restored/removed and personal settings survive.
- Measure end-to-end latency with a camera or dedicated measurement; the GUI's host processing
  time excludes tablet USB polling and application/display delay.
