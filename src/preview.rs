//! Previews: one font file per family, downloaded to the cache and drawn
//! through a font map of its own, so nothing has to be installed to see it and
//! the window's own fonts are left alone.

use crate::catalog::{FontEntry, PreviewFile, Source, google};
use crate::{http, paths};
use anyhow::{Context, Result, bail};
use gtk::glib::translate::from_glib_full;
use gtk::pango;
use std::ffi::{CString, c_void};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

const MAX_FONT: u64 = 64 * 1024 * 1024;

fn cache_name(entry: &FontEntry, ext: &str) -> PathBuf {
    let safe: String = entry.id.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
    let src = match entry.source {
        Source::Google => "google",
        Source::Nerd => "nerd",
        Source::System => "system",
    };
    paths::preview_dir().join(format!("{src}-{safe}-{}.{ext}", entry.version.replace(['/', ' '], "_")))
}

fn ext_of(url: &str) -> &str {
    if url.to_ascii_lowercase().ends_with(".otf") { "otf" } else { "ttf" }
}

fn save(url: &str, dest: &Path) -> Result<()> {
    let bytes = http::get_bytes(url, MAX_FONT)?;
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = dest.with_extension("part");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, dest)?;
    Ok(())
}

/// The font file to preview, fetched if needed (blocking; run it off the UI thread).
pub fn fetch(entry: &FontEntry) -> Result<PathBuf> {
    // An installed font previews from its own files.
    match &entry.preview {
        PreviewFile::Local(p) => {
            if p.is_file() {
                return Ok(p.clone());
            }
            bail!("{} is missing", paths::pretty(p));
        }
        PreviewFile::Url(url) => {
            if url.is_empty() {
                bail!("there's no preview for this font");
            }
            let dest = cache_name(entry, ext_of(url));
            if !dest.is_file() {
                save(url, &dest)?;
            }
            Ok(dest)
        }
        PreviewFile::Google => {
            for ext in ["ttf", "otf"] {
                let dest = cache_name(entry, ext);
                if dest.is_file() {
                    return Ok(dest);
                }
            }
            let manifest = google::manifest(&entry.family)?;
            let file = google::preview_file(&manifest.file_refs).context("Google lists no font files for it")?;
            let dest = cache_name(entry, ext_of(&file.filename));
            save(&file.url, &dest)?;
            Ok(dest)
        }
    }
}

/// Drop cached previews older than a month.
pub fn prune() {
    let Ok(entries) = std::fs::read_dir(paths::preview_dir()) else { return };
    let month = std::time::Duration::from_secs(30 * 24 * 3600);
    for e in entries.flatten() {
        let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age > month);
        if old {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

#[link(name = "fontconfig")]
unsafe extern "C" {
    fn FcConfigCreate() -> *mut c_void;
    fn FcConfigAppFontAddFile(config: *mut c_void, file: *const u8) -> i32;
    fn FcConfigDestroy(config: *mut c_void);
}

#[link(name = "pangoft2-1.0")]
unsafe extern "C" {
    fn pango_fc_font_map_set_config(fontmap: *mut c_void, config: *mut c_void);
}

#[link(name = "pangocairo-1.0")]
unsafe extern "C" {
    fn pango_cairo_font_map_new_for_font_type(font_type: i32) -> *mut pango::ffi::PangoFontMap;
}

const CAIRO_FONT_TYPE_FT: i32 = 1;

/// A font map that holds just this one file. Every family asked for falls back
/// to it, so text drawn with it is always in the previewed font.
pub fn font_map(file: &Path) -> Option<pango::FontMap> {
    let path = CString::new(file.as_os_str().as_bytes()).ok()?;
    // SAFETY: plain fontconfig and pango calls. The font map takes its own
    // reference to the config, so ours is released straight after.
    unsafe {
        let config = FcConfigCreate();
        if config.is_null() {
            return None;
        }
        if FcConfigAppFontAddFile(config, path.as_ptr().cast()) == 0 {
            FcConfigDestroy(config);
            return None;
        }
        let map = pango_cairo_font_map_new_for_font_type(CAIRO_FONT_TYPE_FT);
        if map.is_null() {
            FcConfigDestroy(config);
            return None;
        }
        pango_fc_font_map_set_config(map.cast(), config);
        FcConfigDestroy(config);
        Some(from_glib_full(map))
    }
}
