<!-- Explains the settings app's pressure and handwriting controls and the limits of inference. -->
# Pressure and handwriting settings

All controls below are in the native settings app. Valid changes save automatically and
apply to the running feeder between strokes, without a manual save or driver restart.

The **Pen** tab provides an editable pressure graph, a Soft/Firm slider, and tip/double-click controls. The **Buttons** tab assigns Button 1 and Button 2.
Drag nodes to adjust sensitivity; click between nodes to add one. Add node and Remove node
also provide explicit editing controls. The response is a monotone cubic curve with 2-16 nodes.
The built-in Default remains protected; editing it creates an unsaved copy. Save as creates
a named preset. **Reset curve** restores a straight diagonal with three handles;
button assignments and contact thresholds are retained. **Advanced** provides precise values.
Graph changes save automatically and apply between strokes.

**Mapping** holds handedness and aspect settings. **Line smoothing** holds artistic sliders and
handwriting assistance. **Advanced** holds calibration, timing, simulated tilt and the output backend.

| Control | Effect |
|---|---|
| Pressure curve / gamma | Shapes measured pressure; a higher curve gives a thicker stroke at the same force. |
| Pressure gain | Multiplies the curved pressure, capped at full output; does not add sensor detail. |
| Pressure curve zero point | Measured input pressure mapped to zero brush pressure. |
| Full pressure at | Measured input pressure mapped to full output. |
| Start ink at | Measured-pressure threshold for starting a stroke; the hardware tip signal is also required. |
| Stop ink at | Lower release threshold to avoid contact chatter. Must be below the start threshold. |
| Pressure smoothing | Smooths changes in measured pressure; higher values delay thickness changes. |
| Pressure interpolation | Ramps between processed pressure values over a short time; larger values add lag. |
| Adaptive stroke smoothing | Filters X/Y jitter; uncheck for an unfiltered coordinate comparison. |
| Preserve proportions | Keeps circles and letters proportionate while reaching every screen edge. A centered portion of the tablet maps to the display; excess tablet margins clamp to the screen edges. |

From 0.1.14, measured force drives pressure. Speed changes alone do not change the target
pressure. Deliberate force changes remain visible, with light smoothing for jitter. See
[pressure behavior](SETTINGS.md#consistent-pressure). The raw diagnostic profile uses the
same measured force without time/position smoothing.

The GUI validates numeric ranges and rejects an invalid profile before the feeder starts.
Pressure is processed in floating point, then quantized to the selected API. Exactly 4,098
values means integers 0 through 4097, inclusive. Interpolation can produce intermediate values
over time; a simple scaling of 1,024 measurements still contains only 1,024 measured force values.

## Handwriting assistance

**On** selects handwriting behavior immediately for every new stroke. It uses a speed-adaptive
One Euro filter with a 20 Hz minimum cutoff and beta 0.5 in millimetre/second units. Its filtered
coordinate stays within 0.12 mm of the raw coordinate before rounding (about 0.1271 mm after
integer rounding). In tight turns, a curvature estimate tightens this allowance to about
10% of the local radius, bounded between 0.02 and 0.12 mm. Small motion below 0.04 mm does
not update direction; the allowance relaxes gradually after a turn. This helps preserve
small loops and arches without prediction. A pen-down dot starts with pressure immediately;
pen lifts bypass smoothing and reset per-stroke state. Handwriting caps base pressure
smoothing at 3 ms and interpolation at 2 ms, then applies the selected StreamLine pressure
amount. A continuous pressure drop of at least 3% of the mapped range takes a fast
release path (at most a 2 ms filter, without an additional interpolation ramp). Small
alternating fluctuations use symmetric smoothing to avoid biasing pressure downward. Artistic coordinate sliders follow the handwriting filter in both manual and Auto
modes; their intentional displacement is additional to the bounds above.

These bounds limit spatial displacement, not end-to-end latency. There is no future-sample
buffer and no extrapolation beyond the measured point. The app can still add its own smoothing.
Handwriting mode suppresses artistic tilt so the simulated angle does not change writing width.
The handwriting mode selector and the coordinate-smoothing checkbox remain independent.

**Auto** estimates writing from completed strokes: compact size, duration, speed, curvature,
short gaps and repeated evidence. It needs at least three completed strokes before activating.
Evidence expires after a pause and resets on proximity loss or a stale input stream. Decisions
are latched at the next pen-down, so assistance cannot switch halfway through a stroke.

The live evidence score is **not a calibrated probability**. The heuristic does not recognize
letters or languages, can confuse doodles with handwriting and can miss large or unusual writing.
It keeps only a short in-memory collection of stroke summaries; it does not save handwriting or
send it to a recognition service. Manual mode is predictable when you know you are writing.

Actual text recognition belongs in an application with access to complete strokes and context.
[Windows Ink offers stroke recognition APIs](https://learn.microsoft.com/en-us/windows/uwp/ui-input/convert-ink-to-text),
but they are not invoked by this driver. Rendering pencil grain, stroke blending and typography
also belongs to the application. A driver can improve input continuity; it cannot guarantee the
same appearance or feel as an Apple Pencil system.

Filter reference: [Casiez, Roussel and Vogel's One Euro filter](https://gery.casiez.net/1euro/).
Our tuning and handwriting heuristic are experimental and require real-user testing.

## Combining smoothing controls

**StreamLine - Line shape** smooths the path over distance; **StreamLine - Pressure**
smooths thickness changes and cannot delay the coordinates by itself. The adaptive
filter runs first, followed by Motion filtering, StreamLine, then Stabilization.
These stages accumulate: strong settings together can add drag and flatten small loops.
They do not cancel each other, and pressure remains independent of position filtering.
Stabilization now weights fractional report intervals instead of rounding its time
window to a whole number of tablet reports.

A causal filter reduces small waviness while retaining broad deliberate curves; it
is not a straight-line drawing constraint. Graphite's own smoothing acts on the
already-filtered points, adding further smoothing and delay. To compare the driver's
effect, temporarily set Graphite's smoothing to zero, then adjust one driver control
at a time. See [the reproducible audit](../debug/SMOOTHING-AUDIT.md).

## Button 1 and Button 2

Open **Buttons** to assign the upper and lower side switches independently. Select
**Eraser (hold)** to temporarily report a pressure-sensitive eraser while holding
the button and touching the pen tip. Releasing it restores pen input. Applications
must support pen erasers; **Eraser tool** instead presses the E key once.

Left and middle click can be held for dragging/panning. Right click retains the pen
barrel-button behavior used by Windows and WinTab. Double click sends one pair per
press. Choose **Set shortcut...** to see the current/default combination, then press
one key or a combination and choose **Use shortcut**. **Record again** starts a new
capture; **Cancel** keeps the existing assignment. Captured keys are consumed by the
dialog, not sent to the drawing application. Selecting a preset restores its built-in
combination. **Reset settings** on this tab restores Button 1 to Right click and
Button 2 to None; pressure, mapping and smoothing settings stay unchanged.
Space panning has a separate hold action. The dropdown lists only built-in actions;
a recorded custom chord appears as the current value without adding a menu option.
Existing mappings remain unchanged until you choose another action. Assignments
save automatically and reach the running driver between strokes and button holds.

Shortcut capture is enabled only for keyboard actions (Undo, Redo, Eraser tool,
Brush, or an existing recorded shortcut). Held erasing, Space panning, mouse clicks,
and None disable the shortcut button so recording cannot replace those actions by
accident. Choose a keyboard action before assigning a different key combination.
