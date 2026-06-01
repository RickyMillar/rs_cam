# Marker Test — Star Outline

**What:** Draws a 5-pointed star (~100 mm point-to-point) on paper
using a Sharpie or fineliner in the spindle collet.

**Tool:** 1 mm endmill (declared in project) — but you use a marker.
The G-code commands a 1 mm centerline trace; the marker tip just
needs to fit in the collet.

**G-code:** `marker_star.nc`
- Single trace operation, no compensation (path = exact SVG outline)
- Z = -0.5 mm (touched off so marker just kisses paper at Z=0)
- Feed = 800 mm/min cutting, 400 mm/min plunge
- Safe Z = 5 mm, retract Z = 10 mm (final)
- Spindle M3 S0 → effectively off; M5 at end

**Predicted cycle time:** 28 s

**Workspace required:** ~110 mm × 90 mm clear area from G54 origin.

## Setup

1. Tape paper to the spoilboard
2. Install marker in collet (Sharpie works, fineliner cleaner)
3. Touch off Z=0 so marker tip just touches paper (use paper feeler)
4. Set XY work zero (G54) at the lower-left corner of the drawing area
5. Optional: disconnect spindle relay so M3 is a no-op

## Run

Load `marker_star.nc`, jog to home, hit Cycle Start. Watch the star
get drawn.

## Validation checklist

- [ ] Star drawn as 10 line segments forming a 5-pointed shape
- [ ] No skips / gaps in the lines
- [ ] Marker lifts to Z=+5 only at start and end (not mid-trace)
- [ ] Final retract to Z=+10
- [ ] Cycle time within ±5 s of 28 s predicted

## Findings template

If the result is off, note:
- Which segment is wrong (start, point 1-5, end)
- Direction of error (offset, scale, mirror)
- Actual cycle time vs predicted
