import sys, time, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import stream as st
nm, mode = sys.argv[1], sys.argv[2]
t=time.time()
sh = st.Shape(nm)
U = st.harmonic_field(sh)
print("field", time.time()-t, flush=True)
d = st.Direction(sh, U)
loops = st.march(sh, d, mode, air_move=os.environ.get("AIRMOVE")=="1", max_loops=int(sys.argv[3]), log=lambda s: print(s, round(time.time()-t,1), flush=True))
