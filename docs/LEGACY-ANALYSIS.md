<!-- Static reverse-engineering notes for the user-supplied Wacom installer; observations and inferences are separated. -->
# Legacy Wacom driver analysis

Input: the supplied `pentablet_5.3.5-3.exe`, 40,103,880 bytes.
SHA-256: `ABA1427B876598F016BFE68804C25FF72F01F9DEF4985C397549EDBC8BDEE5EC`.
Its Authenticode signature validated as Wacom Technology Corp. The self-extracting archive was
unpacked for static inspection; its installer was not executed. No instructions inside it were
treated as user instructions. Proprietary binaries remain outside this source distribution.

## Direct observations

The archive contains 32/64-bit user-mode components, `Wintab32.dll`, `Pen_Tablet.exe`,
`Pen_Tablet.dll`, service/helper software, a control panel and several kernel components.
`wachidrouter_con.inf` explicitly matches USB VID 056A / PID 00D4 and references WacHidRouter
with a hidkmdf upper filter. Other INF entries describe a virtual router and a mouse collection
filter. Installer metadata copies and starts the service/helper pieces and deploys WinTab DLLs
for both application architectures.

PE imports include HID capability/read functions, asynchronous I/O, threads, synchronization,
timing, pipes and file mapping. WinTab exports include the expected context and packet APIs
and their legacy ordinals. Those are public ABI facts; implementation code was not copied.

The x64 `Pen_Tablet.exe` contains curve-related names and diagnostics at these file offsets:

| Offset | Observed name or diagnostic |
|---|---|
| 0x57CD30 / 0x57CD58 | `EParameterIDPressCurveWintab` / `EParameterIDPressCurveWacom` |
| 0x57D028 / 0x57D110 | `PressureRange1` / `PressureThreshold1` |
| 0x57E8B0 | `ConvertToProcessedPressure` |
| 0x586E98 / 0x5A97C0 | Bezier and lookup-table curve source-file names |
| 0x5BC078–0x5BC150 | low/middle/high pressure control-point names |
| 0x5BC290 / 0x5BC968 | `PressureCurveControlPoint` / pressure-curve source-file name |
| 0x5BC9A0 | assertion comparing curve length with maximum pressure plus one |
| 0x5BC518 / 0x5BC550 | WinTab curve size and 256-limit assertions |

Selected machine-code references confirm a lookup-table length check: near RVA 0x223A8D,
code computes a vector byte span divided by two and compares it with maximum pressure plus
one. A separate reference near RVA 0x21F961 compares a curve header with 0x100.

## What can be inferred

These observations support configurable pressure curves, thresholds and per-input-value
16-bit lookup tables. The service/WinTab/kernel division and IPC imports support a layered
architecture. They do **not** establish the exact complete Bezier formula, every IPC message,
all report transformations or runtime latency. This is targeted static analysis, not a complete
decompilation or a claim to reproduce every original behavior.

Our pressure path is independently implemented from a calibrated monotonic gamma curve,
adaptive filtering and causal interpolation. It does not reuse Wacom lookup tables. Software
can fill intermediate pressure values in time and reshape the response, but cannot discover
unmeasured physical pressure or true tilt.

## Independent protocol references

Public OpenTabletDriver configuration describes the CTL-460's 14720 × 9200 coordinate range,
maximum pressure 1023, two pen buttons, native 9-byte and legacy 11-byte report formats, and
feature initialization `[02,02]`. Linux Wacom source also documents relevant proximity/ready
flags. These references were used for protocol facts; upstream GPL implementations were not
copied into this MIT project.

- [OpenTabletDriver CTL-460 configuration](https://github.com/OpenTabletDriver/OpenTabletDriver/blob/master/OpenTabletDriver.Configurations/Configurations/Wacom/CTL-460.json)
- [OpenTabletDriver Wacom parsers](https://github.com/OpenTabletDriver/OpenTabletDriver/tree/master/OpenTabletDriver.Configurations/Parsers/Wacom)
- [Linux Wacom report handling](https://github.com/torvalds/linux/blob/master/drivers/hid/wacom_wac.c)
- [Public WinTab API reference](https://developer-docs.wacom.com/docs/icbt/windows/wintab/wintab-reference/)
