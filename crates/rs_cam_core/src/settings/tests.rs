// SAFETY: test module; a failed unwrap is a failed test.
#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::*;

const GIB: u64 = 1 << 30;

/// An environment lookup over fixed pairs. The tests never write the
/// process environment: `std::env::set_var` is `unsafe` in edition 2024.
fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |name: &str| {
        pairs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
    }
}

/// A scratch file path that is unique to this process and this test.
fn scratch(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "rs_cam_settings_{name}_{}.toml",
        std::process::id()
    ))
}

/// A folder of its own, empty, for a test that lists the folder.
fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rs_cam_settings_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Settings with every key away from its default.
fn every_key_changed() -> AppSettings {
    let mut overlays = BTreeMap::new();
    overlays.insert("grid".to_owned(), false);
    overlays.insert("height_planes".to_owned(), true);
    AppSettings {
        memory_limit: MemoryLimit::Bytes(24 * GIB),
        general: GeneralSettings {
            window_width: 1920.0,
            window_height: 1080.0,
            undo_depth: 250,
            toast_info_seconds: 3.5,
            toast_warning_seconds: 10.0,
            toast_error_seconds: 20.0,
            confirm_unsaved_quit: false,
        },
        display: DisplaySettings {
            show_all_toolpaths: true,
            toolpath_colour_mode: ToolpathColourDefault::AdvancePerTooth,
            overlays,
        },
        simulation: SimulationSettings {
            playback_speed: 2000.0,
            stock_view: StockViewDefault::ByHeight,
            stock_opacity: 0.5,
        },
        paths: PathSettings {
            tool_library: Some(PathBuf::from("/lib/tools")),
            machine_library: Some(PathBuf::from("/lib/machines")),
            screenshots: Some(PathBuf::from("/shots")),
        },
        diagnostics: DiagnosticsSettings {
            save_cut_trace: true,
            cut_trace_retain: 12,
            artifact_dir: Some(PathBuf::from("/artifacts")),
            log_level: LogLevel::Debug,
            present_mode: Some(PresentModeSetting::Mailbox),
        },
    }
}

/// The defaults are today's behaviour, and the cut-trace file is OFF
/// (operator ruling 2026-10-02).
#[test]
fn the_defaults_are_the_behaviour_before_the_keys_existed() {
    let d = AppSettings::default();
    assert_eq!(d.memory_limit, MemoryLimit::Default);
    assert_eq!([d.general.window_width, d.general.window_height], [1400.0, 900.0]);
    assert_eq!(d.general.undo_depth, 100);
    assert_eq!(
        [
            d.general.toast_info_seconds,
            d.general.toast_warning_seconds,
            d.general.toast_error_seconds
        ],
        [4.0, 6.0, 8.0]
    );
    assert!(d.general.confirm_unsaved_quit);
    assert!(!d.display.show_all_toolpaths, "WP27: selected only");
    assert_eq!(d.display.toolpath_colour_mode, ToolpathColourDefault::Normal);
    assert!(d.display.overlays.is_empty());
    assert_eq!(d.simulation.playback_speed, 500.0);
    assert_eq!(d.simulation.stock_view, StockViewDefault::Solid);
    assert_eq!(d.simulation.stock_opacity, 1.0);
    assert_eq!(d.paths, PathSettings::default());
    assert!(!d.diagnostics.save_cut_trace, "operator ruling: off");
    assert_eq!(d.diagnostics.cut_trace_retain, 5);
    assert_eq!(d.diagnostics.artifact_dir, None);
    assert_eq!(d.diagnostics.log_level, LogLevel::Info);
    assert_eq!(d.diagnostics.present_mode, None);
}

/// Every key goes through the writer and reads back to the same value.
#[test]
fn every_key_round_trips_through_the_writer() {
    let dir = scratch_dir("every_key");
    let path = dir.join("settings.toml");
    let settings = every_key_changed();
    save_to(&path, &settings).unwrap();
    let loaded = load_from(&path);
    assert_eq!(loaded.warning, None);
    assert_eq!(loaded.settings, settings);

    // Back to the defaults: only `[memory] limit` stays in the file.
    save_to(&path, &AppSettings::default()).unwrap();
    let loaded = load_from(&path);
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(loaded.settings, AppSettings::default());
    let table: toml::Table = text.parse().unwrap();
    assert_eq!(
        table.keys().collect::<Vec<_>>(),
        vec!["memory"],
        "a default value is not written: {text}"
    );
}

