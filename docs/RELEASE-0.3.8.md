# OpenCTL 460 0.3.8

Adds endpoint settling, optional shape closure, and a visible stroke-start marker. Strengthens artistic smoothing on both lines and curves.

## Using the drawing controls

Set Line smoothing > Handwriting assistance to Off - drawing settings for shape closure and the marker. Open the new Drawing aids page:
- Settle the endpoint when the pen pauses: on by default. Catches up while the pen remains in contact; never adds ink after lift.
- Snap nearly closed shapes to their start: off by default. Enable for loop closure. The default attraction radius is 0.6 mm, adjustable from 0.1 to 2 mm.
- Show the stroke-start marker while drawing: on by default. A click-through ring marks the emitted start point. It hides on lift, focus change, or stale input.
- Allow strong artistic smoothing on lines and curves: on by default. Start around StreamLine 40-60%; increase for stronger smoothing and more lag.

First contact starts at the measured position. Near-loop closure requires sufficient travel, turning, and width/height; taps and simple open hooks do not qualify. Closure and the marker are disabled during handwriting, erasing, and barrel-button use.

## Smoothing behavior

The base-filter displacement guard now runs before the artistic stages. Strong drawing smoothing is no longer clipped to the old combined 1 mm limit or reset by ordinary curve turns. Disabling strong artistic smoothing retains the legacy bounded behavior.

StreamLine uses distance-based following, Stabilization integrates recent motion over a speed-dependent time window, and Motion Filtering uses a compensated spatial low-pass filter. Stabilization smooths its speed estimate to prevent noisy reports from rapidly resizing the averaging window.

High settings introduce lag, round corners, and can shrink small loops. Pausing in contact lets endpoint settling catch up. The start marker marks a screen position; it cannot track a canvas that the application pans or transforms during the stroke.

## Validation

155 Rust tests passed. The suite includes 14 Precision Hold tests and 12 shape-control tests. Native GUI checks cover drawing-aid visibility, defaults, persistence, invalid radius rejection, and Precision Hold slider behavior.

The repeatable smoothing replay feeds the actual Engine identical noisy straight lines, circles, and S-curves at 125 Hz. Each control and all three together were checked at 0%, 50%, and 100%. All reduced the test path's angular roughness, with a greater reduction at 100% than 50%. These measurements demonstrate these test cases, not a guarantee for every stroke.

The supplied reference package was inspected statically. Component names support separate stroke-processing stages, but do not establish exact algorithms, parameter curves, or equivalent output. This implementation is independently written; Motion Filtering is not an FFT implementation. The comparison images show this driver's output. No third-party application code or package is included.

The Windows overlay test verifies click-through behavior, no focus activation, expiry, and window destruction. Builds include the application and both WinTab architectures. The existing signed kernel package is unchanged. No physical-tablet drawing session or external application runtime equivalence test was performed.

## Retained from 0.3.7

Automatic tilt is removed. Precision Hold retains full mapped-screen reach and uses Movement reduction (0-90%) in the interface. Button changes during contact take effect after lift.
