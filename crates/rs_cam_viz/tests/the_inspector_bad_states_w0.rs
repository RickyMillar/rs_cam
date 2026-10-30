//! W0: the toolpath inspector in bad states, measured before the tidy.
//!
//! # Why this file exists
//!
//! The IC1 sentry (`the_inspector_text_is_bounded_ic1`) renders one clean
//! fixture: a tool on each op, filled feeds, a clean selection and no reach
//! map. So it never renders an error state (finding F14 of
//! `planning/toolpath_panel_review_2026-10-30/FINDINGS.md`). The tidy
//! programme (W1-W6) moves the error text of the panel. This file records
//! the counts BEFORE that work, so W6 can turn them into asserts.
//!
//! # What it measures
//!
//! The test renders the real inspector (`properties::draw`) at the 280-point
//! panel width of IC1. It renders each tab of a Pocket (2D), a 3D Rough
//! (Adaptive3d) and a 3D Finish (Scallop) in each bad state. Each state has
//! its own fresh fixture. For each screen it reads every painted galley:
//!
//! - the full galley text (not the truncated face), its length, and whether
//!   the text was truncated (elided by the galley, or clipped);
//! - how many times each distinct message paints on the screen;
//! - the number of visible text rows.
//!
//! A second test counts the raw `ui.label(` and `ui.horizontal(` sites per
//! file under `src/ui/properties/`.
//!
//! The tests RECORD. They do not assert the W6 rules. They assert only that
//! each bad state paints its marker more times than the clean state, so a
//! baseline of the clean state cannot pass for a bad state (the F14 lesson).
//!
//! Run with `--nocapture` to print the tables. Set `W0_WRITE_BASELINE=1` to
//! write `planning/toolpath_panel_review_2026-10-30/W0_BASELINE.md`.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; the test prints its measurements for the reader"
)]

mod inspector_bad_states;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use inspector_bad_states::{Family, Painted, State, fixture, screen, visible_lines};
use rs_cam_viz::ui::properties::ToolpathTab;

/// A face over this many characters breaks the W6 rule (PLAN_EXECUTION §3).
const FACE_LIMIT: usize = 60;

