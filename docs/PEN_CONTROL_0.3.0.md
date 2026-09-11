# OpenCTL 460 version 0.3.0

## Using the new control

Open the **Pen control** tab. One slider now handles contact-position filtering for both drawing and handwriting.

- **30%** is the Light smoothing behavior from 0.2.4-handwriting.1, verified sample for sample in the regression fixtures.
- Move toward **Steadier** for stronger sideways damping. **65%** is a useful first comparison for your circles.
- Move toward **Freer** for more direct following. **0%** is unfiltered contact position.
- **Reset to Light** returns immediately to 30%, applying after pen lift.

Pressure sensitivity and mapping stay separate. The slider does not lower the absolute screen mapping speed; the 112% Graphite zoom that helped remains an independent app setting.

Older individual filters are under **Advanced...** on the Pen control page. Opening that view does not switch processing. Click **Use advanced smoothing** to activate the old controls; that choice persists after restarting. Returning to the main page also does not switch processing until you move the slider or choose Reset to Light. No separate curve mode or handwriting detector is needed for the unified position filter.

Ordinary legacy profiles with adaptive smoothing enabled and no artistic position filters upgrade to 30%. Existing Light/manual profiles also upgrade to 30%. Explicit custom, Raw, Responsive, automatic-detection and disabled-smoothing profiles retain their older behavior until you choose Pen control. Pressure, mapping, shortcuts and output backend are retained.

## Downloads and startup

The setup executable is the normal installer. It retains the named-application check before replacing tablet components. Save your work and close any drawing apps it reports before retrying. The installer has been built, not run on your machine as part of this release.

The portable ZIP uses the Virtual HID service already installed on this computer. Extract it, stop the previous driver, then open **Start OpenCTL 460.cmd**. This launcher starts the driver automatically. **Open settings only.cmd** opens the panel without starting input. **Record pen test.cmd** enables a bounded raw trace; click Start driver in that recording panel.

Portable settings stay inside the portable folder. Its initial settings copy your working Light profile, with Pen control at 30%. To compare, keep the brush, zoom and pressure settings fixed and move only Pen control. To return to the previous portable version, stop this driver and launch the earlier version.

## How the steadier side works

A single floating-point filter uses the original tablet coordinates. It estimates motion direction and applies stronger damping perpendicular to that direction than along it. A short history of raw points helps distinguish alternating jitter from sustained travel; this is not a queue that delays delivery. Deliberate turns and reversals reduce damping temporarily. Curvature constrains sideways displacement, and settings above 30% cap total displacement from the original sample at 0.10 mm plus output rounding (at most 0.0071 mm).

First contact starts at the actual measured position. Lift, tool changes, stale input and reconfiguration reset contact state. The filter does not fit circles, invent pressure, predict future samples or finish strokes after pen lift. Pressure processing remains separate.

30% uses the original Light formula without the additional directional damping. Values above 30% are a trade-off: they suppress more noise but can add displacement and slightly change curve shape. They are not guaranteed to improve every stroke or writer.

## Measured validation

All **114 Rust tests passed**. Tests used a separate shared-memory namespace so your active driver did not need to stop. Release binaries and formatting checks passed. The installer package tests and mocked Windows component upgrade/restoration tests passed. Both normal and Advanced layouts were rendered and inspected.

The eight new tests cover Light equivalence, noisy circles/ellipses and stationary input, pressure independence, the complete engine displacement bound, contact/tool/timeout resets, migration and persistent custom settings, rotation, different report intervals, and clean loops at multiple sizes.

Selected synthetic replay results, including initial samples:

| Measurement | Light / 30% | 65% | 100% |
|---|---:|---:|---:|
| Noisy circle: RMS error from known path | 0.02784 mm | 0.02148 mm | 0.02022 mm |
| Noisy ellipse: RMS error from known path | 0.03077 mm | 0.02597 mm | 0.02443 mm |
| Stationary alternating noise: RMS error | 0.02558 mm | 0.01616 mm | 0.01065 mm |
| Clean straight: mean trailing distance | 0.02243 mm | 0.02248 mm | 0.02250 mm |
| Small clean circle: output/input area | 99.46% | 101.09% | 101.42% |

At 65%, that noisy-circle fixture has approximately 23% lower error than Light. The small clean circle expands slightly rather than collapsing. These are synthetic fixture results, not measured improvements in your handwriting or end-to-end display latency. The tuning fixtures were used during development; they are regression checks, not an independent user study. A live comparison is still needed.

Replay data is in replay/metrics.csv and replay/points.csv. The offline replay tool includes the old comparators and control30, control65 and control100. Raw recording keeps the same opt-in bounded worker and unchanged public WinTab stream layout as the prototype.

The user-mode feeder, control panel and package version are 0.3.0. The existing WinTab DLLs and virtual HID kernel package are unchanged. This release does not establish a fix for the earlier intermittent freeze.
