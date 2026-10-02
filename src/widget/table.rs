// SPDX-License-Identifier: GPL-3.0-only

//! Traits and helpers for rendering tabular data.

use std::{borrow::Cow, cmp::Ordering, fmt};

use iced::{
    Alignment, Element, Length, Widget,
    widget::{
        Row, container,
        text::{Ellipsis, Wrapping},
        mouse_area, row, text,
    },
};

use crate::{
    icons::{self, Icon},
    theme,
};

/// A column of a table.
pub trait ItemCategory: Sized + Copy + PartialEq {
    /// The width of the column.
    fn width(&self) -> Length;

    /// The alignment of the data in the column.
    fn data_align(&self) -> Alignment {
        Alignment::End
    }
}

/// A row of a table.
pub trait ItemInterface<Category> {
    /// The icon shown in the given column, if any.
    fn get_icon(&self, category: Category) -> Option<Icon>;

    /// The text shown in the given column.
    fn get_text(&self, category: Category) -> Cow<'static, str>;

    /// Compares two items for the given column.
    fn compare(&self, other: &Self, category: Category) -> Ordering;
}

/// The height of a table row, shared by the header and the data rows.
pub const ROW_HEIGHT: f32 = 40.0;

/// The direction of a sorted column.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SortDirection {
    /// Ascending, sorted with the smallest value first.
    #[default]
    Ascending,
    /// Descending, sorted with the largest value first.
    Descending,
}

impl SortDirection {
    /// Returns the arrow drawn next to a sorted column.
    fn arrow(self) -> &'static str {
        match self {
            Self::Ascending => "\u{25b2}",
            Self::Descending => "\u{25bc}",
        }
    }

    /// Returns whether the direction is descending.
    pub fn is_descending(self) -> bool {
        matches!(self, Self::Descending)
    }

    /// Returns this direction, flipped.
    pub fn flipped(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}

/// Renders the header of a table.
pub fn header<'a, Category, Message>(
    categories: &[Category],
    sort_category: Category,
    sort_direction: SortDirection,
    sortable: bool,
    on_sort: fn(Category) -> Message,
) -> Element<'a, Message>
where
    Category: ItemCategory + fmt::Display + 'a,
    Message: Clone + 'a,
{
    let mut cells: Vec<Element<'a, Message>> = Vec::with_capacity(categories.len());

    for &category in categories {
        let mut label: Vec<Element<'a, Message>> = vec![
            text(category.to_string())
                .size(14)
                .wrapping(Wrapping::None)
                .boxed(),
        ];

        if category == sort_category {
            label.push(text(sort_direction.arrow()).size(10).boxed());
        }

        let cell = container::<_, iced::Theme>(row(label).align_y(Alignment::Center))
            .align_x(category.data_align())
            .align_y(Alignment::Center)
            .padding([0.0, 8.0])
            .height(Length::Fixed(ROW_HEIGHT))
            .width(category.width());

        cells.push(if sortable {
            mouse_area(cell).on_press(on_sort(category)).boxed()
        } else {
            cell.boxed()
        });
    }

    row(cells).align_y(Alignment::Center).boxed()
}

/// Renders a single row of a table.
pub fn row_item<'a, Category, Item, Message>(
    item: &'a Item,
    categories: &[Category],
    selected: bool,
    on_press: Option<Message>,
) -> Element<'a, Message>
where
    Category: ItemCategory + 'a,
    Item: ItemInterface<Category> + 'a,
    Message: Clone + 'a,
{
    let mut cells: Vec<Element<'a, Message>> = Vec::with_capacity(categories.len());

    for &category in categories {
        let mut cell: Vec<Element<'a, Message>> = Vec::with_capacity(2);

        if let Some(icon) = item.get_icon(category) {
            cell.push(icons::view(&icon, 20));
        }

        let label = item.get_text(category);
        if !label.is_empty() {
            cell.push(
                text(label)
                    .wrapping(Wrapping::None)
                    .ellipsis(Ellipsis::End)
                    .boxed(),
            );
        }

        cells.push(
            container(
                Row::with_children(cell)
                    .align_y(Alignment::Center)
                    .spacing(theme::SPACE_XXS),
            )
            .align_x(category.data_align())
            .align_y(Alignment::Center)
            .padding([0.0, 8.0])
            .height(Length::Fixed(ROW_HEIGHT))
            .width(category.width())
            .boxed(),
        );
    }

    let content = container(row(cells).align_y(Alignment::Center)).style(move |iced_theme: &iced::Theme| {
        let palette = iced_theme.palette();

        if selected {
            container::Style {
                text_color: Some(palette.primary.base.text),
                background: Some(palette.primary.base.color.into()),
                ..container::Style::default()
            }
        } else {
            container::Style::default()
        }
    });

    if let Some(message) = on_press {
        mouse_area(content).on_press(message).boxed()
    } else {
        content.boxed()
    }
}

/// A horizontal divider between rows.
pub fn divider<'a, Message: 'a>() -> Element<'a, Message> {
    use iced::widget::Space;

    container(Space::new().height(1))
        .width(Length::Fill)
        .style(|iced_theme: &iced::Theme| {
            let palette = iced_theme.palette();

            container::Style {
                background: Some(palette.background.strong.color.into()),
                ..container::Style::default()
            }
        })
        .boxed()
}
