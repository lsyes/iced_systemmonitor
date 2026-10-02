// SPDX-License-Identifier: GPL-3.0-only

//! A card container used to group content.

use iced::{
    Element, Length, Widget,
    widget::container,
};

use crate::theme;

/// Wraps content in a card with a background and rounded corners.
pub fn card<'a, Message: 'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    container(content)
        .padding(theme::SPACE_S)
        .width(Length::Fill)
        .style(|iced_theme: &iced::Theme| container::Style {
            background: Some(theme::card_background(iced_theme).into()),
            border: iced::border::rounded(8),
            ..container::Style::default()
        })
        .boxed()
}
