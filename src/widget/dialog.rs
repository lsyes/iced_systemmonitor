// SPDX-License-Identifier: GPL-3.0-only

//! A modal dialog drawn inside the application window.

use iced::{
    Alignment, Element, Length, Widget,
    widget::{button, column, container, mouse_area, opaque, row, stack, text},
};

use crate::theme;

/// Wraps `base` with a modal dialog, dismissed by clicking outside of it.
pub fn modal<'a, Message: Clone + 'a>(
    base: Element<'a, Message>,
    dialog: Element<'a, Message>,
    on_dismiss: Message,
) -> Element<'a, Message> {
    stack![
        base,
        opaque(
            mouse_area(
                container(opaque(
                    container(dialog).padding(theme::SPACE_L).style(dialog_style)
                ))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_iced_theme| container::Style {
                    background: Some(
                        iced::Color {
                            a: 0.5,
                            ..iced::Color::BLACK
                        }
                        .into()
                    ),
                    ..container::Style::default()
                })
            )
            .on_press(on_dismiss)
        )
    ]
    .boxed()
}

fn dialog_style(iced_theme: &iced::Theme) -> container::Style {
    let palette = iced_theme.palette();

    container::Style {
        background: Some(palette.background.base.color.into()),
        border: iced::border::rounded(8),
        shadow: iced::Shadow {
            color: iced::Color::BLACK.scale_alpha(0.3),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
        ..container::Style::default()
    }
}

/// Builds the body of a confirmation dialog.
pub fn confirm<'a, Message: Clone + 'a>(
    title: String,
    body: String,
    extra: Option<Element<'a, Message>>,
    primary: (String, Message),
    secondary: (String, Message),
) -> Element<'a, Message> {
    let mut children: Vec<Element<'a, Message>> = Vec::with_capacity(4);

    children.push(text(title).size(20).boxed());
    children.push(text(body).boxed());

    if let Some(extra) = extra {
        children.push(extra);
    }

    children.push(
        row![
            button(text(secondary.0)).on_press(secondary.1),
            button(text(primary.0))
                .style(|iced_theme: &iced::Theme, status: button::Status| {
                    let palette = iced_theme.palette();

                    let mut style = button::Style {
                        background: Some(palette.danger.base.color.into()),
                        text_color: palette.danger.base.text,
                        border: iced::border::rounded(4),
                        ..button::Style::default()
                    };

                    if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                        style.background = Some(palette.danger.strong.color.into());
                        style.text_color = palette.danger.strong.text;
                    }

                    style
                })
                .on_press(primary.1),
        ]
        .spacing(theme::SPACE_XS)
        .align_y(Alignment::Center)
        .boxed(),
    );

    column(children)
        .spacing(theme::SPACE_S)
        .width(Length::Fill)
        .boxed()
}
