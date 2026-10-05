//! Well-known locations. Every path honours the XDG overrides so the whole app
//! can be pointed at a scratch copy of `~/.config` for testing.

use std::path::PathBuf;

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

fn xdg(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home().join(fallback))
}

pub fn config_home() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config")
}

pub fn state_home() -> PathBuf {
    xdg("XDG_STATE_HOME", ".local/state")
}

pub fn data_home() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share")
}

/// `~/.cache/nexus-fonts`: catalogs, previews and downloads in progress.
pub fn cache_dir() -> PathBuf {
    xdg("XDG_CACHE_HOME", ".cache").join("nexus-fonts")
}

/// `~/.config/nexus-fonts`: settings and user themes.
pub fn app_dir() -> PathBuf {
    config_home().join("nexus-fonts")
}

pub fn prefs_file() -> PathBuf {
    app_dir().join("settings.toml")
}

pub fn custom_themes_dir() -> PathBuf {
    app_dir().join("themes")
}

/// What this app installed, and from where.
pub fn installed_file() -> PathBuf {
    state_home().join("nexus-fonts/installed.toml")
}

/// The user's font folder.
pub fn user_fonts_dir() -> PathBuf {
    data_home().join("fonts")
}

/// Where installed fonts go: `~/.local/share/fonts/nexus-fonts`.
pub fn fonts_dir() -> PathBuf {
    user_fonts_dir().join("nexus-fonts")
}

/// One font file per family, for previews.
pub fn preview_dir() -> PathBuf {
    cache_dir().join("preview")
}

pub fn omarchy_theme_dir() -> PathBuf {
    state_home().join("omarchy/current/theme")
}

pub fn omarchy_colors() -> PathBuf {
    omarchy_theme_dir().join("colors.toml")
}

/// Replace `$HOME` with `~` for display.
pub fn pretty(path: &std::path::Path) -> String {
    let home = home();
    match path.strip_prefix(&home) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}
