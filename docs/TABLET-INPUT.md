<!-- Documents the physical pen initialization protocol and the limits of acquisition diagnostics. -->
# Physical tablet input

`run` now always requests pen mode, reads feature report 2 back, and checks for
`02 02`. A failed exchange or a different mode is retried up to three times.
`check` performs the same handshake without injecting input. A successful mode
check does not prove that input reports arrive.

The protocol is documented in the upstream
[OpenTabletDriver CTL-460 configuration](https://github.com/OpenTabletDriver/OpenTabletDriver/blob/master/OpenTabletDriver.Configurations/Configurations/Wacom/CTL-460.json)
(initialization `AgI=` = `02 02`, native report length 9) and the Linux
[Wacom mode initialization](https://github.com/torvalds/linux/blob/master/drivers/hid/wacom_sys.c)
(`wacom_set_device_mode` performs SET followed by GET and retries).

The feeder distinguishes an empty input stream from reports rejected by the
selected decoder. A wrong Legacy 11-byte setting can cause the latter; it cannot
explain an empty stream. Virtual HID status describes the output device, not the
physical USB input connection.

For a bounded diagnostic without cursor injection, run `debug/capture_hardware.ps1`.
It selects the pen collection automatically unless `-Device` is supplied, records
raw and decoded reports in `debug/generated`, and stops after 45 seconds by default.
Capture allows 60 seconds for first input. Normal feeder startup stays ready
until input arrives or Stop driver is clicked. After 15 seconds it logs a waiting
message instead of shutting down. While waiting it updates its status heartbeat
without submitting invented HID/WinTab samples. Device read failures still stop
the feeder. The `ctl460-hid-inspect` debug binary can inspect all
collections; `--mode-only` only reads the current mode. Both diagnostics leave
the Windows cursor still because they do not inject input.

On 2026-09-09, the connected tablet confirmed mode `02 02`. An all-collection
capture received 674 native pen reports. The normal acquisition/decoder path then
processed 818 reports with zero rejects in a coordinated capture; the first
arrived after 37.4 seconds. These observations verify physical input and decoding,
but do not establish why the earlier session received nothing or validate cursor
delivery in the user's drawing application. The added handshake is a reliability
improvement, not a proven explanation of that earlier failure.
