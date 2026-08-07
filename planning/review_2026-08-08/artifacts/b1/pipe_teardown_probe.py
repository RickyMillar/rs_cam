#!/usr/bin/env python3
"""TD3 B-1: what actually kills `rs_cam_gui --mcp`?

The unfiltered `get_cut_trace` was reproduced at wanaka scale and the server
SURVIVED it (60,517,035-byte JSON-RPC line, 1.71 s, +0.5 GB RSS). So the
G-LV.2 death is not the server falling over while serializing. These probes
test the remaining transport-side mechanisms, each in isolation and each
needing no project:

  A  stdin EOF               -- the client drops the transport.
  B  stdout reader stops     -- the client stops draining; the server's
                               write blocks on a full pipe. Then the reader
                               closes: does EPIPE/SIGPIPE take the process?
  C  client SIGKILLs itself  -- both fds vanish at once (what a client
                               teardown/reap actually looks like).

Each probe reports whether the GUI process was still alive afterwards.
"""

import json
import os
import subprocess
import sys
import time

BIN = "/home/ricky/personal_repos/rs_cam/target/release/rs_cam_gui"
REPO = "/home/ricky/personal_repos/rs_cam"


def env():
    e = dict(os.environ)
    e["RUST_BACKTRACE"] = "full"
    e["RUST_LOG"] = "info"
    if e.get("DISPLAY"):
        e.pop("WAYLAND_DISPLAY", None)
    return e


def spawn(log_dir, tag):
    os.makedirs(log_dir, exist_ok=True)
    errf = open(os.path.join(log_dir, f"{tag}_stderr.log"), "wb")
    p = subprocess.Popen(
        [BIN, "--mcp"],
        cwd=REPO,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=errf,
        env=env(),
    )
    return p, errf


def initialize(p):
    req = {
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2025-06-18", "capabilities": {},
                   "clientInfo": {"name": "td3-b1-teardown", "version": "0"}},
    }
    p.stdin.write((json.dumps(req) + "\n").encode())
    p.stdin.flush()
    line = p.stdout.readline()
    p.stdin.write(b'{"jsonrpc":"2.0","method":"notifications/initialized"}\n')
    p.stdin.flush()
    return len(line)


def alive(p, wait=5.0):
    t0 = time.time()
    while time.time() - t0 < wait:
        if p.poll() is not None:
            return False, p.returncode
        time.sleep(0.25)
    return True, None


def probe_a(log_dir):
    """stdin EOF while the process is idle and healthy."""
    p, errf = spawn(log_dir, "A")
    time.sleep(3)
    n = initialize(p)
    time.sleep(1)
    p.stdin.close()
    ok, rc = alive(p, wait=10)
    out = {"probe": "A stdin EOF", "init_bytes": n,
           "alive_10s_after_stdin_close": ok, "returncode": rc,
           "pid": p.pid}
    if ok:
        p.kill()
    errf.close()
    return out


def probe_b(log_dir):
    """Reader stops draining, pipe fills, then reader closes the fd."""
    p, errf = spawn(log_dir, "B")
    time.sleep(3)
    n = initialize(p)
    # Queue many tools/list calls (55 kB each) and never read them. The 64 kB
    # pipe buffer fills after the first one or two; the server's writer then
    # blocks.
    for i in range(2, 22):
        req = {"jsonrpc": "2.0", "id": i, "method": "tools/list", "params": {}}
        p.stdin.write((json.dumps(req) + "\n").encode())
    p.stdin.flush()
    time.sleep(5)
    # Close the read end while the server is blocked writing into it.
    p.stdout.close()
    time.sleep(1)
    ok, rc = alive(p, wait=15)
    out = {"probe": "B stdout reader closes mid-write", "init_bytes": n,
           "alive_15s_after_stdout_close": ok, "returncode": rc, "pid": p.pid}
    if ok:
        p.kill()
    errf.close()
    return out


def probe_c(log_dir):
    """Both ends vanish at once -- a client teardown."""
    p, errf = spawn(log_dir, "C")
    time.sleep(3)
    n = initialize(p)
    p.stdin.close()
    p.stdout.close()
    ok, rc = alive(p, wait=15)
    out = {"probe": "C both fds closed", "init_bytes": n,
           "alive_15s_after_both_closed": ok, "returncode": rc, "pid": p.pid}
    if ok:
        p.kill()
    errf.close()
    return out


if __name__ == "__main__":
    log_dir = sys.argv[1]
    results = [probe_a(log_dir), probe_b(log_dir), probe_c(log_dir)]
    for r in results:
        print(json.dumps(r))
    with open(os.path.join(log_dir, "teardown_results.json"), "w") as f:
        json.dump(results, f, indent=2)
