#!/usr/bin/env python3
"""Minimal MCP-over-stdio client for driving a `rs_cam_gui --mcp` instance.

TD3 wave B-1 (G-LV.2 crash capture). Deliberately dependency-free: rmcp's
stdio transport is newline-delimited JSON-RPC 2.0 with `max_length =
usize::MAX` (rmcp-1.3.0 `transport/async_rw.rs:157`), so a socket-free
line reader is a faithful client.

Two launch modes:
  * direct  -- exec the binary with pipes on stdin/stdout.
  * gdb     -- run the binary under `gdb --batch`, wiring the inferior's
               stdin/stdout to FIFOs via gdb's shell redirection, so a fatal
               signal produces a backtrace without gdb eating the JSON-RPC.
               (`/proc/sys/kernel/yama/ptrace_scope == 1` on this host, so
               attach-after-launch is not available; the debugger has to be
               the parent.)

Everything measured is written as JSONL so a crash mid-census loses nothing.
"""

import json
import os
import subprocess
import sys
import threading
import time
import queue


class McpProcess:
    def __init__(self, binary, workdir, log_dir, mode="direct", extra_env=None,
                 keep_wayland=False):
        self.binary = binary
        self.workdir = workdir
        self.log_dir = log_dir
        self.mode = mode
        os.makedirs(log_dir, exist_ok=True)
        self.stderr_path = os.path.join(log_dir, "gui_stderr.log")
        self.gdb_path = os.path.join(log_dir, "gdb.log")
        self.jsonl_path = os.path.join(log_dir, "calls.jsonl")
        self._next_id = 0
        self._resp = queue.Queue()
        self._proc = None
        self._gui_pid = None
        self._reader = None
        self._env = dict(os.environ)
        self._env.update(
            {
                "RUST_BACKTRACE": "full",
                # B-1 finding: `WINIT_UNIX_BACKEND` is INERT on this build.
                # winit removed it in 0.29 ("in favor of standard
                # WAYLAND_DISPLAY and DISPLAY variables",
                # winit-0.30.13/src/changelog/v0.29.md:134) and this workspace
                # is on winit 0.30.13. A smoke run with the variable set still
                # came up on Wayland (sctk_adwaita in the log) and `load_project`
                # never returned. The way to actually get X11 is to hide
                # WAYLAND_DISPLAY and leave DISPLAY pointing at XWayland.
                "WINIT_UNIX_BACKEND": "x11",
                "RUST_LOG": self._env.get("RUST_LOG", "info"),
            }
        )
        if self._env.get("DISPLAY") and not keep_wayland:
            self._env.pop("WAYLAND_DISPLAY", None)
        if extra_env:
            self._env.update(extra_env)
        self._fifo_in = os.path.join(log_dir, "in.fifo")
        self._fifo_out = os.path.join(log_dir, "out.fifo")
        self._wf = None
        self._rf = None

    # ---------------------------------------------------------------- launch
    def start(self):
        if self.mode == "direct":
            self._start_direct()
        elif self.mode == "gdb":
            self._start_gdb()
        else:
            raise ValueError(self.mode)
        self._reader = threading.Thread(target=self._read_loop, daemon=True)
        self._reader.start()

    def _start_direct(self):
        self._errf = open(self.stderr_path, "wb")
        self._proc = subprocess.Popen(
            [self.binary, "--mcp"],
            cwd=self.workdir,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=self._errf,
            env=self._env,
            preexec_fn=_unlimited_core,
        )
        self._gui_pid = self._proc.pid
        self._wf = self._proc.stdin
        self._rf = self._proc.stdout

    def _start_gdb(self):
        for p in (self._fifo_in, self._fifo_out):
            if os.path.exists(p):
                os.unlink(p)
            os.mkfifo(p)
        cmds = [
            "set pagination off",
            "set confirm off",
            "set print thread-events off",
            "handle SIGPIPE nostop noprint pass",
            "handle SIG32 SIG33 SIG34 nostop noprint pass",
            f"run --mcp < {self._fifo_in} > {self._fifo_out} 2>> {self.stderr_path}",
            "echo \\n=== INFERIOR STOPPED ===\\n",
            "info program",
            "info registers rip rsp",
            "thread apply all bt 40",
            "info proc mappings",
            "kill",
        ]
        argv = ["gdb", "--batch", "-nx"]
        for c in cmds:
            argv += ["-ex", c]
        argv += ["--args", self.binary, "--mcp"]
        self._gdbf = open(self.gdb_path, "wb")
        self._proc = subprocess.Popen(
            argv,
            cwd=self.workdir,
            stdin=subprocess.DEVNULL,
            stdout=self._gdbf,
            stderr=subprocess.STDOUT,
            env=self._env,
            preexec_fn=_unlimited_core,
        )
        # Opening a FIFO for write blocks until a reader appears -- open the
        # read end first in a thread so neither side deadlocks.
        holder = {}

        def _open_read():
            holder["r"] = open(self._fifo_out, "rb")

        t = threading.Thread(target=_open_read, daemon=True)
        t.start()
        self._wf = open(self._fifo_in, "wb")
        t.join(timeout=120)
        self._rf = holder.get("r")
        if self._rf is None:
            raise RuntimeError("inferior never opened its stdout fifo")
        # Resolve the inferior pid (child of gdb).
        deadline = time.time() + 60
        while time.time() < deadline:
            kids = _children_of(self._proc.pid)
            named = [
                p for p in kids if _comm(p) in ("rs_cam_gui", "sh", "bash", "zsh")
            ]
            real = [p for p in kids if _comm(p) == "rs_cam_gui"]
            if real:
                self._gui_pid = real[0]
                break
            for p in named:
                sub = [q for q in _children_of(p) if _comm(q) == "rs_cam_gui"]
                if sub:
                    self._gui_pid = sub[0]
                    break
            if self._gui_pid:
                break
            time.sleep(0.5)

    # ------------------------------------------------------------------ io
    def _read_loop(self):
        try:
            for line in self._rf:
                if not line.strip():
                    continue
                self._resp.put((time.time(), line))
        except Exception as e:  # noqa: BLE001
            self._resp.put((time.time(), None, repr(e)))
        finally:
            self._resp.put((time.time(), None, "EOF"))

    def _send(self, obj):
        data = (json.dumps(obj) + "\n").encode()
        self._wf.write(data)
        self._wf.flush()

    def rpc(self, method, params=None, timeout=60.0, tag=None):
        self._next_id += 1
        rid = self._next_id
        req = {"jsonrpc": "2.0", "id": rid, "method": method}
        if params is not None:
            req["params"] = params
        t0 = time.time()
        try:
            self._send(req)
        except BrokenPipeError as e:
            return {"__transport__": "broken_pipe_on_send", "error": repr(e),
                    "wall_s": time.time() - t0}
        deadline = t0 + timeout
        while True:
            remaining = deadline - time.time()
            if remaining <= 0:
                return {"__transport__": "timeout", "wall_s": time.time() - t0}
            try:
                item = self._resp.get(timeout=min(remaining, 1.0))
            except queue.Empty:
                if self.dead():
                    return {"__transport__": "process_died_while_waiting",
                            "wall_s": time.time() - t0}
                continue
            if item[1] is None:
                return {"__transport__": "stream_closed", "detail": item[2],
                        "wall_s": time.time() - t0}
            ts, line = item[0], item[1]
            try:
                msg = json.loads(line)
            except Exception:  # noqa: BLE001
                continue
            if msg.get("id") != rid:
                continue
            return {
                "wall_s": ts - t0,
                "line_bytes": len(line),
                "msg": msg,
            }

    def notify(self, method, params=None):
        obj = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            obj["params"] = params
        self._send(obj)

    def initialize(self, timeout=60.0):
        r = self.rpc(
            "initialize",
            {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "td3-b1-probe", "version": "0"},
            },
            timeout=timeout,
        )
        self.notify("notifications/initialized")
        return r

    def call_tool(self, name, args=None, timeout=120.0):
        return self.rpc(
            "tools/call", {"name": name, "arguments": args or {}}, timeout=timeout
        )

    # -------------------------------------------------------------- process
    def dead(self):
        if self.mode == "direct":
            return self._proc.poll() is not None
        if self._gui_pid is None:
            return False
        return not os.path.exists(f"/proc/{self._gui_pid}")

    def wait_exit(self, timeout=60):
        try:
            return self._proc.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            return None

    def kill(self):
        try:
            if self._gui_pid and os.path.exists(f"/proc/{self._gui_pid}"):
                os.kill(self._gui_pid, 9)
        except Exception:  # noqa: BLE001
            pass
        try:
            self._proc.kill()
        except Exception:  # noqa: BLE001
            pass


