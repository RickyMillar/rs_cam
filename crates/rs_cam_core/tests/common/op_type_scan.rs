//! The scan of the P29-last guard (`no_limit_reads_the_op_type_p29_last`):
//! the reads of an operation type, a family, a pass role or a depth rule in
//! a limit or metric file.
//!
//! It is not declared in `common/mod.rs`. A guard includes it beside
//! `source_scan.rs`:
//!
//! ```text
//! #[path = "common/source_scan.rs"] mod source_scan;
//! #[path = "common/op_type_scan.rs"] mod op_type_scan;
//! ```
//!
//! The scan reads code only. It removes the comments (`source_scan`), the
//! text of each string and char literal, each `#[cfg(test)]` item and each
//! `use` declaration. It then finds each token of [`TOKENS`] as a whole
//! word, and names the function that holds it (the innermost `fn`), or the
//! item for a hit outside a function.

#![allow(
    dead_code,
    reason = "each guard binary that includes this module uses only part of it"
)]

use std::collections::BTreeMap;
use std::path::Path;

/// The tokens of a read of the op type (`P29-11_DESIGN.md` section 5.1).
/// `OperationConfig::` counts only before a variant (an upper-case name).
/// P29-11 L-3 adds four tokens that section 5.1 does not name: the registry
/// spec of the op (`.spec()`, its fields `feeds_family` and
/// `feeds_pass_role`) and the import alias `FeedsOperationFamily`. Without
/// them, `op.spec().feeds_family == ..` is a read of the family that the
/// scan does not see.
pub const TOKENS: [&str; 17] = [
    "OperationType",
    "OperationFamily",
    "PassRole",
    "UiProcessRole",
    "OperationConfig::",
    "op_type()",
    "feeds_style()",
    "pass_role",
    ".role()",
    "depth_rule",
    "body_depth_limit",
    "depth_cap_of",
    ".capabilities()",
    ".spec()",
    "feeds_family",
    "feeds_pass_role",
    "FeedsOperationFamily",
];

/// The class of an allow entry (`P29-11_DESIGN.md` section 5.2). The list
/// is closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// The vendor row key (row `M6`, P25 / P27).
    V,
    /// The write funnel and the op draft.
    W,
    /// The declaration: `cuts()`, `ConfiguredEntry`, the registry
    /// `Capabilities`, the move model, the precondition dispatch.
    D,
    /// Generation planning.
    G,
    /// A quality hint by role (not a limit).
    Q,
    /// A label or a report field.
    T,
    /// A scope read that rows of `limits::EXEMPTIONS` hold. The entry names
    /// the rows and an exact count.
    X {
        /// The row ids (one or more).
        rows: &'static [&'static str],
        /// The exact count of hits in the function.
        count: usize,
    },
}

/// One allow entry: the file (relative to the scan root), the function
/// (or the item, for a hit outside a function) and the class.
#[derive(Debug, Clone, Copy)]
pub struct Allow {
    pub file: &'static str,
    pub function: &'static str,
    pub class: Class,
}

/// The code of `src` that the scan reads: comments, the text of string and
/// char literals, `#[cfg(test)]` items and `use` declarations removed. The
/// line count stays.
#[must_use]
pub fn code_of(src: &str) -> String {
    let no_comments = crate::source_scan::strip_comments(src);
    let no_strings = blank_literals(&no_comments);
    let no_tests = cut_cfg_test_items(&no_strings);
    cut_use_declarations(&no_tests)
}

