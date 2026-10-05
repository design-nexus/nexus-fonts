//! Every page in the sidebar, in order.

use crate::widgets::Page;

pub mod browse;
pub mod google;
pub mod installed;
pub mod nerd;
pub mod search;
pub mod settings;
pub mod system;

pub struct Section {
    pub id: &'static str,
    pub title: &'static str,
    pub icon: &'static str,
    pub group: &'static str,
    pub description: &'static str,
    pub build: fn(&Page),
    /// The page manages its own scrolling (tables).
    pub fill: bool,
    /// Listed in the sidebar (search and settings aren't).
    pub nav: bool,
}

pub fn all() -> Vec<Section> {
    let s = |id, title, icon, group, description, build: fn(&Page)| Section {
        id,
        title,
        icon,
        group,
        description,
        build,
        fill: true,
        nav: true,
    };
    vec![
        s(
            "google",
            "Google Fonts",
            "fonts-google-symbolic",
            "Browse",
            "Every family on Google Fonts, free to use.",
            google::build,
        ),
        s("nerd", "Nerd Fonts", "fonts-nerd-symbolic", "Browse", "Coding fonts patched with thousands of icons.", nerd::build),
        s(
            "installed",
            "Installed",
            "fonts-installed-symbolic",
            "Library",
            "Fonts installed with this app, ready to update or remove.",
            installed::build,
        ),
        s("system", "System fonts", "fonts-system-symbolic", "Library", "Every font family on this computer.", system::build),
        Section {
            nav: false,
            ..s(
                "search",
                "Search",
                "system-search-symbolic",
                "Browse",
                "Fonts from both catalogs matching your search.",
                search::build,
            )
        },
        Section {
            nav: false,
            fill: false,
            ..s("settings", "Settings", "emblem-system-symbolic", "App", "Installing, previews and this window.", settings::build)
        },
    ]
}
