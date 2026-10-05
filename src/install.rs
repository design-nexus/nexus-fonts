//! The install queue. Jobs run one at a time on a worker thread that never
//! touches GTK; progress and results come back over a channel.

use crate::catalog::{FontEntry, Source, google, nerd};
use crate::installed::{self, Record};
use crate::{catalog, events, http, paths, prefs, window};
use anyhow::{Context, Result, bail};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Install,
    Update,
    Remove,
}

struct Job {
    kind: Kind,
    entry: FontEntry,
    /// The record being updated or removed.
    record: Option<Record>,
    google_files: String,
    nerd_variants: String,
    refresh_cache: bool,
    release: String,
}

enum Msg {
    Progress((Source, String), String),
    Done { key: (Source, String), family: String, kind: Kind, result: Result<Option<Record>, String> },
}

struct Queue {
    tx: std::sync::mpsc::Sender<Job>,
    /// Jobs waiting or running, in order.
    pending: Vec<((Source, String), Kind)>,
    /// What the running job is doing ("Downloading 3 of 8").
    progress: Option<((Source, String), String)>,
}

thread_local! {
    static QUEUE: RefCell<Option<Queue>> = const { RefCell::new(None) };
}

fn queue_tx() -> std::sync::mpsc::Sender<Job> {
    QUEUE.with(|q| {
        let mut q = q.borrow_mut();
        q.get_or_insert_with(|| {
            let (tx, rx) = std::sync::mpsc::channel::<Job>();
            let (mtx, mrx) = async_channel::unbounded::<Msg>();
            std::thread::spawn(move || {
                for job in rx {
                    let key = job.entry.key();
                    let progress = |text: String| {
                        let _ = mtx.send_blocking(Msg::Progress(key.clone(), text));
                    };
                    let result = run(&job, &progress).map_err(|e| format!("{e:#}"));
                    let _ = mtx.send_blocking(Msg::Done {
                        key: key.clone(),
                        family: job.entry.family.clone(),
                        kind: job.kind,
                        result,
                    });
                }
            });
            gtk::glib::spawn_future_local(async move {
                while let Ok(msg) = mrx.recv().await {
                    on_msg(msg);
                }
            });
            Queue { tx, pending: Vec::new(), progress: None }
        })
        .tx
        .clone()
    })
}

fn on_msg(msg: Msg) {
    match msg {
        Msg::Progress(key, text) => {
            QUEUE.with(|q| {
                if let Some(q) = q.borrow_mut().as_mut() {
                    q.progress = Some((key, text));
                }
            });
        }
        Msg::Done { key, family, kind, result } => {
            QUEUE.with(|q| {
                if let Some(q) = q.borrow_mut().as_mut() {
                    if let Some(i) = q.pending.iter().position(|(k, _)| *k == key) {
                        q.pending.remove(i);
                    }
                    q.progress = None;
                }
            });
            match result {
                Ok(Some(record)) => {
                    installed::put(record);
                    window::toast(&match kind {
                        Kind::Update => format!("Updated {family}."),
                        _ => format!("Installed {family}."),
                    });
                }
                Ok(None) => {
                    installed::forget(key.0, &key.1);
                    window::toast(&format!("Removed {family}."));
                }
                Err(e) => {
                    let what = match kind {
                        Kind::Install => "install",
                        Kind::Update => "update",
                        Kind::Remove => "remove",
                    };
                    window::toast(&format!("Couldn't {what} {family}: {e}"));
                }
            }
        }
    }
    events::emit(events::Change::Jobs);
}

fn submit(kind: Kind, entry: FontEntry, record: Option<Record>) {
    let key = entry.key();
    if busy(&key).is_some() {
        return;
    }
    let p = prefs::get();
    let job = Job {
        kind,
        entry,
        record,
        google_files: p.google_files,
        nerd_variants: p.nerd_variants,
        refresh_cache: p.refresh_cache,
        release: catalog::nerd_release(),
    };
    let tx = queue_tx();
    QUEUE.with(|q| {
        if let Some(q) = q.borrow_mut().as_mut() {
            q.pending.push((key, kind));
        }
    });
    let _ = tx.send(job);
    events::emit(events::Change::Jobs);
}

pub fn install(entry: &FontEntry) {
    submit(Kind::Install, entry.clone(), None);
}

/// Fetch the catalog's newer files over an installed font.
pub fn update(record: &Record) {
    if let Some(entry) = catalog::find(record.source, &record.id) {
        submit(Kind::Update, entry, Some(record.clone()));
    }
}

pub fn update_all() {
    for r in installed::outdated() {
        update(&r);
    }
}

pub fn remove(record: &Record) {
    let entry = catalog::find(record.source, &record.id).unwrap_or_else(|| stub(record));
    submit(Kind::Remove, entry, Some(record.clone()));
}

pub fn remove_all() {
    for r in installed::all() {
        remove(&r);
    }
}

