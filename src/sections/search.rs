//! Search across both catalogs, from the top bar.

use super::browse::{self, Browse};
use crate::catalog::{self, FontEntry};
use crate::events;
use crate::fonttable::Col;
use crate::widgets::Page;
use gtk::prelude::*;
use std::cell::RefCell;

thread_local! {
    static PAGE: RefCell<Option<(Browse, String)>> = const { RefCell::new(None) };
}

fn results(query: &str) -> Vec<FontEntry> {
    let terms = catalog::terms(query);
    if terms.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<FontEntry> = catalog::nerd().iter().filter(|e| e.matches(&terms)).cloned().collect();
    let mut google: Vec<FontEntry> = catalog::google().iter().filter(|e| e.matches(&terms)).cloned().collect();
    google.sort_by_key(|e| e.popularity);
    out.extend(google);
    out
}

pub fn build(page: &Page) {
    let b = browse::build(page, &[Col::Family, Col::Source, Col::Category, Col::Status], "source", "Narrow the results");
    let b2 = b.clone();
    events::subscribe(&b.toolbar, move |c| {
        if c == events::Change::Catalog {
            let q = PAGE.with(|p| p.borrow().as_ref().map(|p| p.1.clone())).unwrap_or_default();
            b2.set_entries(&results(&q));
        }
    });
    b.track_state("Search for fonts", "Type in the search box to find fonts in both catalogs.");
    PAGE.with(|p| *p.borrow_mut() = Some((b, String::new())));
}

pub fn set_query(query: &str) {
    PAGE.with(|p| {
        if let Some((b, q)) = p.borrow_mut().as_mut() {
            *q = query.to_string();
            b.filter.set_text("");
            b.set_entries(&results(query));
            b.update_state("No results", "Nothing in either catalog matches.");
        }
    });
}

/// Enter in the top bar's search moves to the results.
pub fn focus_results() {
    PAGE.with(|p| {
        if let Some((b, _)) = p.borrow().as_ref() {
            b.table.view.grab_focus();
        }
    });
}
