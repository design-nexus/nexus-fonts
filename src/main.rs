//! Fonts — browse, preview and install Google Fonts and Nerd Fonts.

mod catalog;
mod cmd;
mod events;
mod fmt;
mod fonttable;
mod http;
mod install;
mod installed;
mod paths;
mod prefs;
mod preview;
mod preview_panel;
mod sections;
mod settings_dialog;
mod system;
mod theme;
mod widgets;
mod window;

use gtk::prelude::*;
use gtk::{gio, glib};
use std::cell::Cell;

pub const APP_ID: &str = "io.github.design_nexus.Fonts";

const USAGE: &str = "Usage: fonts [OPTIONS]\n\
\n\
  --section ID    open (or switch the open window) to a page: google, nerd, installed,\n\
                  system, settings\n\
  --search TEXT   search both catalogs\n\
  --toggle        close the window if it's open, otherwise open it (for a keybinding)\n";

thread_local! {
    static STARTED: Cell<bool> = const { Cell::new(false) };
}

fn start() {
    if STARTED.with(|s| s.replace(true)) {
        return;
    }
    catalog::start();
    std::thread::spawn(preview::prune);
}

fn main() -> glib::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{USAGE}");
        return glib::ExitCode::SUCCESS;
    }

    // GTK's Vulkan renderer enumerates every GPU at startup, which wakes a
    // sleeping discrete GPU on hybrid laptops. GL renders only on the one in use.
    if std::env::var_os("GSK_RENDERER").is_none() {
        // SAFETY: still single-threaded; nothing else reads the environment yet.
        unsafe { std::env::set_var("GSK_RENDERER", "ngl") };
    }

    let app = gtk::Application::builder().application_id(APP_ID).flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE).build();
    app.connect_command_line(|app, cl| {
        let argv: Vec<String> = cl.arguments().iter().map(|a| a.to_string_lossy().to_string()).collect();
        let has = |flag: &str| argv.iter().any(|a| a == flag);
        let value_of = |flag: &str| argv.iter().position(|a| a == flag).and_then(|i| argv.get(i + 1)).cloned();
        if has("--toggle")
            && let Some(w) = window::window()
            && w.is_visible()
        {
            w.close();
            return glib::ExitCode::SUCCESS;
        }
        start();
        window::present(app, value_of("--section").as_deref());
        if let Some(q) = value_of("--search") {
            window::search(&q);
        }
        glib::ExitCode::SUCCESS
    });
    app.connect_shutdown(|_| prefs::flush());
    app.run()
}
