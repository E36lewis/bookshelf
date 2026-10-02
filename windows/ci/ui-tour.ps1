# Drives Bookshelf.exe through its pages on a demo journal (never real
# data), checking the UI through UI Automation and saving a screenshot of
# each page. Run by .github/workflows/windows.yml in Windows PowerShell,
# for its built-in UI Automation client.
#
#   ui-tour.ps1 -Exe <Bookshelf.exe> -Out <folder for PNGs>
#
# Every step runs even if an earlier one failed (so one run shows as much
# as it can); the script fails at the end if any step did.
#
# The writing page gets a tour of its own: highlighting as you type, the
# formatting bar and its keys, one-step undo, prompts, a 10,000-word speed
# check (written to writer-speed.txt beside the screenshots), focus mode and
# full screen; then saving as the window closes (across a restart, with
# --demo-journal-in) and the rescue of text that can't be saved
# (--demo-save-fails).

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

# ---- Writing page helpers -------------------------------------------------------

# Types without the pause Keys adds (to close the window right after, say).
function Type-Now([string] $Keys) {
    [Win]::SetForegroundWindow($script:hwnd) | Out-Null
    Start-Sleep -Milliseconds 150
    [System.Windows.Forms.SendKeys]::SendWait($Keys)
}

function Editor-Pattern {
    (Find-Id 'Editor').GetCurrentPattern([Windows.Automation.TextPattern]::Pattern)
}

# The editor's text, with its line breaks as `n.
function Editor-Text {
    ((Editor-Pattern).DocumentRange.GetText(-1) -replace "`r`n?", "`n").TrimEnd()
}

# Waits until the editor's text ends with $End (compared exactly, so
# Markdown's asterisks are just asterisks).
function Wait-EditorEnd([string] $End, [int] $Seconds = 10) {
    $clock = [Diagnostics.Stopwatch]::StartNew()
    do {
        $text = Editor-Text
        if ($text.EndsWith($End, [StringComparison]::Ordinal)) { return $text }
        Start-Sleep -Milliseconds 250
    } while ($clock.Elapsed.TotalSeconds -lt $Seconds)
    $tail = if ($text.Length -gt 60) { $text.Substring($text.Length - 60) } else { $text }
    throw "the text ends '...$tail', not '...$End'"
}

function Focused-Id {
    [Windows.Automation.AutomationElement]::FocusedElement.Current.AutomationId
}

function Wait-Focused([string] $Id, [int] $Seconds = 5) {
    $clock = [Diagnostics.Stopwatch]::StartNew()
    do {
        $now = Focused-Id
        if ($now -eq $Id) { return }
        Start-Sleep -Milliseconds 200
    } while ($clock.Elapsed.TotalSeconds -lt $Seconds)
    throw "the focus is on '$now', not '$Id'"
}

# True once nothing (visible) has the automation id.
function Wait-Gone([string] $Id, [int] $Seconds = 5) {
    $condition = New-Object Windows.Automation.PropertyCondition($A::AutomationIdProperty, $Id)
    $clock = [Diagnostics.Stopwatch]::StartNew()
    do {
        if (-not $script:window.FindFirst($Scope::Descendants, $condition)) { return }
        Start-Sleep -Milliseconds 250
    } while ($clock.Elapsed.TotalSeconds -lt $Seconds)
    throw "'$Id' is still there"
}

function Invoke-Id([string] $Id) {
    (Find-Id $Id).GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
    Start-Sleep -Milliseconds 700
}

function Invoke-Button([string] $Name, [int] $Seconds = 10) {
    $condition = New-Object Windows.Automation.AndCondition(
        (New-Object Windows.Automation.PropertyCondition($A::NameProperty, $Name)),
        (New-Object Windows.Automation.PropertyCondition($A::ControlTypeProperty, [Windows.Automation.ControlType]::Button)))
    $clock = [Diagnostics.Stopwatch]::StartNew()
    do {
        $button = $script:window.FindFirst($Scope::Descendants, $condition)
        if ($button) {
            $button.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
            Start-Sleep -Milliseconds 700
            return
        }
        Start-Sleep -Milliseconds 250
    } while ($clock.Elapsed.TotalSeconds -lt $Seconds)
    throw "no '$Name' button"
}

