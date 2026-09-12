# OpenCTL 460 0.3.21

Removed the sensor-edge acceleration introduced in 0.3.20. It increased movement independently on each axis near the sensor boundary, which could stretch circles and push strokes toward an edge.

Precision Hold now applies the same constant gain on both axes throughout the working area, including near sensor edges, in hover and contact. There is no automatic edge compensation. Toggle Precision Hold off to restore normal absolute movement. As before, toggles made during contact take effect after lift so an ongoing stroke is not interrupted.

The circle and edge-transition regressions reproduced the acceleration before the fix and pass after its removal. Coverage includes circles near all four sensor edges and corners in hover and contact, constant vertical movement, pen-down/lift continuity, normal mapping, and a 13,824-case full-engine mode matrix. Actual screen boundaries still clamp output and respond on reversal.

No pressure calibration, saved user settings, or calibration-tab layout is changed. The release was built for installation; physical-pen verification of this corrected build remains pending.

Validation: 195 automated tests passed. The exclusive stream-writer status test was skipped to avoid conflicting with a running driver.
