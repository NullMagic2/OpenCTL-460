<!-- Explains the settings introduced in OpenCTL 460 0.1.4 and their verified limits. -->
# OpenCTL 460 settings

The GUI renders at per-monitor DPI v2 and rescales its native controls, fonts, graph and
images when Windows reports a monitor DPI change. The window opens centered in the active
monitor work area. The supplied artwork is used for the pen, normal/180-degree tablet
orientation choices, and application/installer icon. Settings retain the existing
LocalAppData/CTL460Studio location and installer identity for upgrade compatibility.

## System tray
Closing the settings window hides it in the notification area without stopping the driver.
Click the OpenCTL 460 icon to restore the window. Its right-click menu has Open settings
and Exit; Exit uses the existing graceful shutdown and Wacom-service restoration path.
The icon is recreated when Explorer restarts and removed when the app exits. If no tray
is available, settings stay visible so the user can still reach the controls.
This does not enable automatic startup at Windows sign-in. GUI tests verify callbacks,
explicit exit and the no-Explorer fallback on isolated desktops; real tray registration
and hiding checks run when Explorer is available.

## Consistent pressure

From 0.1.14, measured tip pressure controls stroke thickness again. The same measured
pressure produces the same target output through the selected curve, regardless of
stroke speed or direction. Pressing harder/lighter retains deliberate thick/thin control.
The speed-driven pressure introduced in 0.1.11–0.1.13 is removed. Its old speed_pressure
setting is accepted for upgrade compatibility, ignored, and omitted when settings are saved.
No mode switch or manual configuration edit is required.

Light smoothing and interpolation reduce small force jitter and preserve intentional
pressure transitions. A fresh stroke starts at its measured pressure; no fixed midpoint,
velocity-derived pressure or invented taper is applied. Pressure falls as the measured
force falls. Lift releases immediately without a trailing synthetic stroke. Smoothing
can briefly lag a pressure change, and cannot recover force changes absent from USB reports.

Both drawing and handwriting use this pressure response. Handwriting retains its lower
base pressure timing limits and applies the artistic smoothing sliders afterward. Simulated tilt does not alter
pressure output; an application's tilt-aware brush can still change its footprint with
orientation. Texture and other application brush dynamics remain application-controlled.

A continuous mapped-pressure decrease of at least 3% uses at most a 2 ms filter
time constant and no additional interpolation ramp. Small alternating fluctuations
use symmetric smoothing so they do not bias pressure downward. Lift is
submitted at the last contact position before hover movement in Ink, HID and WinTab.
No future samples are buffered and no trailing contact is synthesized.

## Pressure presets
Default is a straight diagonal across the full pressure range, with output equal to normalized input pressure. It matches Reset curve. Default is built into the program and cannot be edited, overwritten or deleted. Drag the graph directly to create an unsaved copy, or select New
to create a new straight diagonal with three starting handles. Decorative bevels stay behind the controls and never intercept clicks. Add node inserts exactly one node in the widest available input interval. Clicking between existing nodes in an editable node curve adds a node at that input position. Drag a
node to move it; select an interior node and use Remove node or Delete to remove it. Arrow
keys move the selected node. Between 2 and 16 nodes are supported. Drag the two end nodes
left/right to adjust the zero/full input points, including by grabbing the outer half of
the dot along the border. Endpoints remain at zero/full output and cannot be deleted.
Advanced pressure zero/full settings edit the same input range.

Interpolation uses continuous monotone cubic Hermite curves. Nodes cannot cross or reduce
pressure as input increases. The graph and actual feeder share this evaluator. Soft/Firm
reshapes the editable nodes; gain is applied afterward and clamps the output at full pressure.
Older gamma-only configurations still work. Save as asks for a new name and creates a preset;
it never overwrites an existing name. Custom presets are stored in pressure-profiles.toml
beside settings.toml. Deleting removes the library entry. The selected curve and other
valid settings save automatically and apply live after pen contact and held side buttons
end. Save as remains available to name a preset; it is separate from automatic settings
persistence. Drawing profiles and pressure presets are separate selectors.

