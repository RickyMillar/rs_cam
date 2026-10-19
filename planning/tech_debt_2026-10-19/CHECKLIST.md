# Tech-debt review checklist (rs_cam, baseline HEAD 99b6fb41)

This checklist defines the categories for one full tech-debt sweep of the
workspace (about 560k code lines in 1406 `.rs` files, `tokei crates`). One
finder agent sweeps one category across all crates with `rg` and file reads.

Read these rules before a sweep:

- Do not edit source. Do not run cargo.
- Paths in `instruments/in_flux_paths.txt` are "in flux". Mark each finding in
  those paths "in flux". Example: `crates/rs_cam_core/src/session/command.rs`
  and `crates/rs_cam_core/src/session/mod.rs` are in flux.
- Cite each claim as `path:line`.
- Give each finding an enforcement grade: TYPES | LINT | SENTRY | DOC |
  CONVENTION | NONE. The grade says what stops the next agent from a silent
  regression.
- Use the severity rubric at the end.

The operator named five minimum lenses: duplication, unused code, bad design
patterns, inconsistent design patterns, and code that is hard to follow. These
are examples. The 13 categories below come from the sources and include all
five.

## Repo facts that the categories use

- Lint policy: `Cargo.toml` `[workspace.lints.clippy]` denies `unwrap_used`,
  `expect_used`, `panic`, `indexing_slicing`, `print_stdout`,
  `needless_pass_by_value`, `wildcard_imports`, `redundant_clone`,
  `manual_let_else` and others. `[workspace.lints.rust]` denies `unsafe_code`.
  No pedantic, nursery or design-restriction lint is on. Each crate opts in
  with `[lints]` (for example `crates/rs_cam_core/Cargo.toml:71`).
- ID newtypes exist but have public fields: `pub struct ToolpathId(pub usize)`
  at `crates/rs_cam_core/src/ids.rs:24` (also `:34`, `:38`, `:44`, `:49`).
  Any caller can make an ID from a raw `usize`.
- One sealed trait exists, as a typestate:
  `crates/rs_cam_core/src/trace/transform_provenance.rs:68`.
- `ProjectSession::apply` is at `crates/rs_cam_core/src/session/command.rs:2339`
  (in flux). The mutation contract is in `CLAUDE.md` prose.
- About 1221 struct fields or parameters carry the unit in the name only
  (`_mm: f64`, `_deg`, `_rad`, `_rpm`). No unit newtype exists
  (`rg "struct Mm|struct Radians"` is empty).
- 136 sites carry `allow(clippy::too_many_arguments)`.
- Existing instruments: `scripts/debt_scan.py` (dead pub surface, allow
  without SAFETY, test-only pub API, legacy residue),
  `scripts/duplicate_sweep.py` (embedding near-duplicates; needs the
  SocratiCode Qdrant store), and the files under `instruments/`.
- 37 `CLAUDE.md` files (`instruments/claude_md_paths.txt`). About 499 test
  files in `crates/rs_cam_core/tests`. Sentries are tests whose names state
  the rule (for example `the_research_arms_are_feature_gated_fin01.rs`).

---

## 1. Primitive obsession and missing newtypes (`primitive-newtype`)

**Definition.** A domain value (a length, an angle, a feed, an index, an ID, a
frame) travels as a bare `f64`, `usize` or `String`. The compiler cannot stop a
mix-up of two values of the same primitive type.

**Rust signals**
- `_mm: f64`, `_deg: f64`, `_rad: f64`, `_rpm: f64`, `_mm_min: f64`: the unit
  lives only in the name.
- Functions with two or more adjacent `f64` parameters of different meaning.
- Newtypes with `pub` inner fields (`ids.rs:24`). The newtype gives no
  construction guarantee.
- `usize` indices into `Vec`s that stand for entity identity.
- Points in different frames (setup frame, machine frame, stock frame) with
  the same type.

