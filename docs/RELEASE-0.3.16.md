# OpenCTL 460 0.3.16

The Calibration tab now has a square drawing pad instead of the curve preview. The Pen tab retains the editable pressure curve.

Click Record light, Record medium, or Record firm to open that stage's instructions. After OK, only pen contact inside the square, with this control panel active, starts the timer and contributes pressure samples. Hover, mouse-only input, side-button actions, strokes elsewhere, and drawing outside an active stage are ignored. Lift between at least two strokes at each level. Use comfortable pressure; never press hard.

Each stage lasts eight seconds after its first eligible stroke. A popup confirms each completed stage and identifies the next level. A final popup announces completion. Apply calibration activates the measured response; Save as keeps a named profile. Cancel and Start over stop recording. Clear removes only preview ink, preserving measurements already collected in the current stage.

The square draws bounded pressure-sensitive preview ink from the same read-only driver stream. Stroke paths break on lift or boundary exit. Idle and completed stages retain the drawing but register no additional strokes. Pressure measurements remain raw regardless of the preview response. Dedicated .calibration_profile import/export and neutral installation defaults are unchanged.

Validation: 12 calibration/model tests and a hidden-window native control test passed. Coverage includes recording gates, idle/completed pad behavior, square dimensions, boundary/lift separation, bounded ink memory, pressure profile isolation, apply and undo. The final tab was rendered and visually inspected. Physical pen calibration through the new square and instructional popup flow still need a hands-on check after installation.