# OpenCTL 460 0.3.1

## Restored line controls

The Line smoothing tab shows all seven sliders directly:

- StreamLine - Line shape
- StreamLine - Pressure
- Stabilization - Amount
- Motion Filtering - Amount
- Motion Filtering - Expression
- Wobble reduction
- Circles and curves

The handwriting assistance, adaptive base smoothing and base smoothing choices are also visible. There is no separate Advanced gate or replacement Pen control page.

Line shape, stabilization and motion filtering now remain active alongside Light, Responsive and the saved 0.3.0 feel. Motion expression still modifies Motion Filtering, so it needs Motion Filtering above zero. Wobble reduction retains its previous behavior: it steadies hover, affects contact in the Original path, and sets the Responsive base strength. Light and Raw base modes bypass its contact stage.

## Circles and curves

0% adds no curve correction. Try 50% for an initial comparison, keeping your other sliders, brush and zoom fixed. Higher values increase directional wobble correction. This does not snap the stroke to a perfect shape.

The correction is the difference between the previously tested steadier filter and its Light baseline, both evaluated on the original tablet reports. It is added to the selected line path, avoiding a second application of the Light baseline's forward lag. The added correction is bounded to 0.12 mm plus coordinate rounding; deliberately selected line filters can have their own larger displacement.

The correction resets on lift, tool changes, stale input and reconfiguration. It does not synthesize pressure or finish a stroke after lifting. Combining strong line filters can still add drag; the new slider does not cancel their intentional smoothing.

## Existing settings

Pressure, mapping, shortcuts and the individual slider values are retained. For a saved 0.3.0 Pen control value above 30%, the base becomes Light and the extra steadiness transfers to Circles and curves. For example, old 65% becomes a Light base plus 50% Circles and curves. This avoids silently discarding your chosen strength.

The saved base is shown as "Saved 0.3.0 feel (drawing + writing)". Selecting Original, Raw, Light or Responsive replaces that saved base. The new circle control remains independent. A saved value below 30% keeps its exact base strength.

Profiles using the old individual settings without Pen control receive circle strength 0%. Existing nonzero line settings are active again, including where the 0.3.0 single-control mode previously bypassed them.

## Startup

Save your work and close drawing apps before running the installer. Stop the previous portable driver before switching versions.

For the portable ZIP, extract it and open Start OpenCTL 460.cmd. It uses the virtual HID service already installed on this computer. Open settings only.cmd opens the panel without starting input. Record pen test.cmd enables a bounded diagnostic trace; click Start driver in that panel.

## Validation

119 Rust tests pass, including five new regression tests for curve noise reduction, independent slider behavior, pressure/contact preservation, zero-strength neutrality, correction bounds, migration, reset and reconfiguration behavior. Tests use an isolated shared-memory namespace. Production binaries are built without that test namespace. No live driver was stopped or installed during this build.

The new noisy circle and ellipse checks show lower path error at 50% on a Light base. These are synthetic development fixtures, not proof of better handwriting for every user. A live comparison is still needed. The earlier intermittent freeze has not been established as fixed by this release.

## Draw-and-hold autoshapes

Recognizing basic lines, circles, ellipses and rectangles can use geometric fitting and confidence thresholds; AI is not required. Reliable replacement belongs in the drawing app, where the in-progress stroke and undo transaction can be changed before committing.

A tablet driver sends input to applications and cannot generally replace the stroke an arbitrary application has already painted. Generic undo-and-redraw automation would depend on the application's undo behavior, tool, layer, pressure and focus. This release therefore adds curve stabilization, not universal draw-and-hold shape replacement.
