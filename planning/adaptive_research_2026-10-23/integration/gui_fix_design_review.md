# Review of the GUI fix design (gui_fix_design.md)

Reviewer: G1 (independent GUI review), 2026-10-28. Read only; no code.
Design under review: `gui_fix_design.md` at worktree HEAD `de1696763`.
Line numbers are at that HEAD unless a file is named.

The test: the operator's constraint. "We have a habit of piling up text
and making a cluttered UI." A fix that adds a line, a paragraph or a new
widget pattern fails, even when it closes the finding.

Overall: the design passes the clutter test. It adds no always-visible
line, it removes two (the `Boundary:` row and Z Blend), and every new
signal is a dash, a grey option, a chip, a badge, a word in a label or a
modal. Three items need a change; one of them hides the main fix in the
non-default view.

## Verdicts

| Item | Verdict | Reason | Concrete change |
|---|---|---|---|
| §0.3 D-1: `Boundary: …` row goes; its text becomes the `Source:` hover | **accept** | It repeats the combo one row above. The sentry `boundary_controls_always_visible_g_boundaryinherit` tests `boundary_summary_line` as a function (`tests/…_g_boundaryinherit.rs:88-151`) and scans sources for writes; neither pins the row. | none |
| §1 (#1) feed caps: dashed moves, legend tail, figures hover, `get_cut_trace` keys, the comment | **change** | Half solved. The dash draws in the Engagement colour mode only ("Move colour: Engagement only", §1 a). The default mode is `Normal` (`state/viewport.rs:233`, `:287`), so in the view a user opens the cap is still silent. A dash is a line style, not a colour; it belongs in every mode. The rest (legend tail, figures hover, MCP keys) is right: no panel text, data where a reviewer reads it. | Dash a capped move in every `ToolpathColorMode` (`Normal`, `Engagement`, `AdvancePerTooth`): the `feed_cap.is_some()` branch goes in the one vertex builder each mode calls, not in `engagement` alone. One legend tail entry `dashed: fed at a load cap` on each legend that the view shows for those modes. The render test covers `Normal` as well as `Engagement`. |
| §2 a: core read door `larger_rest_consumer` | **accept** | The refusal and the panel read one predicate; the generate-time refusal stays as the hard check. | none |
| §2 c: grey `Center` and `Outside` with the core sentence on the disabled hover | **accept** | The existing pattern (`tab_badges.rs:727-732`: `add_enabled_ui` + `on_disabled_hover_text`). The hover is three short sentences with the sizes, the overrun and the fix. No line. | none |
| §2 d: a new header line `⛔ Containment must be Inside` + `[Use Inside]`, Generate off | **change** | It solves the finding, but it is a new header widget pattern (a face plus a button on one row) and the fit is at the margin (§7.3: 264.1 points with `Refused ·`; about 230 without). The header already has a one-line pattern for a tab-owned refusal: `OneLine::new(REFUSED_SEE_GEOMETRY).detail(..)` (`toolpath_panel.rs:151-156`), and `OWNER_TAB` (`tab_badges.rs:68`) puts a dot on the owning tab. The fix belongs beside the control, where the user works. | Header: the existing `OneLine` pattern, face `Refused · see Cut`, detail = the core D53 sentence, Generate off, a dot on the Cut tab. Cut tab: the `Use Inside` small button at the right of the `Containment:` row, same row, shown only in the bad state; its hover is the core sentence. No new header pattern; the header line is the same width as today's `Refused · see Geometry`. The worst-state count stays 26. |
| §2 e (D-2): the `Rest Regions` pick writes `Inside` and shows the `⟲` badge | **accept, with one trap** | It stops the bad state at its usual cause with no text. The badge is the existing `ProvenanceBadge` with `ProvKind::AutoCorrect` (`components/provenance.rs:37`). Trap: §2 e says the badge state "lives in the draft entry (a GUI flag)". The draft is rebuilt from the session every frame (`write_entry_config_to_session` doc, `ui/properties/mod.rs:968-980`), so a flag on it does not survive the frame. The heights refusal had the same problem (ui09). | Keep the flag in `state.panels`, scoped to the toolpath id, as `heights_refusal` is; clear it on the first containment edit or on a toolpath change. State it in the step report. Note for the implementer, not a design change: `Inside` insets each region by the tool radius, so a region narrower than the tool vanishes; the source's region count (§6) does not change, and the consumer may generate few moves. No new text for this; the figures row shows the move count. |
| §3 (#3): the ⚡ chip beside the Strategy combo; click switches | **accept** | The existing `SuggestButton::field` (`components/suggest.rs:57`), in the grid cell of the combo, no row. The hover is one sentence and the act. The chip goes when the warning stops. The write is the draft, so one `ReplaceToolpathConfig`. | none. The `ui04` sentry keys hovers on the parameter name; the chip hover is on the chip, not on a `p(...)` label, so it is outside that rule. Confirm in the step report. |
| §4 (#4): the load refusal modal | **accept** | The same frame as the load-warnings modal (`app.rs:1244-1294`), `NoticeStack` bounds the hits, one fix line, `Copy`. A modal is the right place for a multi-line message with a path to edit; the toast was the clutter. The fix paragraph (three lines of core text) shows once in a modal, which is fine. | none. Order note: G-A (Z Blend, step 1) adds a new load refusal (`z_blend = true`) while the modal lands in G-D (step 4); between the two, that refusal is still the 8 s toast. Either move §4 into G-A's step or order G-D before G-A. |
| §5 (#5, Z Blend): retire the field; D-3 load rule | **accept** | The field is dead for Geodesic (my finding) and the probe shows it breaks the one-Depth/Pass rule on Contour Parallel (4.854 mm at a 3 mm Depth/Pass). Deleting the row beats hiding it. D-3 (`false` loads, `true` refuses with the fix) follows the R2 precedent. | Two implementation notes, no design change: the `RETIRED_VALUES` matcher compares `as_str` (`project_file.rs:395`); a boolean row needs a value compare, as the design says, and the new sentry must cover a `true` written as a TOML boolean, not a string. The probe numbers in §5.2 come from a deleted test; the re-bless log of `contour_parallel_flat` must print the before and after move counts (1198 → 1124) so the claim stays checkable. |
| §6 (#6): `· N regions` in the combo item, the top Z on the hover, `· N left` in the disclosure title | **accept, one wording change** | Words in labels that exist; no row. | One phrase for one number: the combo item and the disclosure title both read `· N regions`. "left" in the title and "regions" in the item are two words for one fact. |
| §7.2 rule 1–3, 5 | **accept** | One line, detail on hover, a fix is an action, the tabs never move. This is the rule the panel needed. | none |
| §7.2 rule 4: "at most three lines, then `+N more`" | **change** | The `NoticeStack` doc forbids that form: "`Showing 4 of 293 · Show all`. Never `+289 more`" (`components/notice.rs:26`). The rule must name the component's own form, or two surfaces will word one bound two ways. | Rule 4 reads: "A list of messages shows at most three, then the `NoticeStack` remainder line (`Showing 3 of N · Show all`)." |
| §7.3 fit numbers | **accept** | Measured widths, a fit test per face. The margin problem of the header line goes with the §2 d change (the button leaves the header). | none |
| §7.5 two refusals at once | **accept** | One line. The last-run refusal becomes the state word; the blocker answers the settings on screen. | With the §2 d change the blocker line is `Refused · see Cut`; the state word for the last run stays `Refused` with its hover (G-E). Two `Refused` words on one row would read as a repeat: the header line should then be the validation form only, and the state word carries the last-run text. §7.5 already says this; keep it. |
| §7.6 counts (39 → 26 with G-E; 31 without) | **accept** | The before count is measured; the after count follows from the rules. The main saving is the 15-row header refusal. The §2 d change does not move the count (one header line either way). | none |
| §7.7 G-E (D-4) | **accept, in this order** | Worth its size. The header refusal is the single largest clutter in the panel (3–15 rows, the tabs move 257 points), and it is not new to this branch: every 3D op shows it today. About 180 lines of code and test for a panel that stays put is cheap. The footer findings are the second unbounded source. | Order inside G-E: (1) the `Error` arm → state word `Refused` + one `OneLine` face, and the IC1 rule "no text wraps in the toolpath panel"; (2) the footer findings bound; (3) the three rest-boundary refusal faces with their `⟳` action; (4) `Feeds unavailable` and the header validation errors. Each a frozen sub-step so the lead can stop after (1) or (2). |
| §8 (#11) wording | **accept** | Shorter hovers than today; the `Rest source` hover drops from four lines to two. | none |
| §10 architecture | **accept** | The chip, the pick's `Inside`, the `Use Inside` button and the badge clear all write the draft, which commits as one `Command::ReplaceToolpathConfig` (`ui/properties/mod.rs:1005-1025`). The modal reads a load error; `load_refusal` is controller GUI state, not session state. The MCP refusals use the param setter. No parallel flow. | none |

## Clutter check, item by item

| Signal | Form | Always visible? | Simpler form exists? |
|---|---|---|---|
| capped move | dash in the view | no (a style on moves already drawn) | no; a dash is the minimum |
| legend tail `dashed: fed at a load cap` | one legend entry | only with the legend | no |
| cap count | figures hover | no | no |
| D53 option | grey + disabled hover | no | no |
| D53 blocker | header one-liner + `Use Inside` button | one line, bad state only | yes: the existing `Refused · see <tab>` pattern (the change above) |
| D-2 | `⟲` badge beside the combo | until the next edit | no |
| Suggest | ⚡ chip | while the warning holds | no; it is the existing pill |
| load refusal | modal | no | no; a modal is the right surface |
| regions | a number in two labels | no new line | no |
| Z Blend | row deleted | — | — |

No hover is an essay: the longest new hover is the D53 option hover, three
sentences. The design adds one new widget pattern (the header line with a
button, §2 d), and the change above removes it.

## Discoverability: can the user still find WHY?

| State | Where the why is | Silent? |
|---|---|---|
| a capped move | the dash, the legend, the figures hover, `get_cut_trace` | silent in the default colour mode as designed; not silent after the §1 change |
| Center or Outside refused | the grey option's hover; the header line's detail; the `Use Inside` hover | no |
| containment auto-set to Inside | the `⟲` badge hover | no, while the badge stands; after the first edit the fact is gone, which is right (the user edited it) |
| Geodesic suggested | the ⚡ hover | no |
| a project refused at load | the modal, with Copy | no |
| Z Blend gone from a project with `true` | the load refusal in the modal | no |
| a Z Blend project with `false` | nothing; the output does not change | silent by design (D-3), and correct |
| regions left | the number; the top Z on hover | no |
| a last-run refusal under G-E | the `Refused` state word hover | no; the hover is the only route, which rule 2 accepts |

One state goes silent that was loud: a generate refusal under G-E is a
word and a hover, not a paragraph. That is the point of G-E; the hover
holds the full core sentence.

## Top three changes

1. **Dash a capped move in every colour mode, not Engagement alone** (§1).
   The default view is `Normal`; as designed the one visible signal for
   the strategy's load mechanism stays hidden where the user looks.
2. **The D53 blocker takes the existing header pattern** (§2 d):
   `Refused · see Cut` with the core sentence as detail and a dot on the
   Cut tab; the `Use Inside` button sits on the `Containment:` row, beside
   the control it fixes. No new header widget; the fit margin problem goes.
   And the `⟲` badge flag lives in `state.panels`, not in the per-frame
   draft.
3. **Do G-E, header refusal first** (§7.7, D-4 yes): the 15-row refusal
   that moves the tab strip is the largest clutter in this panel and is
   older than the branch. Sub-steps in the order above, so the lead can
   stop after the header arm and the findings bound.

One observation outside the brief, for the lead: the tab strip wraps to
two lines at 280 points on every op (`Feeds & Speeds` starts row 2,
§0.2). A shorter tab label (`Feeds`) would save one always-visible line on
every operation, more than any item in this design.