/// Replace the text of each string and char literal by spaces (newlines
/// stay). The quotes stay, so `"a" == b` keeps its shape.
fn blank_literals(text: &str) -> String {
    let c: Vec<char> = text.chars().collect();
    let n = c.len();
    let mut out: Vec<char> = Vec::with_capacity(n);
    let blank = |ch: char| if ch == '\n' { '\n' } else { ' ' };
    let mut i = 0;
    while i < n {
        let ident_before = i > 0 && (c[i - 1].is_alphanumeric() || c[i - 1] == '_');
        // A raw string: r"..", r#".."#, br"..".
        if c[i] == 'r' && !ident_before && i + 1 < n && (c[i + 1] == '"' || c[i + 1] == '#') {
            let mut j = i + 1;
            let mut hashes = 0;
            while j < n && c[j] == '#' {
                hashes += 1;
                j += 1;
            }
            if j < n && c[j] == '"' {
                out.extend(&c[i..=j]);
                j += 1;
                while j < n {
                    if c[j] == '"' && (0..hashes).all(|k| c.get(j + 1 + k) == Some(&'#')) {
                        break;
                    }
                    out.push(blank(c[j]));
                    j += 1;
                }
                let end = (j + 1 + hashes).min(n);
                out.extend(&c[j.min(n)..end]);
                i = end;
                continue;
            }
        }
        if c[i] == '"' {
            out.push('"');
            let mut j = i + 1;
            while j < n && c[j] != '"' {
                if c[j] == '\\' && j + 1 < n {
                    out.push(' ');
                    out.push(blank(c[j + 1]));
                    j += 2;
                } else {
                    out.push(blank(c[j]));
                    j += 1;
                }
            }
            if j < n {
                out.push('"');
            }
            i = j + 1;
            continue;
        }
        // A char literal; a lifetime has no closing quote.
        if c[i] == '\'' {
            if i + 3 < n && c[i + 1] == '\\' {
                if let Some(k) = (i + 2..n.min(i + 12)).find(|&k| c[k] == '\'') {
                    out.push('\'');
                    out.extend(std::iter::repeat_n(' ', k - i - 1));
                    out.push('\'');
                    i = k + 1;
                    continue;
                }
            } else if i + 2 < n && c[i + 2] == '\'' {
                out.extend(['\'', ' ', '\'']);
                i += 3;
                continue;
            }
        }
        out.push(c[i]);
        i += 1;
    }
    out.into_iter().collect()
}

/// The end (exclusive, byte offset) of the item that starts at `from`: the
/// first `;` before any `{`, or the brace that closes the first `{`.
fn item_end(text: &str, from: usize) -> usize {
    let bytes = text.as_bytes();
    let mut depth = 0_i32;
    let mut group = 0_i32;
    let mut k = from;
    while k < bytes.len() {
        match bytes[k] {
            b'(' | b'[' => group += 1,
            b')' | b']' => group -= 1,
            b';' if depth == 0 && group <= 0 => return k + 1,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return k + 1;
                }
            }
            _ => {}
        }
        k += 1;
    }
    bytes.len()
}

/// Blank the range `[from, to)` of `text`, newlines kept.
fn blank_range(text: &mut [u8], from: usize, to: usize) {
    for b in text.iter_mut().take(to).skip(from) {
        if *b != b'\n' {
            *b = b' ';
        }
    }
}

/// Remove each item under a `#[cfg(test)]` attribute (a test module, a
/// test helper), at any depth.
fn cut_cfg_test_items(text: &str) -> String {
    let mut out = text.as_bytes().to_vec();
    let mut from = 0;
    while let Some(at) = text[from..].find("#[cfg(test)]") {
        let start = from + at;
        let after_attr = start + "#[cfg(test)]".len();
        let end = item_end(text, after_attr);
        blank_range(&mut out, start, end);
        from = end.max(after_attr);
    }
    String::from_utf8(out).unwrap_or_default()
}

/// Remove each `use` declaration: an import is not a read.
fn cut_use_declarations(text: &str) -> String {
    let mut out = text.as_bytes().to_vec();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let lead = line.len() - trimmed.len();
        for vis in ["use ", "pub use ", "pub(crate) use ", "pub(super) use "] {
            if trimmed.starts_with(vis) {
                let start = offset + lead;
                let end = item_end(text, start);
                blank_range(&mut out, start, end);
                break;
            }
        }
        offset += line.len();
    }
    String::from_utf8(out).unwrap_or_default()
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// The byte offsets of each whole-word hit of `token` in `code`.
fn token_hits(code: &str, token: &str) -> Vec<usize> {
    let bytes = code.as_bytes();
    let first_ident = token.as_bytes().first().copied().is_some_and(is_ident);
    let last_ident = token.as_bytes().last().copied().is_some_and(is_ident);
    code.match_indices(token)
        .filter(|&(at, _)| {
            let before_ok = !first_ident || at == 0 || !is_ident(bytes[at - 1]);
            let end = at + token.len();
            let after_ok = !last_ident || end >= bytes.len() || !is_ident(bytes[end]);
            // `OperationConfig::` counts only before a variant.
            let variant_ok =
                token != "OperationConfig::" || bytes.get(end).is_some_and(u8::is_ascii_uppercase);
            // A definition is not a read: `fn depth_rule`, a field
            // `depth_rule: DepthLimit` or a field init `depth_rule: value`.
            let fn_def = ["fn ", "enum ", "struct ", "trait "]
                .iter()
                .any(|k| code[..at].ends_with(k));
            let field = bytes.get(end) == Some(&b':') && bytes.get(end + 1) != Some(&b':');
            before_ok && after_ok && variant_ok && !fn_def && !field
        })
        .map(|(at, _)| at)
        .collect()
}