/// An entry for an installed font the catalog no longer lists.
pub fn stub(r: &Record) -> FontEntry {
    FontEntry {
        source: r.source,
        id: r.id.clone(),
        family: r.family.clone(),
        category: String::new(),
        styles: r.files.len(),
        detail: paths::pretty(&r.dir),
        license: String::new(),
        version: r.version.clone(),
        popularity: 0,
        trending: 0,
        added: String::new(),
        variable: false,
        preview: catalog::PreviewFile::Local(r.dir.join(r.files.first().cloned().unwrap_or_default())),
        haystack: r.family.to_lowercase(),
    }
}

/// What's happening to a font: queued, or its progress. None when idle.
pub fn busy(key: &(Source, String)) -> Option<String> {
    QUEUE.with(|q| {
        let q = q.borrow();
        let q = q.as_ref()?;
        let (_, kind) = q.pending.iter().find(|(k, _)| k == key)?;
        if let Some((k, text)) = &q.progress
            && k == key
        {
            return Some(text.clone());
        }
        Some(
            match kind {
                Kind::Remove => "Waiting to remove…",
                _ => "Waiting…",
            }
            .to_string(),
        )
    })
}

/// The status bar's line while jobs run: "Installing Fira Code… 3 of 8 · 2 more".
pub fn status() -> Option<String> {
    QUEUE.with(|q| {
        let q = q.borrow();
        let q = q.as_ref()?;
        let ((_, id), _) = q.pending.first()?;
        let mut text = match &q.progress {
            Some((_, t)) => t.clone(),
            None => format!("{id}…"),
        };
        if q.pending.len() > 1 {
            text.push_str(&format!(" · {} more", q.pending.len() - 1));
        }
        Some(text)
    })
}

// ---------- On the worker thread ----------

fn run(job: &Job, progress: &dyn Fn(String)) -> Result<Option<Record>> {
    let family = &job.entry.family;
    if job.kind == Kind::Remove {
        progress(format!("Removing {family}…"));
        if let Some(r) = &job.record {
            installed::delete_files(r)?;
        }
        if job.refresh_cache {
            refresh_cache(&paths::user_fonts_dir());
        }
        return Ok(None);
    }
    let dir = installed::dir_for(job.entry.source, family);
    let stage = staging(&dir);
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage)?;
    let fetched = match job.entry.source {
        Source::Google => fetch_google(job, &stage, progress),
        Source::Nerd => fetch_nerd(job, &stage, progress),
        Source::System => bail!("system fonts are already installed"),
    };
    let files = match fetched {
        Ok(f) => f,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&stage);
            return Err(e);
        }
    };
    if files.is_empty() {
        let _ = std::fs::remove_dir_all(&stage);
        bail!("no font files were found");
    }
    // Swap the new folder in. An update replaces the old files in one step.
    if let Some(old) = &job.record
        && old.dir != dir
    {
        installed::delete_files(old)?;
    }
    if dir.exists() {
        std::fs::remove_dir_all(&dir).with_context(|| format!("couldn't replace {}", paths::pretty(&dir)))?;
    }
    std::fs::rename(&stage, &dir)?;
    if job.refresh_cache {
        progress(format!("Refreshing the font cache for {family}…"));
        refresh_cache(&dir);
    }
    let bytes = installed::size_of(&dir, &files);
    Ok(Some(Record {
        source: job.entry.source,
        id: job.entry.id.clone(),
        family: family.clone(),
        version: job.entry.version.clone(),
        dir,
        files,
        bytes,
        installed: catalog::now(),
    }))
}

/// A hidden folder beside the target, so a failed download leaves nothing behind.
fn staging(dir: &Path) -> PathBuf {
    let name = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    dir.with_file_name(format!(".{name}.part-{}", std::process::id()))
}

fn base_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

fn fetch_google(job: &Job, stage: &Path, progress: &dyn Fn(String)) -> Result<Vec<String>> {
    let family = &job.entry.family;
    progress(format!("Finding the files for {family}…"));
    let manifest = google::manifest(family)?;
    let picked = google::pick_files(&manifest.file_refs, &job.google_files);
    let mut files = Vec::new();
    for (i, r) in picked.iter().enumerate() {
        progress(format!("Downloading {family}… {} of {}", i + 1, picked.len()));
        let name = base_name(&r.filename);
        http::download(&r.url, &stage.join(&name), |_, _| {})?;
        files.push(name);
    }
    // The license and any notes that come with it.
    for f in &manifest.files {
        let name = base_name(&f.filename);
        if name.to_ascii_lowercase().ends_with(".txt") {
            std::fs::write(stage.join(&name), &f.contents)?;
            files.push(name);
        }
    }
    Ok(files)
}

