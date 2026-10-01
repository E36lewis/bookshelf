# Packaging

## Build the AppImage

One-time setup (Ubuntu 24.04):

    sudo apt install build-essential pkg-config curl file \
        libgtk-4-dev libadwaita-1-dev librsvg2-bin librsvg2-common

Then, with no conda environment active:

    bash packaging/build-appimage.sh

The result lands in `dist/`. The first run downloads `linuxdeploy` and its GTK
plugin into `target/appimage/tools/` (delete that folder to update them).

## What's inside

- `build-appimage.sh`         the whole build, start to finish
- `com.bookshelf.Bookshelf.svg`          app icon
- `com.bookshelf.Bookshelf.desktop.in`   launcher entry (`@EXEC@` is filled in)
- `apprun-hooks/bookshelf-fonts.sh`      makes the bundled fonts visible at launch
- `fonts/`                    iA Writer Duo (writing page) and Source Serif 4
                              (titles), both SIL OFL, licenses included

## Compatibility

- Built on Ubuntu 24.04 because the UI needs libadwaita 1.5. The AppImage runs
  on distributions with the same or a newer glibc (Ubuntu 24.04+, Fedora 40+,
  Debian 13+). It will not run on Ubuntu 22.04.
- Running any AppImage needs FUSE 2: `sudo apt install libfuse2t64`
  (or run with `--appimage-extract-and-run`).
- Data lives in `~/.local/share/bookshelf`, the same place as the
  `install-local.sh` version, so both see the same books.

## Checking that it's self-contained

Copy the AppImage to a machine (or VM) that has no GTK development packages
and run it. Things worth a look: the icon in the dock, the fonts on the writing
page (monospace iA Writer, not DejaVu), the symbolic icons in the formatting
bar, and switching light/dark mode.