/// One function or item span: its name and its byte range.
struct Span {
    name: String,
    start: usize,
    end: usize,
}

/// The spans of each `fn` (from `fn` to the brace that closes its body)
/// and of each top-level item.
fn spans(code: &str) -> (Vec<Span>, Vec<Span>) {
    let bytes = code.as_bytes();
    let mut fns = Vec::new();
    for (at, _) in code.match_indices("fn ") {
        if at > 0 && is_ident(bytes[at - 1]) {
            continue;
        }
        let name: String = code[at + 3..]
            .chars()
            .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
            .collect();
        if name.is_empty() {
            continue;
        }
        // The body: the first `{` after the name, unless a `;` comes first
        // (a trait method with no body).
        let rest = &code[at..];
        let Some(open) = rest.find(['{', ';']) else {
            continue;
        };
        if rest.as_bytes()[open] == b';' {
            continue;
        }
        let end = item_end(code, at + open);
        fns.push(Span {
            name,
            start: at,
            end,
        });
    }
    let mut items = Vec::new();
    let mut offset = 0;
    let lines: Vec<&str> = code.split_inclusive('\n').collect();
    for (k, line) in lines.iter().enumerate() {
        let first = line.as_bytes().first().copied();
        if first.is_some_and(|b| !b.is_ascii_whitespace() && b != b'}' && b != b')') {
            // An attribute takes the name of the item it decorates.
            let name = lines
                .iter()
                .skip(k)
                .find(|l| !l.trim_start().starts_with("#[") && !l.trim().is_empty())
                .map_or_else(|| item_name(line), |l| item_name(l));
            items.push(Span {
                name,
                start: offset,
                end: item_end(code, offset),
            });
        }
        offset += line.len();
    }
    (fns, items)
}

/// The name of the item a column-0 line starts: `impl <type>`, or the name
/// after `fn`, `struct`, `enum`, `const`, `static`, `type`, `trait`, `mod`.
fn item_name(line: &str) -> String {
    let mut rest = line.trim();
    for vis in ["pub(crate) ", "pub(super) ", "pub "] {
        if let Some(r) = rest.strip_prefix(vis) {
            rest = r;
        }
    }
    for k in [
        "const fn ",
        "fn ",
        "struct ",
        "enum ",
        "const ",
        "static ",
        "type ",
        "trait ",
        "mod ",
    ] {
        if let Some(after) = rest.strip_prefix(k) {
            let name: String = after
                .chars()
                .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
                .collect();
            return name;
        }
    }
    if rest.starts_with("impl") {
        let head: String = rest
            .chars()
            .take_while(|ch| *ch != '{' && *ch != '\n')
            .collect();
        return head.trim().to_owned();
    }
    rest.chars().take(40).collect()
}

/// One hit: the function (or item) and the line.
#[derive(Debug, Clone)]
pub struct Hit {
    pub function: String,
    pub line: usize,
    pub token: &'static str,
}

/// The hits of the source `src`.
#[must_use]
pub fn hits(src: &str) -> Vec<Hit> {
    let code = code_of(src);
    let (fns, items) = spans(&code);
    let mut out = Vec::new();
    for token in TOKENS {
        for at in token_hits(&code, token) {
            let inner = fns
                .iter()
                .filter(|s| s.start <= at && at < s.end)
                .min_by_key(|s| s.end - s.start);
            // The getter of a fact (`fn depth_rule(&self)`) reads its own
            // field: a definition, not a read.
            if inner.is_some_and(|f| {
                f.name == token.trim_matches(|c: char| !c.is_alphanumeric() && c != '_')
            }) {
                continue;
            }
            let function = match inner {
                Some(s) => s.name.clone(),
                None => items
                    .iter()
                    .filter(|s| s.start <= at && at < s.end)
                    .min_by_key(|s| s.end - s.start)
                    .map_or_else(|| "?".to_owned(), |s| s.name.clone()),
            };
            out.push(Hit {
                function,
                line: code[..at].matches('\n').count() + 1,
                token,
            });
        }
    }
    out.sort_by_key(|h| h.line);
    out
}

