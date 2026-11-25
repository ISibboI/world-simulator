//! Small choices of the user that the view keeps across restarts (and reloads of the web page),
//! such as whether the time bar is collapsed. The web page keeps them in the browser's local
//! storage (like the render mode, see `crate::web`); the desktop in a text file of `key=value`
//! lines in the user's configuration directory ([`FileSettings`]). Failures (no storage, a
//! read-only directory) only cost the memory.

#[cfg(any(test, not(target_arch = "wasm32")))]
use std::collections::BTreeMap;

/// Remembered values by key.
pub trait Settings {
    /// The value remembered under `key`, if any.
    fn get(&self, key: &str) -> Option<String>;
    /// Remembers `value` under `key`.
    fn set(&mut self, key: &str, value: &str);
}

/// Reads a bool stored by [`set_bool`]; anything else counts as not stored.
pub fn get_bool(settings: &dyn Settings, key: &str) -> Option<bool> {
    match settings.get(key)?.as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

pub fn set_bool(settings: &mut dyn Settings, key: &str, value: bool) {
    settings.set(key, if value { "true" } else { "false" });
}

/// The settings of the desktop version, in `world-simulator/settings.txt` of the user's
/// configuration directory (`$XDG_CONFIG_HOME` or `~/.config` on Linux,
/// `~/Library/Application Support` on macOS, `%APPDATA%` on Windows).
#[cfg(not(target_arch = "wasm32"))]
pub struct FileSettings {
    path: Option<std::path::PathBuf>,
    values: BTreeMap<String, String>,
}

#[cfg(not(target_arch = "wasm32"))]
impl FileSettings {
    /// Reads the file, if there is one.
    pub fn open() -> Self {
        let path = config_dir().map(|dir| dir.join("world-simulator").join("settings.txt"));
        let values = path
            .as_ref()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .map(|text| parse(&text))
            .unwrap_or_default();
        Self { path, values }
    }

    fn write(&self) -> std::io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, format(&self.values))
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Settings for FileSettings {
    fn get(&self, key: &str) -> Option<String> {
        self.values.get(key).cloned()
    }

    fn set(&mut self, key: &str, value: &str) {
        if self.values.get(key).map(String::as_str) == Some(value) {
            return;
        }
        self.values.insert(key.to_owned(), value.to_owned());
        if let Err(error) = self.write() {
            tracing::warn!("Could not save the settings to {:?}: {error}", self.path);
        }
    }
}

/// The user's configuration directory, if known.
#[cfg(not(target_arch = "wasm32"))]
fn config_dir() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;
    let var = |name: &str| {
        std::env::var_os(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    if cfg!(windows) {
        var("APPDATA")
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|home| home.join("Library").join("Application Support"))
    } else {
        var("XDG_CONFIG_HOME").or_else(|| var("HOME").map(|home| home.join(".config")))
    }
}

/// The values of `key=value` lines; other lines are skipped.
#[cfg(any(test, not(target_arch = "wasm32")))]
fn parse(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().to_owned(), value.trim().to_owned()))
        .filter(|(key, _)| !key.is_empty())
        .collect()
}

/// `key=value` lines, sorted by key.
#[cfg(any(test, not(target_arch = "wasm32")))]
fn format(values: &BTreeMap<String, String>) -> String {
    values
        .iter()
        .map(|(key, value)| format!("{key}={value}\n"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_round_trip_and_junk_is_skipped() {
        let values = parse("time-bar.open=false\n# comment\n=x\n  a = b \n");
        assert_eq!(values.len(), 2);
        assert_eq!(values["time-bar.open"], "false");
        assert_eq!(values["a"], "b");
        assert_eq!(parse(&format(&values)), values);
    }

    #[test]
    fn bools_are_read_back_and_unknown_values_ignored() {
        struct Memory(BTreeMap<String, String>);
        impl Settings for Memory {
            fn get(&self, key: &str) -> Option<String> {
                self.0.get(key).cloned()
            }
            fn set(&mut self, key: &str, value: &str) {
                self.0.insert(key.to_owned(), value.to_owned());
            }
        }
        let mut settings = Memory(BTreeMap::new());
        assert_eq!(get_bool(&settings, "k"), None);
        set_bool(&mut settings, "k", false);
        assert_eq!(get_bool(&settings, "k"), Some(false));
        settings.set("k", "maybe");
        assert_eq!(get_bool(&settings, "k"), None);
    }
}