def _unlimited_core():
    import resource

    try:
        resource.setrlimit(resource.RLIMIT_CORE, (resource.RLIM_INFINITY,
                                                  resource.RLIM_INFINITY))
    except Exception:  # noqa: BLE001
        pass


def _children_of(pid):
    try:
        with open(f"/proc/{pid}/task/{pid}/children") as f:
            return [int(x) for x in f.read().split()]
    except Exception:  # noqa: BLE001
        return []


def _comm(pid):
    try:
        with open(f"/proc/{pid}/comm") as f:
            return f.read().strip()
    except Exception:  # noqa: BLE001
        return ""


class RssSampler(threading.Thread):
    """Poll /proc/PID/status VmRSS/VmSize + system MemAvailable.

    Also the safety valve: if system MemAvailable drops under `floor_kb`
    the sampled process is killed, on purpose, rather than letting the
    kernel OOM-killer pick a victim (the operator's own GUI is resident).
    """

    def __init__(self, get_pid, out_path, period=0.5, floor_kb=4 * 1024 * 1024,
                 on_floor=None):
        super().__init__(daemon=True)
        self.get_pid = get_pid
        self.out_path = out_path
        self.period = period
        self.floor_kb = floor_kb
        self.on_floor = on_floor
        self.stop_flag = threading.Event()
        self.marks = []
        self.peak_rss_kb = 0
        self.tripped = False

    def mark(self, label):
        self.marks.append((time.time(), label))
        with open(self.out_path, "a") as f:
            f.write(f"# MARK {time.time():.3f} {label}\n")

    def run(self):
        with open(self.out_path, "a") as f:
            f.write("# t_epoch,vmrss_kb,vmsize_kb,vmswap_kb,mem_available_kb,threads\n")
        while not self.stop_flag.is_set():
            pid = self.get_pid()
            row = None
            if pid and os.path.exists(f"/proc/{pid}/status"):
                st = {}
                try:
                    with open(f"/proc/{pid}/status") as f:
                        for line in f:
                            k, _, v = line.partition(":")
                            st[k] = v.strip()
                except Exception:  # noqa: BLE001
                    st = {}
                rss = _kb(st.get("VmRSS"))
                self.peak_rss_kb = max(self.peak_rss_kb, rss)
                row = (rss, _kb(st.get("VmSize")), _kb(st.get("VmSwap")),
                       _memavail(), st.get("Threads", "?"))
            else:
                row = (0, 0, 0, _memavail(), "?")
            with open(self.out_path, "a") as f:
                f.write(f"{time.time():.3f},{row[0]},{row[1]},{row[2]},{row[3]},{row[4]}\n")
            if row[3] and row[3] < self.floor_kb and pid:
                self.tripped = True
                with open(self.out_path, "a") as f:
                    f.write(f"# SAFETY-KILL MemAvailable={row[3]}kB < floor\n")
                if self.on_floor:
                    self.on_floor()
                break
            self.stop_flag.wait(self.period)


def _kb(s):
    if not s:
        return 0
    try:
        return int(s.split()[0])
    except Exception:  # noqa: BLE001
        return 0


def _memavail():
    try:
        with open("/proc/meminfo") as f:
            for line in f:
                if line.startswith("MemAvailable:"):
                    return int(line.split()[1])
    except Exception:  # noqa: BLE001
        pass
    return 0
