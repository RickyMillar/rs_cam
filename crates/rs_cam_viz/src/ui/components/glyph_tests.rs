//! Every glyph that the kit and the inspector paint is in the loaded fonts.
//!
//! The operator, 2026-10-30: the menu act of the selection list painted
//! "□". Its text was U+22EF, and no font of `tokens::apply_fonts` has that
//! glyph. egui then paints the replacement box and no other gate sees it.
//!
//! The test reads the string literals of the source files of the kit
//! (`ui/components/`), the inspector (`ui/properties/`) and the token
//! module. It asks the real font set for each character that is not ASCII.

use crate::ui::tokens;

/// The source files that the test reads, with their path for the message.
fn sources() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui");
    let mut out = Vec::new();
    let mut dirs = vec![root.join("components"), root.join("properties")];
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "rs")
                && let Ok(text) = std::fs::read_to_string(&path)
            {
                out.push((path.display().to_string(), text));
            }
        }
    }
    let token_path = root.join("tokens.rs");
    if let Ok(text) = std::fs::read_to_string(&token_path) {
        out.push((token_path.display().to_string(), text));
    }
    out
}

/// The string literals of Rust source `text`, with the escapes `\u{..}`,
/// `\n`, `\"` and `\\` decoded. Comments, char literals and lifetimes are
/// not literals. A raw string `r"..."` or `r#"..."#` is a literal.
fn string_literals(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let at = |i: usize| chars.get(i).copied();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(c) = at(i) {
        match c {
            '/' if at(i + 1) == Some('/') => {
                while at(i).is_some_and(|c| c != '\n') {
                    i += 1;
                }
            }
            '/' if at(i + 1) == Some('*') => {
                i += 2;
                while at(i).is_some() && !(at(i) == Some('*') && at(i + 1) == Some('/')) {
                    i += 1;
                }
                i += 2;
            }
            '\'' => {
                // A char literal: 'x' or '\x..'. Else a lifetime.
                if at(i + 1) == Some('\\') {
                    i += 2;
                    while at(i).is_some_and(|c| c != '\'') {
                        i += 1;
                    }
                    i += 1;
                } else if at(i + 2) == Some('\'') {
                    i += 3;
                } else {
                    i += 1;
                }
            }
            'r' if (at(i + 1) == Some('"') || at(i + 1) == Some('#'))
                && !at(i.wrapping_sub(1)).is_some_and(|p| p.is_alphanumeric() || p == '_') =>
            {
                let mut j = i + 1;
                let mut hashes = 0;
                while at(j) == Some('#') {
                    hashes += 1;
                    j += 1;
                }
                if at(j) != Some('"') {
                    i += 1;
                    continue;
                }
                j += 1;
                let mut literal = String::new();
                while let Some(c) = at(j) {
                    if c == '"' && (1..=hashes).all(|k| at(j + k) == Some('#')) {
                        break;
                    }
                    literal.push(c);
                    j += 1;
                }
                out.push(literal);
                i = j + 1 + hashes;
            }
            '"' => {
                let mut j = i + 1;
                let mut literal = String::new();
                while let Some(c) = at(j) {
                    match c {
                        '"' => break,
                        '\\' => {
                            match at(j + 1) {
                                Some('u') if at(j + 2) == Some('{') => {
                                    let mut k = j + 3;
                                    let mut hex = String::new();
                                    while let Some(h) = at(k).filter(|h| *h != '}') {
                                        hex.push(h);
                                        k += 1;
                                    }
                                    if let Some(ch) =
                                        u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32)
                                    {
                                        literal.push(ch);
                                    }
                                    j = k + 1;
                                    continue;
                                }
                                // A line continuation: the newline and the
                                // indent after it are not text.
                                Some('\n') => {
                                    j += 2;
                                    while at(j).is_some_and(char::is_whitespace) {
                                        j += 1;
                                    }
                                    continue;
                                }
                                Some(other) => literal.push(other),
                                None => {}
                            }
                            j += 2;
                        }
                        other => {
                            literal.push(other);
                            j += 1;
                        }
                    }
                }
                out.push(literal);
                i = j + 1;
            }
            _ => i += 1,
        }
    }
    out
}

/// The parser reads literals, escapes and comments as the compiler does.
#[test]
fn the_literal_reader_skips_comments_and_decodes_escapes() {
    let src = "// \"comment\"\nlet a = \"x\\u{2026}y\"; let b: &'static str = r#\"\u{2715}\"#;\n\
               let c = '\"'; /* \"block\" */ let d = \"one \\\n    two\";";
    assert_eq!(
        string_literals(src),
        vec![
            "x\u{2026}y".to_owned(),
            "\u{2715}".to_owned(),
            "one two".to_owned()
        ]
    );
}

/// Every character that is not ASCII in a string literal of the kit, the
/// inspector or the token module has a glyph in each text family of
/// `tokens::apply_fonts`. Red proof: the menu act `MENU` was U+22EF.
#[test]
fn every_painted_glyph_is_in_the_loaded_fonts() {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    // The fonts load in the first pass.
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();

    let families = [
        egui::FontFamily::Proportional,
        egui::FontFamily::Name(tokens::FAMILY_MEDIUM.into()),
        egui::FontFamily::Name(tokens::FAMILY_SEMIBOLD.into()),
    ];
    let files = sources();
    assert!(files.len() > 40, "fixture: read {} files", files.len());
    let mut checked = std::collections::BTreeSet::new();
    let mut missing = Vec::new();
    for (path, text) in &files {
        for literal in string_literals(text) {
            for c in literal.chars().filter(|c| !c.is_ascii()) {
                if !checked.insert(c) {
                    continue;
                }
                for family in &families {
                    if !paints(&ctx, family, c) {
                        missing.push(format!("U+{:04X} {c:?} ({family:?}) in {path}", c as u32));
                    }
                }
            }
        }
    }
    // Non-vacuity: the menu glyph, the danger glyph and the suggest glyph.
    for c in ['\u{2026}', '\u{2715}', '\u{26A1}'] {
        assert!(checked.contains(&c), "fixture: {c:?} is not in a literal");
    }
    assert!(
        missing.is_empty(),
        "glyphs with no font (egui paints a box):\n{}",
        missing.join("\n")
    );
    // The menu act of a list is one of them; the old one paints a box.
    let proportional = egui::FontFamily::Proportional;
    assert!(
        super::list_box::MENU
            .chars()
            .all(|c| paints(&ctx, &proportional, c))
    );
    assert!(!paints(&ctx, &proportional, '\u{22EF}'), "red proof");
}

/// True when `family` paints `c` with a glyph of its own, not the
/// replacement box. epaint `Fonts::has_glyph` is not this test: it gives a
/// false "no" for each character of the face that also holds the
/// replacement glyph (NotoEmoji holds the suggest glyph U+26A1). The test
/// lays out `c` and the replacement character and compares the atlas
/// rectangles: a missing glyph paints the replacement rectangle.
fn paints(ctx: &egui::Context, family: &egui::FontFamily, c: char) -> bool {
    const REPLACEMENT: char = '\u{25FB}';
    let font = egui::FontId::new(tokens::SIZE_BODY, family.clone());
    let uv = |ch: char| {
        ctx.fonts_mut(|f| {
            f.layout_no_wrap(ch.to_string(), font.clone(), egui::Color32::WHITE)
                .rows
                .first()
                .and_then(|row| row.glyphs.first())
                .map(|glyph| glyph.uv_rect)
        })
    };
    c == REPLACEMENT || uv(c) != uv(REPLACEMENT)
}
