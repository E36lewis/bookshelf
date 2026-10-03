Bookshelf preview for Windows 10 (version 1809 or later) and Windows 11, x64

First, install Windows' updates (Settings > Windows Update). On PCs with a
newer processor (roughly Intel 11th generation or AMD Ryzen 5000 and later),
Bookshelf needs Windows updates from October 2024 or later. Without them it
closes at once, with no message. (The installer checks this for you; this
single exe can't.)

1. Double-click Bookshelf.exe. You can move it anywhere first, such as
   the desktop. Nothing needs to be installed.
2. If Windows says "Windows protected your PC", click "More info", then
   "Run anyway". The preview isn't signed yet.
3. The first start can take up to a minute while Bookshelf unpacks its
   files. Later starts are quick.

The preview keeps its own test journal, apart from any real one, in
%LOCALAPPDATA%\Bookshelf Preview (its logs are in the "logs" folder there).

To look around with made-up books instead, open a Command Prompt in the
folder with Bookshelf.exe and run:  Bookshelf.exe --demo-journal
That journal lives in a temporary folder and never touches yours.

If no window opens after a minute:
- Run Windows Update (see above), restart, and try again. An Event Viewer
  entry (below) saying "Your Windows doesn't fully support CET" means
  exactly that.
- Paste %LOCALAPPDATA%\Bookshelf Preview\logs into the File Explorer
  address bar and send startup.log (its last line shows how far Bookshelf
  got) and crash.log, if there is one. If startup.log isn't there, look in
  %TEMP% instead.
- Open Event Viewer > Windows Logs > Application and send any recent
  Error entries from ".NET Runtime" or "Application Error" that mention
  Bookshelf.
- On Windows 11, also check Windows Security > App & browser control >
  Smart App Control: if it's On, it may have blocked the unsigned preview.
- Say which Windows version you have: press Windows+R, type winver,
  press Enter.
