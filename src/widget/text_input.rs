// SPDX-License-Identifier: GPL-3.0-only

//! A single-line text input with its own right-click context menu.
//!
//! The widget owns the [`text_editor::Content`], the editor id and the
//! [`context_menu::State`] used by the menu, so that the application only has
//! to keep a [`State`] around and forward [`Event`]s to it.

use std::sync::Arc;

use iced::{
    Element, Length, Subscription, Task, Theme,
    advanced::text::editor::{Action as EditorAction, Edit},
    widget::{Id, text_editor},
};

use crate::{
    fl, theme,
    widget::{context_menu, context_menu::Event as ContextMenuEvent},
};

/// An entry of the context menu of a text input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Cut,
    Copy,
    Paste,
    SelectAll,
}

impl Action {
    /// Every action, in menu order.
    pub const ALL: [Self; 4] = [Self::Cut, Self::Copy, Self::Paste, Self::SelectAll];

    /// The label shown for the action.
    pub fn label(self) -> String {
        match self {
            Self::Cut => fl!("cut"),
            Self::Copy => fl!("copy"),
            Self::Paste => fl!("paste"),
            Self::SelectAll => fl!("select-all"),
        }
    }
}

/// What happened to a text input.
#[derive(Clone, Debug)]
pub enum Event {
    /// The editor performed an action.
    Edit(text_editor::Action),
    /// The context menu was opened, advanced or chosen from.
    Menu(ContextMenuEvent),
    /// The clipboard was read so that its contents can be pasted.
    Paste(Option<String>),
}

/// The state of a text input.
pub struct State {
    content: text_editor::Content,
    id: Id,
    menu: context_menu::State,
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    /// Creates an empty text input with its own context menu.
    pub fn new() -> Self {
        let mut menu = context_menu::State::new();
        menu.set_items(Action::ALL.iter().map(|action| action.label()));

        Self {
            content: text_editor::Content::new(),
            id: Id::unique(),
            menu,
        }
    }

    /// The current value.
    pub fn text(&self) -> String {
        self.content.text()
    }

    /// Replaces the current value.
    pub fn set_text(&mut self, text: &str) {
        self.content = text_editor::Content::with_text(text);
    }

    /// Records the raw Wayland handles used to parent the native context menu.
    pub fn set_window_handle(&mut self, handle: Option<(usize, usize)>) {
        self.menu.set_window_handle(handle);
    }

    /// Records the size of the window, so that the menu can be flipped when
    /// there is not enough room below the click.
    pub fn set_window_size(&mut self, size: iced::Size) {
        self.menu.set_window_size(size);
    }

    /// Whether the context menu is currently visible.
    pub fn is_menu_open(&self) -> bool {
        self.menu.is_open()
    }

    /// Closes the context menu.
    pub fn close_menu(&mut self) {
        self.menu.close();
    }

    /// The subscriptions the context menu needs while it is open.
    pub fn subscription(&self) -> Subscription<Event> {
        self.menu.subscription().map(Event::Menu)
    }

