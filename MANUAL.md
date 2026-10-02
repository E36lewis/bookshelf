# Bookshelf

A quiet place to keep track of the books you read, and to write down what they meant to you.

Bookshelf is a reading journal for your computer. Look up any book, put it on a shelf, note when you started and finished it, and write a summary in a calm, distraction-free writing space. Everything stays on your own computer, and several people can share one computer with a profile each.

<!-- Text that differs per app sits in platform blocks like the ones below: invisible here on GitHub (which shows every platform's text), and each app keeps only its own. Headings stay outside them. bookshelf-core/src/manual.rs explains the rules and tests them. -->
<!-- platform: linux, windows -->
Press **F1** anywhere in Bookshelf to open this manual, or **Ctrl+?** to see every keyboard shortcut (on most keyboards that's **Ctrl+Shift+/**).
<!-- /platform -->
<!-- platform: mac -->
Choose **Help › Bookshelf Help**, or press **⌘?**, to open this manual. The menus show the keyboard shortcut for each command.
<!-- /platform -->

## Contents

- [Getting started](#getting-started)
  - [Make a profile](#make-a-profile)
  - [Add your first book](#add-your-first-book)
- [Your three shelves](#your-three-shelves)
  - [Moving a book to another shelf](#moving-a-book-to-another-shelf)
  - [Searching your shelves](#searching-your-shelves)
- [A book's page](#a-books-page)
  - [Reading dates](#reading-dates)
  - [Writing and reading your summary](#writing-and-reading-your-summary)
  - [Reading a book again](#reading-a-book-again)
  - [Removing a book](#removing-a-book)
- [Writing](#writing)
  - [It saves as you type](#it-saves-as-you-type)
  - [Prompts on an empty page](#prompts-on-an-empty-page)
  - [Formatting](#formatting)
  - [Focus mode](#focus-mode)
  - [Full screen](#full-screen)
- [Reading](#reading)
- [Settings](#settings)
  - [Deleting a profile](#deleting-a-profile)
- [Your data, backups and privacy](#your-data-backups-and-privacy)
  - [Where your data lives](#where-your-data-lives)
  - [Backups](#backups)
  - [Exporting your summaries](#exporting-your-summaries)
  - [Your privacy](#your-privacy)
- [Keyboard shortcuts](#keyboard-shortcuts)
- [If something goes wrong](#if-something-goes-wrong)
- [Installing](#installing)

## Getting started

### Make a profile

The first time you open Bookshelf, it asks for a name. That's your profile: your own shelves, your own summaries and your own settings.
<!-- platform: linux -->
Anyone else who reads in your home can make their own profile from the same screen, under **New profile**.

You can also add an email address. It's optional: see *Your privacy* below for what it's used for. Hover over the ⓘ next to the field for a reminder.

Next time, just pick your name under **Who's reading?**
<!-- /platform -->
<!-- platform: mac -->

You can also add an email address. It's optional: see *Your privacy* below for what it's used for.

After that, Bookshelf opens with the last profile you used. To switch, use the profile menu at the top of the sidebar and pick a name under **Who's reading?** Anyone else who reads in your home can make their own profile there too, with **New Profile…** (it's also in the **File** menu).
<!-- /platform -->
<!-- platform: windows -->

You can also add an email address. It's optional: see *Your privacy* below for what it's used for.

After that, Bookshelf opens with the last profile you used. To switch, select your name at the bottom of the left pane and pick another. Anyone else who reads in your home can make their own profile there too, with **New profile…**
<!-- /platform -->

### Add your first book

<!-- platform: linux -->
1. Press the **+** button at the top left, or **Ctrl+N**.
<!-- /platform -->
<!-- platform: mac -->
1. Press the **+** button above your list of books, or **⌘N** (**File › Add a Book…**).
<!-- /platform -->
<!-- platform: windows -->
1. Press **Add a book** at the top of the shelf, or **Ctrl+N**.
<!-- /platform -->
2. Type a title, an author, or an ISBN. Results appear as you type.
3. Click the book you mean.
<!-- platform: linux -->
4. Choose a shelf: **Someday**, **Reading now** or **Finished**.
<!-- /platform -->
<!-- platform: mac -->
4. Choose a shelf: **Someday**, **Reading Now** or **Finished**.
<!-- /platform -->
<!-- platform: windows -->
4. Choose a shelf: **Someday**, **Reading now** or **Finished**, and press **Add**.
<!-- /platform -->

That's it. The book's cover and description are fetched for you, and its page opens so you can start writing.

## Your three shelves

<!-- platform: linux -->
Your books live on three shelves. Switch between them with the tabs at the top, or with the number keys **1**, **2** and **3**:
<!-- /platform -->
<!-- platform: mac -->
Your books live on three shelves. Switch between them in the sidebar, or with **⌘1**, **⌘2** and **⌘3**:
<!-- /platform -->
<!-- platform: windows -->
Your books live on three shelves. Switch between them in the left pane, or with **Ctrl+1**, **Ctrl+2** and **Ctrl+3**:
<!-- /platform -->

- **Reading**: books you've started but not finished. Each one shows when you started and which day of reading you're on.
- **Finished**: books you've finished, newest first, grouped by year. Bookshelf also tells you how many you've finished this year.
- **Eventually**: books you'd like to read someday.

Each entry shows the cover, the author, the dates and the first few lines of what you wrote.

### Moving a book to another shelf

A book's shelf follows its dates:

- A **finished** date puts it on **Finished**.
- A **started** date, with no finished date, puts it on **Reading**.
- No dates at all puts it on **Eventually**.

So to move a book, open it and change its dates. Starting a book from your Eventually shelf is as simple as setting today as its start date.

### Searching your shelves

<!-- platform: linux -->
Press the magnifying glass, or **Ctrl+F**, or simply start typing on the home screen. Bookshelf searches titles, authors and everything you've written, across all three shelves at once. Press **Esc** or the magnifying glass again to see everything.

The number keys switch shelves, so to search for something that starts with a number (say, *1984*), press **Ctrl+F** first.
<!-- /platform -->
<!-- platform: mac -->
Click the search field at the top of the window, or press **⌘F** (**Edit › Search Your Shelves**). Bookshelf searches titles, authors and everything you've written, across all three shelves at once. Clear the field, or press **Esc**, to see everything again.
<!-- /platform -->
<!-- platform: windows -->
Type in the **Search your shelves** box at the top of the window: click it, press **Ctrl+F**, or simply start typing on a shelf. Bookshelf searches titles, authors and everything you've written, across all three shelves at once, and each shelf in the left pane shows how many matches it has. Press **Esc** to clear the search and see everything again.
<!-- /platform -->

## A book's page

Click any book to open its page. You'll find:

- **The book itself**: cover, title, author, publisher, year and number of pages.
- **About this book**: the publisher's description, folded away until you want it.
- **Reading dates**: when you started and finished.
- **My summary**: what you've written, laid out like a page in a journal.

### Reading dates

<!-- platform: linux -->
Each date has a **Today** button and a calendar button. In the calendar, use **‹ ›** to move a month at a time and **« »** to move a year, or **Clear date** to remove it. Days that would make the dates impossible, such as finishing before you started, are greyed out.
<!-- /platform -->
<!-- platform: mac -->
Each date has a button that opens a calendar, a **Today** button, and an **ⓧ** button that clears it (the calendar has **Today** and **Clear Date** too). Days that would make the dates impossible, such as finishing before you started, can't be picked.
<!-- /platform -->
<!-- platform: windows -->
Each date has a calendar, a **Today** button, and an **✕** button that clears it. Days that would make the dates impossible, such as finishing before you started, can't be picked.
<!-- /platform -->

Dates are saved the moment you pick them. Once a book has both dates, Bookshelf tells you how many days it took you.

### Writing and reading your summary

<!-- platform: linux -->
- **Write** (or **Edit**, once there's something written), or press **E**: opens the writing page. Clicking the summary itself does the same.
- **Read**, or press **R**: opens your summary on its own, for distraction-free reading.
<!-- /platform -->
<!-- platform: mac -->
- **Write** (or **Edit**, once there's something written), or press **⌘↩** (Command-Return): opens the writing page. Clicking the summary itself does the same.
- **Read**, or press **⌘R**: opens your summary on its own, for distraction-free reading.

The same commands are in the **Book** menu. In your list of books, double-click a book (or select it and press Return) to read what you wrote, or to start writing if there's nothing yet.
<!-- /platform -->
<!-- platform: windows -->
- **Write** (or **Edit**, once there's something written), or press **Ctrl+E**: opens the writing page. Clicking the summary itself does the same.
- **Read**, or press **Ctrl+R**: opens your summary on its own, for distraction-free reading. It appears once you've written something.
<!-- /platform -->

### Reading a book again

<!-- platform: linux -->
Read a favourite twice? Choose **Read it again** from the **⋯** menu at the top right of the book's page. You get a fresh entry, started today, with its own dates and its own summary. Your first one stays as it was.
<!-- /platform -->
<!-- platform: mac -->
Read a favourite twice? Choose **Read It Again** from the **⋯** menu at the top of the book's page (it's also in the **Book** menu, and when you right-click the book in your list). You get a fresh entry, started today, with its own dates and its own summary. Your first one stays as it was.
<!-- /platform -->
<!-- platform: windows -->
Read a favourite twice? Choose **Read it again** from the **⋯** menu next to **Write**. You get a fresh entry, started today, with its own dates and its own summary. Your first one stays as it was.
<!-- /platform -->

The same choice comes up if you add a book you've already logged.

### Removing a book

<!-- platform: linux -->
Choose **Remove from my shelf** from the **⋯** menu. Your summary and dates for that book are removed, and a message appears at the bottom of the window. Press **Undo** within ten seconds to bring everything back exactly as it was.
<!-- /platform -->
<!-- platform: mac -->
Choose **Remove from My Shelf** from the **⋯** menu or the **Book** menu, or press **⌘⌫** (Command-Delete). Your summary and dates for that book are removed, and the page says so, with an **Undo** button that brings everything back exactly as it was. Changed your mind later? **Edit › Undo Remove** (**⌘Z**) still brings it back, until you switch profiles.
<!-- /platform -->
<!-- platform: windows -->
Choose **Remove from my shelf** from the **⋯** menu, or press **Delete**. Your summary and dates for that book are removed, and a notice appears at the bottom of the window. Press **Undo** within ten seconds (or **Ctrl+Z** on the shelf) to bring everything back exactly as it was.
<!-- /platform -->

## Writing

The writing page is deliberately calm: one column of text and nothing else.

<!-- platform: mac -->
**⌘Z** and **⇧⌘Z** undo and redo your typing, and **⌘F** finds words in your summary (**Edit › Find**). **Edit › Spelling and Grammar** checks your spelling when you ask it to.
<!-- /platform -->
<!-- platform: windows -->
**Ctrl+Z** and **Ctrl+Y** undo and redo your typing.
<!-- /platform -->

### It saves as you type

<!-- platform: linux -->
There's no save button to remember. Bookshelf saves a moment after you stop typing, when you leave the page, and when you close the window. The line at the bottom shows your word count and whether everything is saved. If you like to press **Ctrl+S** out of habit, go ahead: it saves straight away.
<!-- /platform -->
<!-- platform: mac -->
There's no saving to remember. Bookshelf saves a moment after you stop typing, when you leave the page, when you switch to another app, and when you quit. The line at the bottom shows your word count and whether everything is saved. If you like to press **⌘S** out of habit, go ahead: it saves straight away.
<!-- /platform -->
<!-- platform: windows -->
There's no saving to remember. Bookshelf saves a moment after you stop typing, when you leave the page, and when you close the window. The line at the bottom shows your word count and whether everything is saved. If you like to press **Ctrl+S** or the **Save** button out of habit, go ahead: it saves straight away.
<!-- /platform -->

If saving ever fails (for example, the disk is full), the bottom line turns red and says why. Your words are never thrown away. When you leave the page, Bookshelf keeps a copy in a *recovery* folder (see *Where your data lives*), or, if even that isn't possible, puts the text on the clipboard so you can paste it somewhere safe. If you try to close Bookshelf while it can't save, it stops and asks first.

### Prompts on an empty page

An empty page shows a few gentle questions to get you going, such as *What stayed with you after the last page?* They fade away as soon as you type.

### Formatting

Bookshelf uses Markdown: simple marks you type around words. The marks stay visible but dimmed, so your text stays readable. You can type them yourself or use the buttons above the page:

<!-- platform: linux, windows -->
- **Bold**: `**like this**`, or **Ctrl+B**
- *Italic*: `*like this*`, or **Ctrl+I**
<!-- /platform -->
<!-- platform: mac -->
- **Bold**: `**like this**`, or **⌘B**
- *Italic*: `*like this*`, or **⌘I**
<!-- /platform -->
- Strikethrough: `~~like this~~`
- Code: `` `like this` ``
<!-- platform: linux -->
- Headings: start a line with `#`, `##` or `###` (the **H1**, **H2** and **H3** buttons)
<!-- /platform -->
<!-- platform: mac -->
- Headings: start a line with `#`, `##` or `###` (the **Heading** menu on the bar)
<!-- /platform -->
<!-- platform: windows -->
- Headings: start a line with `#`, `##` or `###` (the three heading buttons)
<!-- /platform -->
- Quote: start a line with `>`
- Bulleted list: start lines with `-`
- Numbered list: start lines with `1.`, `2.` and so on
<!-- platform: linux -->
- Link: `[words](https://example.com)`
<!-- /platform -->
<!-- platform: mac -->
- Link: `[words](https://example.com)`, or **⌘K**
<!-- /platform -->
<!-- platform: windows -->
- Link: `[words](https://example.com)`, or **Ctrl+K**
<!-- /platform -->

Select some text first and the button applies to the whole selection. Pressing the same button again removes the formatting (all except **Link**).

<!-- platform: mac -->
The same commands are in the **Format** menu.
<!-- /platform -->

### Focus mode

<!-- platform: linux -->
Press **Focus** at the top, or **Ctrl+F**, and everything except the sentence you're writing fades back. The line you're on stays in the middle of the screen, like a typewriter. You can make focus mode the default in Settings.
<!-- /platform -->
<!-- platform: mac -->
Press **Focus** in the toolbar, or **⇧⌘F** (**View › Focus Mode**), and everything except the sentence you're writing fades back. The line you're on stays in the middle of the screen, like a typewriter. You can make focus mode the default in Settings.
<!-- /platform -->
<!-- platform: windows -->
Press the **Focus** button above the page, or **Ctrl+Shift+F**, and everything except the sentence you're writing fades back. The line you're on stays in the middle of the screen, like a typewriter. You can make focus mode the default in Settings.
<!-- /platform -->

### Full screen

<!-- platform: linux -->
Press **F11**, or the full screen button, to fill the whole screen. **Esc** or **F11** brings you back.
<!-- /platform -->
<!-- platform: mac -->
Press **⌃⌘F** (Control-Command-F), choose **View › Enter Full Screen**, or click the window's green button, to fill the whole screen. The toolbar hides until you move the pointer to the top. **Esc** or **⌃⌘F** brings you back.
<!-- /platform -->
<!-- platform: windows -->
<!-- TODO(owner): the Windows app is getting a second full-screen key. Add it here, under Reading, in the Keyboard shortcuts "More keys" list and in "F11 doesn't go full screen". -->
Press **F11**, or the full screen button above the page, to fill the whole screen. **Esc** or **F11** brings you back.
<!-- /platform -->

## Reading

<!-- platform: linux -->
The reading page shows your summary nicely formatted, with nothing around it. Press **F11** for full screen; the title bar disappears too. **Esc** brings you back. To make a change, press the pencil button at the top.
<!-- /platform -->
<!-- platform: mac -->
The reading page shows your summary nicely formatted, with nothing around it. Press **⌃⌘F** for full screen; the toolbar hides too. **Esc** leaves full screen, and pressed again takes you back to your shelf. To make a change, press **Edit**, or **⌘↩**.
<!-- /platform -->
<!-- platform: windows -->
The reading page shows your summary nicely formatted, with nothing around it. Press **F11**, or the full screen button at the top right, for full screen; the title bar disappears too. **Esc** brings you back. To make a change, press the pencil button at the top right.
<!-- /platform -->

## Settings

<!-- platform: linux -->
Open Settings with the gear button on the home screen, or **Ctrl+,** (Ctrl and comma). Every change is saved and applied immediately, and each profile has its own settings.
<!-- /platform -->
<!-- platform: mac -->
Open Settings with **Bookshelf › Settings…**, or **⌘,** (Command and comma), or **Profile Settings…** in the profile menu. Every change is saved and applied immediately, and each profile has its own settings. There's a tab for each part below: **Profile**, **Appearance**, **Writing**, **Reading Log** and **Data**.
<!-- /platform -->
<!-- platform: windows -->
Open Settings with **Settings** at the bottom of the left pane, or **Ctrl+,** (Ctrl and comma). Every change is saved and applied immediately, and each profile has its own settings.
<!-- /platform -->

### Profile

<!-- platform: linux -->
- **Name**: change it and press the tick to save.
<!-- /platform -->
<!-- platform: mac, windows -->
- **Name**: change it and press **Save**.
<!-- /platform -->
- **Email (optional)**: see *Your privacy*.

### Appearance

<!-- platform: linux -->
- **Theme**: match your computer, or always light or always dark.
<!-- /platform -->
<!-- platform: mac -->
- **Theme**: **Match System**, or always **Light** or always **Dark**.
<!-- /platform -->
<!-- platform: windows -->
- **Theme**: **Match Windows**, or always **Light** or always **Dark**. With a contrast theme turned on in Windows, Bookshelf uses its colors instead.
<!-- /platform -->
- **Accent color**: eight colors to choose from, or any color you like with the last button.
- **Titles and summaries**: serif (bookish) or sans-serif (clean).

### Writing

- **Font**: iA Writer Duo (the default), Source Serif, or your system's sans-serif or monospace font.
- **Text size**: from 10 to 28 points.
- **Line spacing**: tight, comfortable or airy.
- **Page width**: narrow, medium or wide.
- **Start in focus mode**: open the writing page with focus mode already on.

A preview shows what your writing will look like as you change these.

### Reading log

- **Date format**: *Sep 20, 2026*, *09/20/2026*, *20/09/2026* or *2026-09-20*.
- **Week starts on**: Sunday or Monday, for the calendar.
- **Open to**: which shelf you see first.

### Your data

<!-- platform: mac -->
On the **Data** tab:

<!-- /platform -->
- **Export my summaries**: see *Exporting your summaries*.
- **Daily backups**: see *Backups*.

### Help

<!-- platform: linux -->
Opens the keyboard shortcuts and this manual.
<!-- /platform -->
<!-- platform: mac -->
Settings has no Help tab on a Mac: this manual is in the **Help** menu, and the menus show every keyboard shortcut.
<!-- /platform -->
<!-- platform: windows -->
Opens the keyboard shortcuts and this manual. It also shows where your journal is kept, with a button to open that folder.
<!-- /platform -->

### Deleting a profile

<!-- platform: linux -->
**Delete this profile…** is at the bottom of Settings. Because it can't be undone, it takes two steps:

1. The first dialog offers **Export first…**, so you can keep a copy of everything that profile wrote.
2. Then you type the profile's name to confirm.
<!-- /platform -->
<!-- platform: mac -->
**Delete Profile…** is at the bottom of the **Profile** tab. Because it can't be undone, it takes two steps:

1. The first dialog offers **Export First…**, so you can keep a copy of everything that profile wrote.
2. Then you type the profile's name to confirm, and press **Delete Forever**.
<!-- /platform -->
<!-- platform: windows -->
**Delete this profile…** is at the bottom of Settings. Because it can't be undone, it takes two steps:

1. The first dialog offers **Export first…**, so you can keep a copy of everything that profile wrote. Once that's done, press **Delete this profile…** again to carry on.
2. Then you type the profile's name to confirm, and press **Delete Forever**.
<!-- /platform -->

The books themselves stay, for anyone else who has them on their shelves.

## Your data, backups and privacy

### Where your data lives

<!-- platform: linux -->
Everything is kept in one folder on your computer: `~/.local/share/bookshelf`. In there:
<!-- /platform -->
<!-- platform: mac -->
Everything is kept in one folder on your Mac. Bookshelf runs in the Mac's app sandbox, so that folder is inside the sandbox's own *container*:

`~/Library/Containers/io.github.e36lewis.Bookshelf/Data/Library/Application Support/io.github.e36lewis.Bookshelf`

To open it, choose **Go › Go to Folder…** in the Finder (**⇧⌘G**), paste that path and press Return. Preview builds of Bookshelf are a separate app with a journal of their own: their folder has `io.github.e36lewis.Bookshelf.Preview` in both places instead.

In there:
<!-- /platform -->
<!-- platform: windows -->
Everything is kept in one folder on your computer: `%LOCALAPPDATA%\Bookshelf` (usually `C:\Users\`*your name*`\AppData\Local\Bookshelf`). To open it, paste `%LOCALAPPDATA%\Bookshelf` into the address bar of File Explorer and press Enter. Preview builds of Bookshelf keep a journal of their own, in `%LOCALAPPDATA%\Bookshelf Preview`. **Settings › Help** shows the folder your copy uses, with a button to open it.

In there:
<!-- /platform -->

- **bookshelf.sqlite3**: your journal. Every profile, book, date and summary.
- **covers**: book cover images.
- **backups**: daily backups (unless you've chosen another folder).
- **recovery**: copies of text that couldn't be saved, if that ever happens.
<!-- platform: windows -->
- **logs**: notes on how Bookshelf started and any problems it ran into, handy if you report one.
<!-- /platform -->

<!-- platform: linux, mac -->
The folder is private to your user account.
<!-- /platform -->
<!-- platform: windows -->
Your journal is private to your user account.
<!-- /platform -->

### Backups

<!-- platform: linux, windows -->
Bookshelf makes a copy of your whole journal once a day, covering every profile, and keeps the last seven. You'll find them in **Settings → Your data → Daily backups**, which shows where they're kept and when the latest one was made.
<!-- /platform -->
<!-- platform: mac -->
Bookshelf makes a copy of your whole journal once a day, covering every profile, and keeps the last seven. You'll find them in **Settings → Data → Daily Backups**, which shows where they're kept and when the latest one was made.
<!-- /platform -->

<!-- platform: linux -->
**Keep them somewhere else.** A backup on the same disk won't help if that disk fails. Press **Change…** to keep backups on a USB drive or in a folder that syncs to the cloud. If that drive isn't plugged in, backups pause and Settings tells you, until it's back or you choose another folder. Press the arrow button to go back to the default folder.
<!-- /platform -->
<!-- platform: mac -->
**Keep them somewhere else.** A backup on the same disk won't help if that disk fails. Press **Change…** to keep backups on a USB drive or in a folder that syncs to the cloud. If that drive isn't plugged in, backups pause and Settings tells you, until it's back or you choose another folder. Press **Use Default** to go back to the default folder, or **Show in Finder** to see your backups.
<!-- /platform -->
<!-- platform: windows -->
**Keep them somewhere else.** A backup on the same disk won't help if that disk fails. Press **Change…** to keep backups on a USB drive or in a folder that syncs to the cloud. If that drive isn't plugged in, backups pause and Settings tells you, until it's back or you choose another folder. Press **Use default** to go back to the default folder, or **Open folder** to see your backups.
<!-- /platform -->

**Restoring a backup:**

<!-- platform: linux -->
1. Quit Bookshelf.
2. In your data folder, delete `bookshelf.sqlite3-wal` and `bookshelf.sqlite3-shm` if they're there.
3. Copy the backup you want over `bookshelf.sqlite3`.
4. Open Bookshelf again.
<!-- /platform -->
<!-- platform: mac -->
1. Quit Bookshelf (**⌘Q**).
2. Open your data folder in the Finder (see *Where your data lives*).
3. Delete `bookshelf.sqlite3-wal` and `bookshelf.sqlite3-shm` if they're there.
4. Copy the backup you want (they're named by date, such as `bookshelf-…-2026-09-20.sqlite3`) over `bookshelf.sqlite3`: drag it into the folder, delete the old `bookshelf.sqlite3`, and rename the copy to `bookshelf.sqlite3`.
5. Open Bookshelf again.
<!-- /platform -->
<!-- platform: windows -->
1. Close Bookshelf.
2. Open your data folder in File Explorer (see *Where your data lives*).
3. Delete `bookshelf.sqlite3-wal` and `bookshelf.sqlite3-shm` if they're there.
4. Copy the backup you want (they're named by date, such as `bookshelf-…-2026-09-20.sqlite3`) over `bookshelf.sqlite3`: copy it into the folder, delete the old `bookshelf.sqlite3`, and rename the copy to `bookshelf.sqlite3`.
5. Open Bookshelf again.
<!-- /platform -->

### Exporting your summaries

<!-- platform: linux -->
**Settings → Your data → Export my summaries** saves every summary of your profile as a separate Markdown file.
<!-- /platform -->
<!-- platform: mac -->
**Settings → Data → Export…** saves every summary of your profile as a separate Markdown file.
<!-- /platform -->
<!-- platform: windows -->
**Settings → Your data → Export my summaries → Export…** saves every summary of your profile as a separate Markdown file.
<!-- /platform -->
These are plain text files you can open in any text editor, keep in a notes app, or print. Choose a folder and they're written into a new **Bookshelf summaries** folder inside it. Each file starts with the book's title, author and your dates. Bookshelf never overwrites a file that's already there; it adds a number to the name instead.
<!-- platform: linux -->
When it's done, press **Open** to see them.
<!-- /platform -->
<!-- platform: mac -->
When it's done, press **Show in Finder** to see them.
<!-- /platform -->
<!-- platform: windows -->
When it's done, press **Open folder** to see them.
<!-- /platform -->

### Your privacy

There's no account and no sign-in, and your summaries never leave your computer. Bookshelf only goes online to look up books on Open Library, a free public library catalogue run by the Internet Archive. It sends what you search for, and fetches book details and covers.

If you add an email address to your profile, it's sent along with those requests, so Open Library can contact you if something you do ever causes them a problem. It goes nowhere else. Leave it blank to send nothing.

Profiles keep each person's shelves separate, but they aren't password protected. Anyone using your computer account can open any profile.

## Keyboard shortcuts

<!-- The lists under the bold group titles are checked word for word against bookshelf-core/src/shortcuts.rs; if the test fails, it prints the list to paste. -->
<!-- platform: linux -->
Press **Ctrl+?** in Bookshelf to see these at any time.

**Your shelves**

- **1**: Reading
- **2**: Finished
- **3**: Eventually
- **Ctrl+N**: Add a book
- **Ctrl+F**: Search your shelves
- **Ctrl+,**: Settings

**A book's page**

- **E**: Write or edit your summary
- **R**: Read your summary

**Writing**

- **Ctrl+S**: Save now (it also saves as you type)
- **Ctrl+B**: Bold
- **Ctrl+I**: Italic
- **Ctrl+F**: Focus mode

**Writing and reading**

- **F11**: Full screen
- **Esc**: Leave full screen

**Help**

- **F1**: User manual
- **Ctrl+?**: Keyboard shortcuts
<!-- /platform -->
<!-- platform: mac -->
The menus show these next to their commands, so you can always look one up there.

**Your shelves**

- **⌘1**: Reading
- **⌘2**: Finished
- **⌘3**: Eventually
- **⌘N**: Add a book
- **⌘F**: Search your shelves
- **⌘,**: Settings

**A book's page**

- **⌘↩**: Write or edit your summary
- **⌘R**: Read your summary

**Writing**

- **⌘S**: Save now (it also saves as you type)
- **⌘B**: Bold
- **⌘I**: Italic
- **⌘K**: Link
- **⇧⌘F**: Focus mode

**Writing and reading**

- **⌃⌘F**: Full screen
- **Esc**: Leave full screen

**Help**

- **⌘?**: User manual

**More keys**

- **⌘⌫**: remove a book from your shelf, on its page
- **⌘Z**, **⇧⌘Z**: undo and redo, including removing a book
- **⌘F** while writing: find words in your summary
- **Esc** while reading: back to your shelf
<!-- /platform -->
<!-- platform: windows -->
Press **Ctrl+?** in Bookshelf (on most keyboards that's **Ctrl+Shift+/**) to see these at any time.

**Your shelves**

- **Ctrl+1**: Reading
- **Ctrl+2**: Finished
- **Ctrl+3**: Eventually
- **Ctrl+N**: Add a book
- **Ctrl+F**: Search your shelves
- **Ctrl+,**: Settings

**A book's page**

- **Ctrl+E**: Write or edit your summary
- **Ctrl+R**: Read your summary

**Writing**

- **Ctrl+S**: Save now (it also saves as you type)
- **Ctrl+B**: Bold
- **Ctrl+I**: Italic
- **Ctrl+K**: Link
- **Ctrl+Shift+F**: Focus mode

**Writing and reading**

- **F11**: Full screen
- **Esc**: Leave full screen

**Help**

- **F1**: User manual
- **Ctrl+?**: Keyboard shortcuts

**More keys**

- **Delete**: remove a book from your shelf, on its page
- **Ctrl+Z** on a shelf: undo removing a book, while its notice shows
- **Ctrl+Z**, **Ctrl+Y** while writing: undo and redo
- **Alt+←**: go back
<!-- /platform -->

## If something goes wrong

**"Bookshelf couldn't start".** Bookshelf shows this instead of your shelves if it can't open your journal, together with the reason. If the journal file is damaged, restore yesterday's backup (see *Backups*).

**"This journal was made by a newer version of Bookshelf".** You've opened your journal with an older copy of Bookshelf than the one that last used it. Update Bookshelf. The older version refuses on purpose, so it can't damage anything the newer one saved.

**A cover is missing.** Not every book on Open Library has a cover. If one appears later, search for the book and pick it again: Bookshelf fetches the cover, then asks whether to open your entry.

**Search finds nothing.** Book search needs an internet connection. Try fewer words, just the author's surname, or the ISBN from the back of the book.

**Search fails, or a firewall or security software blocks Bookshelf.** When book search can't reach Open Library, the message says why. If it says a firewall may be blocking Bookshelf, or that it couldn't verify Open Library's secure connection, then a firewall, antivirus or other security software (or a work or school network that inspects secure connections) is standing in the way.
<!-- platform: linux -->
Allow Bookshelf through your firewall or security software, then search again.
<!-- /platform -->
<!-- platform: mac -->
If you use a firewall or security app, allow Bookshelf to connect, then search again.
<!-- /platform -->
<!-- platform: windows -->
Allow Bookshelf through it, then search again: for Windows' own firewall, that's **Windows Security › Firewall & network protection › Allow an app through firewall**; other security software has a setting of its own.
<!-- /platform -->
Bookshelf only ever connects to `openlibrary.org` and `covers.openlibrary.org`.

<!-- platform: linux, windows -->
**F11 doesn't go full screen.** On many laptops the top row of keys controls brightness, volume and so on, and needs **Fn** held down to work as **F11**. Press **Fn+F11**, or use the full screen button.
<!-- /platform -->

**The bottom line says "Not saved".** See *It saves as you type*. Your text is safe, and Bookshelf tells you where it put it.

## Installing

Bookshelf runs on Linux, macOS and Windows.

<!-- platform: linux -->
**AppImage.** Download the `.AppImage` file and make it executable: in your file manager, open its Properties and allow it to run as a program, or run `chmod +x Bookshelf*.AppImage` in a terminal. Then double-click it. It works on Ubuntu 24.04 or newer, Fedora 40 or newer, and Debian 13 or newer. On Ubuntu, AppImages need `libfuse2t64`: `sudo apt install libfuse2t64`.

**From the source code.** With Rust and the GTK 4 and libadwaita development packages installed, run `bash install-local.sh` in the project folder. Bookshelf then appears in your app menu. Run `bash install-local.sh --uninstall` to remove it again; your journal is left untouched.

Both kinds of install use the same data folder, so they share your shelves.
<!-- /platform -->
<!-- platform: mac -->
Bookshelf needs macOS 14 (Sonoma) or newer.

1. Open the `.dmg` file you downloaded, and drag **Bookshelf** onto **Applications**.
2. Open Bookshelf from your Applications folder.
3. Until Bookshelf is signed by Apple, the Mac won't open it the first time and says it can't check it. Press **Done**, then open **System Settings › Privacy & Security**, scroll down to the message about Bookshelf, press **Open Anyway** and confirm. You only need to do this once.

To remove Bookshelf, drag it from Applications to the Trash. Your journal stays in its folder (see *Where your data lives*).
<!-- /platform -->
<!-- platform: windows -->
Bookshelf needs Windows 10 (version 1809 or newer) or Windows 11.

<!-- TODO(owner): there's no installer yet, only the single Bookshelf.exe. Once there is one, name it here and say where it puts Bookshelf and how to uninstall it. -->
If you downloaded the installer, run it and follow its steps. If you downloaded the single `Bookshelf.exe`, there's nothing to install: put it wherever you like and double-click it. Either way, the very first start can take up to a minute; after that it's quick.

Until Bookshelf is signed, Windows may say *Windows protected your PC* the first time. Press **More info**, then **Run anyway**. You only need to do this once.

Removing Bookshelf, or deleting `Bookshelf.exe`, leaves your journal in its folder (see *Where your data lives*).
<!-- /platform -->
