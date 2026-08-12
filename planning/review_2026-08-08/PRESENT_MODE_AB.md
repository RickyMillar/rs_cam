# PresentMode A/B under the park rig — TD3 wave N-2

**Executes Checkpoint N-2 (BINDING, 2026-08-13): a measurement, not a flip.**
Nothing in this wave changes a default. The one production edit is a rig-only
environment lever whose unset value is byte-identical to today's behaviour.

- Measured revision: `tech-debt-3 @ 5c4e847c`, **debug** binary (release
  forbidden in-wave), built in an isolated worktree so a parallel wave editing
  `rs_cam_core` in the shared tree could not move the binary under the
  measurement. §7 records why.
- Machine: AMD Radeon 890M (RADV GFX1150), Mesa 25.2.8, kernel 6.17.0-1028-oem,
  GNOME Wayland, 60 Hz display. wgpu 29.0.3 / eframe 0.34.3 / winit 0.30.13.
- Rig: `artifacts/n2/present_mode_ab.py`, which **extends** B-4's rather than
  replacing it — B-1's `mcp_stdio_client` transport, B-4's
  `RS_CAM_MINIMIZE_AFTER_FRAMES` park lever, and B-4's cheap / frame-door call
  lists imported from `floor_census.py` so both waves measure one population.
- Raw rows: `artifacts/n2/measurements/*/{result.json,syscall.jsonl,gui_stderr.log}`,
  aggregate in `measurements/SUMMARY.txt`.
- Executor: N-2 agent, over its **own** GUI instances on a scratch **copy** of a
  project file. The operator's live GUI was never touched;
  `planning/airrun_2026-06-01/wanaka.toml` was read once to make the copy and is
  unchanged.

---

## 0. The answer in four lines

1. **AutoVsync (= FIFO) parks the process; Mailbox does not park it at all.**
   Same lever, same code path, same minimise: FIFO → main thread in
   `poll(1 fd, ∞)` in **405/405** samples, frame-door MCP calls time out at 15 s.
   Mailbox → `epoll_wait` in **355/355**, every call live in **1.6–1.9 ms**, and
   the minimised window **keeps painting**.
2. **Both bars B-4 could not reach are met under Mailbox.** `generate_all`
   fixpoint on a minimised window: `ok: true, generated: 2, rounds: 2,
   simulations: 1, errors: []` in **4.4 s**. `screenshot_gui` on a minimised
   window: a real 1400×900 GUI render, **644,918 bytes in 3.79 s**. The same two
   calls under AutoVsync: 90 s wait budget exhausted with zero frames and zero
   pumps, and a 60 s timeout with 0 bytes.
3. **`Immediate` is not available here and asking for it is a hard crash at
   startup**, not a fallback. The surface supports exactly `[Mailbox, Fifo]` —
   enumerated verbatim by wgpu's own validation error.
4. **The interactive cost is real and bounded to active repaint.** Idle
   (heartbeat-driven, ~12 fps): 5.20% → 7.03% of one core. Continuous repaint:
   AutoVsync 59.9 fps / 18.27% of a core against Mailbox 143.8 fps / **81.40%**
   — 4.5× the CPU for frames a 60 Hz display discards. Tearing is **not
   measurable headlessly** and is not claimed either way (§5).

---

## 1. Negotiated modes — requested is not negotiated

wgpu resolves the two `Auto*` rules against the surface's capabilities and logs
its choice; an **explicit** mode the surface does not support is a hard
`UnsupportedPresentMode` **error**, not a silent fallback
(`wgpu-core-29.0.3/src/device/resource.rs:4963-5001`). So the negotiation record
is: a clean launch on an explicit mode *is* the mode in force, and an `Auto*`
mode is read off the caps.

**The surface's supported present modes, quoted from the process that died
asking for one it did not have** (`measurements/park_wayland_immediate/gui_stderr.log:12`):

```
In Surface::configure
  Requested present mode Immediate is not in the list of supported present modes: [Mailbox, Fifo]
```

