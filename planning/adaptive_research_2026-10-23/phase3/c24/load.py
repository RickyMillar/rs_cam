import sys, math
sys.path.insert(0,'/home/ricky/personal_repos/rs_cam/planning/adaptive_research_2026-10-23/phase3/fix2')
import fixture3d_v2 as F
SIM='/home/ricky/personal_repos/rs_cam/planning/adaptive_research_2026-10-23/phase3/fix2/simart/corne/'
def get(arm, lab='Z 5.00'):
    d=F.load_op(SIM+arm,'tp_1_*.json','3D Rough 1')
    return d
if __name__=='__main__':
    d=get(sys.argv[1])
    print(d['levels'], d['offset'])
    lab='Z 5.00'
    rings=[p for i,p in d['rings'] if d['lev'][i]==lab]
    print('rings',len(rings))
    for p in rings[:40]: print('  %.2f %.2f'%(p[0],p[1]))
    n=sum(1 for i,c in enumerate(d['cls']) if c=='cut' and d['lev'][i]==lab)
    print('cut moves',n)
