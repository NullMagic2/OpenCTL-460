# OpenCTL 460 0.3.10

Improves shallow horizontal/diagonal curves, makes Precision Hold uniform, and prevents shape closure from pulling continuously moving spirals toward their start.

## Precision Hold

Hovering uses normal full-screen positioning. Each stroke starts at the visible pen-down position and uses the selected Movement reduction throughout the stroke. Both axes use one constant factor; there is no distance-dependent fade or acceleration.

Lift to reposition anywhere on the screen, then draw the next precise stroke. A reduced stroke covers a smaller range from its starting point; full-screen reach is provided by hover positioning between strokes. Toggle changes during contact take effect on the next stroke. Zero reduction remains an exact bypass.

## Horizontal, diagonal, and oval movement

Strong StreamLine uses partial second-stage compensation to reduce forward lag and oval shrinkage. It processes measured history without generating future points. A circular motion bound replaces the strong Motion Filtering stage's independent horizontal/vertical clamps, making its reversal protection independent of heading.

With the saved drawing settings (StreamLine 50%, Stabilization 20%, Motion Filtering off, Pen Control 30, Circle smoothing 29), a six-angle shallow-oval replay reduced mean same-report following error from 1.939 mm to 1.238 mm, about 36%. Its mean radial contour error fell from 0.201 mm to 0.047 mm. These are synthetic tablet-coordinate measurements, not a physical drawing-session measurement.

The legacy bounded smoothing mode is retained. Pressure shaping, pen-up behavior, and the existing no-tilt output remain intact.

## Shape closure and spirals

The Drawing aids option is now labeled Pause near the start to close a shape. Passing the start while drawing does not attract or snap the line.

For an eligible loop, hold near its start while keeping the pen down. After approximately 80 ms of stability within 0.04 mm, the endpoint eases to the start over another 80 ms. Deliberate movement cancels the pending closure. A snapped endpoint releases when the measured pen moves more than 0.08 mm from its pause position.

Only fresh measured-report timestamps advance the pause. Repeated output timer frames and report gaps cannot create a snap. Lift, erasing, barrel use, and handwriting reset closure state. This detects a deliberate pause, not whether a stroke is semantically a spiral; pausing at a spiral's start can intentionally invoke closure.

## Validation

171 Rust tests pass, including the exclusive-access test with the driver closed. New regressions cover:
- Reversal-bound rotation at every degree.
- Shallow ovals at six headings with contour and following-error limits.
- Uniform full-height precision movement in both directions at multiple reductions.
- Tall ovals and diagonals far from the stroke anchor.
- Full-screen hover and first-contact placement on landscape, portrait, and ultrawide mappings.
- Moving inward/outward spirals, repeated loops, and connected loops.
- Measured pause timing, smooth closure, motion cancellation, timer duplicates, report gaps, and tool/lift resets.

Native Drawing aids and Precision Hold GUI checks pass. The before/after image replays identical synthetic inputs through the old and new code; comparison CSVs are included separately. No physical tablet session in the drawing application was performed.

The installer includes updated application and 32/64-bit WinTab builds. The signed kernel package is unchanged. Existing saved settings are not edited by this development work.
