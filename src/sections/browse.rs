//! The layout every font page shares: a toolbar, the table, and the preview
//! panel under it, split by a draggable hairline.

use crate::catalog::FontEntry;
use crate::fonttable::{Col, FontTable};
use crate::preview_panel::PreviewPanel;
use crate::widgets::{self, Page};
use crate::{catalog, events};
use gtk::prelude::*;
use std::rc::Rc;

#[derive(Clone)]
pub struct Browse {
    pub table: FontTable,
    pub panel: PreviewPanel,
    pub filter: gtk::SearchEntry,
    /// The toolbar's first row: the filter, then whatever the page adds.
    pub toolbar: gtk::Box,
    /// The whole toolbar, which grows a second row for chips.
    pub toolbar_frame: gtk::Box,
    stack: gtk::Stack,
    empty: gtk::Box,
}

pub fn build(page: &Page, cols: &[Col], sort: &str, placeholder: &str) -> Browse {
    let frame = widgets::vbox(0);
    frame.add_css_class("list-toolbar");
    let toolbar = widgets::hbox(8);
    toolbar.add_css_class("toolbar-row");
    let filter = gtk::SearchEntry::new();
    filter.set_placeholder_text(Some(placeholder));
    filter.add_css_class("page-search");
    filter.set_hexpand(true);
    toolbar.append(&filter);
    frame.append(&toolbar);
    page.body.append(&frame);

    let table = FontTable::new(cols, sort);
    let stack = gtk::Stack::new();
    stack.set_vexpand(true);
    stack.add_named(&table.scroll, Some("content"));
    let loading = widgets::vbox(10);
    loading.set_valign(gtk::Align::Center);
    let spinner = gtk::Spinner::new();
    spinner.set_spinning(true);
    spinner.set_size_request(28, 28);
    loading.append(&spinner);
    let l = widgets::label("Loading the font lists…", "dim");
    l.set_xalign(0.5);
    loading.append(&l);
    stack.add_named(&loading, Some("loading"));
    let empty = widgets::vbox(0);
    empty.set_vexpand(true);
    stack.add_named(&empty, Some("empty"));

    let panel = PreviewPanel::new();
    let paned = gtk::Paned::new(gtk::Orientation::Vertical);
    paned.add_css_class("browse-paned");
    paned.set_vexpand(true);
    paned.set_start_child(Some(&stack));
    paned.set_end_child(Some(&panel.root));
    paned.set_resize_start_child(true);
    paned.set_resize_end_child(false);
    paned.set_shrink_start_child(false);
    paned.set_shrink_end_child(false);
    stack.set_size_request(-1, 140);
    panel.root.set_size_request(-1, 250);
    // Start with the preview taking a little under half the page.
    paned.connect_map(|p| {
        let p = p.clone();
        gtk::glib::idle_add_local_once(move || {
            let h = p.height();
            if h > 0 {
                p.set_position((h as f64 * 0.56) as i32);
            }
        });
    });
    page.body.append(&paned);

    let b = Browse { table, panel, filter, toolbar, toolbar_frame: frame, stack, empty };
    let p = b.panel.clone();
    b.table.on_select(move |e| p.show(e));
    let t = b.clone();
    b.filter.connect_search_changed(move |e| t.table.set_query(&e.text()));
    let t = b.clone();
    b.filter.connect_stop_search(move |e| {
        if e.text().is_empty() {
            t.table.view.grab_focus();
        }
        e.set_text("");
    });
    b
}

impl Browse {
    pub fn set_entries(&self, entries: &[FontEntry]) {
        self.table.set_entries(entries);
    }

    /// Show the table, a spinner, or a message, after the rows or the catalog change.
    pub fn update_state(&self, empty_title: &str, empty_desc: &str) {
        let show_empty = |title: &str, desc: &str, retry: bool| {
            while let Some(c) = self.empty.first_child() {
                self.empty.remove(&c);
            }
            let action: widgets::Action<'_> = if retry { Some(("Try again", Box::new(catalog::refresh))) } else { None };
            self.empty.append(&widgets::empty_state("font-x-generic-symbolic", title, desc, action));
            self.stack.set_visible_child_name("empty");
        };
        // No rows, nothing to preview.
        self.panel.root.set_visible(self.table.total() > 0);
        if self.table.total() == 0 {
            if !catalog::loaded() || catalog::loading() {
                self.stack.set_visible_child_name("loading");
            } else if let Some(e) = catalog::error() {
                show_empty("Couldn't load the font lists", &gtk::glib::markup_escape_text(&e), true);
            } else {
                show_empty(empty_title, empty_desc, false);
            }
        } else if self.table.shown() == 0 {
            let q = self.filter.text();
            let what = if q.is_empty() {
                "No fonts in this category.".to_string()
            } else {
                format!("No fonts match “{}”.", gtk::glib::markup_escape_text(q.trim()))
            };
            show_empty("Nothing found", &what, false);
        } else {
            self.stack.set_visible_child_name("content");
        }
    }

    /// Keep `update_state` current as rows filter and the catalog loads.
    pub fn track_state(&self, empty_title: &'static str, empty_desc: &'static str) {
        let b = self.clone();
        self.table.on_shown_changed(move |_| b.update_state(empty_title, empty_desc));
        let b = self.clone();
        events::subscribe(&self.toolbar, move |c| {
            if c == events::Change::Catalog {
                b.update_state(empty_title, empty_desc);
            }
        });
        self.update_state(empty_title, empty_desc);
    }

    /// A second toolbar row of category chips. "" is All.
    pub fn chips(&self, options: &[(&str, &str)]) -> gtk::Box {
        let row = chips(options, {
            let t = self.table.clone();
            move |c| t.set_category(&c)
        });
        self.toolbar_frame.add_css_class("stacked");
        self.toolbar_frame.append(&row);
        row
    }
}

/// Filter chips; exactly one is selected.
pub fn chips(options: &[(&str, &str)], on_change: impl Fn(String) + 'static) -> gtk::Box {
    let row = widgets::hbox(6);
    row.add_css_class("chip-row");
    let on_change = Rc::new(on_change);
    let mut first: Option<gtk::ToggleButton> = None;
    for (id, label) in options {
        let b = gtk::ToggleButton::with_label(label);
        b.add_css_class("chip");
        if let Some(f) = &first {
            b.set_group(Some(f));
        } else {
            b.set_active(true);
            b.add_css_class("selected");
            first = Some(b.clone());
        }
        let (id, cb) = (id.to_string(), on_change.clone());
        b.connect_toggled(move |b| {
            if b.is_active() {
                b.add_css_class("selected");
                cb(id.clone());
            } else {
                b.remove_css_class("selected");
            }
        });
        row.append(&b);
    }
    row
}