fn fetch_nerd(job: &Job, stage: &Path, progress: &dyn Fn(String)) -> Result<Vec<String>> {
    let family = &job.entry.family;
    if job.release.is_empty() {
        bail!("the Nerd Fonts release isn't known yet; refresh the font lists");
    }
    let url = nerd::archive_url(&job.release, &job.entry.id);
    let archive = paths::cache_dir().join("downloads").join(format!("{}.tar.xz", job.entry.id));
    http::download(&url, &archive, |done, total| match (done * 100).checked_div(total) {
        Some(pct) => progress(format!("Downloading {family}… {pct}%")),
        None => progress(format!("Downloading {family}… {}", crate::fmt::bytes(done))),
    })?;
    progress(format!("Unpacking {family}…"));
    let unpack = stage.join(".unpack");
    std::fs::create_dir_all(&unpack)?;
    let status = Command::new("tar")
        .arg("-xJf")
        .arg(&archive)
        .arg("-C")
        .arg(&unpack)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .context("couldn't run tar")?;
    let _ = std::fs::remove_file(&archive);
    if !status.status.success() {
        bail!("couldn't unpack the archive: {}", String::from_utf8_lossy(&status.stderr).trim());
    }
    let mut found = Vec::new();
    collect(&unpack, &mut found);
    let mut files = Vec::new();
    for path in found {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let lower = name.to_ascii_lowercase();
        let font = lower.ends_with(".ttf") || lower.ends_with(".otf");
        let license = lower.starts_with("license") || lower.starts_with("ofl");
        if (font && nerd::keep(&name, &job.nerd_variants)) || license {
            if files.contains(&name) {
                continue;
            }
            std::fs::rename(&path, stage.join(&name))?;
            files.push(name);
        }
    }
    std::fs::remove_dir_all(&unpack)?;
    if !files.iter().any(|f| {
        let f = f.to_ascii_lowercase();
        f.ends_with(".ttf") || f.ends_with(".otf")
    }) {
        bail!("the archive had no fonts in the chosen variants");
    }
    Ok(files)
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn refresh_cache(dir: &Path) {
    let _ = Command::new("fc-cache").arg("-f").arg(dir).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Downloads real fonts into a scratch data folder:
    /// `cargo test -- --ignored installs_and_removes`.
    #[test]
    #[ignore]
    fn installs_and_removes() {
        let scratch = std::env::temp_dir().join(format!("nexus-fonts-test-{}", std::process::id()));
        // SAFETY: this test runs alone (it's ignored by default) and sets these first.
        unsafe {
            std::env::set_var("XDG_DATA_HOME", scratch.join("data"));
            std::env::set_var("XDG_CACHE_HOME", scratch.join("cache"));
        }
        let google_list = google::parse(&http::get_text(google::METADATA_URL).unwrap()).unwrap();
        let release = nerd::parse_release(&http::get_text(nerd::RELEASE_URL).unwrap()).unwrap();
        let nerd_list = nerd::parse(&http::get_text(nerd::FONTS_URL).unwrap(), &release).unwrap();
        let job = |entry: &FontEntry, kind, record| Job {
            kind,
            entry: entry.clone(),
            record,
            google_files: "static".into(),
            nerd_variants: "mono".into(),
            refresh_cache: false,
            release: release.clone(),
        };
        let quiet = |_: String| {};

        let fira = google_list.iter().find(|e| e.family == "Fira Code").unwrap();
        let rec = run(&job(fira, Kind::Install, None), &quiet).unwrap().unwrap();
        assert!(rec.dir.starts_with(scratch.join("data/fonts/nexus-fonts/google")));
        assert!(rec.files.iter().any(|f| f == "FiraCode-Regular.ttf"), "{:?}", rec.files);
        assert!(rec.files.iter().any(|f| f == "OFL.txt"));
        assert!(!rec.files.iter().any(|f| f.contains("VariableFont")));
        assert!(rec.files.iter().all(|f| rec.dir.join(f).is_file()));

        let hack = nerd_list.iter().find(|e| e.id == "Hack").unwrap();
        let nrec = run(&job(hack, Kind::Install, None), &quiet).unwrap().unwrap();
        let fonts: Vec<_> = nrec.files.iter().filter(|f| f.ends_with(".ttf")).collect();
        assert_eq!(fonts.len(), 4, "{:?}", nrec.files);
        assert!(fonts.iter().all(|f| f.contains("NerdFontMono")));
        assert!(!nrec.dir.join(".unpack").exists());

        // Updating swaps the folder in place.
        let again = run(&job(hack, Kind::Update, Some(nrec.clone())), &quiet).unwrap().unwrap();
        assert_eq!(again.dir, nrec.dir);

        for r in [rec, again] {
            assert!(run(&job(fira, Kind::Remove, Some(r.clone())), &quiet).unwrap().is_none());
            assert!(!r.dir.exists());
        }
        let _ = std::fs::remove_dir_all(&scratch);
    }
}
