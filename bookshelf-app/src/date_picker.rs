//! A date row with a calendar popover: tap the calendar icon, pick a day.
//! "Today" is one tap. Days that would make the dates contradict each other
//! (finishing before starting) are greyed out by the `allowed` callback.

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use bookshelf_core::models::format_date;
use chrono::{Datelike, Local, NaiveDate};

pub struct DatePicker {
    inner: Rc<Inner>,
}

struct Inner {
    row: adw::ActionRow,
    popover: gtk::Popover,
    grid: gtk::Grid,
    title: gtk::Label,
    view: Cell<NaiveDate>, // first day of the month being shown
    selected: Cell<Option<NaiveDate>>,
    sunday_first: bool,
    date_format: String,
    allowed: Box<dyn Fn(NaiveDate) -> bool>,
    on_change: Box<dyn Fn(Option<NaiveDate>)>,
}

impl DatePicker {
    pub fn new(
        title: &str,
        initial: Option<NaiveDate>,
        date_format: &str,
        week_start: &str,
        allowed: impl Fn(NaiveDate) -> bool + 'static,
        on_change: impl Fn(Option<NaiveDate>) + 'static,
    ) -> Self {
        let row = adw::ActionRow::builder().title(title).build();

        // ---- popover contents -------------------------------------------
        let month_title = gtk::Label::builder()
            .hexpand(true)
            .css_classes(["heading"])
            .build();
        let nav = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(2)
            .css_classes(["calendar-nav"])
            .build();
        let grid = gtk::Grid::builder().row_spacing(2).column_spacing(2).build();
        let clear = gtk::Button::builder()
            .label("Clear date")
            .css_classes(["flat"])
            .build();

        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(10)
            .margin_end(10)
            .build();
        content.append(&nav);
        content.append(&grid);
        content.append(&clear);
        let popover = gtk::Popover::builder().child(&content).build();

        let calendar_btn = gtk::MenuButton::builder()
            .icon_name("x-office-calendar-symbolic")
            .tooltip_text("Pick a date")
            .valign(gtk::Align::Center)
            .css_classes(["flat"])
            .popover(&popover)
            .build();
        let today_btn = gtk::Button::builder()
            .label("Today")
            .valign(gtk::Align::Center)
            .css_classes(["flat"])
            .build();
        row.add_suffix(&today_btn);
        row.add_suffix(&calendar_btn);
        row.set_activatable_widget(Some(&calendar_btn));

        let start = initial.unwrap_or_else(|| Local::now().date_naive());
        let inner = Rc::new(Inner {
            row,
            popover,
            grid,
            title: month_title,
            view: Cell::new(first_of_month(start)),
            selected: Cell::new(initial),
            sunday_first: week_start != "monday",
            date_format: date_format.to_string(),
            allowed: Box::new(allowed),
            on_change: Box::new(on_change),
        });
        inner.refresh_row();

        // ---- month / year navigation ------------------------------------
        for (label, months) in [("«", -12), ("‹", -1)] {
            nav.append(&nav_button(&inner, label, months));
        }
        nav.append(&inner.title);
        for (label, months) in [("›", 1), ("»", 12)] {
            nav.append(&nav_button(&inner, label, months));
        }

        // ---- actions ------------------------------------------------------
        {
            let this = inner.clone();
            clear.connect_clicked(move |_| {
                this.popover.popdown();
                this.set(None);
            });
        }
        {
            let this = inner.clone();
            today_btn.connect_clicked(move |_| {
                let today = Local::now().date_naive();
                if (this.allowed)(today) {
                    this.set(Some(today));
                } else {
                    this.popover.popup(); // show why: today is greyed out
                }
            });
        }
        {
            let this = inner.clone();
            inner.popover.connect_show(move |_| {
                let base = this.selected.get().unwrap_or_else(|| Local::now().date_naive());
                this.view.set(first_of_month(base));
                this.render();
            });
        }

        Self { inner }
    }

    pub fn row(&self) -> &adw::ActionRow {
        &self.inner.row
    }
}

fn nav_button(inner: &Rc<Inner>, label: &str, months: i32) -> gtk::Button {
    let btn = gtk::Button::builder().label(label).css_classes(["flat"]).build();
    let this = inner.clone();
    btn.connect_clicked(move |_| {
        this.shift_month(months);
        this.render();
    });
    btn
}

impl Inner {
    fn refresh_row(&self) {
        let text = match self.selected.get() {
            Some(d) => format_date(&self.date_format, d),
            None => "Not set".to_string(),
        };
        self.row.set_subtitle(&text);
    }

    fn set(self: &Rc<Self>, date: Option<NaiveDate>) {
        self.selected.set(date);
        self.refresh_row();
        (self.on_change)(date);
    }

    fn shift_month(&self, months: i32) {
        let v = self.view.get();
        let total = v.year() * 12 + v.month0() as i32 + months;
        let (y, m) = (total.div_euclid(12), total.rem_euclid(12) as u32 + 1);
        self.view.set(NaiveDate::from_ymd_opt(y, m, 1).unwrap_or(v));
    }

    fn render(self: &Rc<Self>) {
        while let Some(child) = self.grid.first_child() {
            self.grid.remove(&child);
        }
        let first = self.view.get();
        self.title.set_label(&first.format("%B %Y").to_string());

        let names = if self.sunday_first {
            ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"]
        } else {
            ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"]
        };
        for (i, name) in names.iter().enumerate() {
            self.grid.attach(
                &gtk::Label::builder()
                    .label(*name)
                    .css_classes(["calendar-weekday"])
                    .build(),
                i as i32,
                0,
                1,
                1,
            );
        }

        let lead = if self.sunday_first {
            first.weekday().num_days_from_sunday()
        } else {
            first.weekday().num_days_from_monday()
        } as i32;
        let today = Local::now().date_naive();

        for day in 1..=days_in_month(first) {
            let Some(date) = NaiveDate::from_ymd_opt(first.year(), first.month(), day) else {
                continue;
            };
            let idx = lead + day as i32 - 1;

            let btn = gtk::Button::builder()
                .label(day.to_string())
                .css_classes(["calendar-day"])
                .build();
            if Some(date) == self.selected.get() {
                btn.add_css_class("suggested-action");
            } else {
                btn.add_css_class("flat");
            }
            if date == today {
                btn.add_css_class("calendar-today");
            }
            btn.set_sensitive((self.allowed)(date));

            let this = self.clone();
            btn.connect_clicked(move |_| {
                this.popover.popdown();
                this.set(Some(date));
            });
            self.grid.attach(&btn, idx % 7, idx / 7 + 1, 1, 1);
        }
    }
}

fn first_of_month(d: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap_or(d)
}

fn days_in_month(first: NaiveDate) -> u32 {
    let (y, m) = (first.year(), first.month());
    let next = if m == 12 {
        NaiveDate::from_ymd_opt(y + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(y, m + 1, 1)
    };
    next.map(|n| (n - first).num_days() as u32).unwrap_or(30)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_lengths() {
        let d = |y, m| NaiveDate::from_ymd_opt(y, m, 1).unwrap();
        assert_eq!(days_in_month(d(2026, 9)), 30);
        assert_eq!(days_in_month(d(2026, 12)), 31);
        assert_eq!(days_in_month(d(2028, 2)), 29);
        assert_eq!(days_in_month(d(2026, 2)), 28);
    }
}
