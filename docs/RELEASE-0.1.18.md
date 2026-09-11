<!-- Changes and validation limits for the shortcut-field usability correction. -->
# OpenCTL 460 0.1.18

- Keyboard shortcut fields stay editable for both buttons. Preset actions show
  their key combination (for example Undo shows Ctrl+Z).
- Typing a chord directly selects Custom shortcut automatically. Selecting a
  mouse or held-eraser action clears the field without changing that assignment.
- Incomplete or invalid chords preserve the last valid settings. Valid edits
  save automatically and apply after releasing the pen/buttons.
- Built-in shortcut display and execution use the same action-to-chord lookup.

Validation: 91 core tests passed, plus two WinTab mapping/packet unit tests.
The packet regression now checks both normal and eraser identity, contact and
pressure across the output range. Native GUI smoke checks passed for preset text,
direct custom editing, eraser selection, invalid edits and automatic persistence.
Clippy passed with warnings denied. The shared live-status test was excluded to
avoid interfering with the user's running feeder; the tray test used its no-shell
fallback in the test environment.

The live WinTab eraser capture received zero packets, so it did not establish
hardware-to-application eraser behavior. Source inspection finds that Graphite
currently drops pen/eraser identity. No Graphite files or running process were
changed. See [the Graphite handoff](GRAPHITE-HANDOFF.md) for the required application
changes; this release does not claim to fix erasing inside Graphite by itself.
