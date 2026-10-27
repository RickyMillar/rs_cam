"""D66: a copy of the fix45/fix41 tpsplit.py (writes into fix46). D49: a copy of fix33/tpsplit.py (writes into fix34). D47: a copy of fix31/tpsplit.py. D45: the per-toolpath runtime split of a simulation.json (trace toolpath_summaries). Usage: tpsplit.py <simulation.json>..."""
import json,sys
for p in sys.argv[1:]:
    d=json.load(open(p)); t=d['trace']
    print(p.split('phase3/')[1], 'res',d['resolution_mm'],'ids',d['included_toolpath_ids'],'total',round(t['summary']['total_runtime_s'],1))
    for s in t['toolpath_summaries']:
        print('   tp',s['toolpath_id'],'total',round(s['total_runtime_s'],1),'cut',round(s['cutting_runtime_s'],1),'rapid',round(s['rapid_runtime_s'],1),'air',round(s['air_cut_time_s'],1),'vol',round(s['total_removed_volume_est_mm3']))
