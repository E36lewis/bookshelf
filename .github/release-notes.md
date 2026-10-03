<!-- release.yml fills in @VERSION@ and uses this as the release's notes. -->
Bookshelf @VERSION@: a quiet place to write down what you thought of the books you read.

The first release for Windows, the Mac and Linux. The Mac app hasn't been tried by hand yet, hence 0.9. Your journal stays on your computer.

## Download

| | File | Needs |
|---|---|---|
| Windows | `Bookshelf-@VERSION@-setup-x64.exe` | Windows 10 (version 1809 or later) or Windows 11, on a 64-bit Intel or AMD PC |
| Mac | `Bookshelf-@VERSION@.dmg` | macOS 14 Sonoma or later, Apple silicon or Intel |
| Linux | `Bookshelf-@VERSION@-x86_64.AppImage` | A 64-bit distribution as new as Ubuntu 24.04, Fedora 40 or Debian 13 |

## Windows

1. Run `Bookshelf-@VERSION@-setup-x64.exe`. If your browser warns that the file isn't commonly downloaded, choose to keep it.
2. If Windows says **"Windows protected your PC"**, click **More info**, then **Run anyway**. Bookshelf isn't signed with a paid certificate yet, so Windows doesn't recognize it.
3. Setup needs no administrator rights. It installs Bookshelf for you alone, in `%LOCALAPPDATA%\Programs\Bookshelf`, with a Start menu entry (and a desktop shortcut if you tick the box).

**Windows Update first.** On PCs with a newer processor (roughly Intel 11th generation or AMD Ryzen 5000 and later), Bookshelf needs Windows updates from October 2024 or later; a Windows 11 or Windows 10 22H2 PC that is up to date has them. If an update is missing, setup says so and stops: open **Settings › Windows Update**, install the updates, then run setup again.

To uninstall, find Bookshelf in **Settings › Apps** (**Installed apps** on Windows 11, **Apps & features** on Windows 10) and choose **Uninstall**. Your journal (`%LOCALAPPDATA%\Bookshelf`) is kept.

If Smart App Control is on (Windows 11), it may block Bookshelf with no way to run it, since Bookshelf isn't signed yet.

## Mac

1. Open `Bookshelf-@VERSION@.dmg` and drag Bookshelf onto the Applications folder.
2. Open Bookshelf. The first time, macOS stops it, because it isn't notarized by Apple (that needs a paid developer account). On macOS 15 or newer it says **"Bookshelf" Not Opened**; click **Done**. On macOS 14 it says **the developer cannot be verified**; click **Cancel**. (Not **Move to Trash**.)
3. Open **System Settings › Privacy & Security**, scroll down to the note that Bookshelf was blocked, click **Open Anyway**, and confirm with your password. (On macOS 14 you can instead Control-click Bookshelf in Applications and choose **Open**.) macOS remembers your choice.

Bookshelf runs in the Mac's app sandbox: it can only reach its own journal and the folders you choose for backups and exports.

## Linux

    chmod +x Bookshelf-@VERSION@-x86_64.AppImage
    ./Bookshelf-@VERSION@-x86_64.AppImage

AppImages need FUSE 2 (`sudo apt install libfuse2t64` on Ubuntu), or run it with `--appimage-extract-and-run`. Your journal is in `~/.local/share/bookshelf`.

## Check your download

`SHA256SUMS` lists each file's checksum:

    sha256sum --check --ignore-missing SHA256SUMS                     # Linux
    shasum -a 256 --check --ignore-missing SHA256SUMS                 # Mac
    Get-FileHash Bookshelf-@VERSION@-setup-x64.exe -Algorithm SHA256   # Windows PowerShell: compare by eye

Each file also has a build provenance attestation, which proves it was built from this repository by GitHub Actions. With the GitHub CLI:

    gh attestation verify Bookshelf-@VERSION@.dmg --repo E36lewis/bookshelf
