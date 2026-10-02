// SPDX-License-Identifier: GPL-3.0-only

//! A reusable right-click context menu.
//!
//! On Wayland the menu is a native `xdg_popup` rendered outside of the window
//! by the compositor. On every other platform — or when the popup cannot be
//! created — it falls back to an in-application panel layered on top of the
//! window.
//!
//! # Usage
//!
//! 1. Keep a [`State`] in the application state and feed it the window handle
//!    captured with [`State::set_window_handle`].
//! 2. Describe the entries with [`State::set_items`].
//! 3. Wrap any widget with [`State::wrap`] to make it open the menu on
//!    right-click.
//! 4. Extend the root [`iced::widget::Stack`] with [`State::layers`].
//! 5. Batch [`State::subscription`] and forward [`Event`]s to
//!    [`State::update`], which reports the index of the chosen entry.

use std::{marker::PhantomData, time::Duration};

use iced::{
    Element, Length, Point, Rectangle, Size, Subscription, Theme,
    advanced::{
        layout::{self, Layout},
        renderer,
        widget::{self, Tree},
        Shell, Widget as CoreWidget,
    },
    mouse,
    widget::{container, mouse_area, opaque},
    Widget as _,
};

use crate::widget::{
    menu_bar,
    popup::{self, PopupEvent},
};

/// The width of the menu, shared by the native and the in-application panel.
pub const WIDTH: f32 = popup::WIDTH;

/// What happened to the context menu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// Opens the menu at a position inside the window.
    Open(Point),
    /// The entry at the given index was chosen in the in-application panel.
    ///
    /// Native popups report their choice through [`State::update`] instead.
    Select(usize),
    /// Closes the menu without choosing anything.
    Dismiss,
    /// Advances the native popup by one frame.
    Pump,
}

/// The state of a context menu.
#[derive(Default)]
pub struct State {
    /// Where the in-application fallback panel is shown, if it is open.
    fallback: Option<Point>,
    /// Whether a native popup is currently on screen.
    native: bool,
    /// The raw Wayland display and surface pointers of the main window.
    handle: Option<(usize, usize)>,
    /// The labels of the entries, top to bottom.
    items: Vec<String>,
    /// The size of the window, used to keep the menu inside of it.
    window_size: Option<Size>,
}

impl State {
    /// Creates an empty state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the entries shown by the menu, top to bottom.
    pub fn set_items(&mut self, items: impl IntoIterator<Item = String>) {
        self.items = items.into_iter().collect();
    }

    /// Records the raw Wayland handles used to parent the native popup.
    pub fn set_window_handle(&mut self, handle: Option<(usize, usize)>) {
        self.handle = handle;
    }

    /// Records the size of the window, so that the menu can be flipped when
    /// there is not enough room below the click.
    pub fn set_window_size(&mut self, size: Size) {
        self.window_size = Some(size);
    }

    /// The height of the menu with the current entries.
    fn height(&self) -> f32 {
        popup::height(self.items.len())
    }

    /// The direction the menu has to grow in to stay inside the window.
    fn direction(&self, position: Point) -> popup::Direction {
        let fits_below = self
            .window_size
            .is_none_or(|size| position.y + self.height() <= size.height);

        if fits_below {
            popup::Direction::Down
        } else {
            popup::Direction::Up
        }
    }

    /// The corner of the menu, relative to the click, in the in-application
    /// panel. Mirrors what the compositor does with the native popup.
    fn panel_offset(&self, position: Point) -> Point {
        let mut offset = position;

        if self.direction(position) == popup::Direction::Up {
            offset.y -= self.height();
        }

        if let Some(size) = self.window_size {
            offset.x = offset.x.min((size.width - WIDTH).max(0.0));
            offset.y = offset.y.max(0.0).min((size.height - self.height()).max(0.0));
        }

        offset
    }

    /// Whether the menu is currently visible, natively or in-application.
    pub fn is_open(&self) -> bool {
        self.native || self.fallback.is_some()
    }

    /// Handles an [`Event`], returning the index of the chosen entry.
    ///
    /// `theme` is used to render the native popup, which is drawn by us rather
    /// than by the toolkit.
    pub fn update(&mut self, event: Event, theme: &Theme) -> Option<usize> {
        match event {
            Event::Open(position) => {
                self.fallback = None;
                self.native = false;

                if self.open_native(position, theme) {
                    return None;
                }

                self.fallback = Some(position);

                None
            }
            Event::Select(index) => {
                self.close();

                Some(index)
            }
            Event::Dismiss => {
                self.close();

                None
            }
            Event::Pump => {
                if !self.native {
                    return None;
                }

                match popup::pump() {
                    PopupEvent::Selected(index) => {
                        self.close();

                        Some(index)
                    }
                    PopupEvent::Dismissed => {
                        self.close();

                        None
                    }
                    PopupEvent::None => None,
                }
            }
        }
    }