# Some text element inside the element with this id whose name is like the pattern.
function Find-TextLike([string] $Id, [string] $Like, [int] $Seconds = 10) {
    $isText = New-Object Windows.Automation.PropertyCondition($A::ControlTypeProperty, [Windows.Automation.ControlType]::Text)
    $clock = [Diagnostics.Stopwatch]::StartNew()
    do {
        $root = Find-Id $Id $Seconds
        $found = @($root.FindAll($Scope::Descendants, $isText)) | Where-Object { $_.Current.Name -like $Like } | Select-Object -First 1
        if ($found) { return $found.Current.Name }
        Start-Sleep -Milliseconds 250
    } while ($clock.Elapsed.TotalSeconds -lt $Seconds)
    throw "nothing in '$Id' says '$Like'"
}

# Closes the window as the X button does (so the app can save first).
function Close-Window {
    $script:window.GetCurrentPattern([Windows.Automation.WindowPattern]::Pattern).Close()
}

function Wait-Exit([int] $Seconds = 30) {
    if (-not $script:app.WaitForExit($Seconds * 1000)) { throw "Bookshelf didn't close within $Seconds s" }
    if ($script:app.ExitCode -ne 0) { throw "Bookshelf closed with exit code $($script:app.ExitCode)" }
}

# The demo journal's edit timings: edits, median, p90, max (ms) and more.
function Writer-Timings {
    $t = @{}
    foreach ($pair in ((Find-Id 'WriterTiming').Current.Name -split ' ')) {
        $kv = $pair -split '='
        if ($kv.Count -eq 2) { $t[$kv[0]] = [double]::Parse($kv[1], [Globalization.CultureInfo]::InvariantCulture) }
    }
    return $t
}

function Status-Words {
    $status = Wait-Name 'WriterStatus' '*Saved'
    if ($status -notmatch '([\d,.\s]+) words?') { throw "no word count in '$status'" }
    return [int]($Matches[1] -replace '[^\d]', '')
}

# How far the caret's line is from the middle of the page, as a share of its height.
function Caret-Offset {
    $page = (Find-Id 'Editor').Current.BoundingRectangle
    $rects = @()
    for ($try = 0; $try -lt 5 -and $rects.Count -eq 0; $try++) {
        try {
            $caret = (Editor-Pattern).GetSelection()[0].Clone()
            $caret.ExpandToEnclosingUnit([Windows.Automation.Text.TextUnit]::Character)
            $rects = @($caret.GetBoundingRectangles())
        }
        catch { Start-Sleep -Milliseconds 500 }
    }
    if ($rects.Count -eq 0) { throw 'the caret has no rectangle (is it off the page?)' }
    $middle = $rects[0].Top + $rects[0].Height / 2
    return ($middle - ($page.Top + $page.Height / 2)) / $page.Height
}

# About 10,000 words of mixed Markdown, as someone's long notes might be.
function Long-Text {
    $sb = New-Object System.Text.StringBuilder
    for ($n = 1; $n -le 165; $n++) {
        [void]$sb.AppendLine("## Hall $n")
        [void]$sb.AppendLine('The house is **endless**, and its halls hold *statues* of every kind. I walk them each morning and note down what the tides have done to the lower floors, and which birds have come back.')
        [void]$sb.AppendLine('- A list item with `code` in it, and ~~struck~~ words.')
        [void]$sb.AppendLine('> A quote that runs on for a while, so that this line wraps at least once in the column.')
        [void]$sb.AppendLine('')
    }
    return $sb.ToString()
}

function Open-Book([string] $Shelf, [string] $Title) {
    Keys $Shelf
    Start-Sleep -Milliseconds 500
    $item = List-Items 'ShelfList' | Where-Object { $_.Current.Name -like "$Title*" } | Select-Object -First 1
    if (-not $item) { throw "no '$Title' on the shelf" }
    $item.SetFocus()
    Keys '{ENTER}'
    Wait-Name 'BookTitle' $Title | Out-Null
}

