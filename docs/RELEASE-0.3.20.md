# OpenCTL 460 0.3.20

Precision Hold no longer limits a single sweep to a smaller region inside the screen. Both hovering and drawing retain the selected movement gain in the working area. Near the reported sensor edges, a smooth displacement-based transition recovers the remaining screen distance. No additional setting is needed.

The transition normally occupies the last 6 mm of the sensor area; re-anchoring close to an edge reduces that width. Movement is faster in this region, especially with strong reduction. A stationary pen does not cause automatic panning. Hover and contact use the same mapping to avoid jumps at pen-down. Reposition clutching remains available; returning to an edge after re-entry does not leave that edge unreachable.

The live reproduction reported raw X = 14720 while precision output was X = 10969 on a 0..14720 output range. Raw Y also reached its sensor limit while output stopped inside the display. This established the reduced-range mapping as the cause of the captured wall. Windows cursor clipping covered the full 3840x2160 display. The mouse cursor trace is not treated as a substitute for Windows pen events.

Regression coverage includes one-sweep edge/corner reach across precision gains, portrait/landscape/ultrawide aspect mappings, stationary edge behavior, reverse movement, clutch re-entry, and the existing central-area precision geometry and pressure checks. A full-engine matrix covers 13,824 corner sweeps across handwriting modes and filters, normal/precision movement, smoothing on/off, flowing/legacy smoothing, both handedness settings, aspect modes, hovering/contact, pen/eraser/barrel tools.

The installer does not change saved user pressure calibration or settings. The fixed build still needs a physical-pen retest after installation.

Validation: 195 automated tests passed. The exclusive stream-writer status test was skipped because the user's driver remained running during validation.
