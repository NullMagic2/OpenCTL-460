<!-- Close-to-tray reliability and recovery correction. -->
# OpenCTL 460 0.1.21

Close no longer refuses to hide settings when an edit is incomplete or invalid.
The last valid saved configuration remains intact, and the pending edit is retained
in the settings window for correction when reopened.

If Explorer temporarily refuses tray registration, Close minimizes to the taskbar
and retries registration every 1.5 seconds. Once the icon is available it hides to
the tray. Reopening cancels the pending hide. Explorer recovery no longer forces
settings open, and initial failed registration is retried. Closing does not stop
the feeder; the explicit tray Exit action remains the way to quit.

Native GUI tests exercise invalid-edit closing, preservation of settings/edits,
minimized no-shell fallback, retries and reopening. Actual Explorer tray registration
is skipped when the test desktop has no shell. Build and Clippy checks pass.
