<!-- Describes the installed Virtual HID broker, its permissions and verification limits. -->
# Virtual HID service

Version 0.1.11 installs OpenCTL460Broker once through the administrator installer. Normal
Start driver operations connect to this service without requesting elevation. The GUI,
physical tablet reader, settings and WinTab shared stream remain in the interactive user
session. Only the fixed Virtual HID transport and Wacom service handoff run as LocalSystem.

The service executable resides in Common Files/OpenCTL460 even when the application is
installed elsewhere. Its local named pipe accepts one active-console interactive client,
an eight-byte version handshake and validated ten-byte pen reports. It rejects remote
connections and clients from other sessions. It does not accept commands, executable paths,
settings paths or arbitrary service names. The kernel control device remains restricted
to administrators and SYSTEM. Remote Desktop sessions are not supported by this broker.

Before handing over input, the broker pauses only running TabletServicePen and
TouchServicePen services. On disconnect, shutdown or an invalid stream it releases the
virtual pen and restores the services it paused. Restoration failures are recorded in the
Windows Application event log. The kernel watchdog also releases stalled input.

The installer installs the bundled test certificate and kernel package, then starts the
broker automatically. It does not change Secure Boot, Test Mode or UAC. Existing signing
prerequisites still apply; see SIGNING.md. Run the latest installer to upgrade or repair
the service. Uninstallation removes the service before the virtual device.

The debug broker test verifies local packet exchange, validation and cancellation using
an isolated test pipe. It does not install or execute the service as LocalSystem. Service
installation, Wacom handoff and real pen latency still require testing on Windows with
the installed kernel package; passing unit tests alone does not verify these operations.
