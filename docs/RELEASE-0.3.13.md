# OpenCTL 460 0.3.13

Normal drawing precision and Windows login readiness improvements. No new setting or button binding is required.

## Drawing

- Small sideways fluctuations in a sufficiently straight stroke receive bounded, direction-independent assistance. It follows the drawn heading rather than snapping to horizontal or vertical. Clean arcs, corners, reversals, stroke starts and hover are protected. Existing raw/off settings bypass it; recognized handwriting and eraser/barrel actions bypass this straight-line stage.
- Existing endpoint settling now also catches up when the pen both slows down and releases pressure while still in contact. This helps reduce smoothing-created gaps when joining separate sides of a square. Pressure alone or slowing alone does not activate this extension. It changes filter state, not the pressure curve, and generates no extra ink after lift.
- Precision Hold keeps the working 0.3.12 hover/drawing mapping and gain behavior.

## Windows startup

The installed shortcut and automatic Windows service were present during diagnosis. The GUI previously consumed its one startup attempt even when USB enumeration had not found the tablet. A login launch now checks for the tablet every two seconds for up to one minute. The session helper also waits up to 30 seconds for a service already in StartPending. Stop cancels waiting; manual launches and permanent errors do not enter a restart loop.

This addresses startup timing weaknesses found in the code. The exact cause of the reported missed startup could not be established from the available log, and an actual reboot with this release has not yet been tested.

## Recorded findings and validation

Two physical captures contained 1,619 oval reports and 7,193 shape reports, with zero rejected or dropped reports. Most broad oval drift and bowed square sides were already present in raw tablet coordinates. These captures did not establish a consistent driver-created upward displacement.

Replaying the square capture through the new processor reduced median endpoint following error from approximately 0.60 mm to 0.43 mm on the tablet. This measures distance to the last real contact report, not geometric accuracy against an intended square. It is a replay result, not a claim that live drawing feel has already been validated.

The straight-line helper alone changed this capture only slightly (maximum about 0.03 mm). It is intentionally conservative and will not turn a bowed stroke into a ruler-straight edge or snap separate strokes together. Synthetic wandering lines showed about 34% less lateral RMS error through the normal pipeline. Clean synthetic circles, ellipses and spirals were unchanged by that helper.

178 automated tests passed, including direction/reflection checks, straight-line noise reduction, curve preservation, contact/pressure/lift behavior, endpoint following, startup timeout/cancellation, and Precision Hold. One unrelated exclusive shared-memory status test was omitted while the user's normal driver was running. PowerShell launch scripts passed syntax validation. Release executables and installer were built successfully.

## Install

Run the 0.3.13 installer to apply these changes. The currently running installed driver was restored after each recording and has not been replaced by this build. Existing settings are retained. Close drawing applications when requested by setup.
