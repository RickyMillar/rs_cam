# A-7 rendered evidence — CLI modulation state (K-(g1)+(g2))

Captured 2026-08-13, dev build of `rs_cam_cli` at the A-7 g1/g2 commit.
Fixture: `crates/rs_cam_viz/tests/fixtures/sample_2d_project.toml` (a 0-toolpath
fixture — the point is the DISCLOSURE LINE, not the numbers).

## Default (K-(g1): now ON)

```text

=== Project Diagnostics: Fixture 2D ===
Toolpaths: 0  |  Cutting: -0mm  |  Rapid: -0mm  |  Time: 0s
Air cutting: 0.0% of total runtime  |  Avg engagement: 0.00  |  Peak commanded advance/tooth: 0.000 mm/tooth
Adaptive feed modulation: on (ConstrainedMax, aggressiveness 1.00)
Verdict: OK
Output: /tmp/claude-1001/-home-ricky-personal-repos-rs-cam/0934e428-3a02-4818-b682-76c62c03a29a/scratchpad/cliout
```

## Opt-out (`--no-adaptive-feed-modulation`, reproduces the pre-K default)

```text

=== Project Diagnostics: Fixture 2D ===
Toolpaths: 0  |  Cutting: -0mm  |  Rapid: -0mm  |  Time: 0s
Air cutting: 0.0% of total runtime  |  Avg engagement: 0.00  |  Peak commanded advance/tooth: 0.000 mm/tooth
Adaptive feed modulation: off (--no-adaptive-feed-modulation)
Verdict: OK
Output: /tmp/claude-1001/-home-ricky-personal-repos-rs-cam/0934e428-3a02-4818-b682-76c62c03a29a/scratchpad/cliout2
```

## `summary.json` carries the same fact (K-(g2)) — a script reading only the JSON is not the one reader left guessing

```json
{
  "verdict": "OK",
  "adaptive_feed_modulation": true,
  "modulation_state": "on (ConstrainedMax, aggressiveness 1.00)"
}
```
