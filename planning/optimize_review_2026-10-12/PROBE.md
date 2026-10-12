# Probe output, 2026-10-12

Scratch probe `optprobe` (not in the repo) against `rs_cam_core` at HEAD b20d947d, release, project copy of `planning/rivmap_block_job/testpiece_mini.toml`, sim 0.2 mm, `SimulationOptions::default()` (modulation ConstrainedMax 1.0). The probe source is in the session scratchpad; it loads the project, generates every enabled toolpath, simulates, runs `tool_load::evaluate_toolpath`, prints the chipload population by move decile, the modulation summary, a deflection what-if, and one real `optimize_toolpath` on toolpath id 2.

## Full project, gates per toolpath

```
sim opts: modulation=true strategy=ConstrainedMax scale=1
sim wall 1.3s
predicted_feeds entries: 125899
material: SolidWoodByJanka { janka_lbf: 1290.0, label: "Oak (feeds_oak.toml janka_lbf)", source_id: "rivmap feeds_oak.toml" }; force_line: Err(NoDensity)
== TP id=0 idx=0 'Face back to T1' Face feed=1673 rpm=Some(10026) dpp=Some(0.25) stepover=Some(2.048) runtime(total,cut,rapid,air)=Some((249.2117307195975, 198.56463007762233, 50.64710064197517, 198.56463007762235))
  Chipload: state=Unmodeled bound=None bound_source=None display_peak=None unit=mm/tooth unmodeled=Some(AllSamplesAirCutOrRapid) conf=None
  Power: state=Unmodeled bound=None bound_source=None display_peak=None unit=kW unmodeled=Some(MaterialUnvalidated) conf=None
  Deflection: state=Unmodeled bound=None bound_source=None display_peak=None unit=mm unmodeled=Some(MaterialUnvalidated) conf=None
  DepthOfCut: state=Within bound=Some(1.5875) bound_source=Some(RigidityRuleOfThumb { factor: 0.25, diameter_mm: 6.35 }) display_peak=Some(0.0) unit=mm unmodeled=None conf=Some(Approximate("peak engaged depth 0.000 mm against the machine rigidity factor 0.25 times the engaged diameter 6.35 mm at that depth"))
  GantryPush: state=Unmodeled bound=None bound_source=None display_peak=None unit=N unmodeled=Some(NotImplemented("no machine-side thrust rating is published; register T-10")) conf=None
  modulation_summary: ModulationSummary { moves_touched: 0, moves_total: 70, median_feed_delta_pct: 0.0, binding_constraint_distribution: {MachineMaxFeed: 1.0}, feed_scale: 1.0, strategy: ConstrainedMax }
  chipload population EMPTY
  chip target: Some(Some(ChiploadBand { min_mm_per_tooth: 0.08343799861216172, max_mm_per_tooth: 0.08343799861216172, point: true }))
  depth population n=0 max=0.000
== TP id=2 idx=1 'Channel BACK_CHANNEL_Z22.00' Pocket feed=3877 rpm=Some(18000) dpp=Some(0.5945945945945946) stepover=Some(0.838) runtime(total,cut,rapid,air)=Some((1315.460802941792, 623.5190934972167, 691.9417094445754, 409.9429515481864))
  Chipload: state=Within bound=Some(0.10771665386271448) bound_source=Some(VendorChipBand { floor_mm_per_tooth: None, ceiling_mm_per_tooth: 0.10771665386271448, source: VendorLutPointPreset }) display_peak=Some(0.10771665386271448) unit=mm/tooth unmodeled=None conf=Some(Validated)
  Power: state=Unmodeled bound=None bound_source=None display_peak=None unit=kW unmodeled=Some(MaterialUnvalidated) conf=None
  Deflection: state=Unmodeled bound=None bound_source=None display_peak=None unit=mm unmodeled=Some(MaterialUnvalidated) conf=None
  DepthOfCut: state=Within bound=Some(0.75) bound_source=Some(RigidityRuleOfThumb { factor: 0.25, diameter_mm: 3.0 }) display_peak=Some(0.5945959091186523) unit=mm unmodeled=None conf=Some(Approximate("peak engaged depth 0.595 mm against the machine rigidity factor 0.25 times the engaged diameter 3.00 mm at that depth"))
  GantryPush: state=Unmodeled bound=None bound_source=None display_peak=None unit=N unmodeled=Some(NotImplemented("no machine-side thrust rating is published; register T-10")) conf=None
  modulation_summary: ModulationSummary { moves_touched: 31043, moves_total: 43845, median_feed_delta_pct: 0.020622622071735274, binding_constraint_distribution: {ChiploadMax: 0.6683544303797468, MachineMaxFeed: 0.2919831223628692, KinematicReach: 0.039662447257383965}, feed_scale: 1.0, strategy: ConstrainedMax }
  chipload population n=26936 max=0.1077 
    decile 0: n=1245 mean=0.1068 max=0.1077
    decile 1: n=1262 mean=0.1068 max=0.1077
    decile 2: n=1274 mean=0.1068 max=0.1077
    decile 3: n=1268 mean=0.1068 max=0.1077
    decile 4: n=1262 mean=0.1068 max=0.1077
    decile 5: n=1278 mean=0.1068 max=0.1077
    decile 6: n=1266 mean=0.1068 max=0.1077
    decile 7: n=5271 mean=0.1064 max=0.1077
    decile 8: n=7593 mean=0.1067 max=0.1077
    decile 9: n=5217 mean=0.1077 max=0.1077
  chip target: Some(Some(ChiploadBand { min_mm_per_tooth: 0.10771665386271448, max_mm_per_tooth: 0.10771665386271448, point: true }))
  depth population n=51331 max=0.595
== TP id=3 idx=2 'Channel BACK_CHANNEL_Z23.00' Pocket feed=3877 rpm=Some(18000) dpp=Some(0.5) stepover=Some(0.838) runtime(total,cut,rapid,air)=Some((245.0738627611078, 192.15604184400036, 52.91782091710742, 179.37360137307198))
  Chipload: state=Within bound=Some(0.10771665386271448) bound_source=Some(VendorChipBand { floor_mm_per_tooth: None, ceiling_mm_per_tooth: 0.10771665386271448, source: VendorLutPointPreset }) display_peak=Some(0.10771665386271448) unit=mm/tooth unmodeled=None conf=Some(Validated)
  Power: state=Unmodeled bound=None bound_source=None display_peak=None unit=kW unmodeled=Some(MaterialUnvalidated) conf=None
  Deflection: state=Unmodeled bound=None bound_source=None display_peak=None unit=mm unmodeled=Some(MaterialUnvalidated) conf=None
  DepthOfCut: state=Within bound=Some(0.75) bound_source=Some(RigidityRuleOfThumb { factor: 0.25, diameter_mm: 3.0 }) display_peak=Some(0.5) unit=mm unmodeled=None conf=Some(Approximate("peak engaged depth 0.500 mm against the machine rigidity factor 0.25 times the engaged diameter 3.00 mm at that depth"))
  GantryPush: state=Unmodeled bound=None bound_source=None display_peak=None unit=N unmodeled=Some(NotImplemented("no machine-side thrust rating is published; register T-10")) conf=None
  modulation_summary: ModulationSummary { moves_touched: 1958, moves_total: 12828, median_feed_delta_pct: 0.020622622071735274, binding_constraint_distribution: {ChiploadMax: 0.13907078266292486, MachineMaxFeed: 0.8473651387589648, KinematicReach: 0.013564078578110383}, feed_scale: 1.0, strategy: ConstrainedMax }
  chipload population n=668 max=0.1077 
    decile 0: n=95 mean=0.1055 max=0.1077
    decile 2: n=31 mean=0.0985 max=0.1077
    decile 3: n=16 mean=0.0930 max=0.1077
    decile 5: n=142 mean=0.1026 max=0.1077
    decile 6: n=77 mean=0.1027 max=0.1077
    decile 7: n=157 mean=0.0975 max=0.1077
    decile 8: n=96 mean=0.1023 max=0.1077
    decile 9: n=54 mean=0.1077 max=0.1077
  chip target: Some(Some(ChiploadBand { min_mm_per_tooth: 0.10771665386271448, max_mm_per_tooth: 0.10771665386271448, point: true }))
  depth population n=1442 max=0.500
== TP id=10 idx=3 'Pin Drill' AlignmentPinDrill feed=600 rpm=Some(6000) dpp=None stepover=None runtime(total,cut,rapid,air)=None
  Chipload: state=Unmodeled bound=None bound_source=None display_peak=None unit=mm/tooth unmodeled=Some(NotApplicableForOp("drill cycle — no continuous engagement")) conf=None
  Power: state=Unmodeled bound=None bound_source=None display_peak=None unit=kW unmodeled=Some(NotApplicableForOp("drill cycle — no continuous engagement")) conf=None
  Deflection: state=Unmodeled bound=None bound_source=None display_peak=None unit=mm unmodeled=Some(NotApplicableForOp("drill cycle — no continuous engagement")) conf=None
  DepthOfCut: state=Unmodeled bound=None bound_source=None display_peak=None unit=mm unmodeled=Some(NotApplicableForOp("drill cycle — no continuous engagement")) conf=None
  GantryPush: state=Unmodeled bound=None bound_source=None display_peak=None unit=N unmodeled=Some(NotApplicableForOp("drill cycle — no continuous engagement")) conf=None
  DrillChipWelding: state=Within bound=Some(4.5) bound_source=Some(DrillEnvelope) display_peak=Some(1.0) unit=D/d unmodeled=None conf=None
  DrillPeckAdequacy: state=Within bound=Some(5.0) bound_source=Some(DrillEnvelope) display_peak=Some(1.0) unit=D/d unmodeled=None conf=None
  DrillPlungeFeed: state=Within bound=Some(50.0) bound_source=Some(DrillEnvelope) display_peak=Some(100.0) unit=mm/min per mm Ø unmodeled=None conf=None
  chipload population EMPTY
  chip target: None
  depth population n=0 max=0.000
== TP id=5 idx=4 'Face back overfill' Face feed=1673 rpm=Some(10026) dpp=Some(0.5) stepover=Some(2.048) runtime(total,cut,rapid,air)=Some((248.1567067522096, 198.5596061102344, 49.59710064197521, 178.97342070859472))
  Chipload: state=Within bound=Some(0.08343799861216172) bound_source=Some(VendorChipBand { floor_mm_per_tooth: None, ceiling_mm_per_tooth: 0.08343799861216172, source: VendorLutPointPreset }) display_peak=Some(0.08343799861216172) unit=mm/tooth unmodeled=None conf=Some(Validated)
  Power: state=Unmodeled bound=None bound_source=None display_peak=None unit=kW unmodeled=Some(MaterialUnvalidated) conf=None
  Deflection: state=Unmodeled bound=None bound_source=None display_peak=None unit=mm unmodeled=Some(MaterialUnvalidated) conf=None
  DepthOfCut: state=Within bound=Some(1.5875) bound_source=Some(RigidityRuleOfThumb { factor: 0.25, diameter_mm: 6.35 }) display_peak=Some(0.5) unit=mm unmodeled=None conf=Some(Approximate("peak engaged depth 0.500 mm against the machine rigidity factor 0.25 times the engaged diameter 6.35 mm at that depth"))
  GantryPush: state=Unmodeled bound=None bound_source=None display_peak=None unit=N unmodeled=Some(NotImplemented("no machine-side thrust rating is published; register T-10")) conf=None
  modulation_summary: ModulationSummary { moves_touched: 34, moves_total: 70, median_feed_delta_pct: 0.0, binding_constraint_distribution: {ChiploadMax: 0.4857142857142857, MachineMaxFeed: 0.5142857142857142}, feed_scale: 1.0, strategy: ConstrainedMax }
  chipload population n=2140 max=0.0834 
    decile 1: n=244 mean=0.0834 max=0.0834
    decile 2: n=388 mean=0.0834 max=0.0834
    decile 3: n=321 mean=0.0834 max=0.0834
    decile 4: n=117 mean=0.0834 max=0.0834
    decile 7: n=326 mean=0.0834 max=0.0834
    decile 8: n=374 mean=0.0834 max=0.0834
    decile 9: n=370 mean=0.0834 max=0.0834
  chip target: Some(Some(ChiploadBand { min_mm_per_tooth: 0.08343799861216172, max_mm_per_tooth: 0.08343799861216172, point: true }))
  depth population n=2454 max=0.500
== TP id=6 idx=5 'Back cavity Z9.00' Pocket feed=1673 rpm=Some(10026) dpp=Some(1.2857142857142858) stepover=Some(2.048) runtime(total,cut,rapid,air)=Some((349.33205895026407, 260.8494248791119, 88.48263407115218, 109.71027691190406))
  Chipload: state=Within bound=Some(0.08343799861216172) bound_source=Some(VendorChipBand { floor_mm_per_tooth: None, ceiling_mm_per_tooth: 0.08343799861216172, source: VendorLutPointPreset }) display_peak=Some(0.08343799861216172) unit=mm/tooth unmodeled=None conf=Some(Validated)
  Power: state=Unmodeled bound=None bound_source=None display_peak=None unit=kW unmodeled=Some(MaterialUnvalidated) conf=None
  Deflection: state=Unmodeled bound=None bound_source=None display_peak=None unit=mm unmodeled=Some(MaterialUnvalidated) conf=None
  DepthOfCut: state=Within bound=Some(1.5875) bound_source=Some(RigidityRuleOfThumb { factor: 0.25, diameter_mm: 6.35 }) display_peak=Some(1.2857170104980469) unit=mm unmodeled=None conf=Some(Approximate("peak engaged depth 1.286 mm against the machine rigidity factor 0.25 times the engaged diameter 6.35 mm at that depth"))
  GantryPush: state=Unmodeled bound=None bound_source=None display_peak=None unit=N unmodeled=Some(NotImplemented("no machine-side thrust rating is published; register T-10")) conf=None
  modulation_summary: ModulationSummary { moves_touched: 847, moves_total: 931, median_feed_delta_pct: -71.77887966180818, binding_constraint_distribution: {ChiploadMax: 0.8947368421052632, MachineMaxFeed: 0.09022556390977443, KinematicReach: 0.015037593984962405}, feed_scale: 1.0, strategy: ConstrainedMax }
  chipload population n=10948 max=0.0834 
    decile 0: n=712 mean=0.0834 max=0.0834
    decile 1: n=1374 mean=0.0834 max=0.0834
    decile 2: n=1171 mean=0.0834 max=0.0834
    decile 3: n=821 mean=0.0834 max=0.0834
    decile 4: n=1136 mean=0.0834 max=0.0834
    decile 5: n=1303 mean=0.0834 max=0.0834
    decile 6: n=961 mean=0.0834 max=0.0834
    decile 7: n=873 mean=0.0834 max=0.0834
    decile 8: n=1294 mean=0.0834 max=0.0834
    decile 9: n=1303 mean=0.0834 max=0.0834
  chip target: Some(Some(ChiploadBand { min_mm_per_tooth: 0.08343799861216172, max_mm_per_tooth: 0.08343799861216172, point: true }))
  depth population n=12439 max=1.286
== TP id=7 idx=6 'Face front to T' Face feed=1673 rpm=Some(10026) dpp=Some(0.25) stepover=Some(2.048) runtime(total,cut,rapid,air)=Some((281.515736575396, 221.94363593342064, 59.572100641975325, 63.2358953292116))
  Chipload: state=Within bound=Some(0.08343799861216172) bound_source=Some(VendorChipBand { floor_mm_per_tooth: None, ceiling_mm_per_tooth: 0.08343799861216172, source: VendorLutPointPreset }) display_peak=Some(0.08343799861216172) unit=mm/tooth unmodeled=None conf=Some(Validated)
  Power: state=Unmodeled bound=None bound_source=None display_peak=None unit=kW unmodeled=Some(MaterialUnvalidated) conf=None
  Deflection: state=Unmodeled bound=None bound_source=None display_peak=None unit=mm unmodeled=Some(MaterialUnvalidated) conf=None
  DepthOfCut: state=Within bound=Some(1.5875) bound_source=Some(RigidityRuleOfThumb { factor: 0.25, diameter_mm: 6.35 }) display_peak=Some(0.25) unit=mm unmodeled=None conf=Some(Approximate("peak engaged depth 0.250 mm against the machine rigidity factor 0.25 times the engaged diameter 6.35 mm at that depth"))
  GantryPush: state=Unmodeled bound=None bound_source=None display_peak=None unit=N unmodeled=Some(NotImplemented("no machine-side thrust rating is published; register T-10")) conf=None
  modulation_summary: ModulationSummary { moves_touched: 128, moves_total: 206, median_feed_delta_pct: 0.0, binding_constraint_distribution: {ChiploadMax: 0.6213592233009708, MachineMaxFeed: 0.3786407766990291}, feed_scale: 1.0, strategy: ConstrainedMax }
  chipload population n=16566 max=0.0834 
    decile 0: n=1539 mean=0.0834 max=0.0834
    decile 1: n=2096 mean=0.0834 max=0.0834
    decile 2: n=1798 mean=0.0834 max=0.0834
    decile 3: n=1802 mean=0.0834 max=0.0834
    decile 4: n=1048 mean=0.0834 max=0.0834
    decile 5: n=1801 mean=0.0834 max=0.0834
    decile 6: n=1572 mean=0.0834 max=0.0834
    decile 7: n=1536 mean=0.0834 max=0.0834
    decile 8: n=1572 mean=0.0834 max=0.0834
    decile 9: n=1802 mean=0.0834 max=0.0834
  chip target: Some(Some(ChiploadBand { min_mm_per_tooth: 0.08343799861216172, max_mm_per_tooth: 0.08343799861216172, point: true }))
  depth population n=16590 max=0.250
== TP id=8 idx=7 'Front 3D rough' Adaptive3d feed=1673.098748171067 rpm=Some(10026) dpp=Some(5.854) stepover=Some(1.463) runtime(total,cut,rapid,air)=Some((157.34653540226233, 153.30571777211944, 4.040817630142891, 43.450459406990205))
  Chipload: state=Within bound=Some(0.08343799861216172) bound_source=Some(VendorChipBand { floor_mm_per_tooth: None, ceiling_mm_per_tooth: 0.08343799861216172, source: VendorLutPointPreset }) display_peak=Some(0.08343799861216172) unit=mm/tooth unmodeled=None conf=Some(Validated)
  Power: state=Unmodeled bound=None bound_source=None display_peak=None unit=kW unmodeled=Some(MaterialUnvalidated) conf=None
  Deflection: state=Unmodeled bound=None bound_source=None display_peak=None unit=mm unmodeled=Some(MaterialUnvalidated) conf=None
  DepthOfCut: state=Within bound=Some(12.7) bound_source=Some(RigidityRuleOfThumb { factor: 2.0, diameter_mm: 6.35 }) display_peak=Some(5.354000091552734) unit=mm unmodeled=None conf=Some(Approximate("peak engaged depth 5.354 mm against the machine rigidity factor 2.00 times the engaged diameter 6.35 mm at that depth"))
  GantryPush: state=Unmodeled bound=None bound_source=None display_peak=None unit=N unmodeled=Some(NotImplemented("no machine-side thrust rating is published; register T-10")) conf=None
  modulation_summary: ModulationSummary { moves_touched: 171, moves_total: 350, median_feed_delta_pct: -29.3985712045797, binding_constraint_distribution: {ChiploadMax: 0.5771428571428572, MachineMaxFeed: 0.1, KinematicReach: 0.32285714285714284}, feed_scale: 1.0, strategy: ConstrainedMax }
  chipload population n=9111 max=0.0834 
    decile 0: n=271 mean=0.0832 max=0.0834
    decile 1: n=1508 mean=0.0834 max=0.0834
    decile 2: n=2000 mean=0.0832 max=0.0834
    decile 3: n=1791 mean=0.0831 max=0.0834
    decile 4: n=1236 mean=0.0830 max=0.0834
    decile 5: n=882 mean=0.0829 max=0.0834
    decile 6: n=459 mean=0.0823 max=0.0834
    decile 7: n=70 mean=0.0789 max=0.0834
    decile 9: n=894 mean=0.0831 max=0.0834
  chip target: Some(Some(ChiploadBand { min_mm_per_tooth: 0.08343799861216172, max_mm_per_tooth: 0.08343799861216172, point: true }))
  depth population n=9722 max=5.354
== TP id=9 idx=8 'Front finish parallel N-S' DropCutter feed=725 rpm=Some(19000) dpp=None stepover=Some(0.25) runtime(total,cut,rapid,air)=Some((1067.7302102569593, 1067.0966086717406, 0.6336015852185725, 566.3029004490511))
  Chipload: state=Within bound=Some(0.025444878865208145) bound_source=Some(VendorChipBand { floor_mm_per_tooth: Some(0.012722439432604073), ceiling_mm_per_tooth: 0.025444878865208145, source: VendorLut }) display_peak=Some(0.025444878865208145) unit=mm/tooth unmodeled=None conf=Some(Validated)
  Power: state=Unmodeled bound=None bound_source=None display_peak=None unit=kW unmodeled=Some(MaterialUnvalidated) conf=None
  Deflection: state=Unmodeled bound=None bound_source=None display_peak=None unit=mm unmodeled=Some(MaterialUnvalidated) conf=None
  DepthOfCut: state=Within bound=None bound_source=None display_peak=Some(0.28544044494628906) unit=mm unmodeled=None conf=Some(Approximate("peak engaged depth 0.285 mm; a finishing pass has no axial cap, and the deflection gate is its limit"))
  GantryPush: state=Unmodeled bound=None bound_source=None display_peak=None unit=N unmodeled=Some(NotImplemented("no machine-side thrust rating is published; register T-10")) conf=None
  modulation_summary: ModulationSummary { moves_touched: 66432, moves_total: 67599, median_feed_delta_pct: 33.366261638332354, binding_constraint_distribution: {ChiploadMax: 0.9808133256409118, MachineMaxFeed: 0.01726356898770692, KinematicReach: 0.0019231053713812335}, feed_scale: 1.0, strategy: ConstrainedMax }
  chipload population n=63667 max=0.0254 
    decile 0: n=6447 mean=0.0254 max=0.0254
    decile 1: n=6134 mean=0.0254 max=0.0254
    decile 2: n=6486 mean=0.0254 max=0.0254
    decile 3: n=6487 mean=0.0254 max=0.0254
    decile 4: n=6234 mean=0.0254 max=0.0254
    decile 5: n=6323 mean=0.0254 max=0.0254
    decile 6: n=6487 mean=0.0254 max=0.0254
    decile 7: n=6486 mean=0.0254 max=0.0254
    decile 8: n=6080 mean=0.0254 max=0.0254
    decile 9: n=6503 mean=0.0254 max=0.0254
  chip target: Some(Some(ChiploadBand { min_mm_per_tooth: 0.012722439432604073, max_mm_per_tooth: 0.025444878865208145, point: false }))
  depth population n=101073 max=0.285
== deflection what-if: tool D=3 stickout=30 fz=0.1077 rigidity cap=Some(0.75)
  project material: force_line=Err(NoDensity)
    ae=0.838 ap=0.595: force_n=None tip_deflection_mm=None
    ae=0.838 ap=0.750: force_n=None tip_deflection_mm=None
    ae=0.838 ap=0.830: force_n=None tip_deflection_mm=None
    ae=0.838 ap=1.000: force_n=None tip_deflection_mm=None
    ae=0.838 ap=1.500: force_n=None tip_deflection_mm=None
    ae=0.838 ap=2.000: force_n=None tip_deflection_mm=None
    ae=0.838 ap=3.000: force_n=None tip_deflection_mm=None
    ae=3.000 ap=0.595: force_n=None tip_deflection_mm=None
    ae=3.000 ap=0.750: force_n=None tip_deflection_mm=None
    ae=3.000 ap=0.830: force_n=None tip_deflection_mm=None
    ae=3.000 ap=1.000: force_n=None tip_deflection_mm=None
    ae=3.000 ap=1.500: force_n=None tip_deflection_mm=None
    ae=3.000 ap=2.000: force_n=None tip_deflection_mm=None
    ae=3.000 ap=3.000: force_n=None tip_deflection_mm=None
  WhiteOak FPL: force_line=Ok("ForceLine { ks_n_per_mm2: 58.516266666666674, f_edge_n_per_mm: 4.594986666666666, grain_factor: 1.0, chip_range_mm: (0.04, 0.1), basis: CurtiDensityLaw { density: WoodDensity { specific_gravity: 0.68, rho_kg_m3: 761.6, source: FplRow(FplSgRow { row: \"Oak, white\", specific_gravity: 0.68 }) } } }")
    ae=0.838 ap=0.595: force_n=Some(6.09) tip_deflection_mm=Some(0.0553)
    ae=0.838 ap=0.750: force_n=Some(7.69) tip_deflection_mm=Some(0.0695)
    ae=0.838 ap=0.830: force_n=Some(8.51) tip_deflection_mm=Some(0.0767)
    ae=0.838 ap=1.000: force_n=Some(10.25) tip_deflection_mm=Some(0.092)
    ae=0.838 ap=1.500: force_n=Some(15.37) tip_deflection_mm=Some(0.1363)
    ae=0.838 ap=2.000: force_n=Some(20.5) tip_deflection_mm=Some(0.1794)
    ae=0.838 ap=3.000: force_n=Some(30.75) tip_deflection_mm=Some(0.262)
    ae=3.000 ap=0.595: force_n=Some(6.48) tip_deflection_mm=Some(0.0588)
    ae=3.000 ap=0.750: force_n=Some(8.17) tip_deflection_mm=Some(0.0739)
    ae=3.000 ap=0.830: force_n=Some(9.04) tip_deflection_mm=Some(0.0816)
    ae=3.000 ap=1.000: force_n=Some(10.9) tip_deflection_mm=Some(0.0978)
    ae=3.000 ap=1.500: force_n=Some(16.35) tip_deflection_mm=Some(0.1449)
    ae=3.000 ap=2.000: force_n=Some(21.79) tip_deflection_mm=Some(0.1907)
    ae=3.000 ap=3.000: force_n=Some(32.69) tip_deflection_mm=Some(0.2785)
  GenericHardwood: force_line=Ok("ForceLine { ks_n_per_mm2: 51.91884444444446, f_edge_n_per_mm: 4.076924444444445, grain_factor: 1.0, chip_range_mm: (0.04, 0.1), basis: CurtiDensityLaw { density: WoodDensity { specific_gravity: 0.6033333333333334, rho_kg_m3: 675.7333333333335, source: FplMean([FplSgRow { row: \"Beech, American\", specific_gravity: 0.64 }, FplSgRow { row: \"Maple, red\", specific_gravity: 0.54 }, FplSgRow { row: \"Oak, northern red\", specific_gravity: 0.63 }]) } } }")
    ae=0.838 ap=0.595: force_n=Some(5.41) tip_deflection_mm=Some(0.0491)
    ae=0.838 ap=0.750: force_n=Some(6.82) tip_deflection_mm=Some(0.0616)
    ae=0.838 ap=0.830: force_n=Some(7.55) tip_deflection_mm=Some(0.0681)
    ae=0.838 ap=1.000: force_n=Some(9.09) tip_deflection_mm=Some(0.0817)
    ae=0.838 ap=1.500: force_n=Some(13.64) tip_deflection_mm=Some(0.1209)
    ae=0.838 ap=2.000: force_n=Some(18.19) tip_deflection_mm=Some(0.1591)
    ae=0.838 ap=3.000: force_n=Some(27.28) tip_deflection_mm=Some(0.2324)
    ae=3.000 ap=0.595: force_n=Some(5.75) tip_deflection_mm=Some(0.0522)
    ae=3.000 ap=0.750: force_n=Some(7.25) tip_deflection_mm=Some(0.0655)
    ae=3.000 ap=0.830: force_n=Some(8.02) tip_deflection_mm=Some(0.0724)
    ae=3.000 ap=1.000: force_n=Some(9.67) tip_deflection_mm=Some(0.0868)
    ae=3.000 ap=1.500: force_n=Some(14.5) tip_deflection_mm=Some(0.1286)
    ae=3.000 ap=2.000: force_n=Some(19.34) tip_deflection_mm=Some(0.1692)
    ae=3.000 ap=3.000: force_n=Some(29.0) tip_deflection_mm=Some(0.2471)
timeout 900  base/mini.toml 2 none  3.54s user 0.38s system 228% cpu 1.717 total
```

