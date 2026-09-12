# OpenCTL 460 0.3.15

Calibration profiles now use the dedicated filename extension `.calibration_profile`, for example `Pencil.calibration_profile`.

- Export suggests `pressure.calibration_profile`; the name can be changed.
- Import and Export use a dedicated calibration-profile file filter and validate the selected extension.
- Profiles contain only a format version and pressure response: floor, ceiling, gamma, gain, and curve nodes. They contain no mapping, buttons, drawing assistance, smoothing, startup, or other user settings.
- Import no longer accepts full settings TOML files, even when renamed with the calibration extension. Unknown profile fields, unsupported versions, and invalid pressure curves are rejected.
- Existing named pressure profiles and active settings are retained. Applying a calibration still updates the active pressure response through the existing driver settings mechanism.

The guided recording and preview/apply/save/load/undo workflow introduced in 0.3.14 is unchanged. Native file picker interaction still needs a hands-on check after installation.

Eleven calibration tests and the hidden-window native control integration test pass, including rejection of settings files, pressure-only round-tripping, extension checks, profile save/load, apply, and undo.