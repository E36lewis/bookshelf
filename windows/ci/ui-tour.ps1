# Drives Bookshelf.exe through its pages on a demo journal (never real
# data), checking the UI through UI Automation and saving a screenshot of
# each page. Run by .github/workflows/windows.yml in Windows PowerShell,
# for its built-in UI Automation client.
#
#   ui-tour.ps1 -Exe <Bookshelf.exe> -Out <folder for PNGs>
#
# Every step runs even if an earlier one failed (so one run shows as much
# as it can); the script fails at the end if any step did.

param(
    [Parameter(Mandatory)] [string] $Exe,
    [Parameter(Mandatory)] [string] $Out
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms, System.Drawing
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @"
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

public static class Win
{
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }

    [DllImport("user32.dll")] static extern bool PrintWindow(IntPtr window, IntPtr dc, uint flags);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("dwmapi.dll")] static extern int DwmGetWindowAttribute(IntPtr window, int attribute, out RECT rect, int size);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr window, out RECT rect);

    // The window as it's drawn, including DirectComposition content
    // (PW_RENDERFULLCONTENT), cropped to its visible frame.
    public static void Snap(IntPtr window, string path)
    {
        RECT all, frame;
        GetWindowRect(window, out all);
        if (DwmGetWindowAttribute(window, 9 /* DWMWA_EXTENDED_FRAME_BOUNDS */, out frame, Marshal.SizeOf(typeof(RECT))) != 0) frame = all;
        using (var full = new Bitmap(all.Right - all.Left, all.Bottom - all.Top, PixelFormat.Format32bppArgb))
        {
            using (var g = Graphics.FromImage(full))
            {
                IntPtr dc = g.GetHdc();
                PrintWindow(window, dc, 2);
                g.ReleaseHdc(dc);
            }
            var crop = new Rectangle(frame.Left - all.Left, frame.Top - all.Top, frame.Right - frame.Left, frame.Bottom - frame.Top);
            using (var shot = full.Clone(crop, full.PixelFormat)) shot.Save(path, ImageFormat.Png);
        }
    }
}
"@

$A = [Windows.Automation.AutomationElement]
$Scope = [Windows.Automation.TreeScope]
$failures = New-Object System.Collections.Generic.List[string]
New-Item -ItemType Directory -Force $Out | Out-Null
Write-Host ("Screen: {0}" -f [System.Windows.Forms.Screen]::PrimaryScreen.Bounds)

function Start-Bookshelf([string[]] $Arguments) {
    $app = Start-Process $Exe -ArgumentList $Arguments -PassThru
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $mine = New-Object Windows.Automation.AndCondition(
        (New-Object Windows.Automation.PropertyCondition($A::ProcessIdProperty, $app.Id)),
        (New-Object Windows.Automation.PropertyCondition($A::NameProperty, 'Bookshelf Preview')))
    $found = $null
    while ($clock.Elapsed.TotalSeconds -lt 90 -and -not $app.HasExited -and -not $found) {
        $found = $A::RootElement.FindFirst($Scope::Children, $mine)
        if (-not $found) { Start-Sleep -Milliseconds 300 }
    }
    if (-not $found) { throw "No window for $Arguments" }
    $script:app = $app
    $script:hwnd = [IntPtr]$found.Current.NativeWindowHandle
    $script:window = $found
}

function Stop-Bookshelf {
    if ($script:app -and -not $script:app.HasExited) { Stop-Process -Id $script:app.Id -Force }
    Start-Sleep -Milliseconds 500
}

function Find-Id([string] $Id, [int] $Seconds = 15) {
    $condition = New-Object Windows.Automation.PropertyCondition($A::AutomationIdProperty, $Id)
    $clock = [Diagnostics.Stopwatch]::StartNew()
    do {
        $found = $script:window.FindFirst($Scope::Descendants, $condition)
        if ($found) { return $found }
        Start-Sleep -Milliseconds 250
    } while ($clock.Elapsed.TotalSeconds -lt $Seconds)
    throw "Nothing with the automation id '$Id' after $Seconds s"
}

function Wait-Name([string] $Id, [string] $Like, [int] $Seconds = 15) {
    $clock = [Diagnostics.Stopwatch]::StartNew()
    do {
        $name = (Find-Id $Id $Seconds).Current.Name
        if ($name -like $Like) { return $name }
        Start-Sleep -Milliseconds 250
    } while ($clock.Elapsed.TotalSeconds -lt $Seconds)
    throw "'$Id' says '$name', not '$Like'"
}

