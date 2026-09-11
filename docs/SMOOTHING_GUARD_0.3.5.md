# OpenCTL 460 0.3.5

Corner preservation and a combined smoothing distance limit are enabled automatically.
Existing pressure curves, artistic sliders, shortcuts and tablet mapping are retained.

## Corner and loop preservation

The driver measures direction from raw tablet coordinates after at least 0.12 mm of
movement. A turn exceeding approximately 43 degrees after a straight approach clears
stale position-filter history without ending contact or resetting pressure. Repeated
turns in the same direction tighten the displacement allowance to protect small loops
and hooks. Alternating deviations do not count as a sustained curve. Protection eases
back toward the normal distance limit as the pen travels onward.

This is a causal heuristic. It cannot recover a corner before the tablet reports it,
or correct already-rendered ink. Strong filtering may still soften fine details.
A detected sharp turn can cause a visible catch-up from previously lagging output.

## One limit for the combined filters

After the base filter, Motion Filtering, StreamLine, Stabilization and circle correction,
the contact coordinate is constrained to a 1.0 mm radius around the measured position.
The bound is Euclidean, in tablet millimetres, with at most 0.0071 mm rounding error.
Corner protection may tighten it further. This is a distance bound, not a guaranteed
latency in milliseconds. Existing filters do not add a new queue or delayed reports.
Pressure interpolation has its separate timing and is unaffected by these protections.
Hover behavior is unchanged; release, timeout, tool changes and reconfiguration reset
protection state along with the other stroke state.

## Advanced settings

Both features apply to existing settings files through defaults. No new GUI controls
are introduced. To tune them in settings.toml while the settings app is closed:

```toml
preserve_corners = true
max_smoothing_distance_mm = 1.0
```

The distance accepts finite values from 0 to 5 mm. Zero disables the combined guard,
including corner protection, and restores the preceding position pipeline. Setting
preserve_corners to false keeps the fixed total limit but disables turn detection.
Drawing presets preserve these choices. Live changes apply between strokes.

## Validation

128 driver tests passed, including new tests for total displacement across drawing and
handwriting at several report intervals, pressure equality, a deliberate corner,
a small loop, settings round trips, presets, raw bypass, release and stroke restart.
One existing exclusive-feeder status test was skipped to keep the active driver running.
In the synthetic 0.6 mm radius loop with all artistic sliders at maximum, mean radius
was 0.516 mm with protection versus 0.030 mm with corner protection disabled (the total
1 mm bound remained enabled in both). These are synthetic results, not measured drawing
feel. Release binaries and installer are built without installing or starting them.
