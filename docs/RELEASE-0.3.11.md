# OpenCTL 460 0.3.11

Fixed Precision Hold being silently disabled by live settings updates. Previously, changing Movement reduction or any unrelated setting reset the toggle to off. The toggle now survives settings updates, and the reduction follows the button that enabled it. Reassigning that button disables the toggle so it cannot become inaccessible.

Precision remains a click-to-toggle feature. Hover covers the full screen; movement reduction applies while drawing. Toggles during a stroke take effect on the next stroke. Existing saved settings retain their values.

Validation: 172 distinct Rust tests passed across the full suite and the added native-report replay. One additional test requiring exclusive ownership of the live input stream was skipped because the installed driver was running. The settings-reset regression failed before the fix and passed afterward. Tests cover both button assignments, unrelated edits, reduction changes, binding removal, full-screen hover, constant movement in both axes, and native button reports through drawing filters at 20%, 50%, and 90% reduction. The optimized Windows build passed. No physical drawing trial was performed.

The installer contains rebuilt 0.3.11 application executables. WinTab and the signed kernel package are unchanged from 0.3.10. The installer has been built but has not been run on this computer.

After installing, press the assigned Precision Hold button once, then draw a new stroke. For an obvious comparison, try 50% Movement reduction. Adjusting the slider should now preserve the enabled mode. Hover intentionally stays at normal speed.