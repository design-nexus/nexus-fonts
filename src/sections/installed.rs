//! Fonts installed with this app: update the ones with newer files, or remove them.

use super::browse::{self, Browse};
use crate::catalog::{self, FontEntry, PreviewFile};
use crate::fonttable::Col;
use crate::widgets::{self, Page};
use crate::{events, install, installed};
use gtk::prelude::*;

/// Installed fonts as table rows: the catalog's entry when it has one,
/// previewed from the installed files.
fn entries() -> Vec<FontEntry> {
    installed::all()
        .iter()
        .map(|r| {
            let mut e = catalog::find(r.source, &r.id).unwrap_or_else(|| install::stub(r));
            let font = r.files.iter().find(|f| {
                let f = f.to_ascii_lowercase();
                f.ends_with(".ttf") || f.ends_with(".otf")
            });
            let regular = r.files.iter().find(|f| f.contains("-Regular.")).or(font);
            if let Some(f) = regular {
                e.preview = PreviewFile::Local(r.dir.join(f));
            }
            // Show the installed version; the status column says when there's a newer one.
            e.version = r.version.clone();
            e
        })
        .collect()
}

pub fn build(page: &Page) {
    let b =
        browse::build(page, &[Col::Family, Col::Source, Col::Version, Col::Size, Col::Status], "name", "Filter installed fonts");
    let update_all = widgets::labeled_button("software-update-available-symbolic", "Update all");
    update_all.add_css_class("suggested-action");
    update_all.connect_clicked(|_| install::update_all());
    b.toolbar.append(&update_all);
    let remove_all = widgets::two_click("Remove all", "Click again to remove all", install::remove_all);
    remove_all.set_tooltip_text(Some("Delete every font installed with this app"));
    b.toolbar.append(&remove_all);

    let refresh = {
        let (b, update_all, remove_all) = (b.clone(), update_all.clone(), remove_all.clone());
        move || {
            b.set_entries(&entries());
            let n = installed::outdated().len();
            update_all.set_visible(n > 0);
            update_all.set_tooltip_text(Some(&crate::fmt::count(n, "font has newer files", "fonts have newer files")));
            remove_all.set_visible(!installed::all().is_empty());
        }
    };
    refresh();
    let r = refresh.clone();
    events::subscribe(&b.toolbar, move |c| {
        if matches!(c, events::Change::Installed | events::Change::Catalog) {
            r();
        }
    });
    track(&b);
}

fn track(b: &Browse) {
    b.track_state("Nothing installed yet", "Fonts you install from Google Fonts or Nerd Fonts show up here.");
    // An empty list is final once loaded, not "loading".
    let b2 = b.clone();
    events::subscribe(&b.toolbar, move |c| {
        if c == events::Change::Installed {
            b2.update_state("Nothing installed yet", "Fonts you install from Google Fonts or Nerd Fonts show up here.");
        }
    });
}