**Detection in this repo**
- `rg -n "fn \w+\([^)]*f64[^)]*f64[^)]*f64" -g '*.rs' crates`
- `rg -c "_(mm|deg|rad|rpm)\w*: f64" -g '*.rs' crates | sort -t: -k2 -nr | head`
- `rg -n "pub struct \w+\(pub " -g '*.rs' crates`
- `rg -n "index: usize|idx: usize" crates/rs_cam_viz/src/app/mcp` (MCP handlers
  that take a raw index, for example `simulation.rs:176`).
- Read `crates/rs_cam_core/src/geometry` and `machine` for frame types.

**Type-level remedies**
- Newtype with a private field and a checked constructor (C-NEWTYPE,
  C-NEWTYPE-HIDE).
- Unit newtypes (`Mm`, `Radians`) with `From` only where the conversion is
  exact.
- Frame-tagged points (`Point<SetupFrame>`) with a `PhantomData` marker.

**Sources**: [API Guidelines C-NEWTYPE](https://rust-lang.github.io/api-guidelines/type-safety.html#c-newtype),
[Newtype pattern](https://rust-unofficial.github.io/patterns/patterns/behavioural/newtype.html),
[API checklist](https://rust-lang.github.io/api-guidelines/checklist.html).

## 2. Boolean, `Option` and stringly-typed parameters (`bool-string-params`)

**Definition.** A parameter or field selects behaviour with a `bool`, an
`Option<()>`-style flag, a magic number or a `String` key. The call site does
not say what the value means. A typo in a string key compiles.

**Rust signals**
- `fn f(.., a: bool, b: bool)`; call sites like `f(x, true, false)`.
- Structs with three or more `bool` fields that are mutually exclusive.
- `match s.as_str() { "raster" => .., _ => .. }` dispatch on a string.
- `kind: String`, `mode: &str`, `HashMap<String, f64>` as parameter bags.
- MCP or CLI parameters parsed from strings deep inside the core.

**Detection in this repo**
- `rg -n "fn \w+\([^)]*: bool[^)]*: bool" -g '*.rs' crates` (8 hits at
  baseline).
- `rg -n "\(\w+, (true|false)(, (true|false))+" -g '*.rs' crates`
- `rg -n "\"[a-z_]+\" =>" -g '*.rs' crates/rs_cam_core/src` (top files:
  `material/mod.rs`, `session/compute/params.rs`, `compute/config.rs`).
- `rg -n "kind: String|mode: String|: &str\) ->" -g '*.rs' crates/rs_cam_core/src`
- `rg -c "pub \w+: bool" -g '*.rs' crates | sort -t: -k2 -nr | head`

**Type-level remedies**
- A two-variant enum in place of each `bool` (C-CUSTOM-TYPE).
- One enum in place of a set of exclusive `bool` fields.
- Parse the string once at the MCP/CLI boundary into an enum with `FromStr`
  or `serde`; pass the enum inward.
- Turn on `clippy::fn_params_excessive_bools` and
  `clippy::struct_excessive_bools` (pedantic) in `Cargo.toml`.

**Sources**: [C-CUSTOM-TYPE](https://rust-lang.github.io/api-guidelines/type-safety.html#c-custom-type),
[clippy lint list](https://rust-lang.github.io/rust-clippy/master/index.html),
[Avoid the temptation of bool](https://dev.to/pauljlucas/avoid-the-temptation-of-bool-5g31).

## 3. Representable illegal states and validate-not-parse (`illegal-states`)

**Definition.** A type can hold a value that the domain forbids. Code then
checks the value at run time, often in many places ("shotgun parsing"). A check
returns `()` or `bool` and discards what it learned.

**Rust signals**
- `fn validate_*(..) -> Result<(), _>` or `-> bool` (example:
  `crates/rs_cam_viz/src/app/mcp/generation.rs:815`).
- Structs with several `Option` fields that are only valid together.
- The same range check (`> 0.0`, `is_finite`) in more than one layer.
- `enum` state plus a separate `bool` or `Option` that must agree with it.
- `Vec` that must be non-empty; `min`/`max` pairs that must be ordered.
- State machines held in an enum plus run-time `if state != X { return Err }`
  where a typestate fits.

**Detection in this repo**
- `rg -n "fn (validate|check|ensure)_\w+\([^)]*\) -> (Result<\(\)|bool)" -g '*.rs' crates`
- `rg -n "is_finite\(\)|<= 0\.0|< 0\.0" crates/rs_cam_core/src -g '*.rs' --count-matches | sort -t: -k2 -nr | head`
- `rg -n "Option<[^>]+>,\s*$" crates/rs_cam_core/src/session crates/rs_cam_core/src/compute`
  then read struct definitions with several `Option` fields.
- Read `crates/rs_cam_core/src/compute/validate.rs` (in flux) and
  `compute/operation_configs.rs`.
- Compare with the good example: `trace/transform_provenance.rs:68`.

**Type-level remedies**
- Parse at the boundary into a refined type (`PositiveMm`, `NonEmpty<T>`,
  `ZRange { lo, hi }` with a checked constructor).
- Replace co-dependent `Option` fields with an enum whose variants carry the
  data.
- Typestate with a sealed marker trait for staged pipelines.

**Sources**: [Parse, don't validate](https://lexi-lambda.github.io/blog/2019/11/05/parse-don-t-validate/),
[Making illegal states unrepresentable (corrode)](https://corrode.dev/blog/illegal-state/),
[Typestate pattern in Rust](https://cliffle.com/blog/rust-typestate/),
[C-VALIDATE](https://rust-lang.github.io/api-guidelines/dependability.html).

## 4. Encapsulation, visibility and escape hatches (`encapsulation-visibility`)

**Definition.** A type exposes its fields or internal API too widely. A caller
can bypass an invariant or a contract, for example mutate `ProjectSession`
without `apply(Command)`.

**Rust signals**
- `pub` fields on a type with an invariant (C-STRUCT-PRIVATE).
- `pub fn *_mut(&mut self) -> &mut Inner` and `pub` setters on session state.
- `pub` items used only inside the crate (should be `pub(crate)`).
- `pub` items used only by tests (test-only public API).
- Traits that outside code must not implement but that are not sealed.
- `#[doc(hidden)] pub` used as a privacy substitute.

**Detection in this repo**
- `rg -n "pub fn \w+_mut\(" -g '*.rs' crates/rs_cam_core/src`
- `rg -n "^\s+pub \w+:" crates/rs_cam_core/src/session -g '*.rs'` then read the
  owner type.
- `instruments/dead_pub_surface.md` and `instruments/test_only_pub_api.md`.
- `rg -n "pub trait" -g '*.rs' crates/rs_cam_core/src`; compare with the one
  sealed trait.
- Ratio at baseline: 3567 `pub fn` to 1210 `pub(crate)`.

**Type-level remedies**
- Private fields plus a constructor that checks the invariant.
- `pub(crate)` / `pub(super)`; the rustc lint `unreachable_pub` and clippy
  `redundant_pub_crate` (nursery) report misuse.
- Sealed traits (supertrait in a private module).
- Remove `*_mut` accessors so `apply(Command)` is the only mutation path.

**Sources**: [C-STRUCT-PRIVATE, C-SEALED](https://rust-lang.github.io/api-guidelines/future-proofing.html),
[Sealed traits guide](https://predr.ag/blog/definitive-guide-to-sealed-traits-in-rust/),
[Privacy for extensibility](https://rust-unofficial.github.io/patterns/idioms/priv-extend.html).

## 5. Error-handling debt (`error-handling`)

**Definition.** An error loses its type, its cause or its context. Or code
discards a failure. The caller cannot match on the failure kind.

**Rust signals**
- `Result<_, String>` and `Box<dyn Error>` in library code (54 hits at
  baseline).
- `.map_err(|e| e.to_string())`, `format!("{e}")` at each layer.
- `let _ = fallible();`, `.ok();` with no comment.
- `unwrap_or_default()` / `unwrap_or(0.0)` on a value whose absence is a
  defect.
- Functions whose result must be used but have no `#[must_use]`.
- Error enums with a catch-all `Other(String)` variant.

**Detection in this repo**
- `rg -n "Result<[^,>]+, String>" -g '*.rs' crates`
- `rg -n "let _ = [a-z_:.]+\(" -g '*.rs' crates/rs_cam_core/src crates/rs_cam_viz/src`
- `rg -n "\.ok\(\);" -g '*.rs' crates`
- `rg -n "unwrap_or\((0\.0|0|f64::)" -g '*.rs' crates/rs_cam_core/src`
- `rg -n "Other\(String\)|Message\(String\)" -g '*.rs' crates`
- Read `SessionError` and the MCP error mapping in `crates/rs_cam_mcp/src/response.rs`.

**Type-level remedies**
- One typed error enum per layer with `From` conversions (C-GOOD-ERR).
- `#[must_use]` on result-like types; `clippy::must_use_candidate` and
  `clippy::let_underscore_must_use`.
- `clippy::missing_errors_doc` for public fallible functions.

**Sources**: [C-GOOD-ERR](https://rust-lang.github.io/api-guidelines/interoperability.html#c-good-err),
[C-FAILURE](https://rust-lang.github.io/api-guidelines/documentation.html#c-failure),
[clippy lint list](https://rust-lang.github.io/rust-clippy/master/index.html).

## 6. Duplication: code and knowledge (`duplication`)

**Definition.** Two or more places hold the same logic, the same constant, or
two resolvers for the same quantity. A fix in one place does not reach the
other. The surfaces (GUI, MCP, CLI) then give different numbers.

**Rust signals**
- Near-identical function bodies in two modules.
- Two functions that compute the same physical quantity by different paths.
- The same numeric constant typed in more than one file.
- Per-variant `match` arms that repeat the same body (`match_same_arms`).
- The same conversion written by hand in GUI, MCP and CLI.

**Detection in this repo**
- `python3 scripts/duplicate_sweep.py --src-only` (needs the Qdrant store;
  the index is being rebuilt, so the output can be partial).
- `rg -n "const \w+: f64 = [0-9.]+;" -g '*.rs' crates | sort -t= -k2` and look
  for equal values in different files.
- `rg -n "^\s*(pub )?fn (resolve|compute|calc|estimate)_\w+" -g '*.rs' crates/rs_cam_core/src | sort -t'(' -k1`
  then compare same-named functions.
- Compare the handlers in `crates/rs_cam_viz/src/app/mcp/*.rs` with
  `crates/rs_cam_cli/src/*.rs` for logic that belongs in core.
- Use memory note "Probe the same code path": `feeds::calculate` versus the
  envelope resolver is a known pair.

**Type-level remedies**
- Move the shared logic into one core function; make the copies call it.
- A trait with one implementation per variant in place of parallel `match`
  blocks.
- One `const` in the owner module; others `use` it.

**Sources**: [Li, Avgeriou, Liang 2015, TD mapping study (code debt)](https://research.vu.nl/en/publications/a-systematic-mapping-study-on-technical-debt-and-its-management/),
[Technical Debt Tools mapping study](https://www.scitepress.org/Papers/2021/104591).

## 7. Dead code, unused dependencies and legacy residue (`dead-code`)

**Definition.** Code, features, dependencies or compatibility shims that no
product path uses. The operator ruled "no legacy, breaking OK"
(2026-09-16), so old shapes and aliases are debt.

**Rust signals**
- `#[allow(dead_code)]`, `#[allow(unused)]`, `_unused` names.
- `pub` items with no caller outside their file or outside tests.
- Dependencies in `Cargo.toml` that no `use` reaches.
- `#[serde(alias = ..)]`, `#[deprecated]`, `legacy`, `v2`, `old_` names.
- Feature-gated research code that no gate, test or bench reaches.
- Commented-out code blocks.

**Detection in this repo**
- `instruments/dead_pub_surface.md`, `instruments/legacy_residue.md`,
  `instruments/cargo_machete.txt` (machete has false positives for derive
  crates).
- `rg -n "allow\((dead_code|unused)" -g '*.rs' crates`
- `rg -n "serde\(alias|#\[deprecated|legacy|_old\b|_v2\b" -g '*.rs' crates`
- `rg -n "cfg\(feature = \"research\"\)" -g '*.rs' crates | wc -l`, then
  check each gated item for a caller.
- `rg -n "^\s*// +(let|fn|if|for|match) " -g '*.rs' crates` (commented-out
  code).

**Type-level remedies**
- Delete the item. The compiler then shows every real caller.
- Private visibility so the `dead_code` lint can see the item.
- `cargo-udeps` (nightly, slower, fewer false positives) for a second check.

**Sources**: [cargo-machete](https://blog.benj.me/2022/04/27/cargo-machete),
[Rust patterns: deny(warnings) anti-pattern](https://rust-unofficial.github.io/patterns/anti_patterns/deny-warnings.html),
[Li et al. 2015](https://research.vu.nl/en/publications/a-systematic-mapping-study-on-technical-debt-and-its-management/).

## 8. Inconsistent design patterns and naming (`inconsistency`)

**Definition.** The workspace solves one problem in two or more ways. A reader
cannot predict the shape of the next module. Agents copy whichever variant
they find first.

**Rust signals**
- Constructors: `new` versus `from_*` versus `Default` plus field set
  (C-CTOR).
- Conversions: `to_*` / `as_*` / `into_*` used against the convention
  (C-CONV); hand-written conversion functions where `From` fits
  (C-CONV-TRAITS).
- Getters with and without `get_` (C-GETTER).
- Word order: `ToolLoadReport` versus `ReportToolLoad` (C-WORD-ORDER).
- Some ops use the `OpParams` derive, others hand-write the schema.
- Some config uses builders, some uses public struct literals.
- Mixed module layout: `foo.rs` plus `foo/` versus `foo/mod.rs`
  (`mod_module_files`, `self_named_module_files`).
- Error style differs per crate (typed enum, `String`, `anyhow`).

**Detection in this repo**
- `rg -n "pub fn get_\w+\(&self" -g '*.rs' crates | wc -l` versus
  `rg -n "pub fn \w+\(&self\) -> &" -g '*.rs' crates | wc -l`
- `rg -n "#\[derive\([^)]*OpParams" -g '*.rs' crates` versus the op list in
  `crates/rs_cam_core/src/ops`.
- `find crates -name mod.rs | wc -l` versus `foo.rs` with sibling `foo/`.
- `rg -n "impl From<|fn to_\w+\(self|fn into_\w+\(&self" -g '*.rs' crates/rs_cam_core/src`
- Read three sibling modules per folder in `crates/rs_cam_core/src/ops` and
  `crates/rs_cam_viz/src/app/mcp`, and compare their shape.

**Type-level remedies**
- A trait that each sibling implements (the compiler checks the shape).
- A derive macro for the repeated boilerplate (`rs_cam_derive`).
- Pick one module layout and turn on the matching clippy restriction lint.

**Sources**: [API Guidelines: Naming](https://rust-lang.github.io/api-guidelines/naming.html),
[API Guidelines: Predictability](https://rust-lang.github.io/api-guidelines/predictability.html),
[Rust patterns: Builder](https://rust-unofficial.github.io/patterns/patterns/creational/builder.html).

## 9. Architecture, layering and coupling (`architecture-layering`)

**Definition.** A dependency goes the wrong way across a layer. Or a module
couples to many others. Or a product surface uses a parallel flow and does not
use the core → worker → UI path. `CLAUDE.md` names the layers: import, tool
model, operation generation, dressups, simulation, export, with the toolpath IR
as the boundary.

**Rust signals**
- `rs_cam_core` code that knows of GUI, MCP or CLI concepts.
- A surface (viz, MCP, CLI) that computes a domain number itself.
- Export or post-processing code that reads planner internals and not the
  toolpath IR.
- Module cycles: `a` uses `b::x` and `b` uses `a::y`.
- A module with high fan-in and high fan-out (unstable and depended on).
- `ProjectSession` state changed outside `apply(Command)`.

**Detection in this repo**
- `rg -n "^rs_cam|egui|wgpu" crates/rs_cam_core/Cargo.toml` (only
  `rs_cam_derive` at `:44` at baseline).
- `rg -n "use crate::(\w+)" crates/rs_cam_core/src/<folder> -o -g '*.rs' | sort | uniq -c`
  per folder, to build a folder fan-out table; invert for fan-in.
- `rg -n "use crate::(stock|compute|session)" crates/rs_cam_core/src/gcode crates/rs_cam_core/src/export`
  (export reaching above the IR).
- `rg -n "rs_cam_core::\w+::\w+::" crates/rs_cam_viz/src crates/rs_cam_cli/src | sort | uniq -c | sort -nr | head`
  (deep paths into core internals).
- Compare each folder `CLAUDE.md` layer claim with its `use` lines.
- Do not use the SocratiCode graph or impact tools for counts.

**Type-level remedies**
- Crate boundaries for layers (a crate cannot import a crate it does not
  depend on).
- Private modules plus a narrow `pub use` facade per layer.
- Session mutation only through `apply`; make the session fields private.

**Sources**: [Rust patterns: Prefer small crates](https://rust-unofficial.github.io/patterns/patterns/structural/small-crates.html),
[Architectural TD mapping study](https://sol.sbc.org.br/index.php/sbes/article/view/28288),
Robert C. Martin, package metrics (instability I = Ce/(Ca+Ce), abstractness),
*Agile Software Development: Principles, Patterns, and Practices* (2002).

## 10. Size and complexity: hard to follow (`complexity-size`)

**Definition.** A file, a function, a parameter list or a type signature is
too large for a reader to hold. Control flow is deep. A reader cannot find the
one place that decides a behaviour.

**Rust signals**
- Files over about 1500 lines; functions over about 150 lines.
- `#[allow(clippy::too_many_arguments)]` (136 sites at baseline).
- `#[allow(clippy::type_complexity)]`; long tuples as return types.
- Nesting deeper than four levels; long `if let ... else if let` chains.
- Large `match` blocks with logic inline in each arm.
- `impl` blocks for one type spread across many files.

**Detection in this repo**
- `instruments/inventory.md` (top files; 91 functions of 250 lines or more,
  for example `upload_gpu_data` at `crates/rs_cam_viz/src/app/gpu_upload.rs:155`
  and `calculate` at `crates/rs_cam_core/src/feeds/mod.rs:1493`).
- `rg -n "allow\(clippy::(too_many_arguments|too_many_lines|type_complexity|cognitive_complexity)" -g '*.rs' crates`
- `rg -n "^\s{20,}(if|match|for|while) " -g '*.rs' crates/rs_cam_core/src --count-matches | sort -t: -k2 -nr | head`
- `rg -n "^impl \w+ \{" -g '*.rs' crates -o | sort | uniq -c | sort -nr | head`
  (types with many `impl` blocks).

**Type-level remedies**
- A parameter struct in place of a long argument list (it can carry the
  invariant).
- A named type alias or struct for a complex type.
- Split functions at stage boundaries; give each stage an input and an output
  type.
- Turn on `clippy::too_many_lines` (pedantic) and `clippy::cognitive_complexity`
  (restriction) with thresholds in `clippy.toml`.

**Sources**: [clippy lint list](https://rust-lang.github.io/rust-clippy/master/index.html),
[Compose structs](https://rust-unofficial.github.io/patterns/patterns/structural/compose-structs.html),
[C-INTERMEDIATE](https://rust-lang.github.io/api-guidelines/flexibility.html).

## 11. Navigability and documentation debt (`navigability-docs`)

**Definition.** A new contributor (human or agent) cannot find where a
behaviour lives, or the docs point to the wrong place. Instruction files,
module docs or comments disagree with the code.

**Rust signals**
- Folder `CLAUDE.md` file maps that name files or symbols that do not exist.
- Doc comments that name a parameter, a unit or a default that the code does
  not have.
- `lib.rs` / `mod.rs` without a `//!` module doc.
- Names that do not match the domain term (two names for one concept).
- Stale `planning/…` paths in code comments.
- Public items with no doc comment (`missing_docs`).
- Test file names that do not say what they guard.

**Detection in this repo**
- For each path in `instruments/claude_md_paths.txt`, extract the backticked
  paths and symbols, then check each with `ls` / `rg -n "fn <name>|struct <name>"`.
- `rg -L "^//!" crates/*/src/**/mod.rs crates/*/src/lib.rs`
- `rg -n "planning/[A-Za-z0-9_./-]+" -o -g '*.rs' crates | sort -u` then check
  each path exists. Note: `planning/CLAUDE.md` says the 2026-09-17 purge left
  such citations on purpose; `git show planning-pre-purge-2026-09-17:<path>`
  retrieves them. Report a missing path as L impact only.
- `rg -n "TODO|FIXME|HACK|XXX" -g '*.rs' crates` (6 at baseline).
- Check `FEATURE_CATALOG.md` claims against code (in flux).

**Type-level remedies**
- `#![warn(missing_docs)]` per crate; `rustdoc::broken_intra_doc_links` turns
  a stale `[`Item`]` link into a build error.
- Intra-doc links (C-LINK) in place of plain names.
- A sentry that checks each `CLAUDE.md` path exists (SENTRY, better than DOC).

**Sources**: [API Guidelines: Documentation](https://rust-lang.github.io/api-guidelines/documentation.html),
[Li et al. 2015 (documentation debt)](https://research.vu.nl/en/publications/a-systematic-mapping-study-on-technical-debt-and-its-management/),
[rustdoc lints](https://doc.rust-lang.org/rustdoc/lints.html).

## 12. Test debt (`test-debt`)

**Definition.** Tests are missing for an invariant, test the wrong thing,
are slow or brittle, or leak into the product API. A sentry guards a rule that
a type could guard.

**Rust signals**
- `#[ignore]` without a reason; tests that only print.
- Sentries that read source text (`include_str!` of a `.rs` file) to forbid a
  pattern: a type or lint can often replace them.
- `pub` items or features that exist only for tests (`test-support`).
- Tests that assert on exact floats from a run, not on a derived bound.
- Large test files that copy fixture setup (no shared builder).
- Archived harnesses that still compile.

**Detection in this repo**
- `rg -n "#\[ignore\]" -g '*.rs' crates`
- `rg -l "include_str!\(\"[^\"]*\.rs\"|read_to_string\([^)]*\.rs" crates/*/tests`
- `instruments/test_only_pub_api.md`;
  `rg -n "cfg\(feature = \"test-support\"\)" -g '*.rs' crates/rs_cam_core/src`
- `rg -n "assert_eq!\([^,]+, [0-9]+\.[0-9]{4,}" -g '*.rs' crates`
- `crates/rs_cam_core/tests/ARCHIVED_HARNESSES.md` versus the files present.
- `crates/rs_cam_viz/src/compute/worker/tests.rs` (3476 lines) for fixture
  duplication.

**Type-level remedies**
- Replace a source-scan sentry with a type (private constructor, sealed
  trait, visibility) when possible; keep the sentry only for what types
  cannot express.
- A shared fixture builder under `test-support`.
- `#[ignore = "reason"]`.

**Sources**: [Li et al. 2015 (test debt)](https://research.vu.nl/en/publications/a-systematic-mapping-study-on-technical-debt-and-its-management/),
[TD prioritization SLR](https://arxiv.org/pdf/1904.12538).

## 13. Enforcement gaps: rules held by prose or suppressions (`enforcement-gap`)

**Definition.** A rule that the project depends on lives only in a
`CLAUDE.md`, a comment or a convention (grade DOC / CONVENTION / NONE). Or a
lint suppression weakens a rule that the lint policy holds. This is the
operator's main lens: the next agent without context breaks a weak rule
silently.

**Rust signals**
- `CLAUDE.md` lines with "never", "always", "must", "only", "do not" that no
  type, lint or sentry enforces.
- `#[allow(clippy::…)]` without a `// SAFETY:` line (the repo rule).
- `#[allow(...)]` on a whole module or crate.
- Comments like "callers must …", "call X before Y", "keep in sync with".
- Design lints that would catch a known defect class but are not on.

**Detection in this repo**
- For each file in `instruments/claude_md_paths.txt`:
  `rg -n -i "\b(never|always|must|only|do not|don't)\b" <file>`, then grade
  each rule: find the type, lint or sentry that enforces it.
- `instruments/allows_without_safety.md`.
- `rg -n "#!\[allow" -g '*.rs' crates`
- `rg -n -i "keep in sync|must be called|call .* before|callers must|do not call" -g '*.rs' crates`
- Compare `Cargo.toml` `[workspace.lints]` with candidate lints:
  `fn_params_excessive_bools`, `struct_excessive_bools`, `match_same_arms`,
  `match_wildcard_for_single_variants`, `wildcard_enum_match_arm`,
  `must_use_candidate`, `return_self_not_must_use`, `missing_errors_doc`,
  `float_cmp`, `cast_possible_truncation`, `as_conversions`,
  `allow_attributes_without_reason`, `unreachable_pub`.
- 702 `_ =>` arms at baseline: each is a place where a new enum variant
  compiles silently.

**Type-level remedies**
- Move the rule into a type (newtype, private constructor, typestate, sealed
  trait, exhaustive `match` with no `_` arm, `#[must_use]`).
- If a type cannot hold it, add a lint; then a sentry; the doc line stays as a
  pointer only.
- `#[expect(lint, reason = "…")]` in place of `#[allow]`, with
  `clippy::allow_attributes_without_reason`.

**Sources**: [C-SEALED](https://rust-lang.github.io/api-guidelines/future-proofing.html),
[Typestate](https://cliffle.com/blog/rust-typestate/),
[clippy lint list](https://rust-lang.github.io/rust-clippy/master/index.html),
[Rust patterns: anti-patterns](https://rust-unofficial.github.io/patterns/anti_patterns/index.html).

---

## Severity rubric

Score each finding on two axes, then read the severity from the table.

**Impact**
- H: a wrong number reaches G-code, a surface, or a machine (safety, part
  quality), or two surfaces disagree.
- M: a likely silent regression for the next agent, or a large cost to change
  or to read.
- L: local cost to read or change; no correctness risk.

**Enforcement grade** (what holds the rule now)
- Strong: TYPES.
- Medium: LINT, SENTRY.
- Weak: DOC, CONVENTION, NONE.

| Impact \ Grade | Weak | Medium | Strong |
|---|---|---|---|
| H | S1 | S2 | S3 |
| M | S2 | S3 | S4 |
| L | S3 | S4 | S4 |

- S1: fix in this programme; the fix moves the rule to TYPES where possible.
- S2: fix in this programme, or record a planned package.
- S3: batch with related work.
- S4: record only.

Raise one level if the item is on a hot change path
(`instruments/churn_since_2026-09-18.txt`). Mark "in flux" items; do not
lower their severity, but do not assign a fix until the other session lands.
