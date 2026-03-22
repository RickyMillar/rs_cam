# Remediation Orchestration Plan

How I (the orchestrator) will manage agents to execute the 86 tasks in `REMEDIATION_TRACKER.md`.

---

## Role Definition

**I am the orchestrator. I do NOT write code.** My job:
1. Spawn agents with precise, self-contained prompts
2. Verify every agent's work (double/triple check)
3. Merge worktree branches after verification
4. Update `REMEDIATION_TRACKER.md` as tasks complete
5. Maintain quality as context grows

---

## Hard Rules (follow even when context is huge)

### R1: Never write code directly
All code changes go through agents. I only read, verify, and manage.

### R2: One task = one verification cycle
After every agent completes, I MUST:
1. Read the changed files to confirm correctness
2. Run `cargo test --workspace` in the worktree
3. Run `cargo clippy --workspace --all-targets -- -D warnings`
4. Check that ONLY the intended files changed (`git diff --stat`)
5. If any check fails, spawn a fix-up agent (never fix it myself)

### R3: Update tracker immediately
After verifying a task, update `REMEDIATION_TRACKER.md` checkbox BEFORE moving to next task. Never batch tracker updates.

### R4: Agent prompts must be self-contained
Every agent prompt MUST include:
- The exact FINDINGS.md issue ID (e.g., A2)
- The exact file paths to modify
- The exact bug description and fix
- The specific test to add or verify
- Instruction to run `cargo test` and `cargo clippy` before finishing
- Instruction to NOT touch any files beyond the listed ones

### R5: Parallel worktrees = independent file sets only
Never run two worktree agents that touch the same file. If in doubt, run serial.

### R6: Context management
- After completing each phase, summarize what was done and what's next
- Don't re-read FINDINGS.md or TRACKER.md unless needed — I have the structure memorized
- Keep agent result summaries to 2-3 lines — I verify by reading files, not trusting summaries

### R7: When merging worktrees
Always merge to master one worktree at a time. After each merge:
1. Run full test suite on master
2. If tests fail, fix on master before merging next worktree
3. Never force-merge or skip conflicts

---

## Blocker: Clippy Errors

Before any phase work, fix 6 clippy errors on master:
- `adaptive.rs:1023` — too many arguments (8/7)
- `debug_trace.rs:344,418` — too many arguments (11/7)
- `dexel_stock.rs:663` — too many arguments (13/7)

**Approach**: Single agent, refactor to use struct params or builder pattern. Must pass `cargo clippy -- -D warnings` after.

---

## Phase 1 Execution Plan

### Worktree Strategy (3 parallel streams)

**Stream 1: execute.rs focus** (serial — 4 tasks touch this file)
```
Worktree: worktree-phase1-execute
Tasks (in order):
  A2  — Remove .to_radians() at line ~570             (5m)
  A3  — Tabs only on final depth pass                  (1h)  + dressup.rs
  A4  — Wire FaceDirection OneWay mode                 (30m)
  A10 — Split inlay female/male output                 (1h)
  C14 — Add Scallop BallNose pre-validation            (15m)
```

**Stream 2: Core algorithm fixes** (mostly independent files)
```
Worktree: worktree-phase1-core
Tasks (in order where files overlap):
  A1  — CLI tool radius fix                            (15m) job.rs
  A5  — Inlay male region include holes                (30m) inlay.rs
  A6  — VCarve max_depth=0 semantics                   (15m) vcarve.rs
  A7  — VBit sqrt guard                                (5m)  vbit.rs
  A8  — TaperedBall sqrt guard                         (5m)  tapered_ball.rs
  A9  — Mesh bbox recompute after winding fix          (15m) mesh.rs
  C4  — Triangle index bounds validation               (30m) mesh.rs (after A9)
  C6  — Zero-guard on cell_size                        (30m) dexel.rs, simulation.rs
  C10 — Atomic project saves                           (30m) project.rs
```

