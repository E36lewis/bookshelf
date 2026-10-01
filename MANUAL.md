# Bookshelf

A quiet place to keep track of the books you read, and to write down what they meant to you.

Bookshelf is a reading journal for your computer. Look up any book, put it on a shelf, note when you started and finished it, and write a summary in a calm, distraction-free writing space. Everything stays on your own computer, and several people can share one computer with a profile each.

Press **F1** anywhere in Bookshelf to open this manual, or **Ctrl+?** to see every keyboard shortcut (on most keyboards that's **Ctrl+Shift+/**).

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

The first time you open Bookshelf, it asks for a name. That's your profile: your own shelves, your own summaries and your own settings. Anyone else who reads in your home can make their own profile from the same screen, under **New profile**.

You can also add an email address. It's optional: see *Your privacy* below for what it's used for. Hover over the ⓘ next to the field for a reminder.

Next time, just pick your name under **Who's reading?**

### Add your first book

1. Press the **+** button at the top left, or **Ctrl+N**.
2. Type a title, an author, or an ISBN. Results appear as you type.
3. Click the book you mean.
4. Choose a shelf: **Someday**, **Reading now** or **Finished**.

That's it. The book's cover and description are fetched for you, and its page opens so you can start writing.

## Your three shelves

Your books live on three shelves. Switch between them with the tabs at the top, or with the number keys:

- **Reading** (key **1**): books you've started but not finished. Each one shows when you started and which day of reading you're on.
- **Finished** (key **2**): books you've finished, newest first, grouped by year. The top of the page tells you how many you've finished this year.
- **Eventually** (key **3**): books you'd like to read someday.

Each entry shows the cover, the author, the dates and the first few lines of what you wrote.

### Moving a book to another shelf

A book's shelf follows its dates:

- A **finished** date puts it on **Finished**.
- A **started** date, with no finished date, puts it on **Reading**.
- No dates at all puts it on **Eventually**.

So to move a book, open it and change its dates. Starting a book from your Eventually shelf is as simple as setting today as its start date.

### Searching your shelves

Press the magnifying glass, or **Ctrl+F**, or simply start typing on the home screen. Bookshelf searches titles, authors and everything you've written, across all three shelves at once. Press **Esc** or the magnifying glass again to see everything.

The number keys switch shelves, so to search for something that starts with a number (say, *1984*), press **Ctrl+F** first.

## A book's page

Click any book to open its page. You'll find:

- **The book itself**: cover, title, author, publisher, year and number of pages.
- **About this book**: the publisher's description, folded away until you want it.
- **Reading dates**: when you started and finished.
- **My summary**: what you've written, laid out like a page in a journal.

### Reading dates

Each date has a **Today** button and a calendar button. In the calendar, use **‹ ›** to move a month at a time and **« »** to move a year, or **Clear date** to remove it. Days that would make the dates impossible, such as finishing before you started, are greyed out.

Dates are saved the moment you pick them. Once a book has both dates, Bookshelf tells you how many days it took you.

### Writing and reading your summary

- **Write** (or **Edit**, once there's something written), or press **E**: opens the writing page. Clicking the summary itself does the same.
- **Read**, or press **R**: opens your summary on its own, for distraction-free reading.

### Reading a book again

Read a favourite twice? Choose **Read it again** from the **⋯** menu at the top right of the book's page. You get a fresh entry, started today, with its own dates and its own summary. Your first one stays as it was.

The same choice comes up if you add a book you've already logged.

### Removing a book

Choose **Remove from my shelf** from the **⋯** menu. Your summary and dates for that book are removed, and a message appears at the bottom of the window. Press **Undo** within ten seconds to bring everything back exactly as it was.

## Writing

The writing page is deliberately calm: one column of text and nothing else.

### It saves as you type

There's no save button to remember. Bookshelf saves a moment after you stop typing, when you leave the page, and when you close the window. The line at the bottom shows your word count and whether everything is saved. If you like to press **Ctrl+S** out of habit, go ahead: it saves straight away.

If saving ever fails (for example, the disk is full), the bottom line turns red and says why. Your words are never thrown away. When you leave the page, Bookshelf keeps a copy in a *recovery* folder (see *Where your data lives*), or, if even that isn't possible, puts the text on the clipboard so you can paste it somewhere safe. If you try to close the window, it stops and asks first.

### Prompts on an empty page

An empty page shows a few gentle questions to get you going, such as *What stayed with you after the last page?* They fade away as soon as you type.

### Formatting

Bookshelf uses Markdown: simple marks you type around words. The marks stay visible but dimmed, so your text stays readable. You can type them yourself or use the buttons above the page:

- **Bold**: `**like this**`, or **Ctrl+B**
- *Italic*: `*like this*`, or **Ctrl+I**
- Strikethrough: `~~like this~~`
- Code: `` `like this` ``
- Headings: start a line with `#`, `##` or `###` (the **H1**, **H2** and **H3** buttons)
- Quote: start a line with `>`
- Bulleted list: start lines with `-`
- Numbered list: start lines with `1.`, `2.` and so on
- Link: `[words](https://example.com)`

Select some text first and the button applies to the whole selection. Pressing the same button again removes the formatting (all except **Link**).

### Focus mode

Press **Focus** at the top, or **Ctrl+F**, and everything except the sentence you're writing fades back. The line you're on stays in the middle of the screen, like a typewriter. You can make focus mode the default in Settings.

### Full screen

Press **F11**, or the full screen button, to fill the whole screen. **Esc** or **F11** brings you back.

## Reading

The reading page shows your summary nicely formatted, with nothing around it. Press **F11** for full screen; the title bar disappears too. **Esc** brings you back. To make a change, press the pencil button at the top.

## Settings

Open Settings with the gear button on the home screen, or **Ctrl+,** (Ctrl and comma). Every change is saved and applied immediately, and each profile has its own settings.

### Profile

- **Name**: change it and press the tick to save.
- **Email (optional)**: see *Your privacy*.

### Appearance

- **Theme**: match your computer, or always light or always dark.
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

- **Export my summaries**: see *Exporting your summaries*.
- **Daily backups**: see *Backups*.

### Help

Opens the keyboard shortcuts and this manual.

### Deleting a profile

**Delete this profile…** is at the bottom of Settings. Because it can't be undone, it takes two steps:

1. The first dialog offers **Export first…**, so you can keep a copy of everything that profile wrote.
2. Then you type the profile's name to confirm.

The books themselves stay, for anyone else who has them on their shelves.

## Your data, backups and privacy

### Where your data lives

Everything is kept in one folder on your computer: `~/.local/share/bookshelf`. In there:

- **bookshelf.sqlite3**: your journal. Every profile, book, date and summary.
- **covers**: book cover images.
- **backups**: daily backups (unless you've chosen another folder).
- **recovery**: copies of text that couldn't be saved, if that ever happens.

The folder is private to your user account.

### Backups

Bookshelf makes a copy of your whole journal once a day, covering every profile, and keeps the last seven. You'll find them in **Settings → Your data → Daily backups**, which shows where they're kept and when the latest one was made.

**Keep them somewhere else.** A backup on the same disk won't help if that disk fails. Press **Change…** to keep backups on a USB drive or in a folder that syncs to the cloud. If that drive isn't plugged in, backups pause and Settings tells you, until it's back or you choose another folder. Press the arrow button to go back to the default folder.

**Restoring a backup:**

1. Quit Bookshelf.
2. In your data folder, delete `bookshelf.sqlite3-wal` and `bookshelf.sqlite3-shm` if they're there.
3. Copy the backup you want over `bookshelf.sqlite3`.
4. Open Bookshelf again.

### Exporting your summaries

**Settings → Your data → Export my summaries** saves every summary of your profile as a separate Markdown file. These are plain text files you can open in any text editor, keep in a notes app, or print. Choose a folder and they're written into a new **Bookshelf summaries** folder inside it. Each file starts with the book's title, author and your dates. Bookshelf never overwrites a file that's already there; it adds a number to the name instead. When it's done, press **Open** to see them.

### Your privacy

There's no account and no sign-in, and your summaries never leave your computer. Bookshelf only goes online to look up books on Open Library, a free public library catalogue run by the Internet Archive. It sends what you search for, and fetches book details and covers.

If you add an email address to your profile, it's sent along with those requests, so Open Library can contact you if something you do ever causes them a problem. It goes nowhere else. Leave it blank to send nothing.

Profiles keep each person's shelves separate, but they aren't password protected. Anyone using your computer account can open any profile.

## Keyboard shortcuts

Press **Ctrl+?** in Bookshelf to see these at any time.

### Anywhere

- **F1**: this manual
- **Ctrl+?**: keyboard shortcuts

### Your shelves

- **1**, **2**, **3**: Reading, Finished, Eventually
- **Ctrl+N**: add a book
- **Ctrl+F**: search your shelves
- **Ctrl+,**: settings

### A book's page

- **E**: write or edit your summary
- **R**: read your summary

### Writing

- **Ctrl+S**: save now
- **Ctrl+B**: bold
- **Ctrl+I**: italic
- **Ctrl+F**: focus mode
- **F11**: full screen
- **Esc**: leave full screen

### Reading

- **F11**: full screen
- **Esc**: leave full screen

## If something goes wrong

**"Bookshelf couldn't start".** Bookshelf shows this instead of your shelves if it can't open your journal, together with the reason. If the journal file is damaged, restore yesterday's backup (see *Backups*).

**"This journal was made by a newer version of Bookshelf".** You've opened your journal with an older copy of Bookshelf than the one that last used it. Update Bookshelf. The older version refuses on purpose, so it can't damage anything the newer one saved.

**A cover is missing.** Not every book on Open Library has a cover. If one appears later, search for the book and pick it again: Bookshelf fetches the cover, then asks whether to open your entry.

**Search finds nothing.** Book search needs an internet connection. Try fewer words, just the author's surname, or the ISBN from the back of the book.

**The bottom line says "Not saved".** See *It saves as you type*. Your text is safe, and Bookshelf tells you where it put it.

## Installing

Bookshelf runs on Linux.

**AppImage.** Download the `.AppImage` file and make it executable: in your file manager, open its Properties and allow it to run as a program, or run `chmod +x Bookshelf*.AppImage` in a terminal. Then double-click it. It works on Ubuntu 24.04 or newer, Fedora 40 or newer, and Debian 13 or newer. On Ubuntu, AppImages need `libfuse2t64`: `sudo apt install libfuse2t64`.

**From the source code.** With Rust and the GTK 4 and libadwaita development packages installed, run `bash install-local.sh` in the project folder. Bookshelf then appears in your app menu. Run `bash install-local.sh --uninstall` to remove it again; your journal is left untouched.

Both kinds of install use the same data folder, so they share your shelves.