/// Each token reads back, in any case.
#[test]
fn every_token_reads_back() {
    fn check<T: SettingToken + std::fmt::Debug>() {
        for value in T::ALL {
            assert_eq!(T::from_token(value.token()), Some(*value));
            assert_eq!(
                T::from_token(&value.token().to_ascii_uppercase()),
                Some(*value)
            );
        }
        assert_eq!(T::from_token("no such value"), None);
    }
    check::<ToolpathColourDefault>();
    check::<StockViewDefault>();
    check::<LogLevel>();
    check::<PresentModeSetting>();
}

/// The writer keeps every key it does not own, and the loader reads the
/// written limit back. Each of the three kinds of limit.
#[test]
fn the_writer_keeps_unknown_keys_and_tables() {
    // A folder of its own, so the check for a temporary file that stays
    // sees no file of a parallel test.
    let dir = scratch_dir("round_trip");
    let path = dir.join("settings.toml");
    std::fs::write(
        &path,
        concat!(
            "top = \"kept\"\n\n",
            "[memory]\nlimit = \"3GiB\"\nother = 7\n\n",
            "[general]\nfuture_key = \"kept\"\n\n",
            "[display.overlays]\nfuture_overlay = true\n\n",
            "[diagnostics]\nnew_switch = 1\n\n",
            "[ui]\ntheme = \"dark\"\n",
        ),
    )
    .unwrap();
    let read = |table: &toml::Table, outer: &str, inner: Option<&str>| {
        let value = table.get(outer).unwrap();
        match inner {
            Some(key) => value.get(key).unwrap().clone(),
            None => value.clone(),
        }
    };
    for limit in [
        MemoryLimit::Bytes(24 * GIB),
        MemoryLimit::Bytes(3 * GIB / 2),
        MemoryLimit::Unlimited,
        MemoryLimit::Default,
    ] {
        // The window reads the file, changes keys, and writes it back.
        let mut settings = load_from(&path).settings;
        settings.memory_limit = limit;
        settings.general.undo_depth = 42;
        settings.diagnostics.save_cut_trace = true;
        save_to(&path, &settings).unwrap();
        let loaded = load_from(&path);
        assert_eq!(loaded.warning, None, "{limit:?}");
        assert_eq!(loaded.settings.memory_limit, limit);
        assert_eq!(loaded.settings.general.undo_depth, 42);
        assert!(loaded.settings.diagnostics.save_cut_trace);
        assert_eq!(
            loaded.settings.display.overlays.get("future_overlay"),
            Some(&true),
            "an overlay id this build does not know stays"
        );
        let table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
        assert_eq!(read(&table, "top", None).as_str(), Some("kept"));
        assert_eq!(read(&table, "memory", Some("other")).as_integer(), Some(7));
        assert_eq!(
            read(&table, "general", Some("future_key")).as_str(),
            Some("kept")
        );
        assert_eq!(
            read(&table, "diagnostics", Some("new_switch")).as_integer(),
            Some(1)
        );
        assert_eq!(read(&table, "ui", Some("theme")).as_str(), Some("dark"));
    }
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("limit = \"default\""), "{text}");
    // No temporary file stays beside the settings file.
    let names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(names, vec!["settings.toml".to_owned()]);
}