**Stream 3: Worker & controller safety** (worker.rs + events.rs)
```
Worktree: worktree-phase1-safety
Tasks (serial within each file):
  C2  — catch_unwind() on worker threads               (1h)  worker.rs
  C3  — Replace .expect() with graceful recovery       (1h)  worker.rs (after C2)
  C5  — Tool-in-use check before deletion              (1h)  events.rs
  C16 — Replace unwrap_or(ToolId(0))                   (15m) events.rs (after C5)
```

### Agent Prompt Template (Phase 1)

```
You are fixing bug [ID] in the rs_cam codebase.

## What's wrong
[Exact bug description from FINDINGS.md]

## Files to modify
- [exact paths]

## What to do
[Specific fix instructions]

## Test requirements
- Add a regression test that would have caught this bug
- Run `cargo test --workspace` — all tests must pass
- Run `cargo clippy --workspace --all-targets -- -D warnings` — must pass

## Constraints
- Do NOT modify any files other than those listed above
- Do NOT refactor surrounding code
- Keep changes minimal and focused
```

### Verification Protocol (Phase 1)

For each completed stream:
1. Read every changed file, confirm fix matches FINDINGS.md description
2. Confirm regression test exists and tests the specific bug
3. Run tests in worktree
4. Run clippy in worktree
5. Check `git diff --stat` — no unexpected files
6. If all pass: merge to master, run tests on master, update tracker
7. If any fail: spawn fix-up agent with specific failure details

### Merge Order
1. Stream 2 (core fixes) — least conflict risk, independent files
2. Stream 3 (worker/controller) — independent from execute.rs
3. Stream 1 (execute.rs) — most changes, merge last

---

## Phase 2-6 Execution Approach

Same principles as Phase 1 but with adjusted parallelism:

### Phase 2 (Wire Existing Code)
- **Bottleneck**: properties/mod.rs (B2a-d, B7, B8, B9, G1a-c all touch it)
- **Strategy**: One worktree for properties/mod.rs work (serial). One worktree for gcode/export (B1). One for controller wiring (B3, B6).
- **Key risk**: Undo wiring (B2) touches both history.rs and properties — keep together

### Phase 3 (Performance)
- **Strategy**: Core parallelism (D4, D5) in one worktree. GPU rendering (D1, D2, D3) in another. Fully independent.
- **Verify**: Performance tasks need before/after benchmarks, not just test pass

### Phase 4 (Code Quality)
- **Strategy**: File splits are high-conflict. Run ONE worktree at a time.
- **Key risk**: File splits can break imports everywhere. Run full test suite after each split.
- **Order**: execute.rs split first (most value), then properties, then events, then timeline

### Phase 5 (Testing)
- **Strategy**: Maximum parallelism — test files don't conflict with each other
- **Can run 4-5 worktrees**: CLI tests, core tests, integration tests, proptest, fuzz targets
- **Verify**: New tests must actually test what they claim (read test logic, don't just check pass)

### Phase 6 (Documentation & Polish)
- **Strategy**: Doc fixes in one batch (no code conflicts). UI fixes serial (share UI files). G-code features independent.
- **3 worktrees**: docs, UI polish, G-code/import features

---

## Progress Checkpoints

After each phase completes, I will:
1. Update all checkboxes in REMEDIATION_TRACKER.md
2. Run full test suite on master
3. Run clippy on master
4. Post a phase summary: tasks done, issues found, any deferred items
5. Confirm readiness for next phase before proceeding

---

## Recovery Procedures

### Agent produces wrong fix
→ Do NOT try to patch. Spawn new agent with original prompt + "Previous attempt did X which was wrong because Y. Do Z instead."

### Merge conflict between worktrees
→ Merge the simpler/smaller worktree first. Then rebase the conflicting worktree on updated master. Spawn agent to resolve if complex.

### Test regression after merge
→ Stop all parallel work. Spawn diagnostic agent to identify which change broke what. Fix before resuming.

### Context getting too large
→ Compact. Re-read ORCHESTRATION_PLAN.md and REMEDIATION_TRACKER.md to restore state. The tracker checkboxes are the source of truth for what's done.
