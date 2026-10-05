//! Nerd Fonts: coding fonts patched with icons, from the latest release.

use super::browse;
use crate::catalog;
use crate::events;
use crate::fonttable::Col;
use crate::widgets::Page;

pub fn build(page: &Page) {
    let b = browse::build(page, &[Col::Family, Col::Category, Col::Version, Col::Status], "name", "Filter Nerd Fonts");
    b.chips(&[("", "All"), ("Monospace", "Monospace"), ("Proportional", "Proportional")]);
    b.set_entries(&catalog::nerd());
    let b2 = b.clone();
    events::subscribe(&b.toolbar, move |c| {
        if c == events::Change::Catalog {
            b2.set_entries(&catalog::nerd());
        }
    });
    b.track_state("No fonts", "The Nerd Fonts list is empty.");
}
