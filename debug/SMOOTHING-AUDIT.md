<!-- Reproducible engine measurements for the 0.1.16 smoothing investigation. -->
# Smoothing audit

Run `cargo run --locked --bin ctl460-smoothing-audit` from the project root.
The replay uses the real Engine, default configuration with the listed controls,
125 Hz reports, 0.6 mm sinusoidal lateral displacement, and 15 mm/s horizontal
travel. Alternating physical pressure values 488/512 have a mean of 500. Values
below exclude the first 200 reports. They are synthetic measurements, not a
Graphite rendering test or an end-to-end latency measurement.

Drawing mode, 2 mm wavelength, each enabled control at 80%:

| Controls | Lateral RMS (mm) | Mean horizontal lag (mm) |
|---|---:|---:|
| Base adaptive filter | 0.390 | 0.098 |
| StreamLine | 0.096 | 1.143 |
| Motion filter | 0.109 | 0.020 |
| Stabilization | 0.384 | 0.154 |
| StreamLine + Motion | 0.020 | 1.531 |
| StreamLine + Stabilization | 0.094 | 1.200 |
| Motion + Stabilization | 0.109 | 0.074 |
| All three | 0.020 | 1.587 |
| Pressure slider only | 0.390 | 0.098 |

StreamLine reduces this small waviness by about 76% relative to the base filter,
but reduces a broad 10 mm wave by only about 28%. It preserves broad curvature;
it cannot infer whether a deliberate bend should be a straight line. Combined
filters can add substantial drag and flatten small features. Stabilization remains
a short time filter and has a relatively small effect at this pace; it is not a
replacement for distance-based StreamLine.

Manual handwriting also responds to all artistic sliders. For the same 2 mm
wave, its base lateral RMS is 0.404 mm, StreamLine gives 0.107 mm, and all three
give 0.023 mm. Individual filters need not improve every shape monotonically:
their adaptive behavior and rounding can produce small local differences.

Two implementation defects were corrected:

- The old Stabilization rounded its window to whole reports, sometimes selecting
  only the current report and doing nothing. The new time-weighted integration
  includes fractional intervals. A separate test compares straight motion at
  2, 4, 8, 12 and 16 ms report intervals and checks consistent displacement.
- The old pressure filter accelerated every small downward fluctuation. With
  pressure smoothing at 80%, the 488/512 replay averaged about 490 instead of
  500. Symmetric small-fluctuation filtering now gives 500 in drawing and manual
  handwriting. A continuous mapped-force decrease of at least 3% still takes
  the fast measured release path. Tip lift remains immediate.

The pressure slider alone leaves every output coordinate unchanged. Smoothing
inside Graphite acts on these already-filtered coordinates and can add more
delay. Compare one control at a time with application smoothing disabled.