    /// Handles an [`Event`], returning whether the value changed and the task
    /// the application has to run.
    ///
    /// The clipboard actions are performed by the application, so `on_event`
    /// is used to feed the result of a paste back into the input.
    pub fn update<Message: Clone + Send + 'static>(
        &mut self,
        event: Event,
        theme: &Theme,
        on_event: fn(Event) -> Message,
    ) -> (bool, Task<Message>) {
        match event {
            Event::Edit(action) => {
                self.content.perform(action);

                (true, Task::none())
            }
            Event::Menu(event) => match self.menu.update(event, theme) {
                Some(index) => Action::ALL.get(index).copied().map_or(
                    (false, Task::none()),
                    |action| self.perform(action, on_event),
                ),
                None => (false, Task::none()),
            },
            Event::Paste(Some(text)) => {
                self.content
                    .perform(EditorAction::Edit(Edit::Paste(Arc::new(text))));

                (true, Task::none())
            }
            Event::Paste(None) => (false, Task::none()),
        }
    }

    /// Applies an action of the context menu.
    ///
    /// The selection is owned by iced's text editor, so Copy and Cut act on
    /// the selected text when there is one, and on the whole value otherwise.
    fn perform<Message: Clone + Send + 'static>(
        &mut self,
        action: Action,
        on_event: fn(Event) -> Message,
    ) -> (bool, Task<Message>) {
        match action {
            Action::Copy => {
                let text = self
                    .content
                    .selection()
                    .unwrap_or_else(|| self.content.text());

                (false, iced::clipboard::write(text).discard())
            }
            Action::Cut => {
                let text = self.content.text();
                let remaining = cut_remaining(&self.content);
                let copied = self.content.selection().unwrap_or(text);

                self.content = text_editor::Content::with_text(&remaining);

                (true, iced::clipboard::write(copied).discard())
            }
            Action::Paste => (
                false,
                iced::clipboard::read_text().map(move |result| {
                    on_event(Event::Paste(result.ok().map(|text| text.as_str().to_string())))
                }),
            ),
            Action::SelectAll => {
                self.content.perform(EditorAction::SelectAll);

                (false, Task::none())
            }
        }
    }

    /// The view of the input, with the context menu on right-click.
    pub fn view<'a, Message: Clone + 'a>(
        &'a self,
        placeholder: String,
        on_event: fn(Event) -> Message,
    ) -> Element<'a, Message> {
        let input = text_editor(&self.content)
            .placeholder(placeholder)
            .on_action(move |action| on_event(Event::Edit(action)))
            .padding([6.0, theme::SPACE_S])
            .height(Length::Shrink)
            .wrapping(iced::widget::text::Wrapping::None)
            .width(360.0)
            .style(editor_style)
            .id(self.id.clone());

        self.menu.wrap(input, move |position| {
            on_event(Event::Menu(ContextMenuEvent::Open(position)))
        })
    }

    /// The layers that draw the in-application fallback menu.
    ///
    /// Empty unless the fallback is in use; append them to the root stack.
    pub fn layers<'a, Message: Clone + 'a>(
        &'a self,
        on_event: fn(Event) -> Message,
    ) -> Vec<Element<'a, Message>> {
        self.menu.layers(move |event| on_event(Event::Menu(event)))
    }
}

/// The text that remains once the selection is cut.
fn cut_remaining(content: &text_editor::Content) -> String {
    let text = content.text();
    let cursor = content.cursor();

    let Some(selection) = cursor.selection else {
        return String::new();
    };

    if selection.line != cursor.position.line {
        return text;
    }

    let (start, end) = if selection.index <= cursor.position.index {
        (selection.index, cursor.position.index)
    } else {
        (cursor.position.index, selection.index)
    };

    let (start, end) = (start.min(text.len()), end.min(text.len()));

    match text.get(..start).zip(text.get(end..)) {
        Some((head, tail)) => format!("{head}{tail}"),
        None => text,
    }
}

fn editor_style(iced_theme: &Theme, status: text_editor::Status) -> text_editor::Style {
    let palette = iced_theme.palette();

    let border_color = match status {
        text_editor::Status::Focused { .. } => palette.primary.base.color,
        _ => palette.background.strong.color,
    };

    text_editor::Style {
        background: palette.background.base.color.into(),
        border: iced::border::rounded(6).color(border_color).width(1),
        placeholder: palette.background.strong.color,
        value: palette.background.base.text,
        selection: palette.primary.weak.color,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::advanced::text::{
        Position,
        editor::{Cursor, Motion},
    };

    fn with_selection(value: &str, start: usize, end: usize) -> text_editor::Content {
        let mut content = text_editor::Content::with_text(value);
        content.move_to(Cursor {
            position: Position { line: 0, index: end },
            selection: Some(Position { line: 0, index: start }),
        });
        content
    }

    #[test]
    fn selection_reports_the_selected_text() {
        let content = with_selection("hello world", 0, 5);
        assert_eq!(content.selection().as_deref(), Some("hello"));
    }

    #[test]
    fn cut_removes_only_the_selection() {
        let content = with_selection("hello world", 0, 5);
        assert_eq!(cut_remaining(&content), " world");
    }

    #[test]
    fn cut_removes_a_middle_selection() {
        let content = with_selection("hello world", 6, 11);
        assert_eq!(cut_remaining(&content), "hello ");
    }

    #[test]
    fn cut_without_selection_clears_everything() {
        let content = text_editor::Content::with_text("hello");
        assert_eq!(content.selection(), None);
        assert_eq!(cut_remaining(&content), "");
    }

    #[test]
    fn selection_survives_a_reversed_drag() {
        // Dragging from right to left keeps the anchor after the cursor.
        let content = with_selection("hello world", 11, 6);
        assert_eq!(cut_remaining(&content), "hello ");
    }

    #[allow(dead_code)]
    fn motion_is_available() -> Motion {
        Motion::Left
    }
}