## optimize_toolpath on toolpath 2 (Z22 channel)

```
== OPTIMIZE idx=1 wall 11.6s kind=NoSafeImprovement reason=Some(NoImprovementFound)
  headline: Tried 3 candidates; none were faster than baseline by enough to recommend.
  explanation: no candidate was both faster than baseline and within the gate's safe envelope: every candidate hit a gate limit (chipload, power, or deflection)
  recommended_index: None
  [0] stage=Baseline cycle=1315.5s feed=3877 rpm=Some(18000) dpp=Some(0.5945945945945946) stepover=Some(0.838) air=Some(0.312) states=Chipload=Within,Power=Unmodeled,Deflection=Unmodeled,DepthOfCut=Within,GantryPush=Unmodeled
  [1] stage=Refined cycle=371.3s feed=4652.4 rpm=Some(21600) dpp=Some(1.5) stepover=Some(2.0) air=Some(0.219) states=Chipload=Within,Power=Unmodeled,Deflection=Unmodeled,DepthOfCut=Exceeds,GantryPush=Unmodeled
  [2] stage=Refined cycle=509.3s feed=4652.4 rpm=Some(21600) dpp=Some(1.5) stepover=Some(1.1731999999999998) air=Some(0.189) states=Chipload=Within,Power=Unmodeled,Deflection=Unmodeled,DepthOfCut=Exceeds,GantryPush=Unmodeled
  [3] stage=Refined cycle=538.0s feed=4652.4 rpm=Some(21600) dpp=Some(1.5) stepover=Some(1.0055999999999998) air=Some(0.195) states=Chipload=Within,Power=Unmodeled,Deflection=Unmodeled,DepthOfCut=Exceeds,GantryPush=Unmodeled
  assumptions: {"candidates":{"coarse_resolution_mm":1.0,"refined_resolution_mm":0.5,"auto_resolution":false,"adaptive_feed_modulation":false,"modulation_strategy":"constrained_max","modulation_feed_scale":1.0,"use_predicted_feed_in_gates":false},"baseline":{"adaptive_feed_modulation":true,"sample_step_mm":0.25},"kinematics":"profile_declared","lut_query":{"kind":"routed","declared_family":"pocket","declared_pass_role":"roughing","queried_family":"pocket","queried_pass_role":"roughing"},"boundary_epsilon_rel":1.7763568394002505e-15}
timeout 900  base/mini.toml 2 opt  25.08s user 5.17s system 230% cpu 13.130 total
```

## CLI depth-per-pass sweep of toolpath 2 (setup 1 only, other ops skipped)

```
dpp 0.83
  crosses material an upstream op left standing: 14.3% of cutting samples remove more than 0.79 mm (3x this pass's own 0.26 mm median bite), peaking at 0.81 mm
Toolpaths: 1  |  Cutting: 14510mm  |  Rapid: 7989mm  |  Time: 980s
dpp 1.5
  crosses material an upstream op left standing: 21.4% of cutting samples remove more than 0.83 mm (3x this pass's own 0.28 mm median bite), peaking at 1.47 mm
Toolpaths: 1  |  Cutting: 9451mm  |  Rapid: 4487mm  |  Time: 590s
dpp 3.0
  crosses material an upstream op left standing: 18.8% of cutting samples remove more than 1.05 mm (3x this pass's own 0.35 mm median bite), peaking at 2.75 mm
Toolpaths: 1  |  Cutting: 6498mm  |  Rapid: 2444mm  |  Time: 360s
```
