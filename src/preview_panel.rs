//! The panel under each table: the selected font's details, a live preview in
//! the font itself, and its Install, Update or Remove button.

use crate::catalog::{FontEntry, Source};
use crate::{cmd, events, fmt, install, installed, prefs, preview, widgets};
use gtk::pango;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ\nabcdefghijklmnopqrstuvwxyz\n0123456789 !?&@#$%*()[]{}<>/=+-_";
/// A few Nerd Font icons: folders, git, languages, the terminal.
const ICONS: &str = "\u{f07b} \u{e702} \u{f418} \u{e7a8} \u{e718} \u{e73c} \u{e606} \u{f489} \u{f0e7} \u{f17c} \u{f303} \u{e795}";

struct Inner {
    entry: RefCell<Option<FontEntry>>,
    /// Bumped on every selection, so a slow download can't show the wrong font.
    generation: Cell<u64>,
    /// The action buttons' last state, so progress doesn't rebuild them.
    action_state: RefCell<String>,
    title: gtk::Label,
    meta: gtk::Label,
    detail: gtk::Label,
    actions: gtk::Box,
    progress: gtk::Label,
    fonts: gtk::Box,
    sample: gtk::Label,
    icons: gtk::Label,
    message: gtk::Label,
    stack: gtk::Stack,
    content: gtk::Box,
}

#[derive(Clone)]
pub struct PreviewPanel {
    pub root: gtk::Box,
    inner: Rc<Inner>,
}

fn sized(label: &gtk::Label, points: f64) {
    let attrs = pango::AttrList::new();
    attrs.insert(pango::AttrSize::new((points * pango::SCALE as f64) as i32));
    label.set_attributes(Some(&attrs));
}

impl PreviewPanel {
    pub fn new() -> PreviewPanel {
        let root = widgets::vbox(0);
        root.add_css_class("preview-panel");

        let stack = gtk::Stack::new();
        stack.set_vexpand(true);
        let none = widgets::label("Select a font to preview it.", "dim");
        none.set_xalign(0.5);
        none.set_valign(gtk::Align::Center);
        stack.add_named(&none, Some("none"));

        let content = widgets::vbox(10);
        content.add_css_class("preview-content");
        let head = widgets::hbox(12);
        let text = widgets::vbox(2);
        text.set_hexpand(true);
        let title = widgets::label("", "preview-title");
        title.set_ellipsize(pango::EllipsizeMode::End);
        let meta = widgets::label("", "dim");
        meta.set_ellipsize(pango::EllipsizeMode::End);
        let detail = widgets::label("", "preview-detail");
        detail.set_wrap(true);
        text.append(&title);
        text.append(&meta);
        text.append(&detail);
        head.append(&text);
        let right = widgets::vbox(4);
        right.set_valign(gtk::Align::Start);
        let actions = widgets::hbox(8);
        actions.set_halign(gtk::Align::End);
        let progress = widgets::label("", "preview-progress");
        progress.set_xalign(1.0);
        progress.set_visible(false);
        right.append(&actions);
        right.append(&progress);
        head.append(&right);
        content.append(&head);

        // Sample text and size.
        let controls = widgets::hbox(10);
        let entry = gtk::Entry::new();
        entry.set_text(&prefs::get().sample);
        entry.set_placeholder_text(Some(prefs::DEFAULT_SAMPLE));
        entry.set_hexpand(true);
        entry.set_tooltip_text(Some("Type your own sample text"));
        controls.append(&entry);
        let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 10.0, 120.0, 1.0);
        scale.set_value(prefs::get().preview_size);
        scale.set_draw_value(false);
        scale.set_width_request(140);
        scale.set_tooltip_text(Some("Preview size"));
        controls.append(&scale);
        let readout = widgets::label(&format!("{:.0} pt", prefs::get().preview_size), "value-readout");
        readout.set_width_chars(6);
        controls.append(&readout);
        content.append(&controls);

