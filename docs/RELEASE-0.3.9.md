# OpenCTL 460 0.3.9

Replaces the strong drawing mode's Motion Filtering stage with a real FFT-based filter and improves the StreamLine and Stabilization strength mappings.

## What changed

- Motion Filtering now resamples the measured path at uniform 0.1 mm intervals and applies a windowed-sinc magnitude response using a 512-point FFT. The input history is bounded; mirrored endpoint padding lets it operate on live input. The old spatial low-pass remains available when strong artistic smoothing is disabled.
- StreamLine uses a nonlinear following fraction, 1 - 0.96 * amount^(1/8), converted to a distance-based response at a 0.25 mm reference step. This gives low and middle slider positions more effect than the former squared-distance curve.
- Stabilization uses amount^2.2 to scale its moving-average window, giving more separation between mild and strong settings. Its speed estimate remains smoothed and its live averaging window is bounded.
- Expression changes the spectral pass band and window sharpness at intermediate amounts. Amount 100% takes precedence over Expression.
- Spectral processing precedes StreamLine, which precedes Stabilization. Endpoint settling, shape closure, the start marker, Precision Hold, and pressure handling remain available.

Open Drawing aids and enable Allow strong artistic smoothing on lines and curves. Use Handwriting assistance Off - drawing settings when drawing shapes. High values add lag and round small features. Settings from 0.3.8 load, but the same percentages can feel different because the response curves changed.

## Evidence and limits

Deeper static analysis of the supplied 5.3.15 reference package confirmed:
- Separate frequency-domain, following, and moving-average stages in that order.
- A positional following fraction with exponent 1/8 and coefficient 0.96; a separate pressure mapping.
- A stabilization count proportional to amount^2.2.
- A windowed-sinc construction, a four-term cosine window, FFT magnitude normalization, padding, and overlapping centered processing.
- Separate committed and predicted point handling.

This is an independently written live-driver adaptation of those findings. The reference's time-domain sample rate, full slider-to-internal-value mapping, prediction, and redraw pipeline are not reproduced. Its internal frequency mapping includes a power of 0.01; applying that directly to our public slider and tablet coordinates would not establish a match. Our frequency cutoff uses an explicitly documented driver calibration.

The reference processes and revises buffered points. A system driver cannot replace strokes already committed inside an arbitrary drawing application. We therefore use spatial resampling, mirrored live endpoints, bounded displacement, and the existing contact-only settling. No third-party executable or disassembly is included.

## Validation

162 Rust tests passed, including:
- Independent direct-DFT checks of FFT coefficients and recovery of both packed coordinate axes.
- Constant-position preservation, rotation/translation equivariance of the spectral stage, collinear report subdivision, and slider cutoff ordering.
- Pace independence, expression behavior, short-wavelength rejection, broad-curve preservation, bounded reversals, and no drift with settling disabled.
- Actual Engine line/circle/S-curve replay at 0%, 50%, and 100% for each control and their combination.
- Resuming after endpoint settling without inheriting an old spectral tail.
- Endpoint settling, pressure/release invariants, closure, marker lifecycle, and full-screen Precision Hold.

Native Drawing aids and Precision Hold GUI checks pass. At maximum Motion Filtering, the frequency replay removes the tested 0.6 mm oscillation and retains the tested broad 20 mm variation within about 4%. Those are synthetic test cases, not universal perceptual guarantees.

A release-build benchmark processed 20,000 reports with all three controls at 100%, averaging 8.491 microseconds per report on this machine. This measures filtering CPU cost, not USB, application, display, or end-to-end latency.

No physical-tablet drawing trial or complete reference-application output equivalence test was performed. Exact visual parity remains unverified.