function List-Items([string] $Id) {
    $list = Find-Id $Id
    $isItem = New-Object Windows.Automation.PropertyCondition($A::ControlTypeProperty, [Windows.Automation.ControlType]::ListItem)
    return @($list.FindAll($Scope::Descendants, $isItem))
}

function Keys([string] $Keys) {
    [Win]::SetForegroundWindow($script:hwnd) | Out-Null
    Start-Sleep -Milliseconds 150
    [System.Windows.Forms.SendKeys]::SendWait($Keys)
    Start-Sleep -Milliseconds 700
}

function Snap([string] $Name) {
    Start-Sleep -Milliseconds 600 # let transitions finish
    [Win]::Snap($script:hwnd, (Join-Path $Out "$Name.png"))
}

function Step([string] $Name, [scriptblock] $Body) {
    try {
        & $Body
        Write-Host "ok    $Name"
    }
    catch {
        Write-Host "FAIL  $Name : $_"
        $failures.Add("$Name : $_")
        try { Snap "fail-$($Name -replace '[^\w-]', '_')" } catch { }
    }
}

# ---- First run: an empty journal asks for a profile ---------------------------
Start-Bookshelf @('--demo-empty')
Step 'First run asks for a profile' {
    Wait-Name 'WelcomeHeading' 'Welcome' | Out-Null
    Find-Id 'NewProfileName' | Out-Null
    Snap '00-welcome'
}
Stop-Bookshelf

# ---- Avery: the default look, books everywhere ------------------------------
Start-Bookshelf @('--demo-journal')

