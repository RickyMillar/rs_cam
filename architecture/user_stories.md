# User Stories

Organized by persona and priority tier.

---

## Persona: Hobbyist Wood CNC Router Operator

### Tier 1 -- Core (MVP)

**US-1.1** As a router operator, I want to load an STL file of my 3D model so that I can generate toolpaths for it.

**US-1.2** As a router operator, I want to load SVG/DXF files of my 2D designs so that I can cut profiles, pockets, and follow paths.

**US-1.3** As a router operator, I want to define my tools (diameter, type, flute count, cutting length) so that the toolpaths are computed for the correct geometry.

**US-1.4** As a router operator, I want to define tapered ball end mills (tip diameter, shaft diameter, taper angle) so that I can generate accurate toolpaths for carving tools.

**US-1.5** As a router operator, I want to generate 2D pocket toolpaths from a closed boundary so that I can clear material from enclosed regions.

**US-1.6** As a router operator, I want to generate profile/contour toolpaths along a boundary so that I can cut out parts.

**US-1.7** As a router operator, I want to add tabs/bridges to profile cuts so that my parts don't fly loose during cutting.

**US-1.8** As a router operator, I want to generate 3D raster finishing toolpaths using drop-cutter so that I can machine 3D surfaces with a ball or tapered ball end mill.

**US-1.9** As a router operator, I want to control step-over, step-down, feed rate, and spindle speed so that I get the right balance of speed and surface finish.

**US-1.10** As a router operator, I want to export G-code compatible with my controller (GRBL, LinuxCNC, Mach3) so that I can run the job on my machine.

**US-1.11** As a router operator, I want to define safe Z height and clearance plane so that rapid moves don't crash into my workpiece or clamps.

### Tier 2 -- Enhanced

**US-2.1** As a router operator, I want to generate adaptive clearing toolpaths with constant tool engagement so that I can rough faster with less tool wear.

**US-2.2** As a router operator, I want to generate waterline/contour toolpaths at constant Z heights so that steep walls have good surface finish.

**US-2.3** As a router operator, I want to V-carve text and designs using V-bits so that I can make signs and decorative pieces.

**US-2.4** As a router operator, I want to do rest machining (use a smaller tool to clean up what a larger tool missed) so that I get full detail in tight areas.

**US-2.5** As a router operator, I want helix/ramp entry into material so that my flat end mills don't plunge straight down (which can burn wood).

**US-2.6** As a router operator, I want to visualize the toolpath in 3D before cutting so that I can verify it looks correct.

**US-2.7** As a router operator, I want to follow/trace SVG paths at specified depths so that I can engrave designs and cut along contours.

**US-2.8** As a router operator, I want dogbone fillets on inside corners so that parts fit together with square edges.

### Tier 3 -- Advanced

**US-3.1** As a router operator, I want to create inlay operations (matching male/female V-carved pieces) so that I can make decorative inlays.

**US-3.2** As a router operator, I want to see a material removal simulation so that I can verify the final result before cutting.

**US-3.3** As a router operator, I want constant scallop height finishing so that the surface quality is uniform across varying curvatures.

**US-3.4** As a router operator, I want pencil finishing along internal corners so that small details are cleaned up.

**US-3.5** As a router operator, I want automatic feed rate optimization based on tool engagement so that the machine runs efficiently without overloading.

**US-3.6** As a router operator, I want to define fixture/clamp positions so that toolpaths avoid collisions.

---

## Persona: Power User / Integrator

**US-P.1** As an integrator, I want a CLI interface so that I can script and automate job generation in my workflow.

**US-P.2** As an integrator, I want TOML/JSON job definition files so that I can version-control and parameterize my jobs.

**US-P.3** As an integrator, I want the CAM engine available as a library crate so that I can embed it in my own application.

**US-P.4** As an integrator, I want to define custom tool profiles (arbitrary cross-section curves) so that I can use form tools.

**US-P.5** As an integrator, I want to write custom post-processors so that I can support any CNC controller.

**US-P.6** As an integrator, I want progress reporting during long operations so that I can show status in my application.

---

## Persona: Developer / Contributor

**US-D.1** As a developer, I want the codebase to use well-defined traits for tool geometry, operations, and post-processing so that I can add new types without modifying existing code.

**US-D.2** As a developer, I want comprehensive unit tests for geometric algorithms (drop-cutter, offset, etc.) so that I can contribute with confidence.

**US-D.3** As a developer, I want the toolpath intermediate representation to be independent of G-code so that visualization and analysis tools can consume it.

**US-D.4** As a developer, I want the 2D and 3D algorithm modules to be independent so that I can work on one without understanding the other.
