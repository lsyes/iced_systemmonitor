// SPDX-License-Identifier: GPL-3.0-only

//! A vertical navigation bar drawn inside the application window.

use iced::{
    Element, Length, Widget,
    widget::{button, column, container, scrollable, text},
};

use crate::theme;

/// Renders the navigation bar for the given pages.
pub fn nav_bar<'a, Page, Message>(
    pages: impl IntoIterator<Item = (Page, String)>,
    active: Page,
    on_select: fn(Page) -> Message,
) -> Element<'a, Message>
where
    Page: Copy + PartialEq + 'a,
    Message: Clone + 'a,
{
    let mut children: Vec<Element<'a, Message>> = Vec::new();

    for (page, title) in pages {
        let selected = page == active;

        children.push(
            button(text(title).width(Length::Fill))
                .width(Length::Fill)
                .padding([theme::SPACE_XS, theme::SPACE_S])
                .style(move |iced_theme, status| style(iced_theme, status, selected))
                .on_press(on_select(page))
                .boxed(),
        );
    }

    container(
        scrollable(column(children).spacing(theme::SPACE_XXXS))
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .padding(theme::SPACE_XS)
    .width(Length::Fixed(200.0))
    .height(Length::Fill)
    .style(|iced_theme: &iced::Theme| container::Style {
        background: Some(iced_theme.palette().background.weak.color.into()),
        ..container::Style::default()
    })
    .boxed()
}

fn style(iced_theme: &iced::Theme, status: button::Status, selected: bool) -> button::Style {
    let palette = iced_theme.palette();

    let mut style = button::Style {
        background: None,
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
        button::Status::Pressed if !selected => {
            style.background = Some(palette.background.stronger.color.into());
            style.text_color = palette.background.stronger.text;
        }
        _ => {}
    }

    style
}