| requested | fallback rule | **negotiated here** | how known |
|---|---|---|---|
| `AutoVsync` (today's default) | FifoRelaxed → Fifo | **Fifo** | FifoRelaxed absent from caps; and explicit `fifo` reproduces AutoVsync's park behaviour exactly (§2) |
| `AutoNoVsync` | Immediate → Mailbox → Fifo | **Mailbox** | Immediate absent from caps; behaviour identical to the explicit `mailbox` arm |
| `Mailbox` | — (explicit) | **Mailbox** | launched clean; an unsupported explicit mode cannot launch |
| `Immediate` | — (explicit) | **none — process died** | panic in `Surface::configure`, quoted above |
| `Fifo` | — (explicit) | **Fifo** | launched clean |

**This confirms B-4's attribution with a measurement rather than a source read:**
`AutoVsync` is FIFO on this surface, and the explicit-`fifo` arm reproduces the
park with the same syscall signature (135/135 samples), so the park travels with
FIFO and not with the word "Auto".

---

## 2. The park A/B

Method per arm: launch one isolated `rs_cam_gui --mcp` with
`RS_CAM_PRESENT_MODE=<mode>` as the only variable; minimise via B-4's
`RS_CAM_MINIMIZE_AFTER_FRAMES`; **prove** the park state per measurement (frame
counter static across reads separated by more than `PARKED_FRAME_LOOP`, plus
`healthy`), never infer it; then census 6 cheap calls + **2 pure frame-door
calls that have no snapshot fallback and therefore hang rather than degrade**;
sample `/proc/<pid>/syscall` of the main thread throughout at 4 Hz.

| arm | park proof | main-thread syscall while parked | off-loop door (`generation_status`) | snapshot-capable reads | **pure frame-door reads** | `frames` | `pumps` | `wakeups` |
|---|---|---|---|---|---|---|---|---|
| **Wayland / AutoVsync** (control) | **PARKED** `healthy:false` | `7 … 0x1 0xffffffff` = `poll(1 fd, ∞)` **405/405** | 0.0004 s live | 0.752 s **snapshot** ×15 | **15.0 s TIMEOUT ×6** | 263 **static** | 276 **static** | 0→21 **climbing** |
| **Wayland / Fifo** (explicit) | **PARKED** `healthy:false` | same, **135/135** | 0.0005 s live | 0.753 s **snapshot** ×5 | **15.0 s TIMEOUT ×2** | 263 **static** | 273 **static** | 0→7 climbing |
| **Wayland / Mailbox** | **NO PARK** `healthy:true` | `281 … 0x400 …` = `epoll_wait` **355/355** | 0.0003 s live | **0.0016 s live** ×15 | **0.0018 s live** ×6 | 1371→1391 | 3595→3615 | 0→21 |
| **Wayland / AutoNoVsync** (→ Mailbox) | **NO PARK** `healthy:true` | `epoll_wait` **184/184** | 0.0004 s live | **0.0017 s live** ×15 | **0.0018 s live** ×6 | 836→857 | 1995→2016 | 0→21 |
| **Wayland / Immediate** | — | — | — | — | — | **process panicked before the first frame** | | |
| **X11/XWayland / AutoVsync** (control) | **PARKED** `healthy:false` | `epoll_wait` / `futex` — the 1-fd infinite poll appears in **0 of 36** samples | 0.0002 s live | **0.0002 s live ×150** | **0.0002 s live ×60** | 252 **static** | 255→466 | 0→210 |

Three things this table says that are worth saying in words.

- **`wakeups` climbing while `pumps` is static is the whole diagnosis, and it
  only appears in the FIFO rows.** B-4 added those two counters precisely to
  separate "the ping was never issued" from "the ping was issued and ignored".
  Under FIFO the ping is issued 21 times and answered 0 times. Under Mailbox
  `pumps` runs *ahead* of `frames` (3615 vs 1391) — dispatch is off the paint
  path and both halves are working.
- **The snapshot fallback is what makes FIFO look survivable and is not a
  live read.** Every one of those 0.752 s answers is `served_from: "snapshot"` —
  the frame loop lost its 750 ms race and the caller got the last thing the GUI
  said, not the current state. The two calls with no fallback tell the truth:
  15 s, timeout, six times out of six.
- **X11 is not exempt from the park; it is exempt from the *strand*.** A
  minimised X11 window **also** stops painting — `frames` static at 252,
  `healthy:false`, which is an honest report — but the main thread returns to the
  event loop, so all **240** calls including both frame-door calls answered live
  at 0.2 ms and `pumps` climbed 255→466 in lock-step with `wakeups`. This is B-4's
  steps 1–4 doing exactly their job. It also means the repo's standing sentence
  — "X11 has no such gate: its redraws are client-driven"
  (`crates/rs_cam_viz/src/bin/main.rs:29`, echoed in the runtime warning at `:60`) — is **imprecise**: X11 does stop
  painting when minimised; what it does not do is strand dispatch. Reported, not
  edited: production text is a decision for whichever wave acts on this package.

### 2.1 The two bars B-4 could not reach

| bar | Wayland / AutoVsync (parked) | Wayland / Mailbox (minimised) |
|---|---|---|
| **step 5 — `generate_all` fixpoint round-trip on a non-painting window** | **90.0 s wait budget exhausted.** Reply is the escape hatch's `status: "running"`, "continues in the background" — while `frames` and `pumps` sat **static at 1525 / 1524** and the syscall sampler read `poll(1 fd, ∞)` in **360/360** samples across the whole 90 s. Nothing ran. | **`ok: true, generated: 2, rounds: 2, simulations: 1, errors: [], awaiting_prior_stock: []` in 4.4 s.** A two-round fixpoint *including the simulate-round handoff* — the exact handoff G-LV.1 stranded. |
| **Lane A — `screenshot_gui`** | **60 s timeout, 0 bytes on disk.** | **644,918-byte PNG in 3.79 s**, 1400×900, 3,108 distinct colours — a full GUI render with terrain, toolpath overlay and a live status bar. Saved at `artifacts/n2/evidence/screenshot_gui_minimised_mailbox.png`. |

The step-5 control deserves one more sentence, because it is a reporting defect
in its own right: on a parked loop `generate_all` returns a cheerful,
**literally true and thoroughly misleading** "Generation was NOT cancelled and
continues in the background" for work that was never dispatched at all. That is
the 2026-08-07 incident's exact signature and it is unchanged at HEAD.

---

## 3. Mechanism — what this refines in B-3's design and B-4's finding

B-4 attributed the block to the Wayland/Mesa WSI waiting on a FIFO buffer
release a minimised surface never gives. **Everything here is consistent with
that and adds one fact B-4 could not have had**: with FIFO removed, a minimised
surface *keeps producing frames* (1371→1391 during one census; 10,499→10,505
during another). So on this machine the design's §3.1 model — winit withholding
`RedrawRequested` while a compositor frame callback is pending — **did not
independently park the loop in either non-FIFO arm.**

I am deliberately **not** resolving which of two mechanisms explains that:
either mutter keeps delivering frame callbacks to a minimised surface (so the
gate never closes), or Mesa's Mailbox path never leaves winit's frame-callback
state in `Requested`. Distinguishing them needs a Wayland protocol trace, which
this wave did not take. What the wave *does* establish is the operative fact:
**on this compositor, with FIFO gone, both the paint path and the dispatch path
survive a minimise.**

---

## 4. The interactive cost

Two workloads, because they cost very differently and only one of them is what
an idle agent session looks like.

- **Idle** = no MCP traffic in the window at all; the only repaint driver is the
  100 ms MCP heartbeat (which M-5 keeps while steps 5–7 are unbuilt).
- **Continuous repaint** = `RS_CAM_MINIMIZE_AFTER_FRAMES` set beyond the window,
  which requests a repaint every frame (`app.rs:942-952`). It is the closest
  headless proxy available for an interactive drag, and it is a **worst case**,
  not a typical case.

CPU is `utime+stime` from `/proc/<pid>/stat` over a 30 s window, as a percentage
of one core.

| | AutoVsync (Fifo) | Mailbox | ratio |
|---|---|---|---|
| idle CPU (30 s, no traffic) | **5.20%** of a core | **7.03%** | 1.35× |
| idle frame rate | 11.9 /s | 11.8 /s | — |
| continuous-repaint CPU (30 s) | **18.27%** of a core | **81.40%** | **4.46×** |
| continuous-repaint frame rate | **59.9 /s** (vsync-locked) | **143.8 /s** | 2.40× |

Frame pacing, sampled off the off-loop door and reconstructed from
`frame_loop.last_frame_age_s` so the instrument never itself asks for a frame
(20 s, continuous repaint):

| | frames observed | p50 interval | p95 | max |
|---|---|---|---|---|
| AutoVsync (Fifo) | 1199 (60.0 /s) | **16.97 ms** | 18.01 ms | 34.01 ms (one dropped frame) |
| Mailbox | 8312 (415.6 /s) | **2.01 ms** | 4.00 ms | 21.65 ms |

Read carefully: AutoVsync's distribution is a textbook 60 Hz lock — p50 within
0.3 ms of the refresh interval, p95 within 1 ms of it. Mailbox is not "smoother";
it is **unthrottled**, producing ~7 frames per refresh and discarding six.
That is where the 4.5× CPU goes.

**Caveats on these numbers, all stated rather than discovered.** (a) **Debug
binary** — release was forbidden in-wave, so the absolute percentages are debug
figures and only the *ratios* transfer. (b) The pacing run's own CPU reading
(149% vs 219% of a core) includes the instrument's polling load and is **not**
the cost comparison; the idle-phase table is. (c) One machine, one GPU, one
compositor, one 60 Hz display.

---

## 5. What I could not measure, honestly

- **Tearing. Not measured, not claimable.** Tearing is a scanout artefact: it
  exists between the GPU's scanout and the panel, and every capture path this
  repo has — `screenshot_gui`'s framebuffer read, the CPU rasterisers behind
  `screenshot_simulation`/`screenshot_toolpath` — reads a *completed frame
  buffer*, which by construction cannot contain a tear. No headless method
  available to me can answer it. On this surface the non-FIFO option is
  **Mailbox**, which is the queued, non-tearing non-vsync mode (Immediate, the
  tearing one, is unsupported here and crashes on request), so the theoretical
  expectation is *no tearing on this machine* — but that is an expectation from
  the mode's definition, not a measurement, and a different GPU that supports
  Immediate would negotiate differently under `AutoNoVsync`. **This needs the
  operator's eyes**, ideally during the N-3 session.
- **Perceived smoothness / input latency.** Same reason. Mailbox's lower latency
  is the usual argument for it and I have not tested it.
- **One compositor, one park state.** Everything here is GNOME/mutter Wayland
  with a **minimised** window. The 2026-08-07 incident and the ~9.5 h A-1/A-2
  park were *occluded or merely unfocused, and one was explicitly "visible"*.
  Whether those share the FIFO mechanism is **still** N-3's question, and this
  wave does not answer it. If they do not, a flip may fix a reproduction and not
  the incident.
- **Compositor-side minimised state is not independently verifiable.** Wayland
  gives the client no signal (winit returns `None` from `is_minimized`), and
  GNOME's Eval interface is closed — B-4 had the same gap. What I can say is
  that the identical `ViewportCommand::Minimized(true)`, from the identical code
  path, produced an immediate hard block in **4/4** FIFO-family runs
  (`auto_vsync` ×3, explicit `fifo` ×1) and no block in **4/4** non-FIFO runs
  (`mailbox` ×3, `auto_no_vsync` ×1). Treating that as the minimise silently failing in
  exactly the non-FIFO runs requires a coincidence I do not believe, but it is
  an inference and it is labelled as one.
- **Other surfaces.** `[Mailbox, Fifo]` is *this* surface's capability list. A
  surface without Mailbox exists and matters to the option space (§6).
- **Incidental, unclaimed:** the parked-window screenshot shows a toolpath
  marked `OFF` in the tree ("Back Rough") reported as generating in the status
  bar. Noticed, not investigated, not this wave's territory.

---

## 6. The operator question

**A flip is not mine to make. Three options, from B-4 §9's remaining space.**

**Option 1 — flip the default for all sessions.** *Which mode* is then a second
question with a sharp edge: an explicit `Mailbox` **panics at startup on any
surface that lacks it**, exactly as `Immediate` did here, so an unguarded
explicit flip trades a park for a crash on unknown hardware. `AutoNoVsync` is
safe by construction (it always ends at Fifo) but that safety is the problem:
on a Fifo-only surface it **silently restores the hazard**, and nothing today
tells anyone which mode was negotiated. Cost: the §4 interactive numbers on
every session, including every human one.

**Option 2 — flip only under `--mcp` (my recommendation).** The park hazard
lives entirely in agent sessions; so does the tolerance for unthrottled
rendering, because nobody is watching those pixels for tearing or pacing.
Human-only launches keep today's exact 60 Hz lock and 18% ceiling. Request
`AutoNoVsync` rather than `Mailbox` so an unsupported-mode launch cannot crash,
**and make the negotiated mode observable** — log it, and surface it beside
`frame_loop` in `generation_status`, so "we silently got Fifo back" can never be
a silent state. The honest cost of this option is that the operator sometimes
drives the GUI **by hand while MCP is attached** — that session pays the 81%
continuous-repaint figure, which on a laptop is a battery and thermal question
rather than a correctness one.

**Option 3 — keep AutoVsync and route around it.** Still viable and still
cheaper in risk: the X11 remedy measurably works (§2, 240/240 live at 0.2 ms),
and steps 5–7 would make a parked loop *honest* rather than *working*. This
option's price is that `screenshot_gui` and the Lane A obligations stay
unreachable under Wayland, and the "continues in the background" reply above
stays a lie the architecture cannot fix.

**Secondary question, whichever option wins:** should an unsupported explicit
present mode be a **startup crash**? It is one today, from a `.expect`-shaped
path inside wgpu that this workspace's lint policy would never have permitted in
its own code. If a flip lands, that path wants a guard.

### What an approved flip would unblock

- **B-4 steps 5–7**, whose blocker was the step-4 gate this wave clears:
  step 5's own bar (`generate_all` on a hidden window) is **measured met** under
  Mailbox at 4.4 s; step 6 (six frame-coupled drivers + heartbeat deletion,
  M-5's condition); step 7 — which **changes shape**: `screenshot_gui`'s designed
  *refusal* is no longer the only option on a window that still paints, and
  `visible_on_next_frame` becomes answerable instead of pessimistic.
- **The two Lane A capture obligations** — the live before-fix heat-map
  capture and the live magnitude measurement on wanaka, both raised by A-1 as
  NOT EXERCISED and booked to A-2 — are blocked on a GUI capture that a parked loop cannot serve. Measured:
  `screenshot_gui` served a full render from a minimised window in 3.79 s. **The
  dependency to state plainly:** those two waves were blocked by a *visible or
  occluded* park, and this wave only proves the mechanism for a *minimised* one.
  If N-3 shows the incident's states share it, the obligations are unblocked; if
  not, they are not.
- **G-LV.1 itself** would move from "open, mechanism attributed on one
  reproduction" to "mitigated on the reproduction, with a named residual" —
  never to closed on this evidence alone.

---

## 7. Verification and discipline

- **`rs_cam_viz` compiles clean at HEAD `5c4e847c`.** Built pristine (no local
  edits) in an isolated worktree: `Finished dev profile in 52.70s`, zero errors.
  The editor diagnostics reporting `host::MCP_WAKE_PASS_NR` missing at
  `lib.rs:97` and a missing `wakeups` field at `mcp_bridge.rs:163` were
  **stale**; both symbols resolve.
- **Why a worktree.** The shared tree's `rs_cam_core` was mid-edit by the
  parallel A-5i wave (`feeds/predict.rs` had `predict_observed_chipload_mm`,
  `ObservedChiploadPrediction` and `ArcFitRatioSource` deleted while
  `feeds/suggest.rs` still called them — 4 errors, none in my territory, all in
  files I did not touch). A multi-hour measurement needs a binary that cannot
  move under it, so the binary was built from a detached worktree at `5c4e847c`
  with its own target directory. The A/B is therefore on HEAD's code plus one
  patch — the lever — and nothing else.
- **The lever.** `RS_CAM_PRESENT_MODE`, read once in `rs_cam_viz::run`. Unset —
  every real session — it returns `AutoVsync`, the exact value
  `WgpuConfiguration::default()` carries, so a build with the lever and a build
  without it configure the same surface. No default moved.
- `cargo clippy -p rs_cam_viz --all-targets -- -D warnings` — clean, zero
  diagnostics. `cargo test -p rs_cam_viz -q` — **244 lib + 14 + 14 + 11, 0
  failed**, the counts B-4 left.
- **`cargo fmt --check --all` is red at HEAD and not because of this wave.**
  Two diffs, both in `rs_cam_viz` files B-4 last touched and in lines this wave
  did not write (`src/lib.rs`'s `event_loop` binding; `tests/mcp_escape_hatches.rs:495`).
  Confirmed pre-existing by running the same command on a **pristine** worktree
  at `5c4e847c`. Left alone: they are another wave's lines and `rustfmt`
  cascades into siblings. Recorded so the next `/verify` does not read it as new.
- ONE Cargo job at a time (`pgrep -af "carg[o] "` + `free -g` before each of the
  two launches; both were clean). **No release build.** Disk 115 G free at start
  and end. No core test suite was run: this wave touched no core code and its
  own crate's suite belongs to the commit, not to the measurement.
- The operator's live GUI was never driven. `planning/airrun_2026-06-01/wanaka.toml`
  was read once to make a scratch copy and is unmodified; the trimmed two-op
  cascade used for the fixpoint bar lives in session scratch, not in the repo.