/// Whitespace normalised: the key of the repeat count.
fn normalise(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The repeat key with a leading glyph token removed ("✕ msg" and "msg"
/// are one message to the operator).
fn message_key(text: &str) -> String {
    let normal = normalise(text);
    normal
        .trim_start_matches(|c: char| !c.is_alphanumeric() && c != '\'' && c != '"')
        .to_owned()
}

/// A text of three words or more is a message; a shorter text is a label,
/// a unit or a value, and its repeats are not counted as message repeats.
fn is_message(key: &str) -> bool {
    key.split_whitespace().count() >= 3
}

/// The measures of one screen.
struct Screen {
    family: Family,
    state: State,
    tab: ToolpathTab,
    painted: Vec<Painted>,
    visible: usize,
    /// Each message key and the number of times it paints.
    repeats: BTreeMap<String, usize>,
}

impl Screen {
    fn new(family: Family, state: State, tab: ToolpathTab, painted: Vec<Painted>) -> Self {
        let mut repeats = BTreeMap::new();
        for p in &painted {
            *repeats.entry(message_key(&p.text)).or_insert(0) += 1;
        }
        Self {
            family,
            state,
            tab,
            visible: visible_lines(&painted),
            painted,
            repeats,
        }
    }

    fn max_message_repeat(&self) -> usize {
        self.repeats
            .iter()
            .filter(|(key, _)| is_message(key))
            .map(|(_, n)| *n)
            .max()
            .unwrap_or(0)
    }

    fn truncated(&self) -> usize {
        self.painted.iter().filter(|p| p.truncated()).count()
    }

    fn longest_face(&self) -> usize {
        self.painted
            .iter()
            .map(|p| p.face.chars().count())
            .max()
            .unwrap_or(0)
    }

    fn longest_text(&self) -> usize {
        self.painted
            .iter()
            .map(|p| p.text.chars().count())
            .max()
            .unwrap_or(0)
    }
}

/// The key of an exception row: the family, the state and the text.
type RowKey = (Family, State, String);

/// A long text over the tabs of one family and state: the longest face, the
/// truncated flag and the tabs that paint it.
type LongText = (usize, bool, Vec<String>);

/// The text that proves a bad state painted, or `None` for the clean
/// reference.
fn marker(state: State) -> Option<&'static [&'static str]> {
    match state {
        State::Clean => None,
        // W2b: the header Tool row owns the no-tool state.
        State::NoTool => Some(&["Choose a tool"]),
        // The three empty feed fields paint "set a value". The clean state
        // paints it too, for its unset dressup values, but fewer times.
        State::FeedsUnset | State::FeedsUnsetRefused => Some(&["set a value"]),
        State::WrongTool => Some(&["needs a ball nose"]),
        // W2b: the status word of core `RunState::Failed`.
        State::Refused => Some(&["Failed"]),
        State::ReachOn => Some(&["Unreachable"]),
        // The boundary rows paint only while the boundary is on.
        State::BoundaryOn => Some(&["Containment:"]),
        // W2b: the status word of core `RunState::OutOfDate`.
        State::StaleAfterEdit => Some(&["Out of date"]),
    }
}

/// The number of galleys over every tab of one family and state that hold
/// a marker text.
fn marker_count(screens: &[Screen], family: Family, state: State, needles: &[&str]) -> usize {
    screens
        .iter()
        .filter(|s| s.family == family && s.state == state)
        .flat_map(|s| s.painted.iter())
        .filter(|p| needles.iter().any(|n| p.text.contains(n)))
        .count()
}

fn md_cell(text: &str) -> String {
    normalise(text).replace('|', "\\|")
}

fn render_all() -> Vec<Screen> {
    let mut screens = Vec::new();
    for state in State::ALL {
        for family in Family::ALL {
            if state.skip_reason(family).is_some() {
                continue;
            }
            let (mut app, id) = fixture(family, state);
            for tab in ToolpathTab::ALL {
                let painted = screen(&mut app, id, *tab);
                screens.push(Screen::new(family, state, *tab, painted));
            }
        }
    }
    screens
}

/// The inventory of every screen, as one string. The caller prints it with
/// one call, so the output of the other test does not split it.
fn inventory(screens: &[Screen]) -> String {
    let mut out = String::new();
    for s in screens {
        let _ = writeln!(
            out,
            "== {} / {} / {:?}: {} visible lines, {} galleys, {} truncated, max message repeat {}",
            s.family.name(),
            s.state.name(),
            s.tab,
            s.visible,
            s.painted.len(),
            s.truncated(),
            s.max_message_repeat()
        );
        for p in &s.painted {
            let key = message_key(&p.text);
            let _ = writeln!(
                out,
                "   [{} rows{}{} x{}] {} chars: {}",
                p.rows,
                if p.elided { " ELIDED" } else { "" },
                if p.overflow > 0.5 { " CLIPPED" } else { "" },
                s.repeats.get(&key).copied().unwrap_or(0),
                p.text.chars().count(),
                p.text.replace('\n', " / ")
            );
        }
    }
    out
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one test renders every state and writes the baseline tables in one place"
)]
fn the_inspector_bad_states_w0() {
    let screens = render_all();
    let inventory = inventory(&screens);

    // Non-vacuity: the renders painted the inspector.
    let total: usize = screens.iter().map(|s| s.visible).sum();
    assert!(total > 500, "the renders painted only {total} rows");

    // Preconditions: in each family it applies to, each bad state paints
    // its marker more times than the clean state does. A baseline of the
    // clean state cannot pass for a bad state (F14).
    let mut missing = Vec::new();
    for state in State::ALL {
        let Some(needles) = marker(state) else {
            continue;
        };
        for family in Family::ALL {
            if state.skip_reason(family).is_some() {
                continue;
            }
            let bad = marker_count(&screens, family, state, needles);
            let clean = marker_count(&screens, family, State::Clean, needles);
            if bad <= clean {
                missing.push(format!(
                    "{} / {}: {needles:?} paints {bad} times, the clean state {clean} times",
                    family.name(),
                    state.name()
                ));
            }
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));

    // The summary table: one row per screen.
    let mut summary = String::new();
    let _ = writeln!(
        summary,
        "| Family | State | Tab | Visible rows | Galleys | Truncated | Max repeat | Longest face | Longest text |"
    );
    let _ = writeln!(summary, "|---|---|---|---:|---:|---:|---:|---:|---:|");
    for s in &screens {
        let _ = writeln!(
            summary,
            "| {} | {} | {:?} | {} | {} | {} | {} | {} | {} |",
            s.family.name(),
            s.state.name(),
            s.tab,
            s.visible,
            s.painted.len(),
            s.truncated(),
            s.max_message_repeat(),
            s.longest_face(),
            s.longest_text()
        );
    }

    // Per state: the worst repeat and the longest face over every family
    // and tab.
    let mut per_state = String::new();
    let _ = writeln!(
        per_state,
        "| State | Screens | Worst repeat | Message of the worst repeat | Longest face | Longest text | Texts over {FACE_LIMIT} chars |"
    );
    let _ = writeln!(per_state, "|---|---:|---:|---|---:|---:|---:|");
    for state in State::ALL {
        let of_state: Vec<&Screen> = screens.iter().filter(|s| s.state == state).collect();
        let worst = of_state
            .iter()
            .flat_map(|s| s.repeats.iter().filter(|(k, _)| is_message(k)))
            .max_by_key(|(_, n)| **n);
        let long_texts: std::collections::BTreeSet<String> = of_state
            .iter()
            .flat_map(|s| s.painted.iter())
            .filter(|p| p.text.chars().count() > FACE_LIMIT)
            .map(|p| normalise(&p.text))
            .collect();
        let _ = writeln!(
            per_state,
            "| {} | {} | {} | {} | {} | {} | {} |",
            state.name(),
            of_state.len(),
            worst.map_or(0, |(_, n)| *n),
            worst.map_or_else(String::new, |(k, _)| md_cell(k)),
            of_state.iter().map(|s| s.longest_face()).max().unwrap_or(0),
            of_state.iter().map(|s| s.longest_text()).max().unwrap_or(0),
            long_texts.len()
        );
    }

    // The exceptions: each message that paints two or more times on one
    // screen, and each text over the face limit. One row per family, state
    // and text; the tabs are listed.
    let mut repeats_rows: BTreeMap<RowKey, (usize, Vec<String>)> = BTreeMap::new();
    let mut long_rows: BTreeMap<RowKey, LongText> = BTreeMap::new();
    for s in &screens {
        for (key, n) in &s.repeats {
            if *n >= 2 && is_message(key) {
                let row = repeats_rows
                    .entry((s.family, s.state, key.clone()))
                    .or_insert((0, Vec::new()));
                row.0 = row.0.max(*n);
                row.1.push(format!("{:?}", s.tab));
            }
        }
        for p in &s.painted {
            if p.text.chars().count() > FACE_LIMIT {
                let row = long_rows
                    .entry((s.family, s.state, normalise(&p.text)))
                    .or_insert((0, false, Vec::new()));
                row.0 = row.0.max(p.face.chars().count());
                row.1 |= p.truncated();
                let tab = format!("{:?}", s.tab);
                if !row.2.contains(&tab) {
                    row.2.push(tab);
                }
            }
        }
    }
    let mut repeats_md = String::new();
    let _ = writeln!(
        repeats_md,
        "| Family | State | Max per screen | Tabs | Message |"
    );
    let _ = writeln!(repeats_md, "|---|---|---:|---|---|");
    for ((family, state, key), (n, tabs)) in &repeats_rows {
        let _ = writeln!(
            repeats_md,
            "| {} | {} | {} | {} | {} |",
            family.name(),
            state.name(),
            n,
            tabs.join(", "),
            md_cell(key)
        );
    }
    let mut long_md = String::new();
    let _ = writeln!(
        long_md,
        "| Family | State | Chars | Face chars | Truncated | Tabs | Full text |"
    );
    let _ = writeln!(long_md, "|---|---|---:|---:|---|---|---|");
    for ((family, state, text), (face, truncated, tabs)) in &long_rows {
        let _ = writeln!(
            long_md,
            "| {} | {} | {} | {} | {} | {} | {} |",
            family.name(),
            state.name(),
            text.chars().count(),
            face,
            if *truncated { "yes" } else { "no" },
            tabs.join(", "),
            md_cell(text)
        );
    }

    eprintln!("{inventory}\n{per_state}\n{summary}\n{repeats_md}\n{long_md}");

    if std::env::var_os("W0_WRITE_BASELINE").is_some() {
        let scan = scan_table(&source_scan());
        let skipped: String = State::ALL
            .iter()
            .flat_map(|state| {
                Family::ALL.iter().filter_map(move |family| {
                    state
                        .skip_reason(*family)
                        .map(|why| format!("- {} / {}: {why}.\n", family.name(), state.name()))
                })
            })
            .collect();
        let doc = baseline_doc(
            &per_state,
            &summary,
            &repeats_md,
            &long_md,
            &scan,
            &skipped,
            &tree_state(),
        );
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../planning/toolpath_panel_review_2026-10-30/W0_BASELINE.md");
        std::fs::write(&path, doc).expect("write the baseline");
        eprintln!("wrote {}", path.display());
    }
}

