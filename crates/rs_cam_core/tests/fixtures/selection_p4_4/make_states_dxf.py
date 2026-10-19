# Writes crates/rs_cam_core/tests/fixtures/selection_p4_4/states.dxf (P4.4).
import sys, ezdxf
doc = ezdxf.new("R2010")
doc.header["$INSUNITS"] = 4  # mm
msp = doc.modelspace()
def square(x0, y0, x1, y1, layer):
    msp.add_lwpolyline([(x0, y0), (x1, y0), (x1, y1), (x0, y1)], close=True, dxfattribs={"layer": layer})
# Three nested loops: the middle one is an island, the inner one cut again.
square(0, 0, 60, 60, "OUTER")
square(10, 10, 50, 50, "OUTER")
square(20, 20, 40, 40, "OUTER")
# A circle inside the outer loop only: an island of a Pocket, a hole of a Drill.
msp.add_circle((5, 5), 2.0, dxfattribs={"layer": "HOLES"})
# A point outside every loop.
msp.add_point((80, 10), dxfattribs={"layer": "HOLES"})
# An open path (text-stroke shape).
msp.add_lwpolyline([(0, 70), (15, 80), (30, 70)], close=False, dxfattribs={"layer": "TEXT"})
# A loop that a test excludes, and a loop on a layer that no rule names.
square(70, 30, 90, 50, "KEEP")
square(100, 0, 110, 10, "SPARE")
doc.saveas(sys.argv[1])