/// The hit counts of each `(file, function)` of the files under `root`
/// whose relative path `guarded` accepts. Test files (`tests.rs`,
/// `*_tests.rs`, a `tests/` folder) are not read.
#[must_use]
pub fn counts(root: &Path, guarded: impl Fn(&str) -> bool) -> BTreeMap<(String, String), Vec<Hit>> {
    let mut out: BTreeMap<(String, String), Vec<Hit>> = BTreeMap::new();
    for path in crate::source_scan::rust_files(root) {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if rel.ends_with("/tests.rs")
            || rel == "tests.rs"
            || rel.ends_with("_tests.rs")
            || rel.contains("/tests/")
            || rel.starts_with("tests/")
            || !guarded(&rel)
        {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap_or_default();
        for hit in hits(&src) {
            out.entry((rel.clone(), hit.function.clone()))
                .or_default()
                .push(hit);
        }
    }
    out
}

/// The failures of the hit table against the allow-list: a hit with no
/// entry, a hit in a zero-tolerance file, an X count that differs, an
/// entry with no hit, and an X total above its pin. Each failure is one
/// line that tells the author what to do.
#[must_use]
pub fn failures(
    table: &BTreeMap<(String, String), Vec<Hit>>,
    allow: &[Allow],
    zero_tolerance: impl Fn(&str) -> bool,
    x_total_pin: usize,
) -> Vec<String> {
    let mut out = Vec::new();
    let what_to_do = "declare the fact of the cut in `OperationConfig::cuts` \
                      (`compute/catalog/cuts.rs`) or in the registry `Capabilities`, \
                      and read the fact; or, for a scope read that an exemption row holds, \
                      add an X entry with the row id and the exact count";
    for ((file, function), hits) in table {
        let lines: Vec<String> = hits
            .iter()
            .map(|h| format!("{}:{}", h.line, h.token))
            .collect();
        if zero_tolerance(file) {
            out.push(format!(
                "{file} `{function}`: {} read(s) of the op type ({}) in a limit file with \
                 zero tolerance; {what_to_do} (no entry may name this file)",
                hits.len(),
                lines.join(", ")
            ));
            continue;
        }
        let Some(entry) = allow
            .iter()
            .find(|a| a.file == file && a.function == function)
        else {
            out.push(format!(
                "{file} `{function}`: {} read(s) of the op type ({}) with no allow entry; \
                 {what_to_do}",
                hits.len(),
                lines.join(", ")
            ));
            continue;
        };
        if let Class::X { rows, count } = entry.class
            && hits.len() != count
        {
            out.push(format!(
                "{file} `{function}` (X, rows {}): {} hit(s), the entry says {count}. \
                 A new scope read: {what_to_do}. A removed read: lower the count and \
                 the X total in the same commit",
                rows.join(", "),
                hits.len()
            ));
        }
    }
    let mut x_total = 0;
    for a in allow {
        if zero_tolerance(a.file) {
            out.push(format!(
                "{} `{}`: an entry names a zero-tolerance file; remove it",
                a.file, a.function
            ));
        }
        if let Class::X { count, .. } = a.class {
            x_total += count;
        }
        if !table.contains_key(&(a.file.to_owned(), a.function.to_owned())) {
            out.push(format!(
                "{} `{}`: the entry has no hit; remove it (the list only shrinks)",
                a.file, a.function
            ));
        }
    }
    if x_total > x_total_pin {
        out.push(format!(
            "the X entries hold {x_total} hits, above the pin {x_total_pin}; the pin only \
             goes down"
        ));
    } else if x_total < x_total_pin {
        out.push(format!(
            "the X entries hold {x_total} hits, below the pin {x_total_pin}; lower the pin \
             to {x_total} (it only goes down)"
        ));
    }
    out
}

/// The table as text, one `(file, function): count` line per entry, for a
/// failure message and for the author who writes the allow-list.
#[must_use]
pub fn table_text(table: &BTreeMap<(String, String), Vec<Hit>>) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for ((file, function), hits) in table {
        let tokens: Vec<&str> = hits.iter().map(|h| h.token).collect();
        let _ = writeln!(
            out,
            "{file} | {function} | {} | {}",
            hits.len(),
            tokens.join(" ")
        );
    }
    out
}
