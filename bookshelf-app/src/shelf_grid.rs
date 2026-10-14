//! The shelf's book cards, laid out in a grid: three to a row when there's
//! room, then two, then one. Every card is the same size, a row that isn't
//! full sits centred, and a year heading runs the full width and starts a
//! new row. GtkFlowBox can't centre a short row, hence this layout.
//!
//! The column count follows the width at layout time, so resizing the
//! window only moves the cards; nothing is rebuilt.

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

/// Space between cards, across and down.
pub(crate) const GAP: i32 = 16;
/// The narrowest content width for three columns, and for two.
pub(crate) const THREE_COLUMNS: i32 = 600;
pub(crate) const TWO_COLUMNS: i32 = 400;

/// A box laid out as the shelf grid. Cards are the `gtk::Button`s in it;
/// anything else (a year heading) is a full-width line of its own.
pub(crate) fn shelf_grid() -> gtk::Box {
    let grid = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .valign(gtk::Align::Start)
        .css_classes(["shelf-grid"])
        .build();
    // The box's own layout is never used again: nothing here sets its
    // spacing or orientation.
    grid.set_layout_manager(Some(glib::Object::new::<ShelfLayout>()));
    grid
}

/// How many cards go in a row at this content width.
fn columns_for(width: i32) -> usize {
    if width >= THREE_COLUMNS {
        3
    } else if width >= TWO_COLUMNS {
        2
    } else {
        1
    }
}

/// One line of the grid, by position among the visible children.
#[derive(Debug, PartialEq)]
enum Line {
    Heading(usize),
    Cards(std::ops::Range<usize>),
}

/// Groups the children into lines: cards fill rows of `columns`, and a
/// heading ends the row before it.
fn lines(is_card: &[bool], columns: usize) -> Vec<Line> {
    let mut lines = vec![];
    let mut row_start = None;
    for (i, &card) in is_card.iter().enumerate() {
        if let Some(start) = row_start {
            if !card || i - start == columns {
                lines.push(Line::Cards(start..i));
                row_start = None;
            }
        }
        if !card {
            lines.push(Line::Heading(i));
        } else if row_start.is_none() {
            row_start = Some(i);
        }
    }
    if let Some(start) = row_start {
        lines.push(Line::Cards(start..is_card.len()));
    }
    lines
}

/// Where the first of `count` cards goes so the row is centred in `width`.
fn row_start(width: i32, count: usize, card_width: i32) -> i32 {
    let count = count as i32;
    ((width - count * card_width - (count - 1) * GAP) / 2).max(0)
}

/// The grid at one width: its children, lines, card size and height.
struct Plan {
    children: Vec<gtk::Widget>,
    lines: Vec<Line>,
    card_width: i32,
    card_height: i32,
    height: i32,
}

impl Plan {
    fn new(widget: &gtk::Widget, width: i32) -> Self {
        let mut children = vec![];
        let mut child = widget.first_child();
        while let Some(c) = child {
            child = c.next_sibling();
            if c.should_layout() {
                children.push(c);
            }
        }
        let is_card: Vec<bool> = children.iter().map(|c| c.is::<gtk::Button>()).collect();

        // Fewer columns if the cards wouldn't fit (very large text).
        let narrowest = children
            .iter()
            .zip(&is_card)
            .filter(|(_, &card)| card)
            .map(|(c, _)| c.measure(gtk::Orientation::Horizontal, -1).0)
            .max()
            .unwrap_or(0);
        let mut columns = columns_for(width);
        let card_width = loop {
            let n = columns as i32;
            let card_width = (width - (n - 1) * GAP) / n;
            if columns == 1 || card_width >= narrowest {
                break card_width.max(narrowest);
            }
            columns -= 1;
        };

        // All cards as tall as the tallest, so the rows line up.
        let card_height = children
            .iter()
            .zip(&is_card)
            .filter(|(_, &card)| card)
            .map(|(c, _)| c.measure(gtk::Orientation::Vertical, card_width).1)
            .max()
            .unwrap_or(0);

        let lines = lines(&is_card, columns);
        let mut height = 0;
        let mut after_cards = false;
        for line in &lines {
            match line {
                Line::Heading(i) => {
                    height += children[*i].measure(gtk::Orientation::Vertical, width).1;
                    after_cards = false;
                }
                Line::Cards(_) => {
                    if after_cards {
                        height += GAP;
                    }
                    height += card_height;
                    after_cards = true;
                }
            }
        }
        Self {
            children,
            lines,
            card_width,
            card_height,
            height,
        }
    }
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct ShelfLayout;

    #[glib::object_subclass]
    impl ObjectSubclass for ShelfLayout {
        const NAME: &'static str = "BookshelfShelfLayout";
        type Type = super::ShelfLayout;
        type ParentType = gtk::LayoutManager;
    }

    impl ObjectImpl for ShelfLayout {}