function Open-Writer([string] $Shelf, [string] $Title) {
    Open-Book $Shelf $Title
    Keys '^e'
    Find-Id 'Editor' | Out-Null
    Wait-Name 'WriterStatus' '*Saved' | Out-Null
    Wait-Focused 'Editor'
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
    Wait-Focused 'Editor'
    Keys '^{HOME}'
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
Step 'Profile menu' {
    (Find-Id 'ProfileButton').SetFocus()
    Keys '{ENTER}'
    # (Windows PowerShell reads this file as ANSI: keep names ASCII.)
    $isMenuItem = New-Object Windows.Automation.PropertyCondition($A::ControlTypeProperty, [Windows.Automation.ControlType]::MenuItem)
    Start-Sleep -Seconds 1
    $names = @($A::RootElement.FindAll($Scope::Descendants, $isMenuItem) | ForEach-Object { $_.Current.Name })
    Write-Host ("      menu: " + ($names -join ' | '))
    if (-not ($names | Where-Object { $_ -like 'New profile*' })) { throw 'no profile menu' }
    Snap '17-profile-menu'
    Keys '{ESC}'
}
Stop-Bookshelf

# ---- The writing page, in depth (Avery, light) ----------------------------------
Start-Bookshelf @('--demo-journal')

Step 'Writer: Markdown is highlighted as you type' {
    Wait-Name 'ShelfHeading' 'Reading now' | Out-Null
    Open-Writer '^2' 'Dune'
    Keys '^{END}{ENTER}{ENTER}A **bold** move.'
    Wait-EditorEnd 'A **bold** move.' | Out-Null
    $doc = (Editor-Pattern).DocumentRange
    $word = $doc.FindText('bold', $true, $false)
    $weight = $word.GetAttributeValue([Windows.Automation.TextPattern]::FontWeightAttribute)
    if ([int]$weight -lt 600) { throw "'bold' has weight $weight" }
    $plain = $doc.FindText('move', $true, $false)
    $plainWeight = $plain.GetAttributeValue([Windows.Automation.TextPattern]::FontWeightAttribute)
    if ([int]$plainWeight -ge 600) { throw "'move' is bold too ($plainWeight)" }
    # The marks are dimmed: the span with them has two colors.
    $marked = $doc.FindText('**bold**', $true, $false)
    $color = $marked.GetAttributeValue([Windows.Automation.TextPattern]::ForegroundColorAttribute)
    if ($color -ne [Windows.Automation.TextPattern]::MixedAttributeValue) { throw "'**bold**' is all one color ($color)" }
    $quote = $doc.FindText('Big ideas', $true, $false)
    if (-not $quote.GetAttributeValue([Windows.Automation.TextPattern]::IsItalicAttribute)) { throw 'the quote is not italic' }
    # One font throughout, the writing font (the bundled iA Writer Duo here).
    $fonts = @($word, $plain, $quote) | ForEach-Object { $_.GetAttributeValue([Windows.Automation.TextPattern]::FontNameAttribute) }
    if (@($fonts | Select-Object -Unique).Count -ne 1 -or $fonts[0] -notlike '*iA Writer Duo*') { throw "fonts: $($fonts -join ', ')" }
    Write-Host "      'bold' weight $weight, 'move' $plainWeight; '**bold**' has mixed colors; all in $($fonts[0])"
    Snap '40-writer-typed-markdown'
}
Step 'Writer: Ctrl+B on a word, and one Ctrl+Z takes it back' {
    Keys '{ENTER}plain'
    Wait-EditorEnd "move.`nplain" | Out-Null
    Keys '^b'
    Wait-EditorEnd "`n**plain**" | Out-Null
    Keys '^z'
    Wait-EditorEnd "move.`nplain" | Out-Null
    Keys '^y'
    Wait-EditorEnd "`n**plain**" | Out-Null
    # The bar's buttons work through UI Automation too, on the same selection.
    Invoke-Id 'FormatItalic'
    Wait-EditorEnd "`n***plain***" | Out-Null
    Keys '^z'
    Wait-EditorEnd "`n**plain**" | Out-Null
    Wait-Focused 'Editor'
}
Step 'Writer: the formatting bar from the keyboard' {
    Keys '+{TAB}'
    Wait-Focused 'FormatBold'
    Keys '{RIGHT}'
    Wait-Focused 'FormatItalic'
    $tip = (Find-Id 'FormatBold').Current.HelpText
    $key = (Find-Id 'FormatBold').Current.AcceleratorKey
    if ($key -ne 'Ctrl+B') { throw "Bold's key is '$key'" }
    $names = @('FormatBold', 'FormatItalic', 'FormatStrike', 'FormatCode', 'FormatHeading1', 'FormatHeading2', 'FormatHeading3',
        'FormatQuote', 'FormatBullets', 'FormatNumbered', 'FormatLink', 'FocusModeButton', 'WriterFullScreenButton') |
        ForEach-Object { (Find-Id $_).Current.Name }
    Write-Host ("      bar: " + ($names -join ' | '))
    Snap '41-writer-format-bar'
    Keys '{ESC}'
    Wait-Focused 'Editor'
}
Step 'Writer: the changes save by themselves' {
    $words = Status-Words
    Keys '%{LEFT}'
    Wait-Name 'BookTitle' 'Dune' | Out-Null
    $preview = (Find-Id 'SummaryPreview')
    $text = $preview.GetCurrentPattern([Windows.Automation.TextPattern]::Pattern).DocumentRange.GetText(-1)
    if ($text -notlike '*A bold move.*') { throw "the book page doesn't show the new text" }
    Write-Host "      $words words saved"
}
Step 'Writer: prompts on an empty page' {
    Open-Writer '^1' 'Piranesi'
    $prompt = Wait-Name 'WriterPrompt' 'Where are you in the story?*'
    Write-Host ("      " + ($prompt -replace "`r?`n", ' / '))
    Snap '42-writer-prompts'
    Keys 'x'
    Wait-Gone 'WriterPrompt'
    Keys '{BACKSPACE}'
    Wait-Name 'WriterPrompt' 'Where are you*' | Out-Null
}
Step 'Writer: 10,000 words, typing in the middle stays fast' {
    Set-Clipboard -Value (Long-Text)
    Keys '^v'
    $clock = [Diagnostics.Stopwatch]::StartNew()
    do {
        Start-Sleep -Milliseconds 500
        $words = try { Status-Words } catch { 0 }
    } while ($words -lt 10000 -and $clock.Elapsed.TotalSeconds -lt 60)
    if ($words -lt 10000) { throw "only $words words after pasting" }
    $clock = [Diagnostics.Stopwatch]::StartNew()
    do {
        Start-Sleep -Milliseconds 500
        $pasted = Writer-Timings
    } while ($pasted.background -eq 0 -and $clock.Elapsed.TotalSeconds -lt 30)
    # Open it again, as someone would the next day: that's the restyle that matters.
    Keys '%{LEFT}'
    Wait-Name 'BookTitle' 'Piranesi' | Out-Null
    $clock = [Diagnostics.Stopwatch]::StartNew()
    Keys '^e'
    Find-Id 'Editor' | Out-Null
    Wait-Name 'WriterStatus' '*Saved' | Out-Null
    Wait-Focused 'Editor'
    $opened = $clock.Elapsed.TotalSeconds
    # Typing in the middle, one key at a time, a little faster than people type.
    Keys '^{HOME}{DOWN 500}{END}'
    [Win]::SetForegroundWindow($script:hwnd) | Out-Null
    foreach ($c in ' Typed in the middle one key at a time'.ToCharArray()) {
        [System.Windows.Forms.SendKeys]::SendWait([string]$c)
        Start-Sleep -Milliseconds 60
    }
    $clock = [Diagnostics.Stopwatch]::StartNew()
    do {
        Start-Sleep -Milliseconds 500
        $t = Writer-Timings
    } while (($t.edits -lt 30 -or $t.background -eq 0) -and $clock.Elapsed.TotalSeconds -lt 30)
    $text = Editor-Text
    $at = $text.IndexOf('Typed in the middle one key at a time')
    if ($at -lt $text.Length / 5 -or $at -gt $text.Length * 4 / 5) { throw "the typing landed at $at of $($text.Length)" }
    $line = ("Writer speed on {0:N0} words: {1} keys typed in the middle, median {2:F2} ms, 90% under {3:F2} ms, slowest {4:F2} ms. " +
        "Opening it: {5:N1} s to the page, {6} lines around the caret restyled in {7:F0} ms, the rest in {8:F0} ms while idle. " +
        "The paste: {9} lines in {10:F0} ms, the rest in {11:F0} ms while idle.") -f `
        $words, $t.edits, $t.median, $t.p90, $t.max, $opened, $t.bulkLines, $t.bulk, $t.background, $pasted.bulkLines, $pasted.bulk, $pasted.background
    Write-Host "      $line"
    Add-Content (Join-Path $Out 'writer-speed.txt') $line
    if ($t.edits -lt 30) { throw "only $($t.edits) keys were timed" }
    # Clear regressions only: a frame is 16 ms, and a restyle should be a fraction of it.
    if ($t.median -gt 25 -or $t.p90 -gt 50) { throw "typing got slow: $line" }
    if ($t.bulk -gt 2000) { throw "opening got slow: $line" }
    Snap '43-writer-long'
}
Step 'Writer: focus mode keeps the line mid-page' {
    Keys '^+f'
    $toggle = (Find-Id 'FocusModeButton').GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)
    if ($toggle.Current.ToggleState -ne [Windows.Automation.ToggleState]::On) { throw 'Focus is not pressed' }
    Keys '{DOWN 12}'
    Start-Sleep -Seconds 1
    Keys '{DOWN 12}'
    Start-Sleep -Seconds 1
    $offset = Caret-Offset
    Write-Host ("      the caret's line is {0:P0} of the page from its middle" -f $offset)
    if ([Math]::Abs($offset) -gt 0.15) { throw ("the caret's line is {0:P0} from the middle" -f $offset) }
    Snap '44-writer-focus'
}
Step 'Writer: F11 full screen, Esc to leave' {
    Keys '{F11}'
    Wait-Gone 'SearchBox'
    Start-Sleep -Seconds 1
    Snap '45-writer-full-screen'
    Keys '{ESC}'
    Find-Id 'SearchBox' | Out-Null
    Keys '^+f'
    $toggle = (Find-Id 'FocusModeButton').GetCurrentPattern([Windows.Automation.TogglePattern]::Pattern)
    if ($toggle.Current.ToggleState -ne [Windows.Automation.ToggleState]::Off) { throw 'Focus is still pressed' }
}
Stop-Bookshelf

# ---- Closing the window saves the last words -------------------------------------
$kept = Join-Path $env:RUNNER_TEMP 'demo-journal-kept'
Start-Bookshelf @('--demo-journal-in', "`"$kept`"")
Step 'Writer: closing the window saves what was just typed' {
    Wait-Name 'ShelfHeading' 'Reading now' | Out-Null
    Open-Writer '^2' 'Dune'
    # Straight to closing, before the autosave pause is over.
    Type-Now '^{END}{ENTER}Saved as the window closed.'
    Close-Window
    Wait-Exit
}
Start-Bookshelf @('--demo-journal-in', "`"$kept`"")
Step 'Writer: ...and it is there after starting again' {
    Wait-Name 'ShelfHeading' 'Reading now' | Out-Null
    Open-Book '^2' 'Dune'
    $preview = (Find-Id 'SummaryPreview')
    $text = $preview.GetCurrentPattern([Windows.Automation.TextPattern]::Pattern).DocumentRange.GetText(-1)
    if ($text -notlike '*Saved as the window closed.*') { throw "the text typed before closing is gone" }
}
Stop-Bookshelf

# ---- A save that fails: the text is rescued to a file -------------------------------
Start-Bookshelf @('--demo-journal', '--demo-save-fails')
Step 'Writer: a failed save says so' {
    Wait-Name 'ShelfHeading' 'Reading now' | Out-Null
    Open-Writer '^2' 'Dune'
    Keys '^{END}{ENTER}These words will need rescuing.'
    $problem = Wait-Name 'WriterProblem' 'Not saved:*'
    Write-Host "      $problem"
    Snap '46-writer-not-saved'
}
Step 'Writer: leaving rescues the text to a file' {
    Keys '%{LEFT}'
    Wait-Name 'BookTitle' 'Dune' | Out-Null
    $notice = Find-TextLike 'Notice' "*couldn't be saved*"
    if ($notice -notmatch 'A copy of your text is in (.+?\.md)\.') { throw "no file in '$notice'" }
    $file = $Matches[1]
    if (-not (Test-Path -LiteralPath $file)) { throw "$file isn't there" }
    if ((Get-Content -LiteralPath $file -Raw) -notlike '*These words will need rescuing.*') { throw "$file doesn't hold the text" }
    Write-Host "      rescued to $file"
    Snap '47-writer-rescue-notice'
}
Step 'Writer: closing asks, after rescuing the text' {
    Keys '^e'
    Find-Id 'Editor' | Out-Null
    Wait-Focused 'Editor'
    Keys '^{END}{ENTER}And these too.'
    Wait-Name 'WriterProblem' 'Not saved:*' | Out-Null
    Close-Window
    $message = (Find-Id 'RescueMessage').Current.Name
    if ($message -notmatch 'A copy of your text is in (.+?\.md)\.') { throw "no file in '$message'" }
    $file = $Matches[1]
    if (-not (Test-Path -LiteralPath $file)) { throw "$file isn't there" }
    if ((Get-Content -LiteralPath $file -Raw) -notlike '*And these too.*') { throw "$file doesn't hold the text" }
    Write-Host "      rescued to $file"
    Snap '48-writer-rescue-dialog'
    Invoke-Button 'Keep Open'
    Start-Sleep -Seconds 1
    if ($script:app.HasExited) { throw 'Keep Open closed the window' }
    Close-Window
    Find-Id 'RescueMessage' | Out-Null
    Invoke-Button 'Close Anyway'
    Wait-Exit
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
Step 'Dark writer: Markdown, focus mode, full screen' {
    Open-Writer '^1' 'To the Lighthouse'
    Keys '^{HOME}'
    Snap '50-dark-writer'
    Keys '{DOWN 4}^+f'
    Start-Sleep -Seconds 1
    Snap '51-dark-writer-focus'
    Keys '{F11}'
    Wait-Gone 'SearchBox'
    Start-Sleep -Seconds 1
    Snap '53-dark-writer-full-screen'
    Keys '{ESC}'
    Find-Id 'SearchBox' | Out-Null
    Keys '^+f'
}
Step 'Dark writer: prompts on an empty page' {
    Keys '%{LEFT}'
    Find-Id 'BookTitle' | Out-Null
    Open-Writer '^3' 'Piranesi'
    Wait-Name 'WriterPrompt' 'Why do you want to read this one?*' | Out-Null
    Snap '52-dark-writer-prompts'
}
Stop-Bookshelf

# ---- Jo: nothing yet -----------------------------------------------------------
Start-Bookshelf @('--demo-journal', '--demo-profile', 'Jo')
Step 'Empty shelf' {
    Wait-Name 'ShelfHeading' 'Reading now' | Out-Null
    Find-Id 'EmptyAddBookButton' | Out-Null
    Snap '30-empty-shelf'
}
Stop-Bookshelf

# ---- High Contrast (best effort: shows the writer in Windows' own colors) ---
# Switched on for the runner's session only, and always back off. A runner
# that can't switch just skips these screenshots; nothing fails.
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public static class Contrast
{
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    struct HIGHCONTRAST { public int cbSize; public int dwFlags; public IntPtr lpszDefaultScheme; }

    [DllImport("user32.dll", SetLastError = true)]
    static extern bool SystemParametersInfo(uint action, uint param, ref HIGHCONTRAST value, uint flags);

    // SPI_SETHIGHCONTRAST with HCF_HIGHCONTRASTON, telling running apps.
    public static bool Set(bool on)
    {
        var hc = new HIGHCONTRAST { cbSize = Marshal.SizeOf(typeof(HIGHCONTRAST)), dwFlags = on ? 1 : 0 };
        return SystemParametersInfo(0x0043, (uint)hc.cbSize, ref hc, 0x2);
    }
}
"@
try {
    if (-not [Contrast]::Set($true)) { throw 'SystemParametersInfo refused' }
    Start-Sleep -Seconds 3
    Start-Bookshelf @('--demo-journal')
    Wait-Name 'ShelfHeading' 'Reading now' | Out-Null
    Open-Writer '^2' 'Dune'
    Keys '^{HOME}'
    Snap '60-high-contrast-writer'
    Keys '{DOWN 4}^+f'
    Start-Sleep -Seconds 1
    Snap '61-high-contrast-focus'
    Write-Host 'ok    High Contrast screenshots'
}
catch { Write-Host "skip  High Contrast screenshots: $_" }
finally {
    Stop-Bookshelf
    [Contrast]::Set($false) | Out-Null
}

if ($failures.Count) {
    Write-Host ''
    Write-Host "$($failures.Count) step(s) failed:"
    $failures | ForEach-Object { Write-Host "  $_" }
    exit 1
}
Write-Host 'Every step passed.'
