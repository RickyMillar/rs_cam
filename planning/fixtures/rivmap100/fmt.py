import json,sys
n=sys.argv[1]
for l in sys.stdin:
    r=json.loads(l)
    if r.get("record")=="toolpath":
        d=r["diagnostic"]; c=r["cut_summary"]; b=c["runtime_by_intent"]
        print("%s | moves %d | total %.0f s | cut %.0f | entry %.0f | rapid %.0f | vol %.0f | peakDOC %s | wce %.3f" % (n,d["move_count"],c["total_runtime_s"],b["cutting_s"],b["entry_s"],b["rapid_s"],c["total_removed_volume_est_mm3"],c.get("peak_axial_doc_mm"),r.get("whole_cycle_engagement") or -1))