Step 'Reading shelf lists books' {
    Wait-Name 'ShelfHeading' 'Reading now' | Out-Null
    $items = List-Items 'ShelfList'
    if ($items.Count -lt 2) { throw "only $($items.Count) books on Reading" }
    Write-Host ("      " + (($items | ForEach-Object { $_.Current.Name }) -join ' | '))
    if (-not ($items | Where-Object { $_.Current.Name -like 'The Left Hand of Darkness*' })) { throw 'no Left Hand of Darkness' }
    Snap '01-reading'
}
Step 'Ctrl+2: Finished, by year' {
    Keys '^2'
    Wait-Name 'ShelfHeading' 'Finished' | Out-Null
    if ((List-Items 'ShelfList').Count -lt 4) { throw 'fewer than 4 finished books' }
    Snap '02-finished'
}
Step 'Ctrl+3: Someday' {
    Keys '^3'
    Wait-Name 'ShelfHeading' 'Someday' | Out-Null
    Snap '03-someday'
}
Step 'Open a book' {
    Keys '^2'
    Wait-Name 'ShelfHeading' 'Finished' | Out-Null
    $first = (List-Items 'ShelfList')[0]
    $first.SetFocus()
    Keys '{ENTER}'
    Wait-Name 'BookTitle' 'Dune' | Out-Null
    Snap '04-book'
}
Step 'Ctrl+R: read' {
    Keys '^r'
    Find-Id 'ReaderText' | Out-Null
    Snap '05-reader'
}
Step 'F11: full screen' {
    Keys '{F11}'
    Start-Sleep -Seconds 1
    Snap '06-reader-full-screen'
    Keys '{ESC}'
}
Step 'Alt+Left, then Ctrl+E: write' {
    Keys '%{LEFT}'
    Wait-Name 'BookTitle' 'Dune' | Out-Null
    Keys '^e'
    Find-Id 'Editor' | Out-Null
    Wait-Name 'WriterStatus' '*Saved' | Out-Null
    Snap '07-writer'
}
Step 'Typing saves by itself' {
    $before = (Find-Id 'WriterStatus').Current.Name
    Keys '^{END}{ENTER}{ENTER}Written by the UI tour.'
    # Saved within a second of the last key (the core's autosave delay), with the new word count.
    $clock = [Diagnostics.Stopwatch]::StartNew()
    do {
        $now = (Find-Id 'WriterStatus').Current.Name
        if ($now -like '*Saved' -and $now -ne $before) { break }
        Start-Sleep -Milliseconds 250
    } while ($clock.Elapsed.TotalSeconds -lt 10)
    if ($now -eq $before -or $now -notlike '*Saved') { throw "the status went from '$before' to '$now'" }
    Write-Host "      $before -> $now"
    Snap '08-writer-saved'
    Keys '%{LEFT}'
    Wait-Name 'BookTitle' 'Dune' | Out-Null
    $preview = (Find-Id 'SummaryPreview')
    $text = $preview.GetCurrentPattern([Windows.Automation.TextPattern]::Pattern).DocumentRange.GetText(-1)
    if ($text -notlike '*Written by the UI tour.*') { throw "the book page doesn't show the new text" }
}
Step 'Ctrl+, : settings' {
    Keys '^,'
    Find-Id 'SettingsScroller' | Out-Null
    Snap '09-settings'
    $scroll = (Find-Id 'SettingsScroller').GetCurrentPattern([Windows.Automation.ScrollPattern]::Pattern)
    $scroll.SetScrollPercent(-1, 45)
    Snap '10-settings-middle'
    $scroll.SetScrollPercent(-1, 100)
    Snap '11-settings-end'
}
Step 'F1: the manual' {
    Keys '{F1}'
    Find-Id 'ManualContents' | Out-Null
    Find-Id 'getting-started' | Out-Null
    Snap '12-manual'
}
Step 'Ctrl+?: shortcuts' {
    Keys '^?'
    $condition = New-Object Windows.Automation.PropertyCondition($A::NameProperty, 'Keyboard shortcuts')
    Start-Sleep -Seconds 1
    if (-not $script:window.FindFirst($Scope::Descendants, $condition)) { throw 'no Keyboard shortcuts dialog' }
    Snap '13-shortcuts'
    Keys '{ESC}'
}
Step 'Ctrl+N: add a book' {
    Keys '^1'
    Wait-Name 'ShelfHeading' 'Reading now' | Out-Null
    Keys '^n'
    Find-Id 'BookSearch' | Out-Null
    Snap '14-add-book'
    Keys '{ESC}'
}
Step 'Delete, then Undo' {
    $before = (List-Items 'ShelfList').Count
    (List-Items 'ShelfList')[0].SetFocus()
    Keys '{ENTER}'
    Find-Id 'BookTitle' | Out-Null
    Keys '{DELETE}'
    Wait-Name 'ShelfHeading' 'Reading now' | Out-Null
    Start-Sleep -Seconds 1
    $after = (List-Items 'ShelfList').Count
    if ($after -ne $before - 1) { throw "$before books before removing, $after after" }
    Snap '15-removed-undo'
    Keys '^z'
    Start-Sleep -Seconds 1
    $restored = (List-Items 'ShelfList').Count
    if ($restored -ne $before) { throw "$restored books after Undo, not $before" }
}
Step 'Type to search' {
    Keys '^2'
    Wait-Name 'ShelfHeading' 'Finished' | Out-Null
    Keys '^f'
    Keys 'earthsea'
    Wait-Name 'ShelfCount' '1 match' | Out-Null
    Snap '16-search'
    Keys '{ESC}'
}
Stop-Bookshelf

# ---- Sam: dark, teal and sans-serif ------------------------------------------
Start-Bookshelf @('--demo-journal', '--demo-profile', 'Sam')
Step 'Dark theme' {
    Wait-Name 'ShelfHeading' 'Reading now' | Out-Null
    Snap '20-dark-reading'
    Keys '^2'
    Wait-Name 'ShelfHeading' 'Finished' | Out-Null
    Snap '21-dark-finished'
    (List-Items 'ShelfList')[0].SetFocus()
    Keys '{ENTER}'
    Find-Id 'BookTitle' | Out-Null
    Snap '22-dark-book'
}
Stop-Bookshelf

# ---- Jo: nothing yet -----------------------------------------------------------
Start-Bookshelf @('--demo-journal', '--demo-profile', 'Jo')
Step 'Empty shelf' {
    Wait-Name 'ShelfHeading' 'Reading now' | Out-Null
    Find-Id 'AddBookButton' | Out-Null
    Snap '30-empty-shelf'
}
Stop-Bookshelf

if ($failures.Count) {
    Write-Host ''
    Write-Host "$($failures.Count) step(s) failed:"
    $failures | ForEach-Object { Write-Host "  $_" }
    exit 1
}
Write-Host 'Every step passed.'
