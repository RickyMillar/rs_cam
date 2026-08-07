# G-LV.2 — crash capture and MCP read-size census

TD3 wave **B-1** (research only; no behavioural change). Ledger rows
**G-LV.2** and **C25** (`planning/review_2026-08-04/TECH_DEBT_2_CLOSEOUT.md`
§4), incident provenance in the W10-LV entry of
`planning/review_2026-08-04/ORCHESTRATION_LOG.md`.

Date: 2026-08-08. Branch `tech-debt-3`. Binary under test:
`target/release/rs_cam_gui`, built 2026-08-07 03:08 from `d820226` — the same
build the incident ran on. `git diff --name-only d820226..HEAD` returns
**nothing outside `planning/`**, so the working tree has not moved under the
binary.

Repro scripts: `artifacts/b1/` (committed).
Raw logs stay in session scratch and are quoted verbatim below.

---

## 0. Headline

The two hypotheses the ledger carried are both **falsified**, on a
population-identical reproduction of the incident:

| Hypothesis | Verdict | Evidence |
|---|---|---|
| The server OOMs while serializing the unfiltered response | **FALSIFIED** | The call returned **60,517,035 bytes in 1.71 s**; process RSS moved 4.31 GB → 4.81 GB (**+0.50 GB**) and system MemAvailable never went below 20.0 GB. The process was alive afterwards and answered `list_toolpaths` in **0.133 s**. |
| The server dies when the transport closes (stdin EOF / EPIPE) | **FALSIFIED** | Three isolated teardown probes (stdin EOF; reader closes mid-write with the pipe full; both fds closed at once) — the GUI process **survived all three**. |

What *is* confirmed is the pathology the fix has to address: **one MCP read
emits a 60.5 MB single JSON-RPC line**. Across the 64-call census its payload
is **16,245× the median response** (3,461 B) and **710× the largest response
of any tool other than `get_cut_trace`** (79,135 B).

The **death itself was not reproduced**, and this document does not name a
mechanism it did not observe. Section 4 states exactly which two variables
remained uncontrolled and which of them could not be exercised, and why.

---

## 1. The rig

`artifacts/b1/mcp_stdio_client.py` + `artifacts/b1/glv2_repro.py`. A
dependency-free MCP-over-stdio client (rmcp 1.3's stdio transport is
newline-delimited JSON-RPC 2.0 with `max_length = usize::MAX`,
`rmcp-1.3.0/src/transport/async_rw.rs:157` — there is no server-side framing
limit to trip).

The binary runs **under `gdb --batch`** with the inferior's stdin/stdout wired
to FIFOs through gdb's own shell redirection
(`run --mcp < in.fifo > out.fifo 2>> stderr.log`). Attach-after-launch is not
available on this host: `/proc/sys/kernel/yama/ptrace_scope == 1`, so the
debugger has to be the parent. gdb catches a fatal signal and dumps
`thread apply all bt` before the process is gone.

