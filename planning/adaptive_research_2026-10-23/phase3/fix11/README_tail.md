
## Files

| File | What |
|---|---|
| `audit.py` | the instrument (one run folder) |
| `run_all.py` | the 8 G2 columns; `audit_results.json`, `audit_output.txt` |
| `make_readme.py`, `README_head.md`, `README_tail.md` | this file |
| `assert_audit.py` | its own reader of the G-code and the samples; re-derives every number in this file |

## Limits

- No junction speed: the machine profiles give no acceleration or junction
  deviation. The reversal rule needs neither.
- The time split uses the sample time. Every sample of the op falls on a
  G-code move of the op, so the split's sum equals the sum of the op's
  sample times on all 8 columns.
- The audit reads the first rough op only.
