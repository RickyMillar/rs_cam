#!/usr/bin/env python3
"""Stand-in for /usr/bin/time -v (absent here): run argv, report peak RSS and wall time,
and sample RSS every 2 s to a trace. Run it under prlimit --as for the cap."""
import subprocess, sys, time, resource, os
t0 = time.time()
p = subprocess.Popen(sys.argv[1:])
trace = []
while p.poll() is None:
    try:
        with open(f"/proc/{p.pid}/status") as f:
            for l in f:
                if l.startswith("VmRSS"):
                    trace.append((round(time.time()-t0,1), int(l.split()[1])//1024))
    except Exception:
        pass
    time.sleep(2)
ru = resource.getrusage(resource.RUSAGE_CHILDREN)
print(f"Exit status: {p.returncode}", file=sys.stderr)
print(f"Elapsed (wall clock) s: {time.time()-t0:.1f}", file=sys.stderr)
print(f"Maximum resident set size (kbytes): {ru.ru_maxrss}", file=sys.stderr)
print("RSS trace (s, MB): " + " ".join(f"{a}:{b}" for a,b in trace[::max(1,len(trace)//40)]), file=sys.stderr)
