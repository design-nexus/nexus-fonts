//! The font table every page uses: a filterable, sortable `ColumnView` of
//! catalog entries with an install-status column that follows the queue.

use crate::catalog::{FontEntry, Source};
use crate::{events, fmt, install, installed};
use gtk::prelude::*;
use gtk::{gio, glib};
use std::cell::RefCell;
use std::cmp::Ordering;
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Col {
    Family,
    Category,
    Styles,
    Source,
    Version,
    Size,
    Status,
}

impl Col {
    fn title(self) -> &'static str {
        match self {
            Col::Family => "Family",
            Col::Category => "Category",
            Col::Styles => "Styles",
            Col::Source => "Source",
            Col::Version => "Version",
            Col::Size => "Size",
            Col::Status => "Status",
        }
    }
}

fn entry_of(obj: &glib::Object) -> Option<std::cell::Ref<'_, FontEntry>> {
    obj.downcast_ref::<glib::BoxedAnyObject>().map(|b| b.borrow::<FontEntry>())
}

/// The status column's text and style: progress, "Update", "Installed" or nothing.
pub fn status_of(e: &FontEntry) -> (String, &'static str) {
    if let Some(busy) = install::busy(&e.key()) {
        return (busy, "accent");
    }
    match installed::get(e.source, &e.id) {
        Some(r) if r.outdated() => ("Update".into(), "accent"),
        Some(_) => ("Installed".into(), ""),
        None => (String::new(), ""),
    }
}

fn cell_text(col: Col, e: &FontEntry) -> String {
    match col {
        Col::Family => e.family.clone(),
        Col::Category => e.category.clone(),
        Col::Styles => {
            if e.styles == 0 {
                String::new()
            } else {
                e.styles.to_string()
            }
        }
        Col::Source => e.source.label().to_string(),
        Col::Version => e.version.clone(),
        Col::Size => installed::get(e.source, &e.id).map(|r| fmt::bytes(r.bytes)).unwrap_or_default(),
        Col::Status => status_of(e).0,
    }
}

/// A bound status label and the row it shows.
type StatusCell = (glib::WeakRef<gtk::Label>, glib::WeakRef<glib::BoxedAnyObject>);

#[derive(Clone)]
pub struct FontTable {
    pub view: gtk::ColumnView,
    pub scroll: gtk::ScrolledWindow,
    store: gio::ListStore,
    filtered: gtk::FilterListModel,
    selection: gtk::SingleSelection,
    filter: gtk::CustomFilter,
    sorter: gtk::CustomSorter,
    terms: Rc<RefCell<Vec<String>>>,
    category: Rc<RefCell<String>>,
    sort: Rc<RefCell<String>>,
    /// Status labels currently bound, with the row they show.
    status_cells: Rc<RefCell<Vec<StatusCell>>>,
}

fn compare(sort: &str, a: &FontEntry, b: &FontEntry) -> Ordering {
    let name = || a.family.to_lowercase().cmp(&b.family.to_lowercase());
    let rank = |x: u32| if x == 0 { u32::MAX } else { x };
    match sort {
        "popular" => rank(a.popularity).cmp(&rank(b.popularity)).then_with(name),
        "trending" => rank(a.trending).cmp(&rank(b.trending)).then_with(name),
        "newest" => b.added.cmp(&a.added).then_with(name),
        "source" => a.source.label().cmp(b.source.label()).then_with(name),
        _ => name(),
    }
}

impl FontTable {
    pub fn new(cols: &[Col], sort: &str) -> FontTable {
        let store = gio::ListStore::new::<glib::BoxedAnyObject>();
        let terms: Rc<RefCell<Vec<String>>> = Rc::default();
        let category: Rc<RefCell<String>> = Rc::default();
        let (t, c) = (terms.clone(), category.clone());
        let filter = gtk::CustomFilter::new(move |o| {
            let Some(e) = entry_of(o) else { return false };
            let cat = c.borrow();
            (cat.is_empty() || e.category == *cat) && e.matches(&t.borrow())
        });
        let filtered = gtk::FilterListModel::new(Some(store.clone()), Some(filter.clone()));
        let sort = Rc::new(RefCell::new(sort.to_string()));
        let s = sort.clone();
        let sorter = gtk::CustomSorter::new(move |a, b| match (entry_of(a), entry_of(b)) {
            (Some(a), Some(b)) => compare(&s.borrow(), &a, &b).into(),
            _ => gtk::Ordering::Equal,
        });
        let sorted = gtk::SortListModel::new(Some(filtered.clone()), Some(sorter.clone()));
        let selection = gtk::SingleSelection::new(Some(sorted));
        // The first row is selected, so the preview always shows something.
        selection.set_autoselect(true);
        let view = gtk::ColumnView::new(Some(selection.clone()));
        view.add_css_class("font-table");
        view.set_show_column_separators(false);
        view.set_reorderable(false);

        let table = FontTable {
            scroll: gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&view).build(),
            view: view.clone(),
            store,
            filtered,
            selection,
            filter,
            sorter,
            terms,
            category,
            sort,
            status_cells: Rc::default(),
        };

