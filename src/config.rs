// SPDX-License-Identifier: GPL-3.0-only

//! Application configuration, stored as JSON in the XDG config directory.

use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

/// The application identifier used for the desktop entry and config directory.
pub const APP_ID: &str = "org.iced_systemmonitor.Monitor";

/// The config directory name.
const CONFIG_DIR: &str = "iced_systemmonitor";

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum AppTheme {
    Dark,
    #[default]
    Light,
    System,
}

impl AppTheme {
    /// Returns the iced theme to use, or `None` to follow the system theme.
    pub fn theme(self) -> Option<iced::Theme> {
        match self {
            Self::Light => Some(iced::Theme::Light),
            Self::Dark => Some(iced::Theme::Dark),
            Self::System => None,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct Config {
    pub app_theme: AppTheme,
}

fn config_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;

    Some(base.join(CONFIG_DIR))
}

fn config_path() -> Option<PathBuf> {
    Some(config_dir()?.join("config.json"))
}

impl Config {
    /// Loads the configuration, falling back to the default on any error.
    pub fn load() -> Self {
        let Some(path) = config_path() else {
            return Self::default();
        };

        let Ok(data) = fs::read(&path) else {
            return Self::default();
        };

        match serde_json::from_slice(&data) {
            Ok(config) => config,
            Err(err) => {
                log::warn!("failed to parse {}: {}", path.display(), err);
                Self::default()
            }
        }
    }

    /// Saves the configuration, logging any error.
    pub fn save(&self) {
        let Some(path) = config_path() else {
            return;
        };

        if let Some(dir) = path.parent()
            && let Err(err) = fs::create_dir_all(dir)
        {
            log::warn!("failed to create {}: {}", dir.display(), err);
            return;
        }

        match serde_json::to_vec_pretty(self) {
            Ok(data) => {
                if let Err(err) = fs::write(&path, data) {
                    log::warn!("failed to write {}: {}", path.display(), err);
                }
            }
            Err(err) => log::warn!("failed to serialize config: {}", err),
        }
    }
}
