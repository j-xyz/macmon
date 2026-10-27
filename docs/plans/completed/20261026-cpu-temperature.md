Done: 2026-10-26, PR #1

# CPU temperature history in place of the power row

## Behavior

- Keep CPU clusters, GPU usage and RAM in the upper metrics row. Replace all four watt boxes
  below them with one full-width CPU temperature history graph, including in virtual machines.
- Preserve the current metrics/process height split and process-list behavior.
- Show `CPU Temperature 46°C` (compact `Temp 46°C`), `collecting…` before sampling and `N/A`
  for a missing latest reading. Keep fan RPM on the temperature box when available.
- Retain 1,024 sample slots, newest on the right, one per metrics update. Missing readings
  occupy blank columns; do not estimate or carry values forward. Preserve fractional readings
  internally; round the title to whole degrees.
- Fixed 0–150°C display scale, documented in help, with the existing temperature gradient
  (30°C cool, 100°C hot). This is a visualization range, not a thermal safety threshold.
- The temperature remains a history graph in gauge mode and follows the sampling interval.
- JSON, Prometheus, library API, configuration and metric collection remain compatible.
  Per-process POWER remains unchanged; GPU temperature graphs, alerts and persistence are out
  of scope. Remove obsolete TUI-only watt and smoothed-temperature state where appropriate.

## Verification

- Tests first: prove the full-width replacement layout, history positions and missing-value
  handling, fixed scale and temperature colors, fan label, startup and gauge behavior.
- Exercise 80×24, 110×32, 200×50, tiny sizes, two/three CPU tiers, process list on/off and VMs.
- Run focused tests, `make check`, `make test`, and `make loc REF=origin/main`; manually inspect
  the TUI on Apple Silicon and verify `pipe` continues exporting CPU temperature and power.

## Tasks

- [x] Add focused failing tests for the approved behavior.
- [x] Implement the temperature row and update user documentation and help.
- [x] Run required checks and inspect the running TUI.

## Result

The component/system watt row is replaced by a full-width measured CPU temperature graph.
This prioritizes the owner's thermal monitoring over watt readings while preserving room for
the utilization boxes and process list. Missing samples remain gaps; the fixed scale makes
temperature heights comparable as the visible history changes. Power exports remain intact.

The regression tests failed before implementation. `make check` and all 193 tests pass;
the TUI was inspected on an Apple M4 at 80×24, 110×32 and 200×50, and two `pipe` samples
confirmed temperature and power exports. VM, missing-reading and fan scenarios use fixtures.
Independent Codex review found no actionable regressions. Production TUI code: -102 lines.
