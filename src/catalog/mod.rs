//! The font catalogs: every Google Fonts family and every Nerd Font. Each is
//! fetched once a day, cached in `~/.cache/nexus-fonts`, and read from the cache
//! when offline.

pub mod google;
pub mod nerd;

use crate::{cmd, events, http, paths, prefs};
use anyhow::Result;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Google,
    Nerd,
    /// A font that's already on the system (System fonts page).
    System,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Google => "Google Fonts",
            Source::Nerd => "Nerd Fonts",
            Source::System => "System",
        }
    }
}

/// Where a preview's font file comes from.
#[derive(Debug, Clone, PartialEq)]
pub enum PreviewFile {
    /// Ask Google for the family's files and take the regular one.
    Google,
    Url(String),
    Local(PathBuf),
}

#[derive(Debug, Clone)]
pub struct FontEntry {
    pub source: Source,
    /// The family name for Google and system fonts, the folder name for Nerd Fonts.
    pub id: String,
    pub family: String,
    /// Sans serif, Serif, Display, Handwriting, Monospace (Nerd Fonts: Monospace
    /// or Proportional; system fonts: where they live).
    pub category: String,
    pub styles: usize,
    /// Designers, a description, or the folder.
    pub detail: String,
    pub license: String,
    /// Google's last-modified date or the Nerd Fonts release.
    pub version: String,
    /// Google's popularity and trending ranks (lower is more popular).
    pub popularity: u32,
    pub trending: u32,
    /// When Google added it, `YYYY-MM-DD`.
    pub added: String,
    pub variable: bool,
    pub preview: PreviewFile,
    /// Everything search matches, lowercase.
    pub haystack: String,
}

impl FontEntry {
    pub fn key(&self) -> (Source, String) {
        (self.source, self.id.clone())
    }

    pub fn matches(&self, terms: &[String]) -> bool {
        terms.iter().all(|t| self.haystack.contains(t.as_str()))
    }
}

/// Lowercase search words.
pub fn terms(query: &str) -> Vec<String> {
    query.split_whitespace().map(|t| t.to_lowercase()).collect()
}

#[derive(Default)]
struct State {
    google: Rc<Vec<FontEntry>>,
    nerd: Rc<Vec<FontEntry>>,
    /// The Nerd Fonts release the downloads come from (`v3.5.1`).
    nerd_release: String,
    loading: bool,
    loaded: bool,
    error: Option<String>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

pub fn google() -> Rc<Vec<FontEntry>> {
    STATE.with(|s| s.borrow().google.clone())
}

pub fn nerd() -> Rc<Vec<FontEntry>> {
    STATE.with(|s| s.borrow().nerd.clone())
}

pub fn nerd_release() -> String {
    STATE.with(|s| s.borrow().nerd_release.clone())
}

pub fn loading() -> bool {
    STATE.with(|s| s.borrow().loading)
}

/// True once the cache (or the first fetch) has been read.
pub fn loaded() -> bool {
    STATE.with(|s| s.borrow().loaded)
}

pub fn error() -> Option<String> {
    STATE.with(|s| s.borrow().error.clone())
}

/// The catalog entry for an installed font, when the catalog has it.
pub fn find(source: Source, id: &str) -> Option<FontEntry> {
    let list = match source {
        Source::Google => google(),
        Source::Nerd => nerd(),
        Source::System => return None,
    };
    list.iter().find(|e| e.id == id).cloned()
}

const DAY: i64 = 24 * 60 * 60;

pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

struct Raw {
    google: String,
    nerd: String,
    release: String,
}

fn cache_file(name: &str) -> PathBuf {
    paths::cache_dir().join(name)
}

fn read_cache() -> Option<Raw> {
    let read = |n: &str| std::fs::read_to_string(cache_file(n)).ok();
    Some(Raw { google: read("google.json")?, nerd: read("nerd.json")?, release: read("nerd-release.json")? })
}

fn fetch() -> Result<Raw> {
    let raw = Raw {
        google: http::get_text(google::METADATA_URL)?,
        nerd: http::get_text(nerd::FONTS_URL)?,
        release: http::get_text(nerd::RELEASE_URL)?,
    };
    // Check they parse before replacing a good cache.
    parse(&raw)?;
    cmd::atomic_write(&cache_file("google.json"), &raw.google)?;
    cmd::atomic_write(&cache_file("nerd.json"), &raw.nerd)?;
    cmd::atomic_write(&cache_file("nerd-release.json"), &raw.release)?;
    Ok(raw)
}

type Parsed = (Vec<FontEntry>, Vec<FontEntry>, String);

fn parse(raw: &Raw) -> Result<Parsed> {
    let google = google::parse(&raw.google)?;
    let release = nerd::parse_release(&raw.release)?;
    let nerd = nerd::parse(&raw.nerd, &release)?;
    Ok((google, nerd, release))
}

fn set(parsed: Parsed) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.google = Rc::new(parsed.0);
        s.nerd = Rc::new(parsed.1);
        s.nerd_release = parsed.2;
        s.loaded = true;
    });
}

/// Read the cache, then fetch fresh lists if it's missing or a day old.
pub fn start() {
    cmd::background(
        || read_cache().and_then(|raw| parse(&raw).ok()),
        |parsed| {
            let have = parsed.is_some();
            if let Some(p) = parsed {
                set(p);
                events::emit(events::Change::Catalog);
            }
            if !have || now() - prefs::get().catalog_fetched > DAY {
                refresh();
            }
        },
    );
}

/// Fetch both catalogs now.
pub fn refresh() {
    if loading() {
        return;
    }
    STATE.with(|s| s.borrow_mut().loading = true);
    events::emit(events::Change::Catalog);
    cmd::background(
        || fetch().and_then(|raw| parse(&raw)),
        |res| {
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.loading = false;
                s.loaded = true;
            });
            match res {
                Ok(p) => {
                    set(p);
                    STATE.with(|s| s.borrow_mut().error = None);
                    prefs::update(|p| p.catalog_fetched = now());
                }
                Err(e) => {
                    let msg = format!("{e:#}");
                    STATE.with(|s| s.borrow_mut().error = Some(msg.clone()));
                    crate::window::toast(&format!("Couldn't update the font lists: {msg}"));
                }
            }
            events::emit(events::Change::Catalog);
        },
    );
}
