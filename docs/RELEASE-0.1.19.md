<!-- Version discovery and native shortcut recording update. -->
# OpenCTL 460 0.1.19

- The sidebar shows the version beneath the settings title. The driver supports
  `--version`, `-V` and `version`, reporting the queried executable's product version
  without accessing the tablet or loading settings. Both use Cargo's package version.
- Shortcut textboxes are replaced with **Set shortcut...** buttons. The modal dialog
  displays the current/default combination and records a key or modifier combination.
  Confirm with **Use shortcut**, retry with **Record again**, or cancel without changes.
  Confirmation waits for key release; Enter, Escape, Tab and Space can themselves be
  recorded. Keys are handled locally, with no global keyboard hook or input injection.
- Action lists show simple names such as Undo, Redo, Brush and Eraser tool.
  Existing configuration strings remain compatible. Selecting a preset uses its
  original default combination; recording a replacement selects Custom shortcut.
- **Reset settings** on Buttons restores Button 1 to Right click and Button 2 to None.
  Pressure, mapping, smoothing and pressure presets are preserved. Confirmed changes
  save automatically and apply between strokes/button holds.
- Common punctuation and number-pad keys can also be captured and assigned.

Validation: 92 core tests passed, including captured-key parser round trips.
Native GUI checks cover chord capture, key release, retry, cancellation, single-key
shortcuts, automatic persistence, button-only reset and sidebar version. All three
version command forms succeed without loading a deliberately nonexistent config.
Clippy passes with warnings denied. The live-status test is excluded while the user's
feeder owns the shared mapping; tray tests use their no-shell fallback when required.

No pressure/smoothing behavior or Graphite files were changed.
