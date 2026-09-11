# Handwriting comparison build — 0.2.4-handwriting.1

This is a separate test build. The installed driver, Windows tablet components, and normal saved settings have not been replaced.

## Try it

1. Stop the driver in the original OpenCTL control panel.
2. Open **Start handwriting test.cmd** in the test-build folder. This opens a separate control panel without automatically starting tablet input.
3. Click **Start driver**. The build uses the already-installed Virtual HID service.
4. Write E, o, va and small connected words in Graphite. Keep Graphite stroke smoothing and both tapers at zero, and keep the brush, zoom and mapping fixed.
5. In **Line smoothing**, leave handwriting assistance On. Compare **Light smoothing**, **Responsive – experimental**, **Raw**, and **Original** using **Handwriting feel**. Lift the pen when changing a setting; changes apply after lift.

The test settings initially select Light smoothing and copy the current pressure curve, mapping and shortcuts. Original restores the original position pipeline within this test build. To return completely to the installed version, stop the test driver, exit its panel, then restart the original panel. Test settings stay in this folder and do not change the normal settings.

To record, open **Record handwriting test.cmd** instead. Each press of Start driver creates a new raw trace under settings/pen-trace-*.csv. Keep the settings fixed during one recording; stop the driver between comparison recordings. The recording ends after five minutes measured from the first report, or 120,000 reports, whichever comes first. Recording completion does not stop the driver. The window title identifies a recording session.

## What changed

Old profiles default to Original; there is no silent change to their behavior.

During handwriting, the three new choices bypass the previous wobble and artistic position stages:
- Raw follows original contact coordinates directly.
- Light smoothing uses one minimally tuned One Euro filter (40 Hz minimum cutoff, speed coefficient 0.7 in millimetres/second).
- Responsive uses one position filter with motion evidence from original coordinates, sustained-turn, reversal and slowing checks, and a total displacement budget. Wobble reduction controls its strength; 60% limits displacement to 0.072 mm plus at most 0.0071 mm rounding. Light smoothing uses fixed parameters.

The adaptive-smoothing checkbox disables position filtering in the new paths. Pressure smoothing remains independent. Hover remains damped separately, and the first contact point starts a fresh filter. Pen lift, tool change, timeout and reconfiguration reset contact history. New paths keep floating point state until their output boundary. Neither path predicts future points.

These numeric choices are experimental. Their purpose is comparison, not a claim of scientifically optimal tuning. The raw input remains the device's measured input; no algorithm recovers an unobserved intended trajectory.

## Validation

All 106 Rust tests passed, including eight new regression tests. Formatting and release builds passed. Coverage includes:
- A bound across the actual engine even when artistic position controls are nonzero.
- Equal pressure and contact outcomes across all four position choices.
- Hover-to-contact, release, stale input, tool flips, invalid settings and timestamp discontinuities.
- Unchanged drawing behavior and consistent left-handed rotation.
- Trace creation without overwriting existing files, recording raw versus effective tool flags, successful recorded-data replay, and rejection of incomplete, missing-report or old mapped-only recordings.

The UI was rendered for layout inspection. The test executables report their distinct version. No live handwriting trial with this build has been performed yet.

Synthetic replay used identical inputs, wobble 60%, handwriting On, zero artistic smoothing, and varying 6/8/10/8 ms sample intervals. Selected results:

| Measurement | Original | Raw | Light smoothing | Responsive |
|---|---:|---:|---:|---:|
| 0.4 mm radius loop: retained area | 77.48% | 100% | 99.46% | 99.55% |
| Straight stroke: mean trailing distance | 0.2232 mm | 0 | 0.0224 mm | 0.0274 mm |
| Stationary alternating ±0.05 mm input: RMS position error | 0.0058 mm | 0.0500 mm | 0.0256 mm | 0.0256 mm |

The original pipeline suppresses the synthetic stationary noise more strongly. Both new paths preserve motion better in these cases, at the cost of more residual jitter. Light smoothing matches or beats Responsive on most tested shape errors, so it is the initial trial choice. Responsive does not yet have demonstrated superiority. Area results use the last three synthetic revolutions and close the sampled polygon; quantization is included. These are algorithm measurements, not user-study results or total display latency.

## Raw recording and replay

The existing HID reader is the only reader of the physical tablet. Optional recording queues copies to a bounded worker; disk writes do not run on the acquisition thread. Trace files include original coordinates, pressure and flags, effective tool/button flags, host acquisition time, filtered coordinates, submitted pressure and host submission-return time. Profile markers capture settings. Sequence numbers and a completion marker expose lost or unfinished recordings.

The public WinTab shared-memory layout is unchanged. No new system DLL or kernel driver is required for this test.

Run an offline comparison:
```powershell
.\ctl460-writing-replay.exe --out .\comparison --trace .\settings\pen-trace-EXAMPLE.csv
```
Omit --trace to generate synthetic fixtures. Outputs are metrics.csv and points.csv. The tool forces manual handwriting and enables position filtering for each comparator while retaining the recording's other settings. It rejects recordings whose settings changed or which lost reports. It does not open hardware or inject input.

Acquisition time means host HID read completion, not the instant the sensor measured the pen. Submission return is not app receipt, render submission or pen-to-photon latency. This milestone does not add Graphite receipt/render instrumentation, and no display-latency measurement or predictive preview has been implemented.

Source changes are in src/writing_filter.rs, src/engine.rs, src/config.rs, src/trace.rs, src/transport.rs, src/main.rs, src/lib.rs, the control panel, scripts/hid_session.ps1, the new debug fixtures/replay/tests, and the Cargo manifests. Existing pressure, mapping, WinTab and kernel implementations remain unchanged.

The intermittent cursor freeze has not been reproduced or established as fixed by this change.
