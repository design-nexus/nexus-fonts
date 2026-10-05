//! Every font family on this computer, from fontconfig.

use super::browse;
use crate::fonttable::Col;
use crate::widgets::{self, Page};
use crate::{cmd, events, system};
use gtk::prelude::*;

pub fn build(page: &Page) {
    let b = browse::build(page, &[Col::Family, Col::Category, Col::Styles], "name", "Filter system fonts");
    let reload = widgets::icon_button("view-refresh-symbolic", "Read the font list again");
    b.toolbar.append(&reload);
    b.chips(&[("", "All"), ("Installed here", "Installed here"), ("User", "User"), ("System", "System")]);
    let load = {
        let b = b.clone();
        move || {
            let b = b.clone();
            cmd::background(system::list, move |list| {
                b.set_entries(&list);
                b.update_state("No fonts", "fontconfig lists no fonts.");
            });
        }
    };
    load();
    let l = load.clone();
    reload.connect_clicked(move |_| l());
    // Installs and removals change the list.
    let l = load.clone();
    events::subscribe(&b.toolbar, move |c| {
        if c == events::Change::Installed {
            l();
        }
    });
    let b2 = b.clone();
    b.table.on_shown_changed(move |_| b2.update_state("No fonts", "fontconfig lists no fonts."));
}