/// A missing folder and file give a new file.
#[test]
fn the_writer_makes_the_folder_and_the_file() {
    let dir = std::env::temp_dir().join(format!("rs_cam_settings_new_dir_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("nested").join("settings.toml");
    let settings = AppSettings {
        memory_limit: MemoryLimit::Bytes(12 * GIB),
        ..AppSettings::default()
    };
    save_to(&path, &settings).unwrap();
    let loaded = load_from(&path);
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(loaded.settings.memory_limit, MemoryLimit::Bytes(12 * GIB));
    assert_eq!(loaded.warning, None);
}

/// A file that is not TOML stays as it is: the writer refuses, so it never
/// deletes keys that it could not read. A bad VALUE in a valid file is
/// replaced.
#[test]
fn the_writer_refuses_a_file_that_is_not_toml() {
    let path = scratch("writer_not_toml");
    std::fs::write(&path, "not toml at all [").unwrap();
    let error = save_to(&path, &AppSettings::default()).unwrap_err();
    let after = std::fs::read_to_string(&path).unwrap();
    assert!(error.contains("not valid TOML"), "{error}");
    assert_eq!(after, "not toml at all [");

    std::fs::write(&path, "[memory]\nlimit = \"lots\"\n").unwrap();
    let settings = AppSettings {
        memory_limit: MemoryLimit::Bytes(GIB),
        ..AppSettings::default()
    };
    save_to(&path, &settings).unwrap();
    let loaded = load_from(&path);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(loaded.settings.memory_limit, MemoryLimit::Bytes(GIB));
}

#[test]
fn a_limit_parses_as_a_size_a_byte_count_or_unlimited() {
    let text = "[memory]\nlimit = \"12GiB\"\n";
    assert_eq!(
        parse(text).unwrap().memory_limit,
        MemoryLimit::Bytes(12 * GIB)
    );
    let text = "[memory]\nlimit = 4096\n";
    assert_eq!(parse(text).unwrap().memory_limit, MemoryLimit::Bytes(4096));
    let text = "[memory]\nlimit = \"Unlimited\"\n";
    assert_eq!(parse(text).unwrap().memory_limit, MemoryLimit::Unlimited);
    let text = "[memory]\nlimit = \"Default\"\n";
    assert_eq!(parse(text).unwrap().memory_limit, MemoryLimit::Default);
}

#[test]
fn no_key_is_the_default() {
    assert_eq!(parse("").unwrap(), AppSettings::default());
    assert_eq!(parse("[memory]\n").unwrap(), AppSettings::default());
    assert_eq!(
        parse("[general]\n[display]\n[simulation]\n[paths]\n[diagnostics]\n").unwrap(),
        AppSettings::default()
    );
    assert_eq!(
        parse("[other]\nkey = 1\n").unwrap().memory_limit,
        MemoryLimit::Default
    );
}

#[test]
fn a_bad_limit_is_an_error_that_names_the_key() {
    let error = parse("[memory]\nlimit = \"12GB\"\n").unwrap_err();
    assert!(error.contains("[memory] limit"), "{error}");
    assert!(parse("[memory]\nlimit = -5\n").is_err());
    assert!(parse("not toml at all [").is_err());
}

/// A bad value gives the default for that key alone, and a warning that
/// names the key. The good keys of the same file stay.
#[test]
fn a_bad_value_gives_the_default_for_that_key_only() {
    let text = concat!(
        "[memory]\nlimit = \"2GiB\"\n",
        "[general]\nundo_depth = 0\nwindow_width = \"wide\"\ntoast_info_seconds = 9\n",
        "[display]\ntoolpath_colour_mode = \"rainbow\"\nshow_all_toolpaths = true\n",
        "[simulation]\nstock_opacity = 1.5\nstock_view = \"deviation\"\n",
        "[diagnostics]\ncut_trace_retain = -1\nlog_level = \"loud\"\nsave_cut_trace = \"yes\"\n",
    );
    let (settings, warnings) = parse_lenient(text).unwrap();
    assert_eq!(settings.memory_limit, MemoryLimit::Bytes(2 * GIB));
    assert_eq!(settings.general.undo_depth, 100);
    assert_eq!(settings.general.window_width, 1400.0);
    assert_eq!(settings.general.toast_info_seconds, 9.0, "an integer reads");
    assert_eq!(
        settings.display.toolpath_colour_mode,
        ToolpathColourDefault::Normal
    );
    assert!(settings.display.show_all_toolpaths);
    assert_eq!(settings.simulation.stock_opacity, 1.0);
    assert_eq!(settings.simulation.stock_view, StockViewDefault::Deviation);
    assert_eq!(settings.diagnostics.cut_trace_retain, 5);
    assert_eq!(settings.diagnostics.log_level, LogLevel::Info);
    assert!(!settings.diagnostics.save_cut_trace);
    let all = warnings.join("\n");
    for key in [
        "[general] undo_depth",
        "[general] window_width",
        "[display] toolpath_colour_mode",
        "[simulation] stock_opacity",
        "[diagnostics] cut_trace_retain",
        "[diagnostics] log_level",
        "[diagnostics] save_cut_trace",
    ] {
        assert!(all.contains(key), "no warning for {key}: {all}");
    }
    assert_eq!(warnings.len(), 7, "{all}");
    assert!(parse(text).is_err(), "the strict parse refuses the file");
}

/// A missing file gives the defaults with no warning. The cut-trace file
/// is then OFF.
#[test]
fn a_missing_file_gives_the_defaults_with_no_warning() {
    let path = scratch("missing");
    let loaded = load_from(&path);
    assert_eq!(loaded.settings, AppSettings::default());
    assert!(!loaded.settings.diagnostics.save_cut_trace);
    assert_eq!(loaded.warning, None);
    assert_eq!(loaded.path, Some(path));
}

#[test]
fn a_bad_file_gives_the_defaults_with_a_warning() {
    let path = scratch("bad");
    std::fs::write(&path, "[memory]\nlimit = \"lots\"\n").unwrap();
    let loaded = load_from(&path);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(loaded.settings, AppSettings::default());
    assert!(loaded.warning.is_some_and(|w| w.contains("[memory] limit")));

    let path = scratch("not_toml");
    std::fs::write(&path, "not toml at all [").unwrap();
    let loaded = load_from(&path);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(loaded.settings, AppSettings::default());
    assert!(loaded.warning.is_some());
}

/// The precedence, end to end: `RS_CAM_SETTINGS` names the file, the file
/// gives the limit, and a surface flag overrides the file.
#[test]
fn the_env_file_gives_the_limit_and_a_flag_overrides_it() {
    use crate::budget::settings::resolve_limit;

    let path = scratch("env_file");
    std::fs::write(&path, "[memory]\nlimit = \"3GiB\"\n").unwrap();
    let file_text = path.display().to_string();
    let pairs = [
        (SETTINGS_ENV, file_text.as_str()),
        ("XDG_CONFIG_HOME", "/elsewhere"),
        ("HOME", "/h"),
    ];
    let resolved = settings_path_from(env(&pairs)).unwrap();
    assert_eq!(resolved, path, "RS_CAM_SETTINGS wins over XDG and HOME");

    let loaded = load_from(&resolved);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(loaded.warning, None);
    let file = loaded.settings.memory_limit;
    assert_eq!(file, MemoryLimit::Bytes(3 * GIB));

    assert_eq!(resolve_limit(None, file), MemoryLimit::Bytes(3 * GIB));
    assert_eq!(
        resolve_limit(Some(MemoryLimit::Bytes(GIB)), file),
        MemoryLimit::Bytes(GIB)
    );
    assert_eq!(
        resolve_limit(Some(MemoryLimit::Unlimited), file),
        MemoryLimit::Unlimited
    );
    assert_eq!(
        MemoryBudget::from_setting(resolve_limit(None, file)),
        MemoryBudget::with_limit(3 * GIB)
    );
}

/// The library folders, end to end: the file written by the window gives
/// the folder, and the environment variable still wins over it.
#[test]
fn a_library_folder_in_the_file_is_used_unless_the_env_overrides_it() {
    let path = scratch("library_paths");
    let settings = AppSettings {
        paths: PathSettings {
            tool_library: Some(PathBuf::from("/file/tools")),
            machine_library: Some(PathBuf::from("/file/machines")),
            screenshots: None,
        },
        ..AppSettings::default()
    };
    save_to(&path, &settings).unwrap();
    let loaded = load_from(&path).settings.paths;
    std::fs::remove_file(&path).unwrap();

    let home = [("HOME", "/h")];
    assert_eq!(
        paths::tool_library_dir_from(env(&home), loaded.tool_library.as_deref()),
        Some(PathBuf::from("/file/tools"))
    );
    assert_eq!(
        paths::machine_library_dir_from(env(&home), loaded.machine_library.as_deref()),
        Some(PathBuf::from("/file/machines"))
    );
    let overridden = [(TOOL_DIR_ENV, "/env/tools"), ("HOME", "/h")];
    assert_eq!(
        paths::tool_library_dir_from(env(&overridden), loaded.tool_library.as_deref()),
        Some(PathBuf::from("/env/tools"))
    );
    assert_eq!(
        paths::tool_library_dir_from(env(&home), None),
        Some(Path::new("/h").join(".config").join("rs_cam").join("tools"))
    );
}
