use std::path::{Path, PathBuf};

/// A saved preset: operation label + serialized params as TOML string.
#[derive(Debug, Clone)]
pub struct Preset {
    pub name: String,
    pub operation_label: String,
    pub toml_content: String,
}

/// Get the presets directory path (~/.rs_cam/presets/).
/// Creates the directory if it doesn't exist.
pub fn presets_dir() -> PathBuf {
    let base = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    let dir = base.join(".rs_cam").join("presets");
    if !dir.exists() {
        let _ = std::fs::create_dir_all(&dir);
    }
    dir
}

/// List all available presets (reads .toml files from presets dir).
pub fn list_presets() -> Vec<Preset> {
    let dir = presets_dir();
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut presets = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("toml")
            && let Ok(preset) = load_preset_from_path(&path)
        {
            presets.push(preset);
        }
    }
    presets.sort_by(|a, b| a.name.cmp(&b.name));
    presets
}

/// Save a preset to disk.
pub fn save_preset(name: &str, operation_label: &str, toml_content: &str) -> Result<(), String> {
    let dir = presets_dir();
    let filename = sanitize_filename(name);
    let path = dir.join(format!("{}.toml", filename));

    let file_content = format!(
        "name = \"{}\"\noperation = \"{}\"\ncontent = \"\"\"\n{}\n\"\"\"\n",
        escape_toml_string(name),
        escape_toml_string(operation_label),
        toml_content,
    );

    std::fs::write(&path, file_content)
        .map_err(|e| format!("Failed to save preset '{}': {}", name, e))
}

/// Load a preset from disk by name.
pub fn load_preset(name: &str) -> Result<Preset, String> {
    let dir = presets_dir();
    let filename = sanitize_filename(name);
    let path = dir.join(format!("{}.toml", filename));

    if !path.exists() {
        return Err(format!("Preset '{}' not found", name));
    }

    load_preset_from_path(&path)
}

/// Delete a preset by name.
pub fn delete_preset(name: &str) -> Result<(), String> {
    let dir = presets_dir();
    let filename = sanitize_filename(name);
    let path = dir.join(format!("{}.toml", filename));

    if !path.exists() {
        return Err(format!("Preset '{}' not found", name));
    }

    std::fs::remove_file(&path).map_err(|e| format!("Failed to delete preset '{}': {}", name, e))
}

/// Parse a preset from a .toml file at the given path.
fn load_preset_from_path(path: &Path) -> Result<Preset, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read '{}': {}", path.display(), e))?;

    let name = extract_field(&raw, "name")
        .ok_or_else(|| format!("Missing 'name' in '{}'", path.display()))?;
    let operation = extract_field(&raw, "operation")
        .ok_or_else(|| format!("Missing 'operation' in '{}'", path.display()))?;
    let content = extract_multiline_field(&raw, "content")
        .ok_or_else(|| format!("Missing 'content' in '{}'", path.display()))?;

    Ok(Preset {
        name,
        operation_label: operation,
        toml_content: content,
    })
}

/// Extract a simple `key = "value"` field from TOML text.
fn extract_field(text: &str, key: &str) -> Option<String> {
    let prefix = format!("{} = \"", key);
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&prefix) {
            // Strip prefix and trailing quote
            let value = &trimmed[prefix.len()..];
            if let Some(end) = value.rfind('"') {
                return Some(unescape_toml_string(&value[..end]));
            }
        }
    }
    None
}

/// Extract a multi-line `key = """..."""` field from TOML text.
fn extract_multiline_field(text: &str, key: &str) -> Option<String> {
    let start_marker = format!("{} = \"\"\"", key);
    let end_marker = "\"\"\"";

    let start_pos = text.find(&start_marker)?;
    let after_marker = start_pos + start_marker.len();
    let remaining = &text[after_marker..];

    let end_pos = remaining.find(end_marker)?;
    let content = &remaining[..end_pos];

    // Strip leading/trailing newlines that are part of the TOML multi-line format
    let content = content.strip_prefix('\n').unwrap_or(content);
    let content = content.strip_suffix('\n').unwrap_or(content);

    Some(content.to_string())
}

/// Sanitize a preset name into a safe filename (lowercase, spaces to hyphens).
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else if c == ' ' {
                '-'
            } else {
                '_'
            }
        })
        .collect()
}

/// Escape characters that are special in TOML basic strings.
fn escape_toml_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Unescape TOML basic string escape sequences.
fn unescape_toml_string(s: &str) -> String {
    s.replace("\\\"", "\"").replace("\\\\", "\\")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

    /// Create a temporary presets directory for testing.
    fn temp_presets_dir() -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir()
            .join("rs_cam_presets_test")
            .join(format!("{}_{}", std::process::id(), id));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("Hardwood Roughing"), "hardwood-roughing");
        assert_eq!(sanitize_filename("my/preset"), "my_preset");
        assert_eq!(sanitize_filename("test-preset_1"), "test-preset_1");
    }

    #[test]
    fn test_escape_unescape_roundtrip() {
        let original = r#"value with "quotes" and \ backslash"#;
        let escaped = escape_toml_string(original);
        let unescaped = unescape_toml_string(&escaped);
        assert_eq!(unescaped, original);
    }

    #[test]
    fn test_extract_field() {
        let text = "name = \"Hardwood Roughing\"\noperation = \"Pocket\"\n";
        assert_eq!(
            extract_field(text, "name"),
            Some("Hardwood Roughing".to_string())
        );
        assert_eq!(extract_field(text, "operation"), Some("Pocket".to_string()));
        assert_eq!(extract_field(text, "missing"), None);
    }

    #[test]
    fn test_extract_multiline_field() {
        let text = "name = \"test\"\ncontent = \"\"\"\nstepover = 2.0\ndepth = 3.0\n\"\"\"\n";
        let content = extract_multiline_field(text, "content");
        assert_eq!(content, Some("stepover = 2.0\ndepth = 3.0".to_string()));
    }

    #[test]
    fn test_save_and_load_preset() {
        let dir = temp_presets_dir();
        let filename = sanitize_filename("Test Preset");
        let path = dir.join(format!("{}.toml", filename));

        let toml_content = "stepover = 2.0\ndepth = 3.0";
        let file_content = format!(
            "name = \"{}\"\noperation = \"{}\"\ncontent = \"\"\"\n{}\n\"\"\"\n",
            escape_toml_string("Test Preset"),
            escape_toml_string("Pocket"),
            toml_content,
        );
        std::fs::write(&path, file_content).expect("write failed");

        let preset = load_preset_from_path(&path).expect("load failed");
        assert_eq!(preset.name, "Test Preset");
        assert_eq!(preset.operation_label, "Pocket");
        assert_eq!(preset.toml_content, toml_content);

        // Cleanup
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_list_presets_empty_dir() {
        let dir = temp_presets_dir();
        let entries = std::fs::read_dir(&dir)
            .map(|rd| rd.flatten().count())
            .unwrap_or(0);
        assert_eq!(entries, 0);

        // Cleanup
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_load_preset_not_found() {
        let result = load_preset("nonexistent_preset_xyz_999");
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_preset_not_found() {
        let result = delete_preset("nonexistent_preset_xyz_999");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_field_with_escaped_quotes() {
        let text = "name = \"preset with \\\"quotes\\\"\"\n";
        let name = extract_field(text, "name");
        assert_eq!(name, Some("preset with \"quotes\"".to_string()));
    }
}