    /// Closes the menu and destroys its native popup, if it has one.
    ///
    /// The native popup is shared by every menu, so only the owner destroys
    /// it; another menu may have replaced it in the meantime.
    pub fn close(&mut self) {
        self.fallback = None;

        if self.native {
            self.native = false;
            popup::hide();
        }
    }

    /// Tries to show the menu as a native Wayland popup.
    fn open_native(&mut self, position: Point, theme: &Theme) -> bool {
        #[cfg(target_os = "linux")]
        {
            let Some((display, surface)) = self.handle else {
                return false;
            };

            let result = unsafe {
                popup::wayland::show(
                    display as *mut std::ffi::c_void,
                    surface as *mut std::ffi::c_void,
                    position.x.round() as i32,
                    position.y.round() as i32,
                    self.direction(position),
                    self.items.clone(),
                    popup::render::MenuStyle::from_theme(theme),
                )
            };

            match result {
                Ok(()) => {
                    self.native = true;

                    return true;
                }
                Err(err) => log::warn!("failed to show the native context menu: {err}"),
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = (position, theme);
        }

        false
    }

    /// The subscriptions the menu needs while it is open.
    pub fn subscription(&self) -> Subscription<Event> {
        let mut subscriptions = vec![
            // A left click anywhere dismisses the menu. `listen()` would only
            // report the events no widget handled, which is precisely not the
            // case here: clicking a row or a navigation button has to close
            // the menu as well.
            iced::event::listen_with(|event, _status, _window| match event {
                iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                    Some(Event::Dismiss)
                }
                _ => None,
            }),
        ];

        if self.native {
            subscriptions.push(
                iced::time::every(Duration::from_millis(80)).map(|_| Event::Pump),
            );
        }

        Subscription::batch(subscriptions)
    }

    /// The layers that draw the in-application fallback panel.
    ///
    /// Empty unless the fallback is in use; append them to the root stack. A
    /// click on an entry is reported back as [`Event::Select`].
    pub fn layers<'a, Message: Clone + 'a>(
        &'a self,
        on_event: impl Fn(Event) -> Message + 'a,
    ) -> Vec<Element<'a, Message>> {
        let Some(position) = self.fallback else {
            return Vec::new();
        };

        // Keep the panel inside of the window, flipping it upwards when there
        // is no room below the click.
        let position = self.panel_offset(position);

        let items = self
            .items
            .iter()
            .enumerate()
            .map(|(index, label)| menu_bar::MenuItem::new(label.clone(), on_event(Event::Select(index))));

        let panel = menu_bar::menu_panel(items, WIDTH);

        vec![
            // Clicking the backdrop closes the menu before it reaches anything
            // underneath.
            opaque(
                mouse_area(iced::widget::Space::new().width(Length::Fill).height(Length::Fill))
                    .on_press(on_event(Event::Dismiss)),
            )
            .boxed(),
            container(panel)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(iced::alignment::Horizontal::Left)
                .align_y(iced::alignment::Vertical::Top)
                .padding(iced::Padding {
                    top: position.y,
                    left: position.x,
                    right: 0.0,
                    bottom: 0.0,
                })
                .boxed(),
        ]
    }

    /// Makes `content` open the menu when it is right-clicked.
    pub fn wrap<'a, Message, W, F>(
        &self,
        content: W,
        on_open: F,
    ) -> Element<'a, Message>
    where
        Message: Clone + 'a,
        W: CoreWidget<Message, Theme, iced::Renderer> + 'a,
        F: Fn(Point) -> Message + 'a,
    {
        ContextMenuArea::<Message, W, F> {
            content,
            on_open,
            _lifetime: PhantomData,
        }
        .boxed()
    }
}

/// The wrapper that turns a right click into an [`Event::Open`].
struct ContextMenuArea<'a, Message, W, F> {
    content: W,
    on_open: F,
    _lifetime: PhantomData<&'a Message>,
}

impl<Message, W, F> widget::Meta for ContextMenuArea<'_, Message, W, F> {}

