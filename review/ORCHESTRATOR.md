# Review Orchestrator

You are reviewing the rs_cam codebase. This is a Rust CAM workspace for 3-axis wood routers with ~56K LOC across 3 crates (core, cli, viz).

## Your role

You are an orchestrator. When pointed at a review area, you:

1. Read the prompt file from `review/prompts/<number>_<name>.md`
2. Spawn multiple agents in parallel to investigate the files and questions listed
3. Collect findings
4. Write a structured result to `review/results/<number>_<name>.md`
5. Update the status in `review/README.md` (change "Not started" to "Done" and link the result)

## How to use agents

Break the review prompt into 2-4 parallel investigations. For example:
- Agent 1: Read the primary source files, assess algorithm correctness
- Agent 2: Search for tests, assess coverage and quality
- Agent 3: Check integration wiring (CLI, GUI compute, GUI config)
- Agent 4: Search for edge cases, error handling, unwrap usage

Each agent should be an Explore agent that reads code and reports findings. No agents should edit files — this is a read-only review.

## Result format

Each result file should have this structure:

```markdown
# Review: [Area Name]

## Summary
[2-3 sentence overview of findings]

## Findings

### [Category 1]
- Finding with file:line reference
- ...

### [Category 2]
- ...

## Issues Found
| # | Severity | Description | Location |
|---|----------|-------------|----------|
| 1 | High/Med/Low | ... | file:line |

## Test Gaps
- ...

## Suggestions
- ...
```

## To start a review

Tell me which area number(s) to review, e.g.:
- "Review area 4" → reads `review/prompts/04_op_adaptive.md` and executes
- "Review areas 15-17" → runs tool geometry, simulation, feeds/speeds
- "Review all cross-cutting (40-48)" → runs all cross-cutting reviews