fn baseline_doc(
    per_state: &str,
    summary: &str,
    repeats: &str,
    long: &str,
    scan: &str,
    skipped: &str,
    tree: &str,
) -> String {
    format!(
        "# W0 baseline: the toolpath inspector in bad states\n\
\n\
This file records the counts before the tidy programme (W1-W6). The test\n\
`crates/rs_cam_viz/tests/the_inspector_bad_states_w0.rs` writes it. Do not\n\
edit it by hand. To write it again, run this command:\n\
\n\
```\n\
W0_WRITE_BASELINE=1 scripts/cargo_lane.sh test -p rs_cam_viz -q --test the_inspector_bad_states_w0 -- --nocapture\n\
```\n\
\n\
## The tree that the test measured\n\
\n\
The counts are true for this tree only. A file that differs from HEAD\n\
changes the counts.\n\
\n\
{tree}\n\
## The method\n\
\n\
The test renders the real inspector (`properties::draw`) at the 280-point\n\
panel width of IC1. It renders three op families:\n\
\n\
- Pocket (2D), on a 20 mm square outline, with a 6 mm flat end mill.\n\
- 3D Rough (Adaptive3d), on a 10 mm hemisphere, with the flat end mill.\n\
- 3D Finish (Scallop), on the hemisphere, with a 6 mm ball nose.\n\
\n\
Each state has its own fresh fixture. The state applies to one op only.\n\
The test renders each tab of `ToolpathTab::ALL` in each state. It renders\n\
each tab two times on a fresh egui context and reads the second frame.\n\
\n\
The states are these:\n\
\n\
1. clean: the reference. The tool fits, the feeds and the floor are set,\n\
   and nothing refused.\n\
2. no tool: the op has no tool.\n\
3. feeds unset: the feed, the plunge and the spindle speed are empty. The\n\
   floor stays set.\n\
4. wrong tool kind: the flat end mill on the Scallop.\n\
5. last run refused: the run state holds the D53 refusal text of IC1.\n\
6. reach map on: `show_reach_map` is on and the overlay holds a ready map\n\
   of the hemisphere (Ø6 ball, 0.5 mm cells, tolerance 0.0001 mm). The\n\
   overlay also holds the refined surface, so the scale row paints.\n\
7. boundary on (Selection): the machining boundary is on, and its source\n\
   is the closed shapes of the 2D outline.\n\
8. stale after edit: the op has a result, then the feed changes to\n\
   900 mm/min. The freshness is `EditedSince`.\n\
9. feeds unset + refused (extra): state 3 plus a refusal in the shape of\n\
   the core `NotSet` text. This state is not in the W0 list.\n\
\n\
These states do not apply, and the test skips them:\n\
\n\
{skipped}\n\
The test asserts only one rule: each bad state paints its marker text\n\
more times than the clean state does. It does not assert the W6 rules yet.\n\
\n\
## The measures\n\
\n\
- Visible rows: the row bands of all galleys, merged where they overlap\n\
  (the IC1 measure). A row of label, value and unit is one row.\n\
- Galleys: the painted text galleys on the screen.\n\
- Truncated: a galley that elided its tail, or that the clip rectangle\n\
  cut.\n\
- Repeat: the number of galleys on one screen with the same message key.\n\
  The key is the full galley text, with the whitespace normalised and a\n\
  leading glyph (for example `✕` or `⚠`) removed. The test counts only a\n\
  text of three words or more as a message. A shorter text is a label, a\n\
  unit or a value.\n\
- Longest face: the most characters that one galley paints. The face is\n\
  shorter than the full text when the galley elided the tail.\n\
- Longest text: the most characters in one full galley text.\n\
\n\
The test prints the full galley inventory of each screen with\n\
`--nocapture`. This file holds the tables only.\n\
\n\
## The limits of the measures\n\
\n\
- The repeat key is the text. It does not see one fact in two words. For\n\
  example, the wrong tool state paints the tool refusal as three different\n\
  texts on one screen, and the repeat count is 1. W6 must count a\n\
  diagnostic id (the W1 `anchor` and `face`), not a text.\n\
- The three empty feed fields each paint the hint \"set a value\". The\n\
  repeat count is 3, but each hint belongs to its own field. W6 must not\n\
  count a field hint as a repeated message.\n\
- `OneLine` elides each long text, so no face is long. The full text\n\
  behind the face can be long. The tables give both lengths.\n\
\n\
## Worst values per state\n\
\n\
{per_state}\n\
## Repeated messages\n\
\n\
Each row is a message that paints two or more times on one screen.\n\
\n\
{repeats}\n\
## Texts over {FACE_LIMIT} characters\n\
\n\
Each row is a full galley text over {FACE_LIMIT} characters. W6 sets {FACE_LIMIT}\n\
characters as the face limit.\n\
\n\
{long}\n\
## Raw rows in the source\n\
\n\
The count of `ui.label(` and `ui.horizontal(` sites per file under\n\
`crates/rs_cam_viz/src/ui/properties/`. The scan ignores a line comment.\n\
\n\
{scan}\n\
## Each screen\n\
\n\
{summary}"
    )
}

