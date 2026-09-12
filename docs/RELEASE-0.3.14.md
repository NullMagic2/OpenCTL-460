# OpenCTL 460 0.3.14

Adds a Calibration tab for measuring your pressure range and managing pressure profiles. No new drawing filter is enabled.

## Calibrate

1. Keep the driver running and open your drawing application.
2. Open Calibration and click Record light. Each stage waits for actual pen contact before its eight-second timer begins. Draw at least two strokes and lift between them.
3. Return to Calibration and record ordinary strokes, then comfortably firm strokes. Each stage starts only when you choose Record. Use comfortable pressure; never press hard.
4. Review the curve, then choose Apply preview. Lift the pen so the driver's existing live-settings system can apply the new response. Try light-to-firm strokes. Undo apply restores the preceding pressure curve during this control-panel session, provided you have not edited that response since applying it.

The tab reads raw pressure from the running feeder's read-only shared stream. It does not stop the driver, seize the physical tablet, or record drawings. It ignores hover and side-button/eraser reports, trims stroke starts and ends, and uses the median stroke-body pressure at each guided level. These are time-weighted raw readings from the output stream, not additional independent sensor measurements.

The three measured levels map to 14%, 45%, and 90% output through the existing monotone cubic pressure curve. The endpoints remain 0% and 100% with firm-pressure headroom. Too few strokes, indistinct or reversed levels, saturation, missing input, a restarted feeder, and missed stream samples require a retry rather than applying an unreliable curve. Waiting without contact expires after two minutes. Cancel stage retries just that stage; Start over clears the calibration preview.

## Profiles

- Save as stores the preview (or current pressure when no preview exists) under a unique name in the existing pressure profile library. It does not apply the preview. Named profiles are shared with the Pen tab.
- Load places a saved profile in the preview. Apply preview activates it.
- Import accepts portable pressure-profile TOML files and earlier exported OpenCTL settings TOML files. Only their pressure response is imported; movement, smoothing, button assignments, and other settings are preserved.
- Export writes a portable, versioned pressure-only profile. Import it on another installation, review it, and Apply or Save as.

Existing installed settings and profiles are preserved by setup. The prior personalized pressure curve is not installed as a default for everyone.

## Validation

The existing regression suite passed with the exclusive live shared-memory status test skipped because the user's normal feeder was running. Ten new hardware-independent tests cover contact-triggered timing, waiting timeout, cancellation, retry, measurement validation, monotonicity, firm-pressure headroom, import/export, and pressure-only application. A hidden-window native control test verifies preview/save/load isolation, application, undo, and preservation of non-pressure settings. The rendered Calibration tab was visually inspected and a clipped instruction corrected.

Live physical calibration through the new tab and the native file pickers still need a hands-on check after installation. This release does not implement the separately proposed additional slowdown smoothing.