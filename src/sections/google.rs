//! Google Fonts: every family, filtered by category and sorted by popularity.

use super::browse;
use crate::fonttable::Col;
use crate::widgets::{self, Page};
use crate::{catalog, events, prefs};
use gtk::prelude::*;

const SORTS: &[(&str, &str)] = &[("popular", "Popular"), ("trending", "Trending"), ("name", "Name"), ("newest", "Newest")];

pub fn build(page: &Page) {
    let sort = prefs::get().google_sort;
    let b = browse::build(page, &[Col::Family, Col::Category, Col::Styles, Col::Status], &sort, "Filter Google Fonts");
    let dd = widgets::dropdown(&widgets::opts(SORTS), &sort);
    dd.set_tooltip_text(Some("Sort"));
    let t = b.table.clone();
    dd.connect_selected_notify(move |d| {
        if let Some((id, _)) = SORTS.get(d.selected() as usize) {
            prefs::update(|p| p.google_sort = id.to_string());
            t.set_sort(id);
        }
    });
    b.toolbar.append(&dd);
    b.chips(&[
        ("", "All"),
        ("Sans serif", "Sans serif"),
        ("Serif", "Serif"),
        ("Display", "Display"),
        ("Handwriting", "Handwriting"),
        ("Monospace", "Monospace"),
    ]);
    b.set_entries(&catalog::google());
    let b2 = b.clone();
    events::subscribe(&b.toolbar, move |c| {
        if c == events::Change::Catalog {
            b2.set_entries(&catalog::google());
        }
    });
    b.track_state("No fonts", "The Google Fonts list is empty.");
}
