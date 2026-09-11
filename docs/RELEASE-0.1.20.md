<!-- Pen-tab cleanup requested by the user. -->
# OpenCTL 460 0.1.20

Removed the redundant Customize Button 1 and 2 block, Open Buttons tab button,
and Eraser, clicks and shortcuts description from the Pen tab. Button assignments
remain available through the existing Buttons tab.

Removed Custom shortcut from the action dropdowns. Use Set shortcut... to record a
combination; the current chord is displayed without becoming a selectable menu item.
Existing custom assignments remain compatible and save automatically.

Set shortcut... is disabled for Eraser (hold), Pan (hold), mouse clicks and None.
Keyboard actions and existing recorded shortcuts remain editable. Contextual help
explains held erasing/panning. Native GUI regression checks cover these states and
custom-chord persistence with no Custom shortcut dropdown entry.
