//! User settings, persisted as JSON.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = "settings.json";

/// Interface language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Language {
    #[default]
    Vi,
    En,
}

/// Which number the tray icon and menu bar show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum TrayMetric {
    #[default]
    SessionPercent,
    WeekPercent,
    /// Both, stacked in one icon: session above, week below.
    SessionAndWeek,
    TodayCost,
    TodayTokens,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cards {
    pub session: bool,
    pub forecast: bool,
    pub week_all: bool,
    pub week_model: bool,
    pub cost_today: bool,
    pub chart: bool,
    pub models: bool,
    pub cache_hit_rate: bool,
    pub top_project: bool,
    pub current_session: bool,
}

impl Default for Cards {
    fn default() -> Self {
        Self {
            session: true,
            forecast: true,
            week_all: true,
            week_model: true,
            cost_today: true,
            chart: true,
            models: true,
            // Off by default: useful, but not every day.
            cache_hit_rate: false,
            top_project: false,
            current_session: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Currency {
    /// `USD`, or `VND` to convert at `vnd_rate`.
    pub code: String,
    pub vnd_rate: f64,
}

impl Default for Currency {
    fn default() -> Self {
        Self {
            code: "USD".into(),
            vnd_rate: 25_000.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub language: Language,
    pub tray_metric: TrayMetric,
    pub cards: Cards,
    pub currency: Currency,

    /// The transcript source cannot be turned off; the `/usage` one can.
    pub usage_enabled: bool,
    /// Path to the `claude` executable. `None` means "search for it", which is
    /// what a standard install wants; naming one covers an install the search
    /// does not know about.
    pub claude_path: Option<String>,
    pub transcript_interval_seconds: u64,
    pub usage_interval_seconds: u64,

    pub notifications_enabled: bool,
    pub session_alert_percent: f64,
    pub week_alert_percent: f64,

    pub always_on_top: bool,
    /// Widget background opacity, 0.0 to 1.0.
    pub opacity: f64,
    pub skip_taskbar: bool,
    pub autostart: bool,
    pub window_position: Option<(i32, i32)>,

    /// Keys this version does not know about. Kept so that settings written by
    /// a newer build survive being loaded and saved by an older one.
    #[serde(flatten)]
    pub(crate) unknown: serde_json::Map<String, serde_json::Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: Language::default(),
            tray_metric: TrayMetric::default(),
            cards: Cards::default(),
            currency: Currency::default(),
            usage_enabled: true,
            claude_path: None,
            transcript_interval_seconds: 3,
            // A poll costs ~3s of CLI startup, so this is minutes, not seconds.
            usage_interval_seconds: 180,
            notifications_enabled: true,
            session_alert_percent: 80.0,
            week_alert_percent: 85.0,
            always_on_top: true,
            opacity: 0.88,
            skip_taskbar: false,
            autostart: false,
            window_position: None,
            unknown: serde_json::Map::new(),
        }
    }
}

/// The lowest the `/usage` interval may be set to. Below this the CLI spends
/// more time starting up than the widget spends idle.
pub const MIN_USAGE_INTERVAL_SECONDS: u64 = 60;
pub const MIN_TRANSCRIPT_INTERVAL_SECONDS: u64 = 1;

impl Settings {
    /// Reads settings, falling back to defaults for anything unusable.
    ///
    /// A missing file is the normal first run. A corrupt one is reported and
    /// then ignored, but never deleted: overwriting it would destroy whatever
    /// the user had configured, and the file is the only copy.
    pub fn load(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        // Notepad and PowerShell both write a byte order mark, and this file is
        // meant to be hand-editable. serde rejects one as a stray character, so
        // the whole file would be discarded over an invisible three bytes.
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);

        match serde_json::from_str::<Self>(text) {
            Ok(settings) => settings.sanitized(),
            Err(error) => {
                eprintln!(
                    "[settings] {} is unreadable, using defaults: {error}",
                    path.display()
                );
                Self::default()
            }
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        std::fs::write(path, text)
    }

    /// Clamps values that would make the app misbehave if hand-edited.
    pub fn sanitized(mut self) -> Self {
        self.usage_interval_seconds = self.usage_interval_seconds.max(MIN_USAGE_INTERVAL_SECONDS);
        self.transcript_interval_seconds = self
            .transcript_interval_seconds
            .max(MIN_TRANSCRIPT_INTERVAL_SECONDS);
        self.opacity = self.opacity.clamp(0.1, 1.0);
        self.session_alert_percent = self.session_alert_percent.clamp(1.0, 100.0);
        self.week_alert_percent = self.week_alert_percent.clamp(1.0, 100.0);
        if !self.currency.vnd_rate.is_finite() || self.currency.vnd_rate <= 0.0 {
            self.currency.vnd_rate = Currency::default().vnd_rate;
        }
        // The form sends a cleared box as an empty string, which as a program
        // name would fail every poll instead of falling back to the search.
        self.claude_path = self
            .claude_path
            .take()
            .map(|path| path.trim().to_string())
            .filter(|path| !path.is_empty());
        self
    }
}

/// `<config dir>/claude-usage-widget/settings.json`.
///
/// Tauri can resolve this too, but taking it from the environment keeps this
/// module usable without an app handle, including in tests.
pub fn default_path() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join("Library").join("Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    }?;
    Some(base.join("claude-usage-widget").join(FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("nested").join(FILE_NAME);
        (dir, path)
    }

    #[test]
    fn a_missing_file_yields_defaults() {
        let (_dir, path) = temp();
        assert_eq!(Settings::load(&path), Settings::default());
    }

    #[test]
    fn settings_survive_a_round_trip() {
        let (_dir, path) = temp();
        let settings = Settings {
            tray_metric: TrayMetric::TodayCost,
            cards: Cards {
                cache_hit_rate: true,
                ..Default::default()
            },
            window_position: Some((120, 40)),
            ..Default::default()
        };

        settings.save(&path).expect("save");
        assert_eq!(Settings::load(&path), settings);
    }

    /// A corrupt file must not cost the user their configuration.
    #[test]
    fn a_corrupt_file_falls_back_without_being_overwritten() {
        let (_dir, path) = temp();
        std::fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
        std::fs::write(&path, "{ this is not json").expect("write");

        assert_eq!(Settings::load(&path), Settings::default());
        assert_eq!(
            std::fs::read_to_string(&path).expect("still there"),
            "{ this is not json"
        );
    }

    #[test]
    fn a_partial_file_fills_the_rest_from_defaults() {
        let (_dir, path) = temp();
        std::fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
        std::fs::write(&path, r#"{"opacity":0.5}"#).expect("write");

        let settings = Settings::load(&path);
        assert_eq!(settings.opacity, 0.5);
        assert_eq!(settings.usage_interval_seconds, 180);
    }

    /// Settings written by a newer build must not be stripped by an older one.
    #[test]
    fn unknown_keys_are_written_back() {
        let (_dir, path) = temp();
        std::fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
        std::fs::write(&path, r#"{"opacity":0.5,"futureSetting":{"a":1}}"#).expect("write");

        let settings = Settings::load(&path);
        settings.save(&path).expect("save");

        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("json");
        assert_eq!(written["futureSetting"]["a"], 1);
        assert_eq!(written["opacity"], 0.5);
    }

    /// Editing the file in Notepad or writing it from PowerShell adds a byte
    /// order mark, which would otherwise cost the user every setting they had.
    #[test]
    fn a_byte_order_mark_does_not_discard_the_file() {
        let (_dir, path) = temp();
        std::fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
        std::fs::write(&path, "\u{feff}{\"opacity\":0.4,\"autostart\":true}").expect("write");

        let settings = Settings::load(&path);
        assert_eq!(settings.opacity, 0.4);
        assert!(settings.autostart);
    }

    #[test]
    fn a_hand_edited_poll_interval_is_clamped() {
        let (_dir, path) = temp();
        std::fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
        std::fs::write(&path, r#"{"usageIntervalSeconds":1,"opacity":9}"#).expect("write");

        let settings = Settings::load(&path);
        assert_eq!(settings.usage_interval_seconds, MIN_USAGE_INTERVAL_SECONDS);
        assert_eq!(settings.opacity, 1.0);
    }

    #[test]
    fn a_nonsense_exchange_rate_reverts_to_the_default() {
        let (_dir, path) = temp();
        std::fs::create_dir_all(path.parent().unwrap()).expect("mkdir");
        std::fs::write(&path, r#"{"currency":{"code":"VND","vndRate":0}}"#).expect("write");

        assert_eq!(Settings::load(&path).currency.vnd_rate, 25_000.0);
    }

    #[test]
    fn saving_creates_the_directory() {
        let (_dir, path) = temp();
        Settings::default().save(&path).expect("save");
        assert!(path.exists());
    }
}