    impl LayoutManagerImpl for ShelfLayout {
        fn request_mode(&self, _widget: &gtk::Widget) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(
            &self,
            widget: &gtk::Widget,
            orientation: gtk::Orientation,
            for_size: i32,
        ) -> (i32, i32, i32, i32) {
            if orientation == gtk::Orientation::Horizontal {
                // As narrow as the widest single card or heading; three
                // columns if there's room.
                let mut minimum = 0;
                let mut child = widget.first_child();
                while let Some(c) = child {
                    if c.should_layout() {
                        minimum = minimum.max(c.measure(orientation, -1).0);
                    }
                    child = c.next_sibling();
                }
                return (minimum, minimum.max(THREE_COLUMNS), -1, -1);
            }
            let width = if for_size < 0 {
                self.measure(widget, gtk::Orientation::Horizontal, -1).0
            } else {
                for_size
            };
            let height = Plan::new(widget, width).height;
            (height, height, -1, -1)
        }

        fn allocate(&self, widget: &gtk::Widget, width: i32, _height: i32, _baseline: i32) {
            let plan = Plan::new(widget, width);
            let mut y = 0;
            let mut after_cards = false;
            for line in &plan.lines {
                match line {
                    Line::Heading(i) => {
                        let heading = &plan.children[*i];
                        let height = heading.measure(gtk::Orientation::Vertical, width).1;
                        heading.size_allocate(&gtk::Allocation::new(0, y, width, height), -1);
                        y += height;
                        after_cards = false;
                    }
                    Line::Cards(range) => {
                        if after_cards {
                            y += GAP;
                        }
                        let mut x = row_start(width, range.len(), plan.card_width);
                        for card in &plan.children[range.clone()] {
                            let at = gtk::Allocation::new(x, y, plan.card_width, plan.card_height);
                            card.size_allocate(&at, -1);
                            x += plan.card_width + GAP;
                        }
                        y += plan.card_height;
                        after_cards = true;
                    }
                }
            }
        }
    }

    #[derive(Default)]
    pub struct FixedLayout {
        pub(super) size: std::cell::Cell<(i32, i32)>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FixedLayout {
        const NAME: &'static str = "BookshelfFixedLayout";
        type Type = super::FixedLayout;
        type ParentType = gtk::LayoutManager;
    }

    impl ObjectImpl for FixedLayout {}

    impl LayoutManagerImpl for FixedLayout {
        fn measure(
            &self,
            _widget: &gtk::Widget,
            orientation: gtk::Orientation,
            _for_size: i32,
        ) -> (i32, i32, i32, i32) {
            let (width, height) = self.size.get();
            let size = match orientation {
                gtk::Orientation::Horizontal => width,
                _ => height,
            };
            (size, size, -1, -1)
        }

        fn allocate(&self, widget: &gtk::Widget, width: i32, height: i32, _baseline: i32) {
            let mut child = widget.first_child();
            while let Some(c) = child {
                if c.should_layout() {
                    c.measure(gtk::Orientation::Horizontal, -1); // GTK asks for it
                    c.size_allocate(&gtk::Allocation::new(0, 0, width, height), -1);
                }
                child = c.next_sibling();
            }
        }
    }
}

glib::wrapper! {
    pub struct ShelfLayout(ObjectSubclass<imp::ShelfLayout>)
        @extends gtk::LayoutManager;
}

glib::wrapper! {
    pub struct FixedLayout(ObjectSubclass<imp::FixedLayout>)
        @extends gtk::LayoutManager;
}

/// `child` held at exactly `width`×`height`. A cover's picture asks for
/// its image's own size, which is twice the size on screen (decoded that
/// way to stay sharp on high-density displays).
pub(crate) fn fixed_size(child: &impl IsA<gtk::Widget>, width: i32, height: i32) -> gtk::Box {
    let layout = glib::Object::new::<FixedLayout>();
    layout.imp().size.set((width, height));
    let frame = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    frame.set_layout_manager(Some(layout));
    frame.append(child);
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_follow_the_width() {
        assert_eq!(columns_for(688), 3);
        assert_eq!(columns_for(600), 3);
        assert_eq!(columns_for(599), 2);
        assert_eq!(columns_for(400), 2);
        assert_eq!(columns_for(399), 1);
        assert_eq!(columns_for(0), 1);
    }

    #[test]
    fn cards_fill_rows_and_headings_start_new_ones() {
        // A heading, five cards, a heading, one card.
        let shelf = [false, true, true, true, true, true, false, true];
        assert_eq!(
            lines(&shelf, 3),
            [
                Line::Heading(0),
                Line::Cards(1..4),
                Line::Cards(4..6),
                Line::Heading(6),
                Line::Cards(7..8),
            ]
        );
        assert_eq!(
            lines(&shelf, 2),
            [
                Line::Heading(0),
                Line::Cards(1..3),
                Line::Cards(3..5),
                Line::Cards(5..6),
                Line::Heading(6),
                Line::Cards(7..8),
            ]
        );
        assert_eq!(lines(&[true; 2], 1), [Line::Cards(0..1), Line::Cards(1..2)]);
        assert!(lines(&[], 3).is_empty());
    }

    #[test]
    fn short_rows_are_centred_and_full_rows_fill() {
        // 688 wide, three columns of 218 with 16 between.
        assert_eq!(row_start(688, 3, 218), 1);
        assert_eq!(row_start(688, 2, 218), 118);
        assert_eq!(row_start(688, 1, 218), 235);
        // The centred row's cards sit halfway between the full row's.
        assert_eq!(row_start(688, 2, 218), 1 + (218 + GAP) / 2);
    }
}
