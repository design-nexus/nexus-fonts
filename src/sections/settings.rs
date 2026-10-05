use crate::widgets::{self, Page};
use crate::{catalog, events, fmt, paths, prefs, theme, window};
use gtk::glib;
use gtk::prelude::*;

pub fn build(page: &Page) {
    let p = prefs::get();

    // ----- Installing -----
    let g = page.group("Installing");
    g.note(&format!(
        "Fonts install for your user only, in <tt>{}</tt>.",
        glib::markup_escape_text(&paths::pretty(&paths::fonts_dir()))
    ));
    g.add(&widgets::segmented_row(
        "Google Fonts files",
        "Static files have one file per style and work everywhere. Variable files hold every weight in one.",
        widgets::opts(&[("static", "Static"), ("variable", "Variable"), ("both", "Both")]),
        &p.google_files,
        |v| prefs::update(|p| p.google_files = v),
    ));
    g.add(&widgets::segmented_row(
        "Nerd Fonts variants",
        "Mono has single-width icons, for terminals. The regular and Propo variants suit editors and documents.",
        widgets::opts(&[("all", "All"), ("mono", "Mono only"), ("regular", "No Mono")]),
        &p.nerd_variants,
        |v| prefs::update(|p| p.nerd_variants = v),
    ));
    let (r, _) = widgets::switch_row(
        "Refresh the font cache",
        "Run fc-cache after installing or removing, so other apps see the change straight away.",
        p.refresh_cache,
        |on| prefs::update(|p| p.refresh_cache = on),
    );
    g.add(&r);

    // ----- Preview -----
    let g = page.group("Preview");
    let (r, _) = widgets::button_row("Clear the preview cache", "Delete downloaded preview files.", "Clear", |_| {
        let dir = paths::preview_dir();
        let size: u64 =
            std::fs::read_dir(&dir).map(|d| d.flatten().filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum()).unwrap_or(0);
        let _ = std::fs::remove_dir_all(&dir);
        window::toast(&format!("Cleared {} of previews.", fmt::bytes(size)));
    });
    g.add(&r);

    // ----- Catalog -----
    let g = page.group("Catalog");
    let (row, button) = widgets::button_row("Refresh the font lists", "", "Refresh now", |_| catalog::refresh());
    g.add(&row);
    let desc = row.first_child().and_then(|t| t.first_child()).and_then(|t| t.next_sibling()).and_downcast::<gtk::Label>();
    let label = widgets::label("", "settings-option-description");
    label.set_wrap(true);
    if let Some(text) = row.first_child().and_downcast::<gtk::Box>()
        && desc.is_none()
    {
        text.append(&label);
    }
    let refresh = {
        let (label, button) = (label.clone(), button.clone());
        move || {
            button.set_sensitive(!catalog::loading());
            let fetched = prefs::get().catalog_fetched;
            let when = if catalog::loading() {
                "Updating now…".to_string()
            } else if fetched == 0 {
                "Not fetched yet.".to_string()
            } else {
                format!("Last updated {}. They update once a day.", fmt::date(fetched))
            };
            label.set_text(&format!(
                "{} Google Fonts families and {} Nerd Fonts ({}). {when}",
                fmt::thousands(catalog::google().len()),
                catalog::nerd().len(),
                if catalog::nerd_release().is_empty() { "release unknown".into() } else { catalog::nerd_release() },
            ));
        }
    };
    refresh();
    events::subscribe(&label, move |c| {
        if c == events::Change::Catalog {
            refresh();
        }
    });

    // ----- Fonts window -----
    let g = page.group("Fonts window");
    let app_themes = theme::all();
    let options: Vec<(String, String)> = app_themes.iter().map(|t| (t.id.clone(), t.name.clone())).collect();
    let (theme_row, theme_dd) = widgets::choice_row(
        "Theme",
        "Dracula, Catppuccin, Tokyo Night, One Dark Pro and more. Add your own in <tt>~/.config/nexus-fonts/themes</tt>.",
        options,
        &p.theme,
        |id| {
            prefs::update(|p| {
                p.theme = id;
                p.mode = prefs::ThemeMode::Theme;
            });
            theme::apply();
        },
    );
    theme_dd.set_sensitive(p.mode == prefs::ThemeMode::Theme || !theme::omarchy_available());
    if theme::omarchy_available() {
        let dd = theme_dd.clone();
        let (r, _) = widgets::switch_row(
            "Follow Omarchy theme",
            "Match the desktop's colors and update live whenever the Omarchy theme changes.",
            p.mode == prefs::ThemeMode::Omarchy,
            move |on| {
                prefs::update(|p| p.mode = if on { prefs::ThemeMode::Omarchy } else { prefs::ThemeMode::Theme });
                dd.set_sensitive(!on);
                theme::apply();
            },
        );
        g.add(&r);
    }
    g.add(&theme_row);

    let swatches = widgets::hbox(4);
    let refresh_swatches = {
        let swatches = swatches.clone();
        move || {
            while let Some(c) = swatches.first_child() {
                swatches.remove(&c);
            }
            let pal = theme::current_palette();
            for c in [&pal.bg, &pal.surface, &pal.muted, &pal.text, &pal.accent, &pal.danger] {
                let s = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                s.add_css_class("swatch");
                let provider = gtk::CssProvider::new();
                provider.load_from_string(&format!("box {{ background: {c}; }}"));
                #[allow(deprecated)]
                s.style_context().add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_USER);
                swatches.append(&s);
            }
        }
    };
    refresh_swatches();
    let last = std::cell::RefCell::new(theme::current_palette());
    let weak = swatches.downgrade();
    glib::timeout_add_seconds_local(1, move || {
        if weak.upgrade().is_none() {
            return glib::ControlFlow::Break;
        }
        let now = theme::current_palette();
        if *last.borrow() != now {
            *last.borrow_mut() = now;
            refresh_swatches();
        }
        glib::ControlFlow::Continue
    });
    g.add(&widgets::row("Current colors", "", Some(swatches.upcast_ref())));
    let (r, _) = widgets::switch_row("Glow", "Soft accent glow around focused and selected elements.", p.glow, |on| {
        prefs::update(|p| p.glow = on);
        theme::apply();
    });
    g.add(&r);
    let (r, _) =
        widgets::switch_row("Reduce motion", "Turn off transitions and animations in this window.", p.reduce_motion, |on| {
            prefs::update(|p| p.reduce_motion = on);
            theme::apply();
        });
    g.add(&r);

    // ----- Keyboard -----
    let g = page.group("Keyboard");
    for (keys, what) in window::SHORTCUTS {
        g.add(&widgets::row(what, "", Some(widgets::key_caps(keys).upcast_ref())));
    }
}
