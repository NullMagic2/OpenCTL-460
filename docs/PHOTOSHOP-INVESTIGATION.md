# Photoshop 2021 / supplied Wacom driver investigation

## Evidence, 2026-09-10

The user supplied `pentablet_5.3.5-3.exe`. Windows validates its Wacom Technology
Corp. signature. Installer SHA-256:
`ABA1427B876598F016BFE68804C25FF72F01F9DEF4985C397549EDBC8BDEE5EC`.
Fresh extraction of x86/x64 Wintab32.dll and Pen_Tablet.dll matches the previously
extracted files byte for byte. Inspection was static; the installer was not run.

The x64 Pen_Tablet.dll packet wrapper delegates WTPacketsGet (RVA BC460) to its
context packet operation at C11E0. The operation validates the context and count,
supports discard with a null output buffer, and returns a packet count. OpenCTL
supports those observable API semantics. These observations do not certify all
of the vendor's internal routing or timing behavior.

The supplied Pen_Tablet.dll implementations of WTMgrPacketHookExA/W return zero
without using the supplied hook arguments. The x64 bodies start at BB8B0/BB9D0;
their returns at BB989/BBAA9 clear EAX. WTMgrPacketHookNext similarly returns zero.
There is no working record/playback callback contract to recover from these bodies.
OpenCTL continues to report those optional legacy APIs as unsupported.

## Photoshop trace

Photoshop 2021 was configured with `UseSystemStylus 0`. It loaded the installed
OpenCTL 0.2.0 DLL and opened an enabled context with options 13, packet mask 0x1FFC,
motion mask 0x1F80, and 61440-by-34560 output over a 3840-by-2160 screen.

A debugger trace reached Photoshop's own WTPacketsGet caller. It requests 128
packets, then iterates returned packets in 52-byte steps. It queries normal-pressure
button mapping, axis range, cursor identity and cursor type. The live pressure
range was 0..4097 and press threshold was 1. Its packet conversion reads pressure
at offset 0x20 and buttons at offset 0x10, matching OpenCTL's selected layout.
The conversion feeds Photoshop's internal tablet-event queue. Therefore loading
the DLL, creating the context, and producing pressure are not the missing steps.

The OpenCTL packet buffer in Photoshop contained tip-down events with nonzero,
changing pressure (for example 2889, 3147 and 3602), followed by zero-pressure
tip-up events. The queue was drained. This does not prove that the brush consumed
the events or rendered strokes. Lazy Nezumi was loaded in the traced session;
the user also reported failure after trying a startup with plugins skipped.

Photoshop's conversion uses cursor-number modulo 3 to mark inverted tools.
OpenCTL's current 0/1 pen/eraser numbering does not match that convention. This
is a separate, identified eraser-compatibility concern, not proof of the missing
normal strokes. It has not been changed in this candidate.

## Candidate change and limitations

OpenCTL 0.2.0 supplied Windows pen events concurrently with WinTab packets and
relied on native pen-to-mouse promotion for default mouse buttons. Version 0.2.1
provides explicit classic mouse input to the foreground owner of an enabled
pressure system context, while continuing the WinTab pressure stream. Other apps
retain Windows pen input. The route stays fixed during contact and release.
Input data is published before the matching Windows event is submitted.

The user subsequently confirmed that Photoshop strokes work with version 0.2.1.
That confirms the routing change resolved the reported failure in their setup;
it does not establish eraser compatibility or universal application coverage.
No Photoshop or Graphite executable, plugin, preference file or source was patched.
The debugger was detached and breakpoints removed after the live trace.

Automated tests cover foreground eligibility, contact/release route continuity,
classic tip/barrel button transitions and release cleanup. The existing packet,
coordinate, pressure, queue and x86/x64 ABI suites remain required.
