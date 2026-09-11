<!-- Changes and validation limits for the 0.1.17 button and WinTab precision update. -->
# OpenCTL 460 0.1.17

- New **Buttons** tab names the side switches **Button 1 (upper switch)** and
  **Button 2 (lower switch)**. Each supports hold-to-erase, left/right/middle click,
  double-click, existing drawing shortcuts, and a custom keyboard chord.
- **Eraser (hold)** sends an eraser tool through Ink, HID and WinTab while held.
  **Eraser tool (E shortcut)** remains a separate keyboard shortcut for applications
  using E to select an eraser. Tablet eraser behavior follows
  [Wacom's documented button action](https://101.wacom.com/UserHelp/en/ButtonFunctions.htm).
  The drawing application must support pen eraser input.
- Keyboard shortcuts support Ctrl, Shift, Alt and Win with letters, digits, F1-F24,
  Space, Enter, Tab, Escape, Backspace, Delete, Insert, arrows, Home/End and PageUp/Down.
  A chord fires once per press. Left/middle clicks and Space panning remain held
  until release. Shared assignments release after the last assigned button lifts;
  pre-held input is preserved, and partial shortcut failures unwind owned modifiers.
- Assignments save automatically and apply when the pen/buttons are released,
  without restarting the feeder. Existing assignments and custom pressure curves
  are preserved. Drawing profiles retain custom shortcuts.
- HID/WinTab mapping retains subpixel motion instead of rounding to screen pixels
  and expanding it back to digitizer units. WinTab timestamps now use precise system
  interrupt time. This avoids the 10-16 ms granularity documented for
  [GetTickCount64](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-gettickcount64).
  These are driver-side corrections; Graphite also needs the changes described in
  [the handoff](GRAPHITE-HANDOFF.md) to preserve fractional WinTab positions.

The core suite passes 91 automated tests, including fake-sink button lifecycle and
subpixel mapping regressions. The existing shared live-status test is excluded while
the user's feeder owns that mapping. Tests do not inject keyboard/mouse input into
the desktop. Native GUI smoke tests passed, including custom-shortcut entry,
invalid-edit protection, eraser selection and page visibility. A rendered preview
confirmed alignment, and Clippy passed with warnings denied. Real-application
click, eraser and smoothing behavior still needs a
hands-on check after installation. Graphite was not modified or restarted.