/// The commit and the changed source files of the tree under test, as
/// Markdown. The baseline names the tree it measured.
fn tree_state() -> String {
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .map(|out| String::from_utf8_lossy(&out.stdout).trim_end().to_owned())
            .unwrap_or_default()
    };
    let head = git(&["log", "-1", "--format=%h %s"]);
    let changed = git(&[
        "status",
        "--short",
        "--",
        "../rs_cam_viz/src",
        "../rs_cam_core/src",
    ]);
    let changed = if changed.is_empty() {
        "(none)".to_owned()
    } else {
        changed
    };
    format!(
        "- HEAD: `{head}`\n- Source files that differ from HEAD (viz `src/` and core \
         `src/`):\n\n```\n{changed}\n```\n"
    )
}

// ── The source scan ─────────────────────────────────────────────────────

/// The raw-row sites of one file: the path under `ui/properties/`, the
/// `ui.label(` count and the `ui.horizontal(` count.
type ScanRow = (String, usize, usize);

fn source_scan() -> Vec<ScanRow> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/properties");
    let mut files = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read the properties folder") {
            let path = entry.expect("a folder entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
    }
    let mut rows: Vec<ScanRow> = files
        .iter()
        .map(|path| {
            let source = std::fs::read_to_string(path).expect("read a source file");
            let (mut labels, mut horizontals) = (0, 0);
            for line in source.lines() {
                // A line comment is not code.
                let code = line.split("//").next().unwrap_or("");
                labels += code.matches("ui.label(").count();
                horizontals += code.matches("ui.horizontal(").count();
            }
            let name = path
                .strip_prefix(&root)
                .expect("under the root")
                .display()
                .to_string();
            (name, labels, horizontals)
        })
        .collect();
    rows.sort_by(|a, b| (b.1 + b.2).cmp(&(a.1 + a.2)).then(a.0.cmp(&b.0)));
    rows
}

