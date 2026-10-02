// SPDX-License-Identifier: GPL-3.0-only

//! A selectable tag, used to switch between graph kinds.

use iced::{
    Alignment, Element, Length, Widget,
    widget::{Row, button, text},
};

use crate::theme;

/// Renders a tag button which is highlighted while selected.
pub fn tag<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    selected: bool,
    large: bool,
    message: Message,
) -> Element<'a, Message> {
    let height = if large { 50.0 } else { 29.0 };

    let mut children: Vec<Element<'a, Message>> = Vec::with_capacity(2);

    if selected {
        children.push(text("\u{2714}").size(14).boxed());
    }

    children.push(content.into());

    button(
        Row::with_children(children)
            .align_y(Alignment::Center)
            .spacing(theme::SPACE_XXXS),
    )
    .padding([theme::SPACE_XXXS, theme::SPACE_XS])
    .height(Length::Fixed(height))
    .style(move |iced_theme, status| style(iced_theme, status, selected))
    .on_press(message)
    .boxed()
}

fn style(iced_theme: &iced::Theme, status: button::Status, selected: bool) -> button::Style {
    let palette = iced_theme.palette();

    let mut style = button::Style {
        background: Some(palette.background.weak.color.into()),
        text_color: palette.background.weak.text,
        border: iced::border::rounded(6),
        ..button::Style::default()
    };

    if selected {
        style.background = Some(palette.primary.base.color.into());
        style.text_color = palette.primary.base.text;
    }

    match status {
        button::Status::Hovered if !selected => {
            style.background = Some(palette.background.strong.color.into());
            style.text_color = palette.background.strong.text;
        }
        button::Status::Pressed => {
            style.background = Some(palette.primary.strong.color.into());
            style.text_color = palette.primary.strong.text;
        }
        _ => {}
    }

    style
}
