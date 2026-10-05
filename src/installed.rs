//! The fonts this app installed: `~/.local/state/nexus-fonts/installed.toml`
//! records each one's source, version and files, so it can be updated and
//! removed cleanly.

use crate::catalog::{self, Source};
use crate::{cmd, events, paths};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Record {
    pub source: Source,
    /// The catalog id (Google family, Nerd Fonts folder).
    pub id: String,
    pub family: String,
    /// Google's last-modified date or the Nerd Fonts release.
    pub version: String,
    /// The folder the files are in.
    pub dir: PathBuf,
    /// File names inside `dir`.
    pub files: Vec<String>,
    pub bytes: u64,
    /// Unix time.
    pub installed: i64,
}

impl Record {
    /// Newer files are available.
    pub fn outdated(&self) -> bool {
        catalog::find(self.source, &self.id).is_some_and(|e| !e.version.is_empty() && e.version != self.version)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct File {
    #[serde(default, rename = "font")]
    fonts: Vec<Record>,
}

thread_local! {
    static RECORDS: RefCell<Vec<Record>> = RefCell::new(load());
}

fn load() -> Vec<Record> {
    let Ok(text) = std::fs::read_to_string(paths::installed_file()) else { return Vec::new() };
    let mut fonts = toml::from_str::<File>(&text).map(|f| f.fonts).unwrap_or_default();
    // Fonts deleted by hand drop out.
    fonts.retain(|r| r.dir.is_dir());
    fonts
}

fn save() {
    let fonts = all();
    if let Ok(text) = toml::to_string_pretty(&File { fonts }) {
        let _ = cmd::atomic_write(&paths::installed_file(), &text);
    }
}

pub fn all() -> Vec<Record> {
    RECORDS.with(|r| r.borrow().clone())
}

pub fn get(source: Source, id: &str) -> Option<Record> {
    RECORDS.with(|r| r.borrow().iter().find(|x| x.source == source && x.id == id).cloned())
}

pub fn outdated() -> Vec<Record> {
    all().into_iter().filter(Record::outdated).collect()
}

/// Add or replace a record (after an install or update).
pub fn put(record: Record) {
    RECORDS.with(|r| {
        let mut r = r.borrow_mut();
        r.retain(|x| !(x.source == record.source && x.id == record.id));
        r.push(record);
        r.sort_by_key(|x| x.family.to_lowercase());
    });
    save();
    events::emit(events::Change::Installed);
}

/// Forget a record (its files are already gone).
pub fn forget(source: Source, id: &str) {
    RECORDS.with(|r| r.borrow_mut().retain(|x| !(x.source == source && x.id == id)));
    save();
    events::emit(events::Change::Installed);
}

/// Delete a record's files and its folder, when it's one of ours.
pub fn delete_files(record: &Record) -> Result<()> {
    let ours = paths::fonts_dir();
    for f in &record.files {
        let p = record.dir.join(f);
        if p.starts_with(&ours) {
            let _ = std::fs::remove_file(&p);
        }
    }
    if record.dir.starts_with(&ours) && record.dir != ours && record.dir.is_dir() {
        std::fs::remove_dir_all(&record.dir)?;
    }
    Ok(())
}

/// The folder a font installs into.
pub fn dir_for(source: Source, family: &str) -> PathBuf {
    let safe: String = family.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    let sub = match source {
        Source::Google => "google",
        Source::Nerd => "nerd",
        Source::System => "other",
    };
    paths::fonts_dir().join(sub).join(safe)
}

/// Total size of the files in a folder.
pub fn size_of(dir: &Path, files: &[String]) -> u64 {
    files.iter().filter_map(|f| std::fs::metadata(dir.join(f)).ok()).map(|m| m.len()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let rec = Record {
            source: Source::Nerd,
            id: "Hack".into(),
            family: "Hack Nerd Font".into(),
            version: "v3.5.1".into(),
            dir: "/tmp/x".into(),
            files: vec!["HackNerdFont-Regular.ttf".into()],
            bytes: 10,
            installed: 1,
        };
        let text = toml::to_string_pretty(&File { fonts: vec![rec.clone()] }).unwrap();
        assert!(text.contains("source = \"nerd\""));
        let back: File = toml::from_str(&text).unwrap();
        assert_eq!(back.fonts, vec![rec]);
    }

    #[test]
    fn safe_dirs() {
        let d = dir_for(Source::Google, "Noto Sans JP");
        assert!(d.ends_with("nexus-fonts/google/Noto_Sans_JP"));
    }
}
