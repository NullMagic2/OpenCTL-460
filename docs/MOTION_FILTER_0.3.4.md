# OpenCTL 460 0.3.4 - Motion Filtering

This release replaces direction-based clipping with an independently implemented,
compensated two-stage spatial low-pass filter. Its state is integrated exactly
along each measured segment. The compensated output is 2 * first stage - second
stage. Straight travel is retained after the initial transient while short spatial
wavelengths are attenuated. No extra report delay, look-ahead, or worker thread is added.

The characteristic length is 2 mm times the squared normalized Amount. Total
motion-filter displacement is bounded to 0.8 mm times squared Amount, plus coordinate
rounding. Each output axis is clamped between the preceding output and the current
measured endpoint before blending Expression. This controls ringing at reversals.
Identical coordinates hold position. Pressure is processed separately and releases
remain immediate. Existing zero/off settings and saved slider values are preserved.

## Using it

Install the update, then use the existing Line smoothing page. Motion Filtering is
the changed control. Start with Amount 25 and Expression 50, or keep your saved
values. Change one slider at a time. The Smooth inking preset also changes other
controls, so use the individual Motion Filtering slider for a direct comparison.

## Validation

123 offline driver tests passed. One existing status-publisher test was excluded
because it requires exclusive ownership of the active tablet feeder. No running
feeder was stopped and the update was not installed during validation.

At Amount 70 and Expression 0, synthetic sine wobble of amplitude 0.15 mm and
wavelengths 0.6 and 1.2 mm was substantially attenuated. The old direction clip
amplified some repetitive patterns. See the included audit metrics for exact results.
Tests measure perpendicular RMS error, so forward cursor lag alone cannot pass.
Clean circles with radii 1, 2.5 and 10 mm retain mean radius within 8% at this setting.
Other checks cover release, tool changes, pressure independence, stationary samples,
tiny valid strengths, zero bypass, and collinear report subdivision.

## Limits

This is a causal driver filter: it cannot revise pixels already drawn by an application.
Strong filtering can soften sub-millimetre loops and corners. The compensated response
has mild gain on some broad curves; synthetic broad-wave RMS gain stayed below 12%
in the tested cases. This is not a guarantee for arbitrary strokes. Additional path
filters can compound displacement. Hardware drawing feel still needs user evaluation.
The update contains no new kernel code or changes to pressure mapping.
