# OpenCTL 460 0.3.17

Fixes calibration strokes not reaching the square and improves the guided layout.

The square previously decoded shared feeder coordinates as 0..65535 across the virtual desktop. The feeder actually publishes 0..14720 horizontally and 0..9200 vertically mapped to the primary display. As a result, a visible pen stroke could be classified outside the square, leaving the light-stage timer waiting. Calibration now uses the same coordinate units and primary-display mapping as the feeder.

After a valid Light stage, the instruction popup advances directly to Medium when OK is clicked; Medium advances to Firm in the same way. Cancel leaves the next stage ready to start manually. Recording still begins only with eligible contact inside the square. Final completion is announced, and unsuccessful captures now also show their retry reason in a popup.

The Raw pressure meter is a single line, without hovering, tip-contact, or out-of-range messages underneath it. The drawing pad heading, Clear, explanatory text, Apply calibration, and Undo apply align with the square. Beveled groups separate stage controls, the pad, and profile management.

Validation: 13 calibration/model tests and the native control integration test pass. New tests feed real driver-coordinate mappings at 1080p and 4K, verify Light reaches Medium, exercise all three page-completion handoffs, and check control alignment. Existing profile isolation, pad bounds, apply and undo checks remain passing. The final tab is rendered and visually inspected. Physical verification with the user's pen is still needed after installing this build.