//! The directory mechanics the two on-disk TOML libraries share.
//!
//! [`crate::io::tool_library`] and [`crate::io::machine_library`] both store named
//! records as `<name>.toml` in one directory, and both grew the same three
//! pieces: the name validator, the `dir.join("<name>.toml")` builder, and the
//! `read_dir` walk that lists the file stems. Their `rename_in` was the same
//! code as well, differing only in which error enum it named.
//!
//! This module is the one copy of the mechanism. Everything that is a
//! decision stays with the library that makes it: its directory resolution
//! (the two read different environment variables), its record type, its
//! serialisation, and its error enum — whose messages name what the library
//! stores ("catalog", "machine"), so the two must stay two. [`LibraryError`]
//! is how a library hands its own constructors to the shared code.

use std::path::{Path, PathBuf};

/// The three errors the shared directory mechanics can raise, built by the
/// library that owns the message text.
pub(crate) trait LibraryError: Sized {
    /// The name has a path separator, a `..`, or is empty.
    fn invalid_name(name: &str) -> Self;
    /// A rename target that is already on disk.
    fn already_exists(name: &str) -> Self;
    /// A filesystem call that failed, attributed to `name`.
    fn io(name: &str, source: std::io::Error) -> Self;
}

/// True for a name that can become a file stem in the library directory.
///
/// Rejects the empty name, both path separators and any `..`, so a library
/// name can never address a file outside its own directory.
pub(crate) fn name_is_valid(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && name != ".."
        && !name.contains("..")
}

/// The path of the record `name` in `dir`, or [`LibraryError::invalid_name`].
pub(crate) fn path_in<E: LibraryError>(dir: &Path, name: &str) -> Result<PathBuf, E> {
    if !name_is_valid(name) {
        return Err(E::invalid_name(name));
    }
    Ok(dir.join(format!("{name}.toml")))
}

/// The record names (file stems) in `dir`, sorted. A missing directory
/// returns an empty list, not an error: a library nobody has written to yet
/// is empty, not broken.
pub(crate) fn list_in(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            if path.extension().and_then(|s| s.to_str()) != Some("toml") {
                return None;
            }
            path.file_stem().and_then(|s| s.to_str()).map(str::to_owned)
        })
        .collect();
    names.sort();
    names
}

/// Rename record `old` to `new` in `dir`. Refuses when `new` is already on
/// disk, so a rename never silently destroys a record.
pub(crate) fn rename_in<E: LibraryError>(dir: &Path, old: &str, new: &str) -> Result<(), E> {
    let old_path = path_in::<E>(dir, old)?;
    let new_path = path_in::<E>(dir, new)?;
    if new_path.exists() {
        return Err(E::already_exists(new));
    }
    std::fs::rename(&old_path, &new_path).map_err(|source| E::io(old, source))?;
    Ok(())
}
