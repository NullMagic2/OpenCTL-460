<!-- Release changes and verification limits for the experimental 0.1.15 update. -->
# OpenCTL 460 0.1.15

- Manual and automatic handwriting now combine with all artistic smoothing controls.
  The handwriting filter runs first; selected artistic filters follow it. Their extra
  displacement is intentional, so the combined result is not subject to the handwriting
  stage's 0.12 mm bound. Zero artistic amounts retain the existing handwriting behavior.
- Falling measured pressure uses a short response (at most a 2 ms time constant) without
  the extra interpolation ramp. Force still controls pressure; there is no speed-derived
  width, future-sample buffer, or artificial post-lift contact.
- HID and WinTab lift at the last submitted contact point before moving in hover, matching
  the synthetic Ink lifecycle and preventing a raw hover jump from extending an endpoint.
- The main Save settings button is removed. Valid settings save atomically and reach the
  running feeder automatically, between strokes and held pen buttons. The status confirms
  actual application or reports a failed backend switch. The physical reader and WinTab
  stream remain open during ordinary tuning. Invalid numbers preserve valid settings.
- The report-format option persists and updates live. Backend changes reuse the feeder;
  the new backend must be available. Existing HID service ownership is retained when
  switching to Ink to prevent the official driver from reclaiming the reader.
- Save as remains for named pressure presets. Existing custom curves are preserved.

Validation: 81 automated Rust tests passed; the shared live-status test was excluded after
it reported an existing session mapping owner. Clippy passed with warnings denied. Native
GUI tests passed, including real-timer autosave, invalid edits, slider availability,
pressure editing, presets and DPI. Explorer tray hiding was unavailable on the isolated
test desktop; the fallback and callbacks passed. A separate rendered preview verified
layout. No driver installation, kernel reload, or live Graphite drawing was performed for
this build. Stroke feel and live backend switching still require real-application testing.
