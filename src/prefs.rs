//! This app's own preferences (`~/.config/nexus-fonts/settings.toml`).

use crate::{cmd, paths};
use gtk::glib;
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeMode {
    /// Follow the active Omarchy theme live.
    Omarchy,
    /// Use a bundled or custom theme.
    Theme,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub mode: ThemeMode,
    pub theme: String,
    pub reduce_motion: bool,
    /// The sidebar shows only icons, whatever the window width.
    pub sidebar_collapsed: bool,
    pub glow: bool,
    pub last_section: String,
    /// Which Google Fonts files to install: static, variable or both.
    pub google_files: String,
    /// Which Nerd Font variants to keep: all, mono or regular (regular and Propo).
    pub nerd_variants: String,
    /// Run fc-cache after installing or removing.
    pub refresh_cache: bool,
    /// The preview's sample text.
    pub sample: String,
    /// The preview's size in points.
    pub preview_size: f64,
    /// How the Google Fonts table is sorted: popular, trending, name or newest.
    pub google_sort: String,
    /// Unix time the catalogs were last fetched.
    pub catalog_fetched: i64,
}

pub const DEFAULT_SAMPLE: &str = "The quick brown fox jumps over the lazy dog";

impl Default for Prefs {
    fn default() -> Self {
        Self {
            mode: ThemeMode::Omarchy,
            theme: "tokyo-night".into(),
            reduce_motion: false,
            sidebar_collapsed: false,
            glow: true,
            last_section: "google".into(),
            google_files: "static".into(),
            nerd_variants: "all".into(),
            refresh_cache: true,
            sample: DEFAULT_SAMPLE.into(),
            preview_size: 32.0,
            google_sort: "popular".into(),
            catalog_fetched: 0,
        }
    }
}

thread_local! {
    static BROKEN: Cell<bool> = const { Cell::new(false) };
    static PREFS: RefCell<Prefs> = RefCell::new(load());
    static PENDING: Cell<Option<glib::SourceId>> = const { Cell::new(None) };
}

fn load() -> Prefs {
    let file = paths::prefs_file();
    let Ok(text) = std::fs::read_to_string(&file) else { return Prefs::default() };
    match toml::from_str(&text) {
        Ok(p) => p,
        Err(e) => {
            // Don't lose a file with a typo in it: keep a copy before the
            // defaults are saved over it.
            let backup = file.with_extension("toml.bak");
            let _ = std::fs::copy(&file, &backup);
            eprintln!("fonts: {} couldn't be read ({e}); kept a copy as {}", file.display(), backup.display());
            BROKEN.with(|b| b.set(true));
            Prefs::default()
        }
    }
}

/// True (once) when the settings file couldn't be read at start.
pub fn take_broken() -> bool {
    BROKEN.with(|b| b.replace(false))
}

pub fn get() -> Prefs {
    PREFS.with(|p| p.borrow().clone())
}

/// Change the prefs now; the file is written about 450 ms after the last change,
/// so dragging a slider doesn't rewrite it on every tick.
pub fn update(change: impl FnOnce(&mut Prefs)) {
    PREFS.with(|p| change(&mut p.borrow_mut()));
    if let Some(id) = PENDING.with(|p| p.take()) {
        id.remove();
    }
    let id = glib::timeout_add_local_once(std::time::Duration::from_millis(450), || {
        PENDING.with(|p| p.set(None));
        save();
    });
    PENDING.with(|p| p.set(Some(id)));
}

/// Write any pending change now (on quit).
pub fn flush() {
    if let Some(id) = PENDING.with(|p| p.take()) {
        id.remove();
        save();
    }
}

fn save() {
    let text = PREFS.with(|p| toml::to_string_pretty(&*p.borrow()));
    if let Ok(text) = text {
        let _ = cmd::atomic_write(&paths::prefs_file(), &text);
    }
}
