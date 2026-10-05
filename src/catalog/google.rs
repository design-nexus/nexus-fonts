//! Google Fonts: the family list from `fonts.google.com/metadata/fonts`, and
//! each family's files from the download manifest its Download button uses.

use super::{FontEntry, PreviewFile, Source};
use crate::http;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;

pub const METADATA_URL: &str = "https://fonts.google.com/metadata/fonts";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Metadata {
    family_metadata_list: Vec<Family>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Family {
    family: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    fonts: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    axes: Vec<serde_json::Value>,
    #[serde(default)]
    designers: Vec<String>,
    #[serde(default)]
    popularity: u32,
    #[serde(default)]
    trending: u32,
    #[serde(default)]
    date_added: String,
    #[serde(default)]
    last_modified: String,
    #[serde(default)]
    subsets: Vec<String>,
}

/// Google spells it "Sans Serif"; the app uses sentence case.
fn category(c: &str) -> String {
    match c {
        "Sans Serif" => "Sans serif".into(),
        other => other.to_string(),
    }
}

pub fn parse(text: &str) -> Result<Vec<FontEntry>> {
    let meta: Metadata = serde_json::from_str(text).context("unexpected Google Fonts list")?;
    Ok(meta
        .family_metadata_list
        .into_iter()
        .map(|f| {
            let category = category(&f.category);
            let detail = f.designers.join(", ");
            let haystack = format!("{} {} {} {}", f.family, category, detail, f.subsets.join(" ")).to_lowercase();
            FontEntry {
                source: Source::Google,
                id: f.family.clone(),
                family: f.family,
                category,
                styles: f.fonts.len(),
                detail,
                license: "Open Font License".into(),
                version: f.last_modified,
                popularity: f.popularity,
                trending: f.trending,
                added: f.date_added,
                variable: !f.axes.is_empty(),
                preview: PreviewFile::Google,
                haystack,
            }
        })
        .collect())
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileRef {
    pub filename: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InlineFile {
    pub filename: String,
    pub contents: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    #[serde(default)]
    pub files: Vec<InlineFile>,
    #[serde(default)]
    pub file_refs: Vec<FileRef>,
}

#[derive(Deserialize)]
struct ManifestAnswer {
    manifest: Manifest,
}

/// The answer starts with `)]}'` to stop it being run as a script.
pub fn parse_manifest(text: &str) -> Result<Manifest> {
    let json = text.trim_start().strip_prefix(")]}'").unwrap_or(text);
    let a: ManifestAnswer = serde_json::from_str(json).context("unexpected Google Fonts download list")?;
    Ok(a.manifest)
}

pub fn manifest(family: &str) -> Result<Manifest> {
    let url = format!("https://fonts.google.com/download/list?family={}", http::encode(family));
    parse_manifest(&http::get_text(&url)?)
}

fn is_font(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with(".ttf") || n.ends_with(".otf")
}

/// The font files to install: `static`, `variable` or `both`. A family with
/// only one kind gets that kind whatever the choice.
pub fn pick_files<'a>(refs: &'a [FileRef], which: &str) -> Vec<&'a FileRef> {
    let fonts: Vec<&FileRef> = refs.iter().filter(|r| is_font(&r.filename)).collect();
    let variable: Vec<&FileRef> = fonts.iter().copied().filter(|r| r.filename.contains("VariableFont")).collect();
    let fixed: Vec<&FileRef> = fonts.iter().copied().filter(|r| !r.filename.contains("VariableFont")).collect();
    match which {
        "variable" if !variable.is_empty() => variable,
        "static" if !fixed.is_empty() => fixed,
        _ => fonts,
    }
}

/// The one file a preview needs: the regular style, or the upright variable font.
pub fn preview_file(refs: &[FileRef]) -> Option<&FileRef> {
    let fonts: Vec<&FileRef> = refs.iter().filter(|r| is_font(&r.filename)).collect();
    let base = |r: &&FileRef| r.filename.rsplit('/').next().unwrap_or(&r.filename).to_string();
    fonts
        .iter()
        .find(|r| base(r).ends_with("-Regular.ttf"))
        .or_else(|| fonts.iter().find(|r| r.filename.contains("VariableFont") && !base(r).contains("Italic")))
        .or_else(|| fonts.first())
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    const META: &str = r#"{"axisRegistry":[],"familyMetadataList":[
        {"family":"Fira Code","category":"Monospace","fonts":{"300":{},"400":{},"700":{}},
         "axes":[{"tag":"wght"}],"designers":["Nikita Prokopov"],"popularity":66,"trending":82,
         "dateAdded":"2019-05-07","lastModified":"2025-01-01","subsets":["latin","cyrillic"]},
        {"family":"ABeeZee","category":"Sans Serif","fonts":{"400":{},"400i":{}},"axes":[],
         "designers":["Anja Meiners"],"popularity":90,"trending":1066}
    ]}"#;

    #[test]
    fn parses_families() {
        let list = parse(META).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].family, "Fira Code");
        assert_eq!(list[0].styles, 3);
        assert!(list[0].variable);
        assert!(list[0].haystack.contains("cyrillic"));
        assert_eq!(list[1].category, "Sans serif");
        assert!(!list[1].variable);
    }

    fn refs(names: &[&str]) -> Vec<FileRef> {
        names.iter().map(|n| FileRef { filename: n.to_string(), url: format!("https://x/{n}") }).collect()
    }

    #[test]
    fn reads_manifest() {
        let text = r#")]}'
{"zipName":"Roboto_Mono.zip","manifest":{"files":[{"filename":"OFL.txt","contents":"license"}],
 "fileRefs":[{"filename":"static/RobotoMono-Regular.ttf","url":"https://g/a.ttf","date":{}}]}}"#;
        let m = parse_manifest(text).unwrap();
        assert_eq!(m.files[0].filename, "OFL.txt");
        assert_eq!(m.file_refs[0].url, "https://g/a.ttf");
    }

    #[test]
    fn picks_files() {
        let r = refs(&[
            "RobotoMono-VariableFont_wght.ttf",
            "RobotoMono-Italic-VariableFont_wght.ttf",
            "static/RobotoMono-Regular.ttf",
            "static/RobotoMono-Bold.ttf",
            "README.txt",
        ]);
        assert_eq!(pick_files(&r, "static").len(), 2);
        assert_eq!(pick_files(&r, "variable").len(), 2);
        assert_eq!(pick_files(&r, "both").len(), 4);
        assert_eq!(preview_file(&r).unwrap().filename, "static/RobotoMono-Regular.ttf");
        // Only static files: "variable" still installs something.
        let only = refs(&["ABeeZee-Regular.ttf", "ABeeZee-Italic.ttf"]);
        assert_eq!(pick_files(&only, "variable").len(), 2);
        let var = refs(&["Inter-Italic-VariableFont_opsz,wght.ttf", "Inter-VariableFont_opsz,wght.ttf"]);
        assert_eq!(preview_file(&var).unwrap().filename, "Inter-VariableFont_opsz,wght.ttf");
    }
}
