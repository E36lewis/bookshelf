Bookshelf preview for Windows 10 (version 1809 or later) and Windows 11, x64

1. Double-click Bookshelf.exe. You can move it anywhere first, such as
   the desktop. Nothing needs to be installed.
2. If Windows says "Windows protected your PC", click "More info", then
   "Run anyway". The preview isn't signed yet.
3. The first start can take up to a minute while Bookshelf unpacks its
   files. Later starts are quick.

The preview keeps its own test data in %LOCALAPPDATA%\Bookshelf.Preview.

If no window opens after a minute:
- Look for %LOCALAPPDATA%\Bookshelf.Preview\crash.log (paste that path
  into the File Explorer address bar) and send it.
- Open Event Viewer > Windows Logs > Application and send any recent
  Error entries from ".NET Runtime" or "Application Error" that mention
  Bookshelf.
- On Windows 11, also check Windows Security > App & browser control >
  Smart App Control: if it's On, it may have blocked the unsigned preview.
- Say which Windows version you have (Settings > System > About).
