<!-- Explains artistic smoothing controls, independent implementation, and tuning limits. -->
# Line smoothing

Choose **Smooth inking** in the settings app, then the **Line smoothing** tab.
Adjust the sliders; valid edits save automatically and apply between strokes without restarting.
The five artistic controls default to zero. Existing nonzero values now also affect handwriting.

The controls use separate causal filters for path flow, measured pressure,
speed-sensitive averaging, and spatial noise suppression. Percentages are this
driver's tuning scale. Drawing feel still requires evaluation with the tablet.

| Control | Our implementation | At high values |
|---|---|---|
| StreamLine Amount | Distance-based exponential follower; maximum characteristic length 2.5 mm | Flowing curves with a trailing pen and softer corners |
| StreamLine Pressure | Adds up to 60 ms of rising-pressure response time | Gradual pressure transitions; the first dot and release remain immediate |
| Stabilization Amount | Bounded 32-sample moving average, with a speed-dependent window up to 28 ms | Straighter fast strokes; some fine detail is lost |
| Motion Filtering Amount | Compensated two-stage spatial low-pass filter, with total displacement bounded to 0.8 mm | Less short-wavelength wobble; small details can be softened |
| Motion Filtering Expression | Restores part of the removed displacement | More character; no effect with filtering off or at its maximum |

The **Smooth inking** starting values are 35 / 20 / 20 / 25 / 50, respectively. Tune one control
at a time. The preset disables the older adaptive stroke filter so its speed response does not
mask the new controls. That filter remains separately available on the Line smoothing tab.
Motion Filtering alone uses distance, not time; combining it with Stabilization or the older
adaptive filter makes the overall output speed-dependent.

Manual handwriting and automatically detected writing apply the artistic sliders too.
Coordinates pass through the optional adaptive handwriting filter first, then Motion
Filtering, StreamLine and Stabilization. Zero artistic amounts preserve the handwriting
filter's fine-turn behavior. Higher amounts intentionally add smoothing and displacement;
the handwriting stage's 0.12 mm bound does not constrain the combined result. Expression
continues to adjust Motion Filtering. Turning off the adaptive checkbox does not disable
the artistic controls. Auto switches the adaptive mode only between strokes.

StreamLine Pressure smooths measured force in both drawing and handwriting. Handwriting
keeps its shorter base timing, then adds the selected pressure smoothing. Falling force
uses at most a 2 ms filter time constant and skips the extra interpolation ramp, so a
release does not hold the previous thick width. This is measured-pressure tapering, with
no look-ahead buffer, speed-derived width, or artificial post-lift tail. If the tablet
jumps directly from substantial force to pen-up, a perfectly pointed tip cannot be
reconstructed from those reports without app support or delaying earlier output.

Processing runs once per valid contact report before either Windows Ink or virtual HID output
and before WinTab publication. There is no look-ahead, sleep, extra thread, or stroke replay.
Memory and work per report are bounded. The pressure scheduler still interpolates between reports.
Filters reset on release, tool changes and stale input. Hover movement is not artistically filtered.

A driver cannot revise ink an application has already rendered or reproduce brush texture and
taper behavior. These controls therefore affect future pen coordinates and pressure globally.
An application's own smoothing can compound the effect. This version has no per-application or
per-brush smoothing profiles and no retrospective endpoint correction.

Behavioral regression tests live in `debug/line_smoothing_tests.rs`; native slider persistence and
page switching are checked in `debug/gui_smoke.ps1`. Real CTL-460 input and our Ink canvas worked
before this feature was added. These new smoothing settings still require subjective drawing
evaluation; subjective drawing feel has not been measured.

## Combined protection in 0.3.5

The final contact output now has a total 1 mm displacement limit. Corner and loop
protection can tighten it further. This final bound applies even when several
filters are combined. See [settings and limits](SMOOTHING_GUARD_0.3.5.md).