Instrumentation: `RUST_BACKTRACE=full`, stderr to file, `RLIMIT_CORE`
unlimited, and a 500 ms sidecar sampler on `/proc/PID/status`
(`VmRSS`/`VmSize`/`VmSwap`) plus system `MemAvailable`, with a hard safety
valve that kills *my* process if MemAvailable drops under 4 GB (the
operator's own GUI was resident throughout — it never fired).

Core-dump capture was **degraded, stated**: `/proc/sys/kernel/core_pattern`
pipes to `apport`, `coredumpctl` is not installed, `dmesg` is restricted
(`Operation not permitted`), and the release binary is **stripped**
(`readelf -S | grep -c debug_info` → `0`). gdb-as-parent was chosen precisely
because it does not depend on any of those. In the event nothing faulted, so
none of it was needed.

### 1.1 `WINIT_UNIX_BACKEND=x11` is inert on this build — use `WAYLAND_DISPLAY`

TD3 §0 rule 10 and this wave's brief both prescribe `WINIT_UNIX_BACKEND=x11`.
**That variable does nothing here.** winit removed it in 0.29 — "`WINIT_UNIX_BACKEND`
was removed in favor of standard `WAYLAND_DISPLAY` and `DISPLAY` variables"
(`winit-0.30.13/src/changelog/v0.29.md:134`) — and this workspace is on winit
**0.30.13** (`Cargo.lock:6173-6174`).

Measured, not inferred: a first smoke run with `WINIT_UNIX_BACKEND=x11` set
came up on **Wayland** (`sctk_adwaita::buttons` in the log) and `load_project`
never returned — killed at **162.6 s**. The rig now **unsets
`WAYLAND_DISPLAY`** and leaves `DISPLAY=:0`; the identical call then answered
in **0.515 s**. Recommend correcting the rule's wording where it is cited.

---

## 2. Reproduction — population-identical, and it did not die

Fixture: a scratch **copy** of `planning/airrun_2026-06-01/wanaka.toml`
(`wanaka_full_tuned`, 2 setups, 9 toolpaths, 7 enabled). The operator's file
was never opened for write, never staged; its four model references resolve
read-only under `/home/ricky/Downloads/wanaka100/`.

Recipe, exactly as the ledger describes it:

1. `load_project` — 0.975 s
2. `generate_all {fixpoint: true, simulation_resolution_mm: 0.1, timeout_s: 2700}`
   → `generated: 7, rounds: 3, simulations: 2`, **401.8 s**
3. `run_simulation {resolution: 0.1}` — **107.5 s**
4. …census of every bounded read (§5) …
5. `get_cut_trace {}` — unfiltered

The trace this produced is **the incident's trace, number for number**:

| | this run | W10-LV incident |
|---|---|---|
| `summary.sample_count` | **1,588,883** | 1,588,883 |
| `summary.issue_count` (coalesced segments) | **73,326** | 73,326 |
| `summary.hotspot_count` | 394 | 394 (`total_matching` in the triage entry) |

### 2.1 The unfiltered call

```
[10:32:37] get_cut_trace {} -> line=60517035 content=56225225 wall=1.71s transport=None
after unfiltered call: process alive = True
[10:32:40] list_toolpaths {} -> line=2774 content=2322 wall=0.133s transport=None
```

- **60,517,035 bytes** on one JSON-RPC line; **56,225,225 bytes** of tool text
  inside it (the JSON-RPC envelope + string escaping adds **7.6 %**).
- **1.71 s** wall.
- The process was **alive**, and the very next read answered in 133 ms.

### 2.2 RSS timeline across the fatal call

Sidecar sampler, 500 ms period, `t = 0` at the mark preceding the unfiltered
call:

```
  t(s)   VmRSS kB   VmSize kB   VmSwap  MemAvailable kB
 -6.0    4257696    10155544      0        20905980
 -4.0    4301492    10199448      0        20838812
 -3.5    4692356    10614548      0        20445324
 -1.0    4713284    10614548      0        20303916
  0.0    4313180    10204632      0        20699224     <- FATAL mark
 +1.5    4807212    10718536      0        20251440     <- response served
 +2.0    4762448    10652996      0        20074968
 +4.5    4762448    10652996      0        20099580
```

Peak RSS **for the whole session** was **8,841,056 kB (8.84 GB)** and it
occurred during `run_simulation`, **not** during any read. The unfiltered read
cost about **half a gigabyte** of transient RSS. System MemAvailable never
dropped below **20.0 GB**. There is no allocation cliff here to hang an OOM
story on.

### 2.3 Nothing faulted

`gdb.log` for the whole run, in full after the load banner:

```
[Thread debugging using libthread_db enabled]
Using host libthread_db library "/usr/lib/x86_64-linux-gnu/libthread_db.so.1".
```

gdb never stopped the inferior — no signal was delivered. And in the GUI's own
stderr:

```
$ grep -icE "panic|memory allocation|abort|SIGSEGV|SIGABRT|stack overflow" gui_stderr.log gdb.log
gui_stderr.log:0
gdb.log:0
```

No panic (which the lint policy forbids in non-test code anyway), no
`memory allocation of N bytes failed`, no abort, no stack overflow. The
process ended when **the script killed it** at teardown.

### 2.4 Run twice, identical both times

The whole recipe was executed a second time in a fresh process (`run3`, used
for the section accounting in §5.1). The unfiltered call returned the **same
byte counts to the byte** — 60,517,035 line / 56,225,225 payload, 1.672 s vs
1.710 s — and the process survived again. `generate_all` took 388.7 s vs
401.8 s and `run_simulation` 107.494 s vs 107.535 s. Nine of the census's
`get_cut_trace` filtered responses are byte-identical across the two runs.
This is not a one-off observation.

---

## 3. Transport teardown probes — the server survives all of them

`artifacts/b1/pipe_teardown_probe.py`. Each probe launches a fresh
`rs_cam_gui --mcp`, initializes MCP, then breaks the transport a different
way. No project needed.

| probe | what it does | GUI alive after? |
|---|---|---|
| **A** | client closes **stdin** (transport dropped, EOF to the server's reader) | **yes** (10 s) |
| **B** | client stops draining **stdout**, queues 20 × `tools/list` (≈55 kB each) so the 64 kB pipe fills and the server's writer blocks, then **closes the read fd** mid-write | **yes** (15 s) |
| **C** | both fds closed at once — what a client-side teardown looks like | **yes** (15 s) |

```
{"probe": "A stdin EOF", "alive_10s_after_stdin_close": true, "returncode": null}
{"probe": "B stdout reader closes mid-write", "alive_15s_after_stdout_close": true, "returncode": null}
{"probe": "C both fds closed", "alive_15s_after_both_closed": true, "returncode": null}
```

This matches the code: `router.serve(rmcp::transport::stdio())` runs on a
dedicated `mcp-server` thread (`crates/rs_cam_viz/src/app.rs:84-118`); when
the service ends it logs `Embedded MCP server shut down` and the thread
returns. Nothing in that path touches the GUI's lifetime. **A broken MCP
transport cannot, by itself, kill this process.**

---

## 4. What is still uncontrolled — stated, not guessed

The incident says the connection closed mid-response **and** the process was
gone. §2 and §3 rule out the server serializing itself to death and the server
dying of a broken pipe. Two variables differed between the incident and this
reproduction:

**(a) The MCP client.** The incident's client was Claude Code; mine is a
line reader with no size limit. `rs_cam_gui --mcp` runs as a **direct child of
`claude`** — verified read-only on the operator's live instance, which was not
otherwise touched:

```
$ ps -o pid,ppid,args -p 3837079
3837079  114759 target/release/rs_cam_gui --mcp
   114759 claude
```

So the client owns the process lifecycle: a client that cannot ingest a
60.5 MB frame, gives up on the transport and reaps its child produces exactly
the observed signature (connection closed mid-response, process gone, system
RAM fine, no fault in the server). This is **consistent with every
observation and is not established** — instrumenting the operator's Claude
Code client was out of scope for this wave, and the standing instruction
forbade using the live MCP session. It is offered as the leading candidate,
labelled as such.

**(b) The windowing backend. NOT EXERCISED.** The incident ran on Wayland;
§2 ran on X11. A Wayland re-run was attempted (`run2`, same recipe,
`WAYLAND_DISPLAY` retained) and **aborted at `load_project` after 90 s with no
response** — the G-LV.1 mechanism verbatim: an occluded/unmapped Wayland
surface receives no frame callbacks, so nothing dispatched from a repaint
advances. The run could not get far enough to have a trace to read, let alone
a large response to serve. Recorded as **NOT EXERCISED, with the reason**;
this is corroborating G-LV.1 evidence, not a result about G-LV.2.

Consequence for B-2: **a bounded read fixes the pathology regardless of which
of (a) or (b) delivered the kill.** A response that is never 60 MB cannot
break any client, and cannot spend 1.7 s of frame loop building something
nobody can read. But the crash cause is **named as a candidate, not as a
finding**, and the ledger row should say so.

---

## 5. Read-size census

All measurements on the trace of §2 (1,588,883 samples, cell 0.1 mm), one
call at a time, release build, X11. "response bytes" is the tool's own text
payload; "line bytes" is the whole JSON-RPC line the client must buffer.

| # | call | params | response bytes (tool text) | JSON-RPC line bytes | wall s |
|---|---|---|---|---|---|
| 1 | `load_project` | `{"path":"<scratch>/wanaka_copy.toml"}` | 51 | 141 | 0.975 |
| 2 | `list_toolpaths` | `(none)` | 3,160 | 3,650 | 0.752 |
| 3 | `generate_all` | `{"fixpoint":true,"simulation_resolution_mm":0.1,"timeout_s":2700}` | 228 | 348 | 401.795 |
| 4 | `generation_status` | `(none)` | 843 | 1,005 | 0.0 |
| 5 | `run_simulation` | `{"resolution":0.1}` | 79,135 | 86,943 | 107.535 |
| 6 | `get_diagnostics` | `(none)` | 79,135 | 86,943 | 0.864 |
| 7 | `get_project_diagnostics` | `(none)` | 2 | 92 | 0.169 |
| 8 | `get_tool_load_report` | `(none)` | 35,873 | 38,781 | 0.933 |
| 9 | `narrate_toolpath` | `{"index":0}` | 3,719 | 3,839 | 0.713 |
| 10 | `narrate_toolpath` | `{"index":1}` | 4,640 | 4,761 | 1.012 |
| 11 | `narrate_toolpath` | `{"index":2}` | 3,363 | 3,479 | 0.997 |
| 12 | `narrate_toolpath` | `{"index":3}` | 61 | 152 | 0.96 |
| 13 | `narrate_toolpath` | `{"index":4}` | 4,563 | 4,690 | 1.044 |
| 14 | `narrate_toolpath` | `{"index":5}` | 4,573 | 4,700 | 1.004 |
| 15 | `narrate_toolpath` | `{"index":6}` | 3,559 | 3,677 | 0.999 |
| 16 | `narrate_toolpath` | `{"index":7}` | 61 | 152 | 0.957 |
| 17 | `narrate_toolpath` | `{"index":8}` | 4,200 | 4,326 | 1.059 |
| 18 | `get_toolpath_diagnostics` | `{"index":0}` | 2,792 | 3,298 | 1.3 |
| 19 | `get_toolpath_diagnostics` | `{"index":1}` | 2,815 | 3,266 | 0.965 |
| 20 | `get_toolpath_diagnostics` | `{"index":2}` | 2,786 | 3,292 | 0.981 |
| 21 | `get_toolpath_diagnostics` | `{"index":3}` | 2 | 93 | 1.004 |
| 22 | `get_toolpath_diagnostics` | `{"index":4}` | 1,699 | 2,029 | 1.008 |
| 23 | `get_toolpath_diagnostics` | `{"index":5}` | 1,699 | 2,029 | 0.994 |
| 24 | `get_toolpath_diagnostics` | `{"index":6}` | 2,763 | 3,206 | 1.006 |
| 25 | `get_toolpath_diagnostics` | `{"index":7}` | 1,061 | 1,311 | 1.001 |
| 26 | `get_toolpath_diagnostics` | `{"index":8}` | 3,616 | 4,101 | 0.994 |
| 27 | `inspect_spans` | `{"index":0}` | 571 | 743 | 0.699 |
| 28 | `inspect_spans` | `{"index":1}` | 2,053 | 2,412 | 1.001 |
| 29 | `inspect_spans` | `{"index":2}` | 565 | 737 | 0.998 |
| 30 | `inspect_spans` | `{"index":3}` | 54 | 145 | 1.0 |
| 31 | `inspect_spans` | `{"index":4}` | 632 | 810 | 1.002 |
| 32 | `inspect_spans` | `{"index":5}` | 634 | 812 | 1.0 |
| 33 | `inspect_spans` | `{"index":6}` | 1,238 | 1,492 | 1.003 |
| 34 | `inspect_spans` | `{"index":7}` | 54 | 145 | 0.999 |
| 35 | `inspect_spans` | `{"index":8}` | 685 | 866 | 1.003 |
| 36 | `get_generation_debug_trace` | `{"index":0}` | 1,970 | 2,313 | 1.0 |
| 37 | `get_generation_debug_trace` | `{"index":1}` | 32,567 | 36,338 | 0.993 |
| 38 | `get_generation_debug_trace` | `{"index":2}` | 2,931 | 3,409 | 1.001 |
| 39 | `get_generation_debug_trace` | `{"index":3}` | 94 | 191 | 0.998 |
| 40 | `get_generation_debug_trace` | `{"index":4}` | 3,343 | 3,866 | 1.001 |
| 41 | `get_generation_debug_trace` | `{"index":5}` | 3,349 | 3,872 | 1.004 |
| 42 | `get_generation_debug_trace` | `{"index":6}` | 21,041 | 23,470 | 0.994 |
| 43 | `get_generation_debug_trace` | `{"index":7}` | 94 | 191 | 0.999 |
| 44 | `get_generation_debug_trace` | `{"index":8}` | 14,834 | 17,088 | 0.999 |
| 45 | `get_cut_trace` | `{"toolpath_id":0}` | 3,972 | 4,358 | 1.005 |
| 46 | `get_cut_trace` | `{"toolpath_id":1}` | 3,972 | 4,358 | 0.998 |
| 47 | `get_cut_trace` | `{"toolpath_id":2}` | 3,972 | 4,358 | 0.999 |
| 48 | `get_cut_trace` | `{"toolpath_id":3}` | 3,972 | 4,358 | 1.002 |
| 49 | `get_cut_trace` | `{"toolpath_id":4}` | 842,642 | 909,016 | 1.017 |
| 50 | `get_cut_trace` | `{"toolpath_id":5}` | 2,122,146 | 2,283,067 | 1.015 |
| 51 | `get_cut_trace` | `{"toolpath_id":6}` | 1,283,292 | 1,381,771 | 0.987 |
| 52 | `get_cut_trace` | `{"toolpath_id":7}` | 4,434 | 4,869 | 0.981 |
| 53 | `get_cut_trace` | `{"toolpath_id":8}` | 3,972 | 4,358 | 0.992 |
| 54 | `get_cut_trace` | `{"toolpath_id":10}` | 620,539 | 668,694 | 1.015 |
| 55 | `get_cut_trace` | `{"toolpath_id":11}` | 3,972 | 4,358 | 0.987 |
| 56 | `get_cut_trace` | `{"toolpath_id":12}` | 3,972 | 4,358 | 1.004 |
| 57 | `get_cut_trace` | `{"toolpath_id":14}` | 4,435 | 4,870 | 1.006 |
| 58 | `get_cut_trace` | `{"toolpath_id":15}` | 51,595,653 | 55,534,346 | 1.74 |
| 59 | `get_cut_trace` | `{"toolpath_id":0,"max_hotspots":10000,"max_issues":100000}` | 3,972 | 4,358 | 0.172 |
| 60 | `get_cut_trace` | `{"toolpath_id":1,"max_hotspots":10000,"max_issues":100000}` | 3,972 | 4,358 | 0.995 |
| 61 | `get_cut_trace` | `{"toolpath_id":2,"max_hotspots":10000,"max_issues":100000}` | 3,972 | 4,358 | 1.0 |
| 62 | `get_cut_trace` | `{"span_kind":"depth_pass","max_issues":50}` | 459,120 | 496,972 | 1.075 |
| 63 | `get_cut_trace` | `(none)` | 56,225,225 | 60,517,035 | 1.71 |
| 64 | `list_toolpaths` | `(none)` | 2,322 | 2,774 | 0.133 |

**Side observation, for B-3.** The wall-clock column is almost entirely
**dispatch latency, not work**: **50 of the 64** calls land between 0.94 s and
1.08 s regardless of whether they return 51 bytes or 2.1 MB, and the 56 MB
call took 1.71 s — i.e. ~1.0 s of waiting plus ~0.7 s of building. Only four
answered under 0.5 s. On an **idle, visible, X11** window with nothing
generating, an MCP round-trip on the frame-loop path therefore has a floor of
about **one second**. That is not a G-LV.2 finding and nothing here depends
on it, but it is a measured number the dispatch-decoupling wave should have.

### 5.1 Section breakdown of the unfiltered response

Measured on the **second, independent** full run (`run3`), which
reproduced the response size **byte for byte**: 60,517,035 line /
56,225,225 payload, 1.672 s, process alive afterwards. Per top-level key:

| key | entries | bytes (as served, pretty) | bytes if compact | share |
|---|---|---|---|---|
| `span_summaries` | 35,838 | 52,852,388 | 42,494,107 | 94.00 % |
| `semantic_summaries` | 394 | 352,810 | 293,709 | 0.63 % |
| `issues` | 50 | 33,817 | 24,712 | 0.06 % |
| `hotspots` | 20 | 17,722 | 14,105 | 0.03 % |
| `toolpath_summaries` | 5 | 3,289 | 2,653 | 0.01 % |
| `summary` | 17 | 3,288 | 2,743 | 0.01 % |
| `drill_summaries` | 2 | 859 | 678 | 0.00 % |
| *(19 scalar keys)* | — | 62 | 62 | 0.00 % |
| **total** | | **56,225,225** | **42,833,152** | |

**Instrument note, so the residual is not read as a mystery.** Each section
is measured by re-serializing that value *standalone*, which loses the two
extra spaces of indentation every line carries once it is nested inside the
response object. The section figures therefore sum to 53,264,235 of the
56,225,225 served; the missing **2,960,990 bytes are that indentation**, and
essentially all of it belongs to `span_summaries` (35,838 objects × ~40
nested lines each). Read the section bytes as a **lower bound** per key and
the share column as a lower bound on `span_summaries`.

**`span_summaries` is at least 94.00 % of the payload** — 35,838 objects
averaging 1,475 bytes each. **Every other key put together is 411,847
bytes (0.73 %)**, which would fit inside any bound under discussion. Per
toolpath:

| filter | response bytes | `span_summaries` entries | its share |
|---|---|---|---|
| `{"toolpath_id":4}` | 842,642 | 470 | 83.6 % |
| `{"toolpath_id":5}` | 2,122,146 | 1,123 | 85.7 % |
| `{"toolpath_id":6}` | 1,283,292 | 758 | 87.8 % |
| `{"toolpath_id":10}` | 620,539 | 292 | 77.7 % |
| `{"toolpath_id":15}` | 51,595,653 | 33,195 | 94.4 % |
| `(unfiltered)` | 56,225,225 | 35,838 | 94.0 % |

One operation — the Unified Finish, `id 15` / index 8 — carries **33,195 of
the 35,838 span summaries** and 48.7 MB of the 56.2 MB. Its 205,892-move,
32,001-`GeometryRefit`-span structure is the W10-LV entry's own S-COST
figure; this is that structure being rendered one JSON object per span.

**Pretty-printing costs 31.3 % on this payload** (56,225,225 served vs
42,833,152 compact — 13,392,073 bytes of indentation and newlines). The
ratio is worse on small deeply-nested responses: `run_simulation` /
`get_diagnostics` measured **1.871×** (79,135 vs 42,295), i.e. **46.6 %** of
those responses is whitespace.

### 5.1b `include_drill_samples` — measured, and it is not the problem

The one array whose tool description already warns it "can be verbose"
(`mcp_server.rs:502`) was measured on both drill toolpaths (`run3`):

| call | bytes without | bytes with `include_drill_samples: true` | delta |
|---|---|---|---|
| `get_cut_trace {toolpath_id: 14}` (Pin Drill, 2 holes) | 4,435 | 10,719 | +6,284 |
| `get_cut_trace {toolpath_id: 7}` (Holes) | 4,434 | 24,846 | +20,412 |

Per-peck samples are cheap on this fixture. The warning is well-placed for a
many-hole cycle but the array is nowhere near the binding constraint here;
a cap on it is prudence, not a fix.

### 5.2 Static boundedness of the parameterised reads

What the code caps today, at `d820226`:

| call | array | cap | where |
|---|---|---|---|
| `get_cut_trace` | `hotspots` | `max_hotspots`, default **20** | `app/mcp.rs:1332` |
| `get_cut_trace` | `issues` | `max_issues`, default **50** | `app/mcp.rs:1333` |
| `get_cut_trace` | `span_summaries` | **none** | built at `app/mcp.rs:1405-1411`, `fn build_span_cut_summaries` at `:4604-4705` |
| `get_cut_trace` | `semantic_summaries` | **none** | `app/mcp.rs:1413-1417` |
| `get_cut_trace` | `toolpath_summaries` | **none** | `app/mcp.rs:1477-1500` |
| `get_cut_trace` | `drill_summaries` | **none** | `app/mcp.rs:1455-1461` |
| `get_cut_trace` | `drill_samples` | **none** (opt-in via `include_drill_samples`) | `app/mcp.rs:1462-1472` |
| `inspect_spans` | detail mode | `max_spans`, default **50** | `app/mcp.rs:5138` |
| `inspect_spans` | **summary mode** (`top_level`, no filter) | **none**; `child_count` is an **O(top_level × spans)** nested scan | `app/mcp.rs:5063-5089` |
| `get_generation_debug_trace` | `spans` | `max_spans`, default **100**; **`0` means unlimited** | `app/mcp.rs:1576,1586-1590` |
| `get_toolpath_diagnostics` | whole `Vec<Diagnostic>` | **none** | `app/mcp.rs:3048-3063` |
| `get_project_diagnostics` | whole `Vec<Diagnostic>` | **none** | `app/mcp.rs:3070-3076` |

Two further amplifiers, both cheap to remove:

- **Every response is pretty-printed.** `json_str` is
  `serde_json::to_string_pretty` (`crates/rs_cam_mcp/src/server.rs:630-632`),
  used by every handler. The compact/pretty ratio measured on the real payload
  is in §5.1.
- The response is built as a full `serde_json::Value` tree first and *then*
  stringified, so the peak holds tree + text + rmcp's escaped copy at once.
  At 60 MB that is survivable (§2.2); it is the reason a cap should be applied
  **while building**, not by truncating the finished string.

### 5.3 `truncated` vocabulary exists — on two arrays out of seven

`get_cut_trace` already ships `hotspots_truncated` / `hotspots_total_matching`
/ `hotspots_returned` and the `issues_*` triple (R-1, computed at
`app/mcp.rs:1437-1438`, emitted at `:1512-1521`). The **five** uncapped arrays
(`span_summaries`, `semantic_summaries`, `toolpath_summaries`,
`drill_summaries`, `drill_samples`) carry **no such keys**, so an agent cannot
tell a complete `span_summaries` from a filtered one. Whatever
Checkpoint L rules, the existing three-key vocabulary is the shape to extend —
it is already documented and already consumed.

---

## 6. `toolpath_id` — filters by **id**, documented as **index**, and wrong
   answers are silent

The parameter doc an agent reads in `tools/list`:

```rust
// crates/rs_cam_mcp/src/server.rs:297-299
pub struct CutTraceParam {
    /// Optional: filter results to a single toolpath by index
    pub toolpath_id: Option<usize>,
```

What the handler does with it:

```rust
// crates/rs_cam_viz/src/app/mcp.rs:1380
let toolpath_id = toolpath_id.map(rs_cam_core::ToolpathId);
```

`ToolpathId` is the project-level **raw id**, matched against
`SimulationCutSample.toolpath_id`. Every other per-toolpath MCP read
(`narrate_toolpath`, `inspect_spans`, `get_toolpath_diagnostics`,
`get_generation_debug_trace`) takes an **index**. This one parameter is the
odd one out and its own doc string says the wrong thing.

Measured index→id map for this fixture:

| index | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---|---|---|---|---|---|---|---|---|
| **id** | 14 | 4 | 7 | 12 | 5 | 6 | 10 | 11 | 15 |

Evidence, from the census (`get_cut_trace` response bytes):

| `toolpath_id=` | bytes | what the agent got |
|---|---|---|
| `0` | 3,972 | **empty skeleton** — no toolpath has id 0. Every array length 0. |
| `8` | 3,972 | empty skeleton — no toolpath has id 8, though index 8 exists |
| `14` | 4,435 | Pin Drill = **index 0** |
| `15` | **51,595,653** | Unified Finish = **index 8** |
| `4` | 842,642 | Back Rough = **index 1** — but `4` is *also* a valid index (Rivers (back), id 5) |
| `5` | 2,122,146 | Rivers (back) = **index 4** — and `5` is *also* a valid index (Lakes, id 6) |

The ledger recorded the benign half ("index 0 returns empty, id 14 answers").
The census shows the sharp half: for ids 4, 5, 6 the value is simultaneously a
valid index *and* a valid id **pointing at a different toolpath**, so an agent
following the documentation gets **another toolpath's data with no error and
no warning**. The empty skeleton is also indistinguishable from "this toolpath
genuinely produced no samples" — `issue_count_project_wide: 73326` sits right
beside `issue_count: 0`, which reads as a real measurement.

B-2 owns the fix. The evidence says: **keep the id semantics** (the trace is
keyed by id; drill summaries and samples are keyed by id; renaming behaviour
would break the one thing that works), **fix the doc**, and **refuse an
unmatched value** instead of returning an empty skeleton — an id that matches
no toolpath is a caller error, not an empty result.

---

## 7. Checkpoint L package — bounded reads

Not self-approved. The orchestrator runs the checkpoint; the questions are
in §7.4.

### 7.1 What the numbers say the design must do

1. The problem is **`span_summaries`**, not the caps that already exist.
   It is ≥ 94 % of the payload (§5.1) and every other key together is
   411,847 bytes. Raising `max_issues` to 100,000 and `max_hotspots` to
   10,000 on a filtered call changed the response by **zero bytes**
   (census rows 59-61 vs 45-47) — the existing caps bind nothing.
2. The problem is **one toolpath**. `toolpath_id: 15` (Unified Finish) alone
   is 51,595,653 of the unfiltered call's 56,225,225 bytes (**92 %**), and
   33,195 of the 35,838 span summaries. A per-toolpath filter is therefore
   *not* a sufficient bound: the largest single filtered response is already
   51.6 MB.
3. It is **not** a time problem. 1.71 s on the frame loop is bad manners, not
   an outage, and C25's re-open condition (a *timed* read exceeding the gate)
   is still not met by this call. The bound should be sized in **items and
   bytes**, and C25's "work units" framing carried for the truncation
   *reporting*, not used as the primary limit.
4. **23.8 % of the payload is whitespace**, removable with no wire change at
   all (56,225,225 served vs 42,833,152 compact, §5.1). On smaller responses
   the share is larger — 46.6 % on `get_diagnostics`. That is a separate,
   smaller decision the
   operator may want to take independently.

### 7.2 Options

**Option 1 — per-array caps + the existing `truncated` vocabulary. No token.**
Give `span_summaries`, `semantic_summaries`, `toolpath_summaries`,
`drill_summaries` and `drill_samples` the same three keys `hotspots` and
`issues` already have (`*_truncated`, `*_total_matching`, `*_returned`) plus a
`max_*` parameter each. Ordering must become deterministic and documented so a
cap is reproducible. Wire-additive; no snapshot revision; no new concepts for
an agent to learn.
*Cost:* an agent that legitimately wants all 32k spans of the Unified Finish
must narrow by `span_kind`/`pass_index` — which already works
(`span_kind: depth_pass` across the whole project returned 459 kB).

**Option 2 — Option 1 plus a global response-byte backstop.** Same as above,
plus a hard `MAX_RESPONSE_BYTES` checked *while building*; on overflow the
response is completed with the sections that fit and names the ones that did
not under `sections_not_computed`, with `complete: false`. Guarantees a bound
even if a future array is added uncapped — which is exactly how
`span_summaries` got here.
*Cost:* one more concept; needs care that "did not fit" is never rendered as
a zero (C25's standing rule).

**Option 3 — C25 in full: work-unit budget + continuation token pinned to a
snapshot `revision`.** Adds `complete`, `sections_complete`,
`sections_not_computed`, `work_units_budget`, `work_units_consumed`,
`continuation_token`; a continuation against a newer revision refuses.
*Cost:* the largest item; needs the `McpStateSnapshot` revision C25 never
built, and this census shows nothing that *needs* pagination — the natural
unit an agent wants is "one toolpath" or "one span kind", and both are already
filters.

### 7.3 Recommendation

**Option 2**, with the continuation token explicitly deferred.

Rationale from the measurements: pagination solves a problem the census did
not find. Every legitimate read in this census is under 2.2 MB once a
toolpath filter is applied; the single outlier is one operation's 33,195-entry
`span_summaries`, and an agent asking for that array in full is asking for
something it cannot use in one response anyway. Caps + honest truncation keys
give it a correct, bounded answer and a documented way to narrow. The global
byte backstop is what makes the guarantee hold for the *next* uncapped array
rather than only for the five known ones.

Suggested defaults, justified by the census rather than round numbers:

| array | default cap | why |
|---|---|---|
| `span_summaries` | **200** | keeps a whole-project response in the same order as the largest *bounded* read today (`get_generation_debug_trace` at its 100-span default, 32.6 kB); 200 spans is more than a human or an agent reads in one pass |
| `semantic_summaries` | **200** | same shape; the array is already sorted by `wasted_runtime_s` descending (`simulation_cut.rs:944-948`) so the first 200 are the interesting ones — this ordering is already deterministic and should be documented as part of the contract |
| `toolpath_summaries` | **uncapped** | one per toolpath *with samples*; **5** rows here out of 9 toolpaths, 3,289 bytes total |
| `drill_summaries` | **uncapped** | one per drill toolpath; **2** rows here, 859 bytes |
| `drill_samples` | **500** | already opt-in and already documented "can be verbose" |
| `MAX_RESPONSE_BYTES` | **8 MiB** | **3.95×** the largest response any *useful* filtered call produced in this census (2,122,146 B) and **106×** the largest non-`get_cut_trace` response (79,135 B), while sitting **7.2×** below the 60.5 MB frame that a real client did not survive |

`inspect_spans` summary mode also lacks a cap, and its `child_count` is a
nested scan of all spans per top-level span. **Measured, it is not a problem
today**: on the 33,195-span Unified Finish it returned 685 bytes in ~1 s (the
frame-loop cadence, not compute). It is listed as latent, not observed —
`top_level` only contains `Operation` and `DepthPass` spans, of which this
fixture has few. Recommend B-2 give it the same cap in the same commit for
uniformity, and *not* claim it fixed a measured cost.

### 7.4 Questions for the operator

- **L-1.** The crash cause is **not** named. Serialization-OOM and
  transport-close are both falsified with measurements (§2, §3); the leading
  remaining candidate is client-side reap (§4a) and the Wayland variable could
  not be exercised (§4b). Is B-2 authorised to proceed on "bound the read
  regardless of which of the two delivered the kill", with **G-LV.2 staying
  open** as *cause-not-attributed* rather than closing?
- **L-2.** Option 1, 2 or 3 (§7.2)? The recommendation is **2**, deferring the
  continuation token as unmotivated by the census.
- **L-3.** Approve the default caps in §7.3 — in particular
  `span_summaries: 200` and `MAX_RESPONSE_BYTES: 8 MiB`.
- **L-4.** `json_str` pretty-prints **every MCP response**
  (`rs_cam_mcp/src/server.rs:630-632`). The measured saving from compact
  output is in §5.1. Switch to compact, keep pretty, or make it a per-call
  parameter? This changes the bytes on every one of the 68 tools, so it is a
  separate ruling from L-2/L-3 even though it is a one-line change.
- **L-5.** `toolpath_id` (§6): confirm **keep id semantics + fix the doc +
  refuse an unmatched id** rather than switching the parameter to index.
  An index switch would be the smaller doc change and the larger behaviour
  change, and would silently alter every existing agent transcript.
- **L-6.** TD3 §0 rule 10 prescribes `WINIT_UNIX_BACKEND=x11`, which is inert
  on winit 0.30 (§1.1). Correct the rule to "unset `WAYLAND_DISPLAY`"?

---

## 8. Not exercised, with reasons

- **The crash itself.** Not reproduced. §4.
- **Wayland backend.** `run2` aborted at `load_project` after 90 s — G-LV.1's
  parked frame loop. §4b.
- **Core dump / kernel log attribution.** No core was needed (nothing
  faulted). Had one been needed the capture would have been degraded:
  `coredumpctl` absent, `dmesg` restricted, release binary stripped. §1.
- **Claude Code's client-side limits.** Out of scope for this wave and
  forbidden by the standing instruction not to use the live MCP session.
- **`get_generation_debug_trace` with `max_spans: 0`** (its documented
  "unlimited" mode) was not measured; the census used the default 100.
- **A response larger than 60.5 MB.** No attempt was made to find a
  server-side size cliff above the observed payload (e.g. unfiltered plus
  `include_drill_samples` plus raised issue/hotspot caps, ≈110 MB). The
  finding is "60.5 MB is served without fault", not "there is no cliff".
