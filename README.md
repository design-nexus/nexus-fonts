# Fonts

A font manager for [Omarchy](https://omarchy.org). Search every family on
[Google Fonts](https://fonts.google.com) and every [Nerd Font](https://www.nerdfonts.com),
preview them in your own sample text, and install them with one click. It takes its
colors from your Omarchy theme and fits a half-screen tile.

## What it does

- **Google Fonts:** all ~1,950 families in one table. Filter by name, designer or
  language, show one category (sans serif, serif, display, handwriting, monospace), and
  sort by popularity, trending, name or newest.
- **Nerd Fonts:** every font from the latest
  [Nerd Fonts release](https://github.com/ryanoasis/nerd-fonts/releases), patched with
  thousands of icons for terminals and editors.
- **Live preview:** select a font to see your sample text in it at any size, with the
  full alphabet and numbers. Nothing is installed to preview a font: one file is
  downloaded to a cache.
- **Install for your user:** fonts go to `~/.local/share/fonts/nexus-fonts`, no sudo
  needed, and the font cache is refreshed so other apps see them right away. Choose
  static or variable files for Google Fonts, and which Nerd Font variants to keep
  (Mono for terminals, regular and Propo).
- **Installed:** everything installed with Fonts, with its version and size. When a
  newer version comes out, update one font or all of them; remove fonts you don't need.
- **System fonts:** every font family on your computer, marked as installed by Fonts,
  by you, or by the system, each with a preview.
- **Search** both catalogs at once from the top bar.
- The font lists are cached and refreshed once a day, so browsing works offline.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/design-nexus/nexus-fonts/main/install.sh | bash
```

This installs GTK 4 if it's missing, builds with Cargo, and installs `fonts` to
`~/.local/bin` along with a launcher entry.

To remove it, run the same line with `uninstall.sh` in place of `install.sh`. Fonts you
installed stay installed; add `-s -- --purge` to also remove them and the app's settings.

## Usage

```
fonts [OPTIONS]
```

| Option | What |
| --- | --- |
| `--section ID` | Open a page: `google`, `nerd`, `installed`, `system`, `settings` |
| `--search TEXT` | Search both catalogs |
| `--toggle` | Close the window if it's open, otherwise open it |

| Key | What |
| --- | --- |
| <kbd>Ctrl</kbd>+<kbd>F</kbd> | Filter this page |
| <kbd>Ctrl</kbd>+<kbd>K</kbd> | Search both catalogs |
| <kbd>Esc</kbd> | Clear the search |
| <kbd>Ctrl</kbd>+<kbd>1</kbd>–<kbd>4</kbd> | Go to a page in the sidebar |
| <kbd>Alt</kbd>+<kbd>←</kbd> or the mouse's back button | Back |
| <kbd>Ctrl</kbd>+<kbd>B</kbd> | Collapse or expand the sidebar |
| <kbd>Ctrl</kbd>+<kbd>,</kbd> | Settings |
| <kbd>F1</kbd> or <kbd>?</kbd> | Show the shortcuts |
| <kbd>Ctrl</kbd>+<kbd>Q</kbd> | Close |

## Files

| Path | What |
| --- | --- |
| `~/.local/share/fonts/nexus-fonts/` | Installed fonts, one folder per family |
| `~/.local/state/nexus-fonts/installed.toml` | What was installed, from where, and which version |
| `~/.config/nexus-fonts/settings.toml` | Preferences: install options, preview, theme |
| `~/.config/nexus-fonts/themes/*.toml` | Your own themes |
| `~/.cache/nexus-fonts/` | The font lists and preview files |

## License

MIT. Fonts you install come with their own licenses (mostly the SIL Open Font
License), saved next to the font files.