**Reset curve** returns the graph to a straight diagonal with three handles. It removes
extra nodes and restores the full input range (0-1023), gamma 1 and gain 1, so output
pressure equals normalized input pressure. It creates an unsaved copy; Default and saved
presets remain intact. Other pen settings are preserved. The reset applies automatically
between strokes; Save as stores a named pressure preset.

## Eraser button
Assign Erase (hold) to either side button. Holding the button temporarily makes the tip an
eraser; releasing restores the pen. Proximity loss, timeout and shutdown release input.
Switching tool while touching ends the old contact before starting the new tool. Pressure
uses the selected pressure curve for both tools. Erase takes precedence over a simultaneous
right-click assignment. No E/B keys are generated by this assignment.

Windows Ink receives eraser/inverted flags. Virtual HID reports invert/eraser usages.
WinTab reports separate Pen/Eraser cursor identities, inverted status and cursor-change
notifications. Applications must support and honor eraser input. The legacy Eraser (E)
action is still an explicitly separate shortcut and depends on the application's bindings.

Sources: [Wacom button functions](https://101.wacom.com/UserHelp/en/ButtonFunctions.htm),
[Windows pen flags](https://learn.microsoft.com/en-us/windows/win32/inputmsg/pen-flags-constants),
[Wacom WinTab reference](https://developer-docs.wacom.com/docs/icbt/windows/wintab/wintab-reference/).

## Double-click distance
Wacom describes this as the allowed cursor movement between taps; a larger setting can
interfere with stroke beginnings. OpenCTL independently implements bounded pen-only
assistance rather than changing Windows' global mouse double-click settings.

Off leaves coordinates untouched. A nearby second tap inside the current Windows
double-click time is anchored to the first short click's position. The anchor ends on lift,
excessive movement, timeout, eraser/barrel mode, handwriting classification or proximity loss.
First strokes and hover are never delayed. A larger distance can hold the beginning of a
second stroke briefly at the first point; choose Off for drawing when this is unwanted.
The app or Windows still decides whether the resulting events count as a double-click.
This is not a claim of binary-identical Wacom driver behavior.

Source: [Wacom double-tap distance documentation](https://101.wacom.com/UserHelpPDF_Legacy/DTK-2700_en.pdf).

## Verification
Automated tests under debug cover monotone curves, preset persistence/protection,
eraser mapping and lifecycle, double-tap anchoring/release, actual 32/64-bit WinTab DLL
packets, and native GUI save/delete operations. Visual previews inspect the settings pages.
The new eraser and double-tap behavior still needs application and real-tablet testing;
no kernel loading or system-driver replacement is performed by these tests.

Opening the settings application starts the driver using the current settings.
Reopening it from the tray starts it if stopped; reopening the shortcut brings the
existing window forward. Stop driver keeps it stopped until you start it or reopen
the window. A failed or cancelled start is not retried continuously. Virtual HID
uses a service installed once with administrator approval; startup has no UAC prompt. Debug/preview launches with an explicit
settings directory do not start the driver automatically.

Live handwriting status reports the running feeder's configured mode separately
from current stroke assistance. On (manual) remains enabled while the pen is idle;
Auto alone shows heuristic evidence. Changes apply between strokes without restarting
the feeder; the status confirms when the running settings have been updated.

## Automatic settings
The main Save settings button has been removed. Valid edits are atomically saved within
about 150 ms. A separate worker checks for changes every 100 ms, away from pen acquisition
and output, then the feeder applies the newest complete settings between strokes and
button holds. The status confirms application or reports a failure. Invalid or unfinished
numbers retain the previous valid configuration. Slider changes do not reopen the tablet,
restart the feeder, or disconnect WinTab consumers. Pressure presets still use Save as
when you want to keep a named curve. The settings also persist when the driver is stopped.

Changing output API opens the new backend before retiring the old one, using the same
feeder and WinTab stream. A failed backend connection leaves the previous output active
and reports the error; changing settings never silently installs a missing service. The
HID service handoff is retained when switching to Ink so Wacom cannot reclaim the reader.
Legacy report framing is saved alongside other settings and can change while lifted.