        for &col in cols {
            let factory = gtk::SignalListItemFactory::new();
            factory.connect_setup(move |_, item| {
                let item = item.downcast_ref::<gtk::ListItem>().expect("list item");
                let l = gtk::Label::new(None);
                l.set_xalign(0.0);
                l.set_ellipsize(gtk::pango::EllipsizeMode::End);
                match col {
                    Col::Family => {}
                    Col::Status => {
                        l.add_css_class("tag");
                        l.set_halign(gtk::Align::Start);
                    }
                    _ => l.add_css_class("cell-dim"),
                }
                item.set_child(Some(&l));
            });
            let cells = table.status_cells.clone();
            factory.connect_bind(move |_, item| {
                let item = item.downcast_ref::<gtk::ListItem>().expect("list item");
                let (Some(l), Some(obj)) = (item.child().and_downcast::<gtk::Label>(), item.item()) else { return };
                let Some(e) = entry_of(&obj) else { return };
                if col == Col::Status {
                    set_status(&l, &e);
                    if let Some(b) = obj.downcast_ref::<glib::BoxedAnyObject>() {
                        let mut cells = cells.borrow_mut();
                        cells.retain(|(w, _)| w.upgrade().is_some_and(|x| x != l));
                        cells.push((l.downgrade(), b.downgrade()));
                    }
                } else {
                    let text = cell_text(col, &e);
                    if col == Col::Family {
                        l.set_tooltip_text(Some(&text));
                    }
                    l.set_text(&text);
                }
            });
            let c = gtk::ColumnViewColumn::new(Some(col.title()), Some(factory));
            match col {
                Col::Family => c.set_expand(true),
                Col::Category => c.set_fixed_width(140),
                Col::Styles => c.set_fixed_width(84),
                Col::Source => c.set_fixed_width(130),
                Col::Version => c.set_fixed_width(120),
                Col::Size => c.set_fixed_width(90),
                Col::Status => c.set_fixed_width(150),
            }
            c.set_resizable(col != Col::Family);
            view.append_column(&c);
        }

        // Install status follows the queue without rebuilding rows.
        let t = table.clone();
        events::subscribe(&view, move |c| {
            if matches!(c, events::Change::Installed | events::Change::Jobs) {
                t.refresh_status();
            }
        });
        table
    }

    pub fn set_entries(&self, entries: &[FontEntry]) {
        let selected = self.selected().map(|e| e.key());
        let objs: Vec<glib::BoxedAnyObject> = entries.iter().cloned().map(glib::BoxedAnyObject::new).collect();
        self.store.splice(0, self.store.n_items(), &objs);
        if let Some(key) = selected {
            self.select_key(&key);
        }
    }

    pub fn set_query(&self, query: &str) {
        *self.terms.borrow_mut() = crate::catalog::terms(query);
        self.filter.changed(gtk::FilterChange::Different);
    }

    /// Show one category only ("" for all).
    pub fn set_category(&self, category: &str) {
        *self.category.borrow_mut() = category.to_string();
        self.filter.changed(gtk::FilterChange::Different);
    }

    pub fn set_sort(&self, sort: &str) {
        *self.sort.borrow_mut() = sort.to_string();
        self.sorter.changed(gtk::SorterChange::Different);
        self.view.scroll_to(0, None, gtk::ListScrollFlags::NONE, None);
    }

    /// Rows shown after filtering.
    pub fn shown(&self) -> u32 {
        self.filtered.n_items()
    }

    pub fn total(&self) -> u32 {
        self.store.n_items()
    }

    pub fn selected(&self) -> Option<FontEntry> {
        self.selection.selected_item().and_then(|o| entry_of(&o).map(|e| e.clone()))
    }

    fn select_key(&self, key: &(Source, String)) {
        for i in 0..self.selection.n_items() {
            if let Some(o) = self.selection.item(i)
                && entry_of(&o).is_some_and(|e| e.key() == *key)
            {
                self.selection.set_selected(i);
                return;
            }
        }
    }

    pub fn on_select(&self, f: impl Fn(Option<FontEntry>) + 'static) {
        // Also fires when filtering hides the selected row.
        let t = self.clone();
        self.selection.connect_selected_item_notify(move |_| f(t.selected()));
    }

    /// Notify `f` when the shown rows change (for an empty-state message).
    pub fn on_shown_changed(&self, f: impl Fn(u32) + 'static) {
        self.filtered.connect_items_changed(move |m, _, _, _| f(m.n_items()));
    }

    pub fn refresh_status(&self) {
        let mut cells = self.status_cells.borrow_mut();
        cells.retain(|(l, o)| l.upgrade().is_some() && o.upgrade().is_some());
        for (l, o) in cells.iter() {
            if let (Some(l), Some(o)) = (l.upgrade(), o.upgrade()) {
                set_status(&l, &o.borrow::<FontEntry>());
            }
        }
    }
}

fn set_status(l: &gtk::Label, e: &FontEntry) {
    let (text, class) = status_of(e);
    l.set_visible(!text.is_empty());
    l.set_text(&text);
    l.set_tooltip_text(Some(&text));
    if class == "accent" {
        l.add_css_class("accent");
    } else {
        l.remove_css_class("accent");
    }
}
