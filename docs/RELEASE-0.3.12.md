# OpenCTL 460 0.3.12

Precision Hold now slows both hovering and drawing using the selected Movement reduction. The 0.3.10/0.3.11 behavior intentionally left hover at full speed, which was the missing effect identified during the live investigation.

Click the assigned side button once to enable or disable precision. A continuous mapping is shared between hover and contact, so touching down does not jump to the unscaled pen position. Both axes use a constant gain; distance does not increase the speed. Toggles during contact remain deferred until pen lift.

To reach other parts of the screen, lift the pen completely out of the tablet's sensing range, reposition it, then bring it back. The cursor remains at its last precision position and movement resumes from the new pen position. Repeat as needed. Toggling precision off restores normal absolute positioning. Screen-edge clamping responds immediately when direction reverses.

The settings-reset fix from 0.3.11 remains: adjusting Movement reduction or unrelated settings preserves the enabled toggle, and gain changes preserve cursor position. Removing the activating button's binding switches precision off. Zero reduction remains a bit-exact bypass. Pressure, smoothing, shape closure, and the existing no-tilt behavior remain unchanged.

## Verification

169 automated tests passed, including the exclusive-access stream check. Obsolete absolute-hover expectations were replaced with tests for reduced hover/contact, continuous pen-down positioning, constant-rate circles and vertical motion, lift-to-reposition coverage of all screen corners, immediate edge reversal, repeated-report stability, live settings changes, and both button assignments.

An isolated native Windows receiver tested the actual Output implementation through the installed HID service. At 75% reduction:

| Delivered movement | Normal | Precision | Ratio |
|---|---:|---:|---:|
| Hover | 434.81 px | 108.85 px | 0.25034 |
| Drawing | 445.22 px | 111.07 px | 0.24947 |

Windows delivered 168 pen events, two pen downs and two pen ups. Pixel rounding accounts for the small ratio differences. Input was generated automatically inside the foreground test canvas; the test aborts on focus loss. The receiver closes after the test.

The preceding physical-pen capture verified native side-button reports and 0.3.11's drawing reduction. The new 0.3.12 behavior was verified with the automated Windows receiver, not a physical drawing session inside Graphite Studio.

The installer contains rebuilt application executables. WinTab and the signed kernel package remain unchanged from 0.3.10. The diagnostic processes have stopped, saved user settings were not changed, and this installer has not been run automatically.
