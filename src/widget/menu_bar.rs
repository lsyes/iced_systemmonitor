// SPDX-License-Identifier: GPL-3.0-only

//! An in-application menu. The menu is drawn inside of the application window
//! instead of using a native popup surface.

use iced::{
    Element, Length, Theme, Widget, border,
    widget::{Column, Space, button, container, mouse_area, opaque, text},
};

use crate::theme;

/// An entry of the application menu.
#[derive(Clone, Debug)]
pub struct MenuItem<Message> {
    /// The label of the entry.
    pub label: String,
    /// The message sent when the entry is selected.
    pub message: Message,
}

impl<Message> MenuItem<Message> {
    pub fn new(label: impl Into<String>, message: Message) -> Self {
        Self {
            label: label.into(),
            message,
        }
    }
}

/// The button which opens the application menu.
pub fn menu_button<'a, Message: Clone + 'a>(
    label: &'static str,
    open: bool,
    on_press: Message,
) -> Element<'a, Message> {
    button(text(label).size(20))
        .padding([theme::SPACE_XXS, theme::SPACE_S])
        .style(move |iced_theme, status| button_style(iced_theme, status, open))
        .on_press(on_press)
        .boxed()
}

/// The menu panel, drawn inside of the application window.
pub fn menu_panel<'a, Message: Clone + 'a>(
    items: impl IntoIterator<Item = MenuItem<Message>>,
    width: f32,
) -> Element<'a, Message> {
    let mut children: Vec<Element<'a, Message>> = Vec::new();

    for item in items {
        children.push(
            button(text(item.label))
                .width(Length::Fill)
                .padding([theme::SPACE_XS, theme::SPACE_S])
                .style(item_style)
                .on_press(item.message)
                .boxed(),
        );
    }

    container(Column::with_children(children).spacing(theme::SPACE_XXXS))
        .width(Length::Fixed(width))
        .padding(theme::SPACE_XXS)
        .style(panel_style)
        .boxed()
}

/// A transparent layer that dismisses the menu when clicked.
pub fn menu_dismiss<'a, Message: Clone + 'a>(on_dismiss: Message) -> Element<'a, Message> {
    opaque(
        mouse_area(Space::new().width(Length::Fill).height(Length::Fill)).on_press(on_dismiss),
    )
    .boxed()
}

fn button_style(iced_theme: &Theme, status: button::Status, open: bool) -> button::Style {
    let palette = iced_theme.palette();

    let mut style = button::Style {
        background: None,
        text_color: palette.background.base.text,
        border: border::rounded(6),
        ..button::Style::default()
    };

    if open {
        style.background = Some(palette.background.strong.color.into());
        style.text_color = palette.background.strong.text;
    } else {
        match status {
            button::Status::Hovered => {
                style.background = Some(palette.background.strong.color.into());
                style.text_color = palette.background.strong.text;
            }
            button::Status::Pressed => {
                style.background = Some(palette.background.stronger.color.into());
                style.text_color = palette.background.stronger.text;
            }
            _ => {}
        }
    }

    style
}

fn panel_style(iced_theme: &Theme) -> container::Style {
    let palette = iced_theme.palette();

    container::Style {
        background: Some(palette.background.weak.color.into()),
        border: border::rounded(8),
        shadow: iced::Shadow {
            color: iced::Color::BLACK.scale_alpha(0.25),
            offset: iced::Vector::new(0.0, 2.0),
            blur_radius: 12.0,
        },
        ..container::Style::default()
    }
}

fn item_style(iced_theme: &Theme, status: button::Status) -> button::Style {
    let palette = iced_theme.palette();

    let mut style = button::Style {
        background: None,
        text_color: palette.background.weak.text,
        border: border::rounded(4),
        ..button::Style::default()
    };

    match status {
        button::Status::Hovered => {
            style.background = Some(palette.background.strong.color.into());
            style.text_color = palette.background.strong.text;
        }
        button::Status::Pressed => {
            style.background = Some(palette.primary.base.color.into());
            style.text_color = palette.primary.base.text;
        }
        _ => {}
    }

    style
}