        // The text drawn in the previewed font.
        let fonts = widgets::vbox(10);
        fonts.add_css_class("preview-fonts");
        let sample = gtk::Label::new(None);
        sample.add_css_class("preview-text");
        sample.set_xalign(0.0);
        sample.set_wrap(true);
        sample.set_wrap_mode(pango::WrapMode::WordChar);
        sample.set_selectable(true);
        let alphabet = gtk::Label::new(Some(ALPHABET));
        alphabet.add_css_class("preview-text");
        alphabet.add_css_class("dim");
        alphabet.set_xalign(0.0);
        alphabet.set_wrap(true);
        alphabet.set_wrap_mode(pango::WrapMode::Char);
        sized(&alphabet, 15.0);
        fonts.append(&sample);
        fonts.append(&alphabet);
        let message = widgets::label("", "dim");
        message.set_wrap(true);
        message.set_visible(false);
        // Icons come from the window's own fonts: the base font has none.
        let icons = widgets::label(ICONS, "preview-icons");
        icons.set_tooltip_text(Some("Nerd Fonts add thousands of icons like these"));
        icons.set_visible(false);
        let body = widgets::vbox(10);
        body.append(&message);
        body.append(&fonts);
        body.append(&icons);
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::External)
            .vexpand(true)
            .child(&body)
            .build();
        content.append(&scroll);
        stack.add_named(&content, Some("content"));
        root.append(&stack);

        let inner = Rc::new(Inner {
            entry: RefCell::new(None),
            generation: Cell::new(0),
            action_state: RefCell::new(String::new()),
            title,
            meta,
            detail,
            actions,
            progress,
            fonts,
            sample,
            icons,
            message,
            stack,
            content,
        });
        let panel = PreviewPanel { root, inner };
        panel.set_sample(&prefs::get().sample);
        panel.set_size(prefs::get().preview_size);

        let p = panel.clone();
        entry.connect_changed(move |e| {
            let text = e.text().to_string();
            prefs::update(|pr| pr.sample = text.clone());
            p.set_sample(&text);
        });
        let p = panel.clone();
        scale.connect_value_changed(move |s| {
            let v = s.value().round();
            prefs::update(|pr| pr.preview_size = v);
            readout.set_text(&format!("{v:.0} pt"));
            p.set_size(v);
        });
        let p = panel.clone();
        events::subscribe(&panel.root, move |c| {
            if matches!(c, events::Change::Installed | events::Change::Jobs | events::Change::Catalog) {
                p.refresh_actions();
            }
        });
        panel
    }

    fn set_sample(&self, text: &str) {
        let text = if text.trim().is_empty() { prefs::DEFAULT_SAMPLE } else { text };
        self.inner.sample.set_text(text);
    }

    fn set_size(&self, points: f64) {
        sized(&self.inner.sample, points);
    }

    pub fn show(&self, entry: Option<FontEntry>) {
        let i = &self.inner;
        let same = match (&*i.entry.borrow(), &entry) {
            (Some(a), Some(b)) => a.key() == b.key() && a.version == b.version,
            (None, None) => true,
            _ => false,
        };
        if same {
            self.refresh_actions();
            return;
        }
        *i.entry.borrow_mut() = entry.clone();
        i.action_state.borrow_mut().clear();
        let Some(e) = entry else {
            i.stack.set_visible_child_name("none");
            return;
        };
        i.stack.set_visible_child(&i.content);
        i.title.set_text(&e.family);
        i.title.set_tooltip_text(Some(&e.family));
        let mut meta = vec![e.source.label().to_string()];
        if !e.category.is_empty() {
            meta.push(e.category.clone());
        }
        if e.styles > 0 {
            meta.push(fmt::count(e.styles, "style", "styles"));
        }
        if e.variable {
            meta.push("Variable".into());
        }
        if !e.license.is_empty() {
            meta.push(e.license.clone());
        }
        i.meta.set_text(&meta.join(" · "));
        i.detail.set_text(&e.detail);
        i.detail.set_visible(!e.detail.is_empty());
        i.icons.set_visible(e.source == Source::Nerd);
        self.refresh_actions();

        // Fetch the font file, then draw the sample in it.
        let generation = i.generation.get() + 1;
        i.generation.set(generation);
        i.fonts.set_opacity(0.0);
        i.message.set_text("Loading the preview…");
        i.message.set_visible(true);
        let me = self.clone();
        let wanted = e.clone();
        cmd::background(
            move || preview::fetch(&wanted).map_err(|e| format!("{e:#}")),
            move |res| {
                let i = &me.inner;
                if i.generation.get() != generation {
                    return;
                }
                match res.map(|path| preview::font_map(&path)) {
                    Ok(Some(map)) => {
                        i.fonts.set_font_map(Some(&map));
                        i.message.set_visible(false);
                        i.fonts.set_opacity(1.0);
                    }
                    Ok(None) => i.message.set_text("This font file couldn't be read."),
                    Err(e) => i.message.set_text(&format!("No preview: {e}")),
                }
            },
        );
    }

    /// Install, Update, Remove or Open folder, and any progress.
    fn refresh_actions(&self) {
        let i = &self.inner;
        let Some(e) = i.entry.borrow().clone() else { return };
        let busy = install::busy(&e.key());
        let record = installed::get(e.source, &e.id);
        i.progress.set_visible(busy.is_some());
        i.progress.set_text(busy.as_deref().unwrap_or(""));
        let state =
            format!("{:?}|{}|{}|{}", e.key(), busy.is_some(), record.is_some(), record.as_ref().is_some_and(|r| r.outdated()));
        if *i.action_state.borrow() == state {
            return;
        }
        *i.action_state.borrow_mut() = state;
        while let Some(c) = i.actions.first_child() {
            i.actions.remove(&c);
        }
        if e.source == Source::System {
            if let crate::catalog::PreviewFile::Local(p) = &e.preview
                && let Some(dir) = p.parent()
            {
                let open = widgets::labeled_button("folder-open-symbolic", "Open folder");
                let dir = dir.to_path_buf();
                open.connect_clicked(move |_| cmd::spawn(&["xdg-open", &dir.to_string_lossy()]));
                i.actions.append(&open);
            }
            return;
        }
        if busy.is_some() {
            let b = gtk::Button::with_label("Working…");
            b.set_sensitive(false);
            i.actions.append(&b);
            return;
        }
        match record {
            Some(r) => {
                let open = widgets::icon_button("folder-open-symbolic", "Open the font's folder");
                let dir = r.dir.clone();
                open.connect_clicked(move |_| cmd::spawn(&["xdg-open", &dir.to_string_lossy()]));
                i.actions.append(&open);
                let r2 = r.clone();
                let remove = widgets::two_click("Remove", "Click again to remove", move || install::remove(&r2));
                remove.set_tooltip_text(Some("Delete this font's files"));
                i.actions.append(&remove);
                if r.outdated() {
                    let update = widgets::labeled_button("software-update-available-symbolic", "Update");
                    update.add_css_class("suggested-action");
                    update.set_tooltip_text(Some(&format!("{} → {}", r.version, e.version)));
                    update.connect_clicked(move |_| install::update(&r));
                    i.actions.append(&update);
                }
            }
            None => {
                let b = widgets::labeled_button("folder-download-symbolic", "Install");
                b.add_css_class("suggested-action");
                b.set_tooltip_text(Some("Install for your user, in ~/.local/share/fonts"));
                let e2 = e.clone();
                b.connect_clicked(move |_| install::install(&e2));
                i.actions.append(&b);
            }
        }
    }
}