impl<Message, W, F> CoreWidget<Message, Theme, iced::Renderer> for ContextMenuArea<'_, Message, W, F>
where
    Message: Clone,
    W: CoreWidget<Message, Theme, iced::Renderer>,
    F: Fn(Point) -> Message,
{
    fn size(&self) -> Size<Length> {
        CoreWidget::<Message, Theme, iced::Renderer>::size(&self.content)
    }

    fn tag(&self) -> widget::tree::Tag {
        CoreWidget::<Message, Theme, iced::Renderer>::tag(&self.content)
    }

    fn state(&self) -> widget::tree::State {
        CoreWidget::<Message, Theme, iced::Renderer>::state(&self.content)
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children::<Message, Theme, iced::Renderer>(std::slice::from_mut(
            &mut self.content,
        ));
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &layout::Limits) {
        let child = &mut tree.children[0];

        self.content.layout(child, renderer, limits);

        tree.size = child.size;
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.content
            .operate(&mut tree.children[0], layout, viewport, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            shell,
            viewport,
        );

        if let iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) = event
            && let Some(position) = cursor.position_over(layout.bounds())
        {
            shell.publish((self.on_open)(position));
            shell.capture_event();
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    enum TestMessage {
        Event(Event),
    }

    /// Opens the menu, which falls back to the panel without a window handle.
    fn opened() -> State {
        let mut state = State::new();
        state.set_items(["One".to_string(), "Two".to_string()]);
        state.update(Event::Open(Point::new(10.0, 20.0)), &Theme::Light);

        state
    }

    #[test]
    fn the_menu_opens_as_a_fallback_without_a_window_handle() {
        let state = opened();

        assert!(state.is_open());
        assert!(state.fallback.is_some());
        assert!(!state.native);
    }

    #[test]
    fn the_fallback_panel_draws_a_backdrop_and_the_entries() {
        let state = opened();
        let layers = state.layers(TestMessage::Event);

        assert_eq!(layers.len(), 2);
    }

    #[test]
    fn choosing_an_entry_reports_its_index_and_closes() {
        let mut state = opened();

        assert_eq!(state.update(Event::Select(1), &Theme::Light), Some(1));
        assert!(!state.is_open());
    }

    #[test]
    fn dismissing_closes_without_a_choice() {
        let mut state = opened();

        assert_eq!(state.update(Event::Dismiss, &Theme::Light), None);
        assert!(!state.is_open());
    }

    /// A menu with two entries, which is 76 pixels tall.
    fn positioned() -> State {
        let mut state = State::new();
        state.set_items(["One".to_string(), "Two".to_string()]);
        state.set_window_size(Size::new(800.0, 600.0));

        state
    }

    #[test]
    fn the_menu_grows_downwards_when_there_is_room_below() {
        let state = positioned();

        assert_eq!(
            state.direction(Point::new(100.0, 100.0)),
            popup::Direction::Down
        );
    }

    #[test]
    fn the_menu_grows_upwards_when_it_would_not_fit_below() {
        let state = positioned();

        assert_eq!(
            state.direction(Point::new(100.0, 550.0)),
            popup::Direction::Up
        );
    }

    #[test]
    fn the_menu_grows_downwards_without_a_known_window_size() {
        let mut state = State::new();
        state.set_items(["One".to_string()]);

        assert_eq!(
            state.direction(Point::new(100.0, 5000.0)),
            popup::Direction::Down
        );
    }

    #[test]
    fn an_upwards_menu_ends_at_the_click() {
        let state = positioned();
        let offset = state.panel_offset(Point::new(100.0, 550.0));

        // The bottom edge of the menu sits on the click.
        assert_eq!(offset.y, 550.0 - state.height());
    }

    #[test]
    fn the_panel_is_kept_inside_of_the_window() {
        let state = positioned();

        // Clicking near the right and bottom edges still shows the whole menu.
        let offset = state.panel_offset(Point::new(790.0, 590.0));

        assert!(offset.x + WIDTH <= 800.0);
        assert!(offset.y + state.height() <= 600.0);
        assert!(offset.x >= 0.0);
    }

    #[test]
    fn a_tiny_window_does_not_push_the_panel_negative() {
        let mut state = State::new();
        state.set_items(["One".to_string(), "Two".to_string()]);
        state.set_window_size(Size::new(100.0, 50.0));

        let offset = state.panel_offset(Point::new(90.0, 40.0));

        assert_eq!(offset.x, 0.0);
        assert_eq!(offset.y, 0.0);
    }
}