fn scan_table(rows: &[ScanRow]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "| File | `ui.label(` | `ui.horizontal(` | Total |");
    let _ = writeln!(out, "|---|---:|---:|---:|");
    for (name, labels, horizontals) in rows {
        let _ = writeln!(
            out,
            "| `{name}` | {labels} | {horizontals} | {} |",
            labels + horizontals
        );
    }
    let labels: usize = rows.iter().map(|r| r.1).sum();
    let horizontals: usize = rows.iter().map(|r| r.2).sum();
    let _ = writeln!(
        out,
        "| **all {} files** | {labels} | {horizontals} | {} |",
        rows.len(),
        labels + horizontals
    );
    out
}

#[test]
fn the_raw_rows_are_counted_w0() {
    let rows = source_scan();
    eprintln!("{}", scan_table(&rows));
    // Non-vacuity: the scan read the folder and found the raw rows that the
    // toolpath panel has today.
    assert!(rows.len() > 10, "the scan read {} files", rows.len());
    let panel = rows
        .iter()
        .find(|r| r.0 == "toolpath_panel.rs")
        .expect("the scan reads toolpath_panel.rs");
    assert!(
        panel.1 + panel.2 > 0,
        "toolpath_panel.rs has raw rows today"
    );
    // The comment rule: a line that is only a comment counts zero.
    let code = "// ui.label(\"x\")".split("//").next().unwrap_or("");
    assert_eq!(code.matches("ui.label(").count(), 0);
}
